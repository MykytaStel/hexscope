//! A PDF rewritten without what it says about the people behind it.
//!
//! Blanking the author in place is not enough: a PDF edited after it was
//! first saved still holds the earlier version, the old author and the old
//! text with it. So the copy is written anew, as one revision: only the
//! objects the document still uses, reached from its catalog, each copied
//! byte for byte, and a fresh cross-reference table. What nothing reaches
//! is left behind: the document information, objects an update replaced,
//! objects an update deleted. XMP streams are kept, but empty.

use super::facts::{MAX_DECODED_TOTAL, is_photo, unpack};
use super::lexer::{Lexer, Obj};
use super::pictures::{Edited, edit, jpeg_dict};
use super::{ObjRec, parse_with};
use crate::clean::{CleanError, Cleaned, Removed};
use crate::model::NodeKind;

/// An XMP packet with nothing in it.
const EMPTY_XMP: &[u8] = b"<?xpacket begin=\"\xEF\xBB\xBF\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/><?xpacket end=\"w\"?>";

/// Where the current version of an object comes from.
enum Source<'a> {
    Top(&'a ObjRec),
    /// Unpacked from an object stream: its value's bytes, and the value.
    Packed(Vec<u8>, Obj),
    /// A page, content, or form object written by copy-on-write redaction.
    Rewritten {
        body: Vec<u8>,
        value: Obj,
        generation: u16,
    },
}

/// The current version of every object, sorted by number: of the versions
/// of one number, the last met wins, as an update comes after what it
/// replaces — except that an object in the file's body never gives way to
/// one before it. A sorted list, not a map: maps cost the WebAssembly build
/// several kilobytes.
struct Current<'a>(Vec<(u32, u64, Source<'a>)>);

impl<'a> Current<'a> {
    fn new(versions: Vec<(u32, u64, Source<'a>)>) -> Self {
        let mut order: Vec<(u64, usize)> = versions
            .iter()
            .enumerate()
            .map(|(i, v)| ((u64::from(v.0) << 32) | (i as u64 & 0xFFFF_FFFF), i))
            .collect();
        order.sort_unstable();
        let mut slots: Vec<Option<(u32, u64, Source<'a>)>> =
            versions.into_iter().map(Some).collect();
        let mut out: Vec<(u32, u64, Source<'a>)> = Vec::new();
        for (_, i) in order {
            let Some(v) = slots[i].take() else { continue };
            match out.last_mut() {
                Some(last) if last.0 == v.0 => {
                    let wins = match v.2 {
                        Source::Packed(..) => true,
                        Source::Rewritten { .. } => true,
                        Source::Top(_) => last.1 <= v.1,
                    };
                    if wins {
                        *last = v;
                    }
                }
                _ => out.push(v),
            }
        }
        Current(out)
    }

    fn index(&self, n: u32) -> Option<usize> {
        self.0.binary_search_by_key(&n, |v| v.0).ok()
    }

    fn get(&self, n: u32) -> Option<&Source<'a>> {
        self.index(n).map(|i| &self.0[i].2)
    }
}

/// Annotations that are only a note: their whole point is the comment.
const NOTES: [&str; 2] = ["Text", "Popup"];
/// Marks on the text whose words are a note, not what the page shows.
const MARKS: [&str; 5] = ["Highlight", "Underline", "StrikeOut", "Squiggly", "Caret"];
/// What says who made an annotation, when, and in answer to what.
const WHO: [&str; 6] = ["T", "M", "CreationDate", "NM", "IRT", "Popup"];
/// An annotation that shows and says nothing, in a note's place in its page's list.
const RETIRED: &[u8] = b"<< /Type /Annot /Subtype /Link /Rect [0 0 0 0] /Border [0 0 0] /F 2 >>";

/// An object's value, when it is one without a stream.
fn value_of<'b>(s: &'b Source) -> Option<&'b Obj> {
    match s {
        Source::Top(rec) if rec.stream.is_none() => Some(&rec.value),
        Source::Top(_) => None,
        Source::Packed(_, obj) => Some(obj),
        Source::Rewritten { value, .. } => Some(value),
    }
}

/// Object `i` written as `body` instead, its generation kept.
fn rewrite(current: &mut Current, i: usize, body: Vec<u8>) {
    let generation = match &current.0[i].2 {
        Source::Top(rec) => rec.gen_,
        Source::Packed(..) => 0,
        Source::Rewritten { generation, .. } => *generation,
    };
    if let Some(value) = Lexer::new(&body, 0).value().map(|v| v.obj) {
        current.0[i].2 = Source::Rewritten {
            body,
            value,
            generation,
        };
    }
}

/// A dictionary without `key`'s entry.
fn without(entries: &[super::lexer::Entry], key: &str) -> Obj {
    Obj::Dict(entries.iter().filter(|e| e.key != key).cloned().collect())
}

/// Takes out notes and who wrote every annotation, and the files carried
/// inside — unless the document names them as part of itself (`/AF`), as an
/// e-invoice its data or a PDF/A-3 file its sources. Says whether it took
/// out comments, and files.
fn people_and_files(current: &mut Current, root: u32) -> (bool, bool) {
    use super::form_rewrite::serialize;
    let catalog = current.get(root).and_then(value_of).cloned();
    let keep_files = catalog.as_ref().is_some_and(|c| c.get("AF").is_some());
    let (mut notes, mut files) = (false, false);
    // The name tree of files goes, and with it what nothing else reaches.
    if !keep_files && let Some(Obj::Dict(entries)) = &catalog {
        match entries
            .iter()
            .find(|e| e.key == "Names")
            .map(|e| &e.value.obj)
        {
            Some(Obj::Dict(names)) if names.iter().any(|e| e.key == "EmbeddedFiles") => {
                let mut cat = entries.clone();
                for e in cat.iter_mut().filter(|e| e.key == "Names") {
                    e.value.obj = without(names, "EmbeddedFiles");
                }
                if let Some(i) = current.index(root) {
                    rewrite(current, i, serialize(&Obj::Dict(cat)));
                    files = true;
                }
            }
            Some(&Obj::Ref(n, _)) => {
                if let Some(i) = current.index(n)
                    && let Some(Obj::Dict(names)) = value_of(&current.0[i].2)
                    && names.iter().any(|e| e.key == "EmbeddedFiles")
                {
                    let body = serialize(&without(names, "EmbeddedFiles"));
                    rewrite(current, i, body);
                    files = true;
                }
            }
            _ => {}
        }
    }
    for i in 0..current.0.len() {
        let Some(Obj::Dict(entries)) = value_of(&current.0[i].2) else {
            continue;
        };
        let kind = entries
            .iter()
            .find(|e| e.key == "Subtype")
            .and_then(|e| e.value.obj.name())
            .unwrap_or("");
        let file = kind == "FileAttachment" && !keep_files;
        let body = if NOTES.contains(&kind) || file {
            RETIRED.to_vec()
        } else if super::redact::COMMENTS.contains(&kind) {
            let note =
                |key: &str| MARKS.contains(&kind) && matches!(key, "Contents" | "RC" | "Subj");
            let kept: Vec<_> = entries
                .iter()
                .filter(|e| !WHO.contains(&e.key.as_str()) && !note(&e.key))
                .cloned()
                .collect();
            if kept.len() == entries.len() {
                continue;
            }
            serialize(&Obj::Dict(kept))
        } else {
            continue;
        };
        notes |= !file;
        files |= file;
        rewrite(current, i, body);
    }
    (notes, files)
}

/// Append an unsigned decimal with a minimum width, without an intermediate
/// String allocation or a separate formatting instantiation at each call.
#[inline(never)]
fn decimal(out: &mut Vec<u8>, mut value: u64, width: usize) {
    let mut digits = [b'0'; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    debug_assert!(width <= digits.len());
    start = start.min(digits.len() - width);
    out.extend_from_slice(&digits[start..]);
}

#[inline(never)]
fn object_header(out: &mut Vec<u8>, number: u32, generation: u16) {
    decimal(out, u64::from(number), 0);
    out.push(b' ');
    decimal(out, u64::from(generation), 0);
    out.extend_from_slice(b" obj\n");
}

#[inline(never)]
fn xref_entry(out: &mut Vec<u8>, offset: usize, generation: u16) {
    decimal(out, offset as u64, 10);
    out.push(b' ');
    decimal(out, u64::from(generation), 5);
    out.extend_from_slice(b" n\r\n");
}

pub(crate) fn clean_pdf(data: &[u8]) -> Result<Cleaned, CleanError> {
    clean_pdf_with(data, &[], &[])
}

/// The clean copy, with `extra` areas blacked out as well: page number
/// counted from 1, and `[left, bottom, right, top]` in its points.
pub(crate) fn clean_pdf_with(
    data: &[u8],
    extra: &[(u32, [f64; 4])],
    painted: &[(u32, Vec<u8>)],
) -> Result<Cleaned, CleanError> {
    let (doc, ctx) = parse_with(data);
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.encrypted {
        return Err(CleanError::Locked);
    }
    let trailer = |key: &str| ctx.trailers.iter().rev().find_map(|t| t.get(key));
    let Some(&Obj::Ref(root, root_gen)) = trailer("Root") else {
        return Err(CleanError::Damaged);
    };

    // Every version of every object, in the order they are met.
    let mut versions: Vec<(u32, u64, Source)> = Vec::new();
    let mut budget = MAX_DECODED_TOTAL;
    for rec in &ctx.objects {
        if let Some((bytes, packed)) = unpack(data, rec, ctx.crypt.as_ref(), &mut budget) {
            for (n, start, end) in packed {
                let Some(value) = Lexer::new(&bytes[..end], start).value() else {
                    return Err(CleanError::Damaged);
                };
                // Only the value: whatever follows it before the next
                // object is not part of it.
                let r = value.range;
                let text = bytes[r.start as usize..r.end() as usize].to_vec();
                versions.push((n, rec.start, Source::Packed(text, value.obj)));
            }
        } else if rec.value.get("Type").and_then(Obj::name) == Some("ObjStm") {
            // Objects we cannot unpack could be the ones the document uses.
            return Err(CleanError::Unreadable);
        }
        versions.push((rec.num, rec.start, Source::Top(rec)));
    }
    // Pages with text under black boxes, marked for redaction, or hidden:
    // their content written again without it.
    let super::redact::Rewrites {
        streams,
        source_edits,
        form_edits,
        prefixed_streams,
        verification_complete,
        removed: taken_out,
        applied,
        cuts,
        inline,
    } = super::redact::rewrites(data, &ctx, extra);
    if !verification_complete {
        return Err(CleanError::FormContentIncomplete);
    }
    let rewrite_plan = super::form_rewrite::plan_form_rewrites(
        data,
        &ctx,
        &streams,
        &source_edits,
        &form_edits,
        &prefixed_streams,
    )
    .map_err(|_| CleanError::FormContentIncomplete)?;
    // Pictures under the boxes, their pixels there zeroed: each written
    // again, dictionary and data.
    if inline {
        return Err(CleanError::PictureUnderBox);
    }
    let mut pictures: Vec<(u32, Vec<u8>, Vec<u8>)> = Vec::new();
    for cut in &cuts {
        let Some(rec) = ctx.latest(cut.num) else {
            continue;
        };
        match edit(data, &ctx, rec, &cut.unit) {
            Edited::Written(dict, bytes) => pictures.push((cut.num, dict, bytes)),
            Edited::Jpeg => match painted.iter().find(|(n, _)| *n == cut.num) {
                Some((_, jpeg)) => {
                    pictures.push((cut.num, jpeg_dict(data, rec, jpeg, &cut.unit), jpeg.clone()))
                }
                None => return Err(CleanError::JpegUnderBox),
            },
            Edited::Cannot => return Err(CleanError::PictureUnderBox),
        }
    }

    // The new objects are inserted before indexing so the graph walk sees
    // every cloned page, stream, form, and resource binding.
    for replacement in rewrite_plan.replacements {
        versions.retain(|(num, _, _)| *num != replacement.num);
        let value = Lexer::new(&replacement.body, 0)
            .value()
            .ok_or(CleanError::Damaged)?
            .obj;
        versions.push((
            replacement.num,
            u64::MAX,
            Source::Rewritten {
                body: replacement.body,
                value,
                generation: replacement.generation,
            },
        ));
    }
    for added in rewrite_plan.added {
        let value = Lexer::new(&added.body, 0)
            .value()
            .ok_or(CleanError::Damaged)?
            .obj;
        versions.push((
            added.num,
            u64::MAX,
            Source::Rewritten {
                body: added.body,
                value,
                generation: 0,
            },
        ));
    }
    let mut current = Current::new(versions);
    // Comments and who wrote them, and the files carried inside, as Word's
    // copy loses them; what a person typed or signed on the page stays.
    let (notes, files) = people_and_files(&mut current, root);

    // Everything the catalog reaches, and nothing else.
    let mut reached = vec![false; current.0.len()];
    let mut queue = vec![root];
    while let Some(n) = queue.pop() {
        let Some(i) = current.index(n) else { continue };
        if std::mem::replace(&mut reached[i], true) {
            continue;
        }
        match &current.0[i].2 {
            Source::Top(rec) => refs(&rec.value, &mut queue),
            Source::Packed(_, obj) => refs(obj, &mut queue),
            Source::Rewritten { value, .. } => refs(value, &mut queue),
        }
    }

    let version = doc.version.as_deref().unwrap_or("1.7");
    let mut out = b"%PDF-".to_vec();
    out.extend_from_slice(version.as_bytes());
    out.extend_from_slice(b"\n%");
    out.extend_from_slice(&[0xE2, 0xE3, 0xCF, 0xD3, b'\n']);
    let mut offsets: Vec<(u32, u16, usize)> = Vec::new();
    let mut xmp_bytes = 0u64;
    let mut photo_bytes = 0u64;
    for (n, _, source) in current
        .0
        .iter()
        .zip(&reached)
        .filter(|(_, r)| **r)
        .map(|(c, _)| c)
    {
        let n = *n;
        let at = out.len();
        // A redaction applied: its mark is retired, a hidden empty annotation
        // in its place so the page's list of annotations still holds.
        if applied.contains(&n) {
            object_header(&mut out, n, 0);
            out.extend_from_slice(RETIRED);
            out.extend_from_slice(b"\nendobj\n");
            offsets.push((n, 0, at));
            continue;
        }
        match source {
            Source::Rewritten {
                body, generation, ..
            } => {
                object_header(&mut out, n, *generation);
                out.extend_from_slice(body);
                out.extend_from_slice(b"\nendobj\n");
                offsets.push((n, *generation, at));
            }
            Source::Top(rec) => {
                let gen_ = rec.gen_;
                object_header(&mut out, n, gen_);
                let is_xmp = rec.value.get("Type").and_then(Obj::name) == Some("Metadata");
                // A picture rewritten for the copy, found here and not in a
                // match guard: `if let` guards need a newer compiler than
                // the crate asks for (rust-version).
                let picture = match rec.stream {
                    Some(_) if !is_xmp => pictures.iter().find(|p| p.0 == n),
                    _ => None,
                };
                match (rec.stream, picture) {
                    (Some((range, _)), _) if is_xmp => {
                        xmp_bytes += range.len;
                        out.extend_from_slice(b"<< /Type /Metadata /Subtype /XML /Length ");
                        decimal(&mut out, EMPTY_XMP.len() as u64, 0);
                        out.extend_from_slice(b" >>\nstream\n");
                        out.extend_from_slice(EMPTY_XMP);
                        out.extend_from_slice(b"\nendstream");
                    }
                    (Some(_), Some((_, dict, bytes))) => {
                        out.extend_from_slice(dict);
                        out.extend_from_slice(b"\nstream\n");
                        out.extend_from_slice(bytes);
                        out.extend_from_slice(b"\nendstream");
                    }
                    (Some((range, _)), None) => {
                        out.extend_from_slice(slice(data, rec.value_range));
                        out.extend_from_slice(b"\nstream\n");
                        let start = out.len();
                        out.extend_from_slice(slice(data, range));
                        if is_photo(rec) {
                            // The output owns these bytes already. Blank metadata
                            // there rather than allocating a second full JPEG.
                            photo_bytes += blank_photo(&mut out[start..]);
                        }
                        out.extend_from_slice(b"\nendstream");
                    }
                    (None, _) => out.extend_from_slice(slice(data, rec.value_range)),
                }
                out.extend_from_slice(b"\nendobj\n");
                offsets.push((n, gen_, at));
            }
            Source::Packed(text, _) => {
                object_header(&mut out, n, 0);
                out.extend_from_slice(text);
                out.extend_from_slice(b"\nendobj\n");
                offsets.push((n, 0, at));
            }
        }
    }

    // One subsection per run of consecutive numbers, and the free head.
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n0 1\n0000000000 65535 f\r\n");
    let mut i = 0;
    while i < offsets.len() {
        let mut j = i + 1;
        while j < offsets.len() && offsets[j - 1].0.checked_add(1) == Some(offsets[j].0) {
            j += 1;
        }
        decimal(&mut out, u64::from(offsets[i].0), 0);
        out.push(b' ');
        decimal(&mut out, (j - i) as u64, 0);
        out.push(b'\n');
        for &(_, gen_, at) in &offsets[i..j] {
            // Classic xref entries have exactly ten offset digits.
            if at as u64 > 9_999_999_999 {
                return Err(CleanError::Damaged);
            }
            xref_entry(&mut out, at, gen_);
        }
        i = j;
    }
    let size = offsets.last().map_or(1, |&(n, ..)| u64::from(n) + 1);
    out.extend_from_slice(b"trailer\n<< /Size ");
    decimal(&mut out, size, 0);
    out.extend_from_slice(b" /Root ");
    decimal(&mut out, u64::from(root), 0);
    out.push(b' ');
    decimal(&mut out, u64::from(root_gen), 0);
    out.extend_from_slice(b" R >>\nstartxref\n");
    decimal(&mut out, xref_at as u64, 0);
    out.extend_from_slice(b"\n%%EOF\n");

    // What went, in words.
    let mut removed = Vec::new();
    let length = |rec: &ObjRec| doc.tree.get(rec.node).range.len;
    let info = match trailer("Info") {
        Some(&Obj::Ref(n, _)) => Some(n),
        _ => None,
    };
    let (mut info_bytes, mut earlier, mut unused) = (0u64, 0u64, 0u64);
    for rec in &ctx.objects {
        let ty = rec.value.get("Type").and_then(Obj::name);
        let superseded = match current.get(rec.num) {
            Some(Source::Top(current)) => !std::ptr::eq(*current, rec),
            Some(Source::Rewritten { .. }) => false,
            Some(Source::Packed(..)) | None => true,
        };
        if Some(rec.num) == info {
            // Counted below, wherever its current version is.
        } else if matches!(ty, Some("XRef" | "ObjStm")) {
            // Structure the rewrite replaces.
        } else if superseded {
            earlier += length(rec);
        } else if !current.index(rec.num).is_some_and(|i| reached[i]) {
            unused += length(rec);
        }
    }
    match info.and_then(|n| current.get(n)) {
        Some(Source::Top(rec)) => info_bytes = length(rec),
        Some(Source::Packed(text, _)) => info_bytes = text.len() as u64,
        Some(Source::Rewritten { body, .. }) => info_bytes = body.len() as u64,
        None => {}
    }
    if info_bytes > 0 {
        removed.push(Removed {
            what: "Document information: author, title, programs, dates".into(),
            bytes: info_bytes,
        });
    }
    if xmp_bytes > 0 {
        removed.push(Removed {
            what: "XMP: editing history and author".into(),
            bytes: xmp_bytes,
        });
    }
    if notes {
        removed.push(Removed {
            what: "Comments and who wrote them".into(),
            bytes: 0,
        });
    }
    if files {
        removed.push(Removed {
            what: "Files carried inside".into(),
            bytes: 0,
        });
    }
    // First: it is what someone checking a redaction wants to read.
    if taken_out > 0 {
        removed.insert(
            0,
            Removed {
                what: format!(
                    "Text under black boxes, marked for redaction, or hidden: {taken_out} {}",
                    if taken_out == 1 {
                        "character"
                    } else {
                        "characters"
                    }
                ),
                bytes: taken_out,
            },
        );
    }
    if !pictures.is_empty() {
        removed.insert(
            usize::from(taken_out > 0),
            Removed {
                what: format!(
                    "What {} under the boxes showed",
                    if pictures.len() == 1 {
                        "a picture".to_string()
                    } else {
                        format!("{} pictures", pictures.len())
                    }
                ),
                bytes: pictures.iter().map(|p| p.2.len() as u64).sum(),
            },
        );
    }
    if photo_bytes > 0 {
        removed.push(Removed {
            what: "EXIF and XMP of the photos in it: camera, location, dates".into(),
            bytes: photo_bytes,
        });
    }
    if doc.edits > 0 || earlier > 0 {
        removed.push(Removed {
            what: "Earlier versions of the document, kept by later edits".into(),
            bytes: earlier,
        });
    }
    if unused > 0 {
        removed.push(Removed {
            what: "Objects the document no longer uses".into(),
            bytes: unused,
        });
    }
    if let Some(Obj::Array(ids)) = trailer("ID") {
        let bytes = ids
            .iter()
            .map(|i| match &i.obj {
                Obj::Str(s) => s.len() as u64,
                _ => 0,
            })
            .sum();
        removed.push(Removed {
            what: "The file identifier that links copies of the document".into(),
            bytes,
        });
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

/// Zeroes, in place, what a camera writes into a JPEG: EXIF and XMP (APP1)
/// and Photoshop's IPTC (APP13). Every length stays, so the stream's
/// `/Length` does too, and a reader skips a segment it does not recognise.
/// Returns the bytes zeroed.
pub(crate) fn blank_photo(jpeg: &mut [u8]) -> u64 {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return 0;
    }
    let mut at = 2;
    let mut blanked = 0;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF {
        let marker = jpeg[at + 1];
        match marker {
            // Fill bytes before a marker.
            0xFF => {
                at += 1;
                continue;
            }
            // The picture starts, or ends: nothing after is metadata.
            0xDA | 0xD9 => break,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => {
                at += 2;
                continue;
            }
            _ => {}
        }
        let len = u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]) as usize;
        let end = at + 2 + len;
        if len < 2 || end > jpeg.len() {
            break;
        }
        if marker == 0xE1 || marker == 0xED {
            jpeg[at + 4..end].fill(0);
            blanked += (len - 2) as u64;
        }
        at = end;
    }
    blanked
}

fn slice(data: &[u8], range: crate::model::ByteRange) -> &[u8] {
    data.get(range.start as usize..range.end() as usize)
        .unwrap_or_default()
}

/// Every object number `obj` refers to.
fn refs(obj: &Obj, out: &mut Vec<u32>) {
    match obj {
        Obj::Ref(n, _) => out.push(*n),
        Obj::Array(items) => items.iter().for_each(|i| refs(&i.obj, out)),
        Obj::Dict(entries) => entries.iter().for_each(|e| refs(&e.value.obj, out)),
        _ => {}
    }
}

#[cfg(test)]
mod writer_tests {
    use super::*;

    #[test]
    fn decimal_matches_standard_formatting_at_boundaries() {
        for value in [
            0,
            1,
            9,
            10,
            99,
            100,
            65_535,
            u32::MAX as u64,
            9_999_999_999,
            u64::MAX,
        ] {
            for width in [0, 1, 5, 10, 20] {
                let mut out = Vec::new();
                decimal(&mut out, value, width);
                assert_eq!(out, format!("{value:0width$}").as_bytes());
            }
        }
    }

    #[test]
    fn object_and_xref_fields_keep_exact_wire_bytes() {
        let mut out = Vec::new();
        object_header(&mut out, u32::MAX, u16::MAX);
        assert_eq!(out, b"4294967295 65535 obj\n");
        out.clear();
        xref_entry(&mut out, 42, 7);
        assert_eq!(out, b"0000000042 00007 n\r\n");
        out.clear();
        xref_entry(&mut out, 0, u16::MAX);
        assert_eq!(out, b"0000000000 65535 n\r\n");
    }

    #[test]
    fn blanking_jpeg_in_output_leaves_prefix_and_picture_unchanged() {
        let jpeg = [0xff, 0xd8, 0xff, 0xe1, 0, 4, 1, 2, 0xff, 0xda, 9, 8];
        let mut expected = jpeg;
        let expected_removed = blank_photo(&mut expected);
        let mut out = b"stream\n".to_vec();
        let start = out.len();
        out.extend_from_slice(&jpeg);
        assert_eq!(blank_photo(&mut out[start..]), expected_removed);
        assert_eq!(&out[..start], b"stream\n");
        assert_eq!(&out[start..], &expected);
        assert_eq!(expected_removed, 2);
    }
}
