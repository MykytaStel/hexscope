//! Copy-on-write redaction of PDF content and Form XObjects.
//!
//! Page content streams, forms, and their resource dictionaries can be shared
//! by several pages or invocations. A clean copy therefore writes new streams
//! and binds only the selected page and invocation to those copies.

use super::facts::{self, Found, MAX_DECODED_TOTAL};
use super::lexer::{Entry, Item, Obj};
use super::page::{StreamEdit, StreamPath, apply_edits};
use super::redact::{self, ContentRewrite};
use super::{Ctx, ObjRec};
use crate::model::ByteRange;

const MAX_REWRITES: usize = 10_000;

#[derive(Debug, Clone)]
pub(super) struct ObjectReplacement {
    pub num: u32,
    pub generation: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(super) struct NewObject {
    pub num: u32,
    pub body: Vec<u8>,
}

#[derive(Debug, Default)]
pub(super) struct FormRewritePlan {
    pub replacements: Vec<ObjectReplacement>,
    pub added: Vec<NewObject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormRewriteError {
    MissingPage,
    MissingObject,
    MissingContent,
    InvalidPath,
    ConflictingPaths,
    InvalidResources,
    TooManyObjects,
}

#[derive(Clone)]
struct EditGroup {
    path: StreamPath,
    edits: Vec<StreamEdit>,
}

struct PageState {
    num: u32,
    dict: Obj,
    resources: Obj,
    generation: u16,
}

#[derive(Clone)]
struct Patch {
    start: usize,
    end: usize,
    bytes: Vec<u8>,
}

/// Plans stream, resource, and page-object copies without changing an
/// unselected reference to a shared stream or form.
pub(super) fn plan_form_rewrites(
    data: &[u8],
    ctx: &Ctx,
    streams: &[ContentRewrite],
    source_edits: &[StreamEdit],
    form_edits: &[StreamEdit],
    prefixed_streams: &[(u32, u32)],
) -> Result<FormRewritePlan, FormRewriteError> {
    if streams.is_empty() && form_edits.is_empty() {
        return Ok(FormRewritePlan::default());
    }
    if streams.len() > MAX_REWRITES || form_edits.len() > MAX_REWRITES {
        return Err(FormRewriteError::TooManyObjects);
    }

    let mut budget = MAX_DECODED_TOTAL;
    let pages = redact::pages(data, ctx, &mut budget).pages;
    let max_object = ctx.objects.iter().map(|rec| rec.num).max().unwrap_or(0);
    let mut next_object = max_object
        .checked_add(1)
        .ok_or(FormRewriteError::TooManyObjects)?;
    let mut plan = FormRewritePlan::default();
    let mut page_states = Vec::new();
    let mut stream_ids = Vec::new();
    let mut stream_patches: Vec<(u32, u32, Vec<Patch>)> = Vec::new();

    for rewrite in streams {
        if stream_ids
            .iter()
            .any(|(page, object, _)| *page == rewrite.page && *object == rewrite.object)
        {
            return Err(FormRewriteError::InvalidPath);
        }
        let page = pages
            .iter()
            .find(|page| page.num == rewrite.page)
            .ok_or(FormRewriteError::MissingPage)?;
        let rec = ctx
            .latest(rewrite.object)
            .ok_or(FormRewriteError::MissingObject)?;
        let new_stream = allocate(&mut next_object)?;
        let body = stream_body(&rec.value, &rewrite.bytes);
        plan.added.push(NewObject {
            num: new_stream,
            body,
        });
        stream_ids.push((rewrite.page, rewrite.object, new_stream));

        if !page_states
            .iter()
            .any(|state: &PageState| state.num == page.num)
        {
            let resources = page
                .resources
                .as_ref()
                .map(|resources| resolve_obj(data, ctx, resources, &mut budget))
                .transpose()?
                .unwrap_or_else(empty_dict);
            let page_rec = ctx
                .latest(page.num)
                .ok_or(FormRewriteError::MissingObject)?;
            page_states.push(PageState {
                num: page.num,
                dict: page.dict.clone(),
                resources,
                generation: page_rec.gen_,
            });
        }
    }

    let groups = group_edits(form_edits);
    // Two different edits below one root invocation need a shared cloned
    // ancestor. Refuse that shape until its nested rewrite can be combined.
    for (i, group) in groups.iter().enumerate() {
        let Some(root) = group.path.calls.first() else {
            return Err(FormRewriteError::InvalidPath);
        };
        if groups[..i].iter().any(|earlier| {
            earlier.path.page == group.path.page
                && earlier.path.calls.first() == Some(root)
                && earlier.path != group.path
        }) {
            return Err(FormRewriteError::ConflictingPaths);
        }
    }

    for group in groups {
        let path = &group.path;
        if path.page == 0 || path.calls.is_empty() || path.calls.len() > 4 {
            return Err(FormRewriteError::InvalidPath);
        }
        pages
            .iter()
            .find(|page| page.num == path.page)
            .ok_or(FormRewriteError::MissingPage)?;
        let page_state = page_states
            .iter_mut()
            .find(|state| state.num == path.page)
            .ok_or(FormRewriteError::MissingPage)?;

        let mut scopes = Vec::with_capacity(path.calls.len());
        let mut inherited = page_state.resources.clone();
        for index in 0..path.calls.len() {
            let form_num = path
                .calls
                .get(index + 1)
                .map_or(path.object, |site| site.owner);
            let form = ctx
                .latest(form_num)
                .ok_or(FormRewriteError::MissingObject)?;
            let scope = match form.value.get("Resources") {
                Some(resources) => resolve_obj(data, ctx, resources, &mut budget)?,
                None => inherited.clone(),
            };
            inherited = scope.clone();
            scopes.push(scope);
        }

        let target = ctx
            .latest(path.object)
            .ok_or(FormRewriteError::MissingObject)?;
        let target_bytes = decode_stream(data, ctx, target, &mut budget)?;
        let local_edits = checked_edits(&group.edits, &target_bytes)?;
        let rewritten = apply_edits(&target_bytes, &local_edits);
        let mut child = allocate(&mut next_object)?;
        plan.added.push(NewObject {
            num: child,
            body: stream_body(&target.value, &rewritten),
        });

        for site_index in (1..path.calls.len()).rev() {
            let site = &path.calls[site_index];
            let parent = ctx
                .latest(site.owner)
                .ok_or(FormRewriteError::MissingObject)?;
            let parent_bytes = decode_stream(data, ctx, parent, &mut budget)?;
            let original_scope = scopes[site_index - 1].clone();
            let alias = fresh_alias(
                data,
                ctx,
                &original_scope,
                plan.added.len() as u32 + 1,
                &mut budget,
            )?;
            let patched = patch_one(&parent_bytes, site.start, site.end, &name_bytes(&alias))?;
            let resources = add_xobject(data, ctx, original_scope, &alias, child, &mut budget)?;
            let mut parent_dict = parent.value.clone();
            set_entry(&mut parent_dict, "Resources", resources)?;
            child = allocate(&mut next_object)?;
            plan.added.push(NewObject {
                num: child,
                body: stream_body(&parent_dict, &patched),
            });
        }

        let root = &path.calls[0];
        let alias = fresh_alias(
            data,
            ctx,
            &page_state.resources,
            plan.added.len() as u32 + 1,
            &mut budget,
        )?;
        page_state.resources = add_xobject(
            data,
            ctx,
            page_state.resources.clone(),
            &alias,
            child,
            &mut budget,
        )?;
        if !stream_ids
            .iter()
            .any(|(page_num, object, _)| *page_num == path.page && *object == root.owner)
        {
            return Err(FormRewriteError::MissingContent);
        }
        let stream_rewrite = streams
            .iter()
            .find(|stream| stream.page == path.page && stream.object == root.owner)
            .ok_or(FormRewriteError::MissingContent)?;
        let mut start = root.start;
        let mut end = root.end;
        for edit in source_edits.iter().filter(|edit| {
            edit.target.page == path.page
                && edit.target.calls.is_empty()
                && edit.target.object == root.owner
                && edit.end <= root.start
        }) {
            let delta = edit.replacement.len() as isize - (edit.end - edit.start) as isize;
            start = start
                .checked_add_signed(delta)
                .ok_or(FormRewriteError::InvalidPath)?;
            end = end
                .checked_add_signed(delta)
                .ok_or(FormRewriteError::InvalidPath)?;
        }
        if prefixed_streams.contains(&(path.page, root.owner)) {
            start = start.checked_add(2).ok_or(FormRewriteError::InvalidPath)?;
            end = end.checked_add(2).ok_or(FormRewriteError::InvalidPath)?;
        }
        if end > stream_rewrite.bytes.len() {
            return Err(FormRewriteError::InvalidPath);
        }
        let patches = stream_patches
            .iter_mut()
            .find(|(page_num, object, _)| *page_num == path.page && *object == root.owner);
        if let Some((_, _, patches)) = patches {
            patches.push(Patch {
                start,
                end,
                bytes: name_bytes(&alias),
            });
        } else {
            stream_patches.push((
                path.page,
                root.owner,
                vec![Patch {
                    start,
                    end,
                    bytes: name_bytes(&alias),
                }],
            ));
        }
    }

    for (page_num, object, patches) in stream_patches {
        let stream = streams
            .iter()
            .find(|stream| stream.page == page_num && stream.object == object)
            .ok_or(FormRewriteError::MissingContent)?;
        let mut bytes = stream.bytes.clone();
        apply_patches(&mut bytes, patches)?;
        let (_, _, new_num) = stream_ids
            .iter()
            .find(|(page, old, _)| *page == page_num && *old == object)
            .ok_or(FormRewriteError::MissingContent)?;
        let rec = ctx.latest(object).ok_or(FormRewriteError::MissingObject)?;
        let generated = plan
            .added
            .iter_mut()
            .find(|generated| generated.num == *new_num)
            .ok_or(FormRewriteError::MissingObject)?;
        generated.body = stream_body(&rec.value, &bytes);
    }

    for mut state in page_states {
        let mut content_refs = Vec::new();
        map_contents(&mut state.dict, state.num, &stream_ids, &mut content_refs)?;
        let required = stream_ids
            .iter()
            .filter(|(page, _, _)| *page == state.num)
            .map(|(_, object, _)| *object)
            .collect::<Vec<_>>();
        if required.iter().any(|object| !content_refs.contains(object)) {
            return Err(FormRewriteError::MissingContent);
        }
        set_entry(&mut state.dict, "Resources", state.resources)?;
        plan.replacements.push(ObjectReplacement {
            num: state.num,
            generation: state.generation,
            body: serialize(&state.dict),
        });
    }

    Ok(plan)
}

fn group_edits(edits: &[StreamEdit]) -> Vec<EditGroup> {
    let mut groups: Vec<EditGroup> = Vec::new();
    for edit in edits {
        if let Some(group) = groups.iter_mut().find(|group| group.path == edit.target) {
            group.edits.push(edit.clone());
        } else {
            groups.push(EditGroup {
                path: edit.target.clone(),
                edits: vec![edit.clone()],
            });
        }
    }
    groups
}

fn checked_edits(
    edits: &[StreamEdit],
    bytes: &[u8],
) -> Result<Vec<(usize, usize, Vec<u8>)>, FormRewriteError> {
    let mut out = edits
        .iter()
        .map(|edit| (edit.start, edit.end, edit.replacement.clone()))
        .collect::<Vec<_>>();
    out.sort_by_key(|edit| edit.0);
    let mut end = 0;
    for (start, next_end, _) in &out {
        if *start < end || *next_end < *start || *next_end > bytes.len() {
            return Err(FormRewriteError::InvalidPath);
        }
        end = *next_end;
    }
    Ok(out)
}

fn map_contents(
    page: &mut Obj,
    page_num: u32,
    streams: &[(u32, u32, u32)],
    matched: &mut Vec<u32>,
) -> Result<(), FormRewriteError> {
    let replacement = |object: u32| {
        streams
            .iter()
            .find(|(page, old, _)| *page == page_num && *old == object)
            .map(|(_, _, new)| *new)
    };
    let contents = page
        .get("Contents")
        .cloned()
        .ok_or(FormRewriteError::MissingContent)?;
    let mapped = match contents {
        Obj::Ref(num, _) => {
            let Some(new) = replacement(num) else {
                return Err(FormRewriteError::MissingContent);
            };
            matched.push(num);
            Obj::Ref(new, 0)
        }
        Obj::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for mut item in items {
                if let Obj::Ref(num, _) = &item.obj {
                    let num = *num;
                    if let Some(new) = replacement(num) {
                        matched.push(num);
                        item.obj = Obj::Ref(new, 0);
                    }
                }
                out.push(item);
            }
            Obj::Array(out)
        }
        _ => return Err(FormRewriteError::MissingContent),
    };
    set_entry(page, "Contents", mapped)
}

fn resolve_obj(
    data: &[u8],
    ctx: &Ctx,
    object: &Obj,
    budget: &mut u64,
) -> Result<Obj, FormRewriteError> {
    match object {
        Obj::Ref(num, _) => match facts::resolve(data, ctx, *num, budget) {
            Some(Found::Top(rec)) => Ok(rec.value.clone()),
            Some(Found::Packed(value, _)) => Ok(value),
            None => Err(FormRewriteError::MissingObject),
        },
        other => Ok(other.clone()),
    }
}

fn decode_stream(
    data: &[u8],
    ctx: &Ctx,
    rec: &ObjRec,
    budget: &mut u64,
) -> Result<Vec<u8>, FormRewriteError> {
    facts::decode(data, rec, ctx.crypt.as_ref(), budget, false)
        .ok_or(FormRewriteError::MissingContent)
}

fn allocate(next: &mut u32) -> Result<u32, FormRewriteError> {
    let num = *next;
    *next = next
        .checked_add(1)
        .ok_or(FormRewriteError::TooManyObjects)?;
    Ok(num)
}

fn empty_dict() -> Obj {
    Obj::Dict(Vec::new())
}

fn add_xobject(
    data: &[u8],
    ctx: &Ctx,
    mut resources: Obj,
    alias: &str,
    object: u32,
    budget: &mut u64,
) -> Result<Obj, FormRewriteError> {
    let xobjects = match resources.get("XObject").cloned() {
        Some(xobjects) => match xobjects {
            Obj::Ref(_, _) => resolve_obj(data, ctx, &xobjects, budget)?,
            Obj::Dict(_) => xobjects,
            _ => return Err(FormRewriteError::InvalidResources),
        },
        None => empty_dict(),
    };
    let mut xobjects = xobjects;
    set_entry(&mut xobjects, alias, Obj::Ref(object, 0))?;
    set_entry(&mut resources, "XObject", xobjects)?;
    Ok(resources)
}

fn fresh_alias(
    data: &[u8],
    ctx: &Ctx,
    resources: &Obj,
    seed: u32,
    budget: &mut u64,
) -> Result<String, FormRewriteError> {
    let xobjects = match resources.get("XObject") {
        Some(Obj::Ref(_, _)) => Some(resolve_obj(
            data,
            ctx,
            resources
                .get("XObject")
                .ok_or(FormRewriteError::InvalidResources)?,
            budget,
        )?),
        Some(xobjects) => Some(xobjects.clone()),
        None => None,
    };
    for offset in 0..MAX_REWRITES as u32 {
        let number = seed
            .checked_add(offset)
            .ok_or(FormRewriteError::TooManyObjects)?;
        let name = format!("HexscopeRedact{number}");
        if !xobjects
            .as_ref()
            .is_some_and(|xobjects| xobjects.entries().iter().any(|entry| entry.key == name))
        {
            return Ok(name);
        }
    }
    Err(FormRewriteError::TooManyObjects)
}

fn set_entry(dict: &mut Obj, key: &str, value: Obj) -> Result<(), FormRewriteError> {
    let Obj::Dict(entries) = dict else {
        return Err(FormRewriteError::InvalidResources);
    };
    entries.retain(|entry| entry.key != key);
    let item = Item {
        obj: value,
        range: ByteRange::new(0, 0),
    };
    entries.push(Entry {
        key: key.to_owned(),
        value: item,
        range: ByteRange::new(0, 0),
    });
    Ok(())
}

fn stream_body(dict: &Obj, bytes: &[u8]) -> Vec<u8> {
    let mut dict = dict.clone();
    if let Obj::Dict(entries) = &mut dict {
        entries.retain(|entry| {
            !matches!(
                entry.key.as_str(),
                "Filter" | "DecodeParms" | "DP" | "Length"
            )
        });
    }
    let _ = set_entry(&mut dict, "Length", Obj::Int(bytes.len() as i64));
    let mut out = serialize(&dict);
    out.extend_from_slice(b"\nstream\n");
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\nendstream");
    out
}

fn patch_one(
    bytes: &[u8],
    start: usize,
    end: usize,
    replacement: &[u8],
) -> Result<Vec<u8>, FormRewriteError> {
    if start > end || end > bytes.len() {
        return Err(FormRewriteError::InvalidPath);
    }
    let mut out = Vec::with_capacity(bytes.len() + replacement.len());
    out.extend_from_slice(&bytes[..start]);
    out.extend_from_slice(replacement);
    out.extend_from_slice(&bytes[end..]);
    Ok(out)
}

fn apply_patches(bytes: &mut Vec<u8>, mut patches: Vec<Patch>) -> Result<(), FormRewriteError> {
    patches.sort_by_key(|patch| std::cmp::Reverse(patch.start));
    let mut previous_start = bytes.len();
    for patch in patches {
        if patch.start > patch.end || patch.end > previous_start || patch.end > bytes.len() {
            return Err(FormRewriteError::InvalidPath);
        }
        bytes.splice(patch.start..patch.end, patch.bytes);
        previous_start = patch.start;
    }
    Ok(())
}

fn name_bytes(name: &str) -> Vec<u8> {
    let mut out = vec![b'/'];
    for byte in name.bytes() {
        if byte.is_ascii_graphic()
            && !matches!(
                byte,
                b'#' | b'%' | b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/'
            )
        {
            out.push(byte);
        } else {
            out.extend_from_slice(format!("#{byte:02X}").as_bytes());
        }
    }
    out
}

fn serialize(object: &Obj) -> Vec<u8> {
    let mut out = Vec::new();
    serialize_into(object, &mut out);
    out
}

fn serialize_into(object: &Obj, out: &mut Vec<u8>) {
    match object {
        Obj::Null => out.extend_from_slice(b"null"),
        Obj::Bool(value) => out.extend_from_slice(if *value { b"true" } else { b"false" }),
        Obj::Int(value) => out.extend_from_slice(value.to_string().as_bytes()),
        Obj::Real(value) => out.extend_from_slice(value.as_bytes()),
        Obj::Str(value) => {
            out.push(b'<');
            for byte in value {
                out.extend_from_slice(format!("{byte:02X}").as_bytes());
            }
            out.push(b'>');
        }
        Obj::Name(value) => out.extend_from_slice(&name_bytes(value)),
        Obj::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    out.push(b' ');
                }
                serialize_into(&item.obj, out);
            }
            out.push(b']');
        }
        Obj::Dict(entries) => {
            out.extend_from_slice(b"<<");
            for entry in entries {
                out.push(b' ');
                out.extend_from_slice(&name_bytes(&entry.key));
                out.push(b' ');
                serialize_into(&entry.value.obj, out);
            }
            out.extend_from_slice(b" >>");
        }
        Obj::Ref(num, generation) => {
            out.extend_from_slice(format!("{num} {generation} R").as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_pdf_names_with_delimiter_escapes() {
        assert_eq!(name_bytes("A B#C"), b"/A#20B#23C");
    }

    #[test]
    fn patches_are_applied_from_the_end() {
        let mut bytes = b"/A Do /B Do".to_vec();
        apply_patches(
            &mut bytes,
            vec![
                Patch {
                    start: 0,
                    end: 2,
                    bytes: b"/LongName".to_vec(),
                },
                Patch {
                    start: 6,
                    end: 8,
                    bytes: b"/X".to_vec(),
                },
            ],
        )
        .unwrap();
        assert_eq!(bytes, b"/LongName Do /X Do");
    }
}
