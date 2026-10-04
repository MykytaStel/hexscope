//! What a PDF's pages hold that a reader of the page does not see.
//!
//! - Text a black box covers but does not remove. Drawing a filled
//!   rectangle over a name hides it on screen and on paper, and nothing
//!   else: the name is still in the page, for anyone to select, copy or
//!   search.
//! - Areas marked for redaction (Redact annotations, 12.5.6.23) and never
//!   applied: until a tool applies them, what they mark is all still there.
//! - Text no one can see: drawn with no ink, in white on white, off the
//!   page, or too small — which a search, a screen reader or a program
//!   reading the file finds, such as hidden instructions in a CV.
//! - Comments, and who wrote them (12.5.6.2).
//! - Text an update took off a page, which the file still holds.
//!
//! Each page is painted by [`super::page::walk`]; the clean copy uses the
//! same walk to take the covered and hidden glyphs out of the content.

use super::facts::{Found, cap, decode, insert_update, resolve, text};
use super::fonts::{Fonts, page_fonts};
use super::lexer::Obj;
use super::page::{Area, Glyph, Hidden, StreamEdit, Walked, apply_edits, num, readable, walk};
use super::{Ctx, ObjRec};
use crate::model::{NodeId, ParseTree, Value};
use crate::zip::DocumentFact;

/// Pages walked, at most.
const MAX_PAGES: usize = 2000;
/// Pieces of text kept to draw a page, and pages drawn.
const MAX_DRAWN: usize = 200;
const MAX_DRAWN_PAGES: usize = 50;
/// Facts about hidden text, at most: one per page and kind.
const MAX_HIDDEN_FACTS: usize = 20;
/// Content decompressed for these checks, in all.
const BUDGET: u64 = 64 * 1024 * 1024;
/// Comment authors named in a line, at most.
const MAX_NAMES: usize = 4;
/// US Letter, when a page does not say its size.
const LETTER: [f64; 4] = [0.0, 0.0, 612.0, 792.0];

/// Annotation types that are comments: notes, highlights, marks (12.5.6.2).
/// Attached files are named apart, in [`super::forms`].
const COMMENTS: [&str; 14] = [
    "Text",
    "FreeText",
    "Highlight",
    "Underline",
    "StrikeOut",
    "Squiggly",
    "Ink",
    "Caret",
    "Stamp",
    "Line",
    "Square",
    "Circle",
    "Polygon",
    "PolyLine",
];

/// One page as the checks need it.
pub(super) struct Page {
    pub num: u32,
    pub(super) dict: Obj,
    pub(super) node: NodeId,
    pub(super) media: [f64; 4],
    pub(super) resources: Option<Obj>,
}

/// The pages found in the tree, and whether every declared child was read.
pub(super) struct PageSet {
    pub pages: Vec<Page>,
    pub complete: bool,
}

/// The pages in reading order, from the catalog down the page tree (7.7.3),
/// each with the size and resources it may inherit (7.7.3.4).
pub(super) fn pages(data: &[u8], ctx: &Ctx, budget: &mut u64) -> PageSet {
    let mut out = Vec::new();
    let Some(root) = ctx.trailers.iter().rev().find_map(|t| match t.get("Root") {
        Some(Obj::Ref(n, _)) => Some(*n),
        _ => None,
    }) else {
        return PageSet {
            pages: out,
            complete: false,
        };
    };
    let catalog = match resolve(data, ctx, root, budget) {
        Some(Found::Top(rec)) => rec.value.clone(),
        Some(Found::Packed(obj, _)) => obj,
        None => {
            return PageSet {
                pages: out,
                complete: false,
            };
        }
    };
    let mut seen = Vec::new();
    let mut stack = vec![(catalog.get("Pages").cloned(), LETTER, None::<Obj>)];
    let mut complete = true;
    while let Some((next, inherited, resources)) = stack.pop() {
        if out.len() >= MAX_PAGES || seen.len() > MAX_PAGES * 4 {
            complete = false;
            break;
        }
        let Some(Obj::Ref(n, _)) = next else {
            complete = false;
            continue;
        };
        if seen.contains(&n) {
            complete = false;
            continue;
        }
        seen.push(n);
        let (dict, node) = match resolve(data, ctx, n, budget) {
            Some(Found::Top(rec)) => (rec.value.clone(), rec.node),
            Some(Found::Packed(obj, id)) => (obj, id),
            None => {
                complete = false;
                continue;
            }
        };
        let media = match dict.get("MediaBox") {
            Some(Obj::Array(a)) if a.len() == 4 => {
                let v: Vec<f64> = a.iter().filter_map(|i| num(&i.obj)).collect();
                match v[..] {
                    [l, b, r, t] if r != l && t != b => [l.min(r), b.min(t), l.max(r), b.max(t)],
                    _ => inherited,
                }
            }
            _ => inherited,
        };
        let resources = dict.get("Resources").cloned().or(resources);
        match (dict.get("Type").and_then(Obj::name), dict.get("Kids")) {
            (Some("Pages"), Some(Obj::Array(kids))) => {
                // Last first, so the first page comes off the stack first.
                stack.extend(
                    kids.iter()
                        .rev()
                        .map(|k| (Some(k.obj.clone()), media, resources.clone())),
                );
            }
            (Some("Page"), None) => out.push(Page {
                num: n,
                dict,
                node,
                media,
                resources,
            }),
            _ => complete = false,
        }
    }
    PageSet {
        pages: out,
        complete,
    }
}

/// A top-level object by number, the latest one.
fn top(ctx: &Ctx, n: u32) -> Option<&ObjRec> {
    ctx.latest(n)
}

struct PageContent<'a> {
    bytes: Vec<u8>,
    parts: Vec<(&'a ObjRec, usize, usize)>,
    complete: bool,
}

/// A page's content, its streams read as one (7.8.2), and where each
/// stream's bytes are in it: `(object number, start, end)`.
fn content<'a>(data: &[u8], ctx: &'a Ctx, page: &Obj, budget: &mut u64) -> PageContent<'a> {
    let mut complete = true;
    let refs: Vec<u32> = match page.get("Contents") {
        None => Vec::new(),
        Some(Obj::Ref(n, _)) => vec![*n],
        Some(Obj::Array(a)) => a
            .iter()
            .filter_map(|i| match i.obj {
                Obj::Ref(n, _) => Some(n),
                _ => {
                    complete = false;
                    None
                }
            })
            .collect(),
        Some(_) => {
            complete = false;
            Vec::new()
        }
    };
    let mut bytes = Vec::new();
    let mut parts = Vec::new();
    for n in refs {
        let Some(rec) = top(ctx, n) else {
            complete = false;
            continue;
        };
        if let Some(b) = super::page::decode_content_stream(data, rec, ctx, budget) {
            let start = bytes.len();
            bytes.extend_from_slice(&b);
            parts.push((rec, start, bytes.len()));
            bytes.push(b'\n');
        } else {
            complete = false;
        }
    }
    PageContent {
        bytes,
        parts,
        complete,
    }
}

/// A page's annotations, with the node of each and its object number when
/// it is an object of its own.
fn annotations(ctx: &Ctx, page: &Page) -> Vec<(Obj, NodeId, Option<u32>)> {
    let Some(Obj::Array(annots)) = page.dict.get("Annots") else {
        return Vec::new();
    };
    annots
        .iter()
        .filter_map(|a| match &a.obj {
            Obj::Ref(n, _) => top(ctx, *n).map(|r| (r.value.clone(), r.node, Some(*n))),
            d @ Obj::Dict(_) => Some((d.clone(), page.node, None)),
            _ => None,
        })
        .collect()
}

/// An annotation's rectangle.
fn rect(a: &Obj) -> Option<Area> {
    let Some(Obj::Array(r)) = a.get("Rect") else {
        return None;
    };
    let v: Vec<f64> = r.iter().filter_map(|i| num(&i.obj)).collect();
    match v[..] {
        [l, b, r, t] => Some(Area([l.min(r), b.min(t), l.max(r), b.max(t)])),
        _ => None,
    }
}

fn subtype(a: &Obj) -> &str {
    a.get("Subtype").and_then(Obj::name).unwrap_or("")
}

/// Everything about one page the checks and the clean copy need.
struct Painted<'a> {
    content: Vec<u8>,
    parts: Vec<(&'a ObjRec, usize, usize)>,
    walked: Walked,
    marks: Vec<(Area, NodeId, Option<u32>)>,
    annots: Vec<(Obj, NodeId, Option<u32>)>,
}

/// A page painted, with its marks for redaction and any `extra` areas to
/// black out, which count as marks of no annotation.
fn paint<'a>(
    data: &[u8],
    ctx: &'a Ctx,
    page: &Page,
    budget: &mut u64,
    invocations: &mut usize,
    extra: &[Area],
) -> Painted<'a> {
    let fonts: Fonts = page_fonts(data, ctx, page.resources.as_ref(), budget);
    let content = content(data, ctx, &page.dict, budget);
    let source_parts: Vec<(u32, usize, usize)> = content
        .parts
        .iter()
        .map(|(rec, start, end)| (rec.num, *start, *end))
        .collect();
    let annots = annotations(ctx, page);
    let mut marks: Vec<(Area, NodeId, Option<u32>)> = annots
        .iter()
        .filter(|(a, ..)| subtype(a) == "Redact")
        .filter_map(|(a, n, num)| Some((rect(a)?, *n, *num)))
        .collect();
    marks.extend(extra.iter().map(|a| (*a, page.node, None)));
    let areas: Vec<Area> = marks.iter().map(|m| m.0).collect();
    let version = super::effective_version(data, ctx, super::pdf_header_version(data).as_deref());
    let mut walked = super::page::walk_page(
        &content.bytes,
        &fonts,
        Area(page.media),
        &areas,
        data,
        ctx,
        page.resources.clone(),
        version,
        page.num,
        budget,
        invocations,
        &source_parts,
    );
    walked.complete &= content.complete;
    Painted {
        content: content.bytes,
        parts: content.parts,
        walked,
        marks,
        annots,
    }
}

/// The glyphs someone reading the page cannot see: under a box or a mark,
/// or hidden.
fn unseen(g: &Glyph) -> bool {
    g.covered || g.marked || g.hidden.is_some()
}

/// What the checks find on each page: warnings in the tree, facts, and
/// pages to draw.
pub(super) fn check(
    data: &[u8],
    tree: &mut ParseTree,
    ctx: &Ctx,
    facts: &mut Vec<DocumentFact>,
) -> Vec<Blackout> {
    let mut drawn = Vec::new();
    let mut budget = BUDGET;
    let mut invocations = 0;
    let mut hidden_facts = 0;
    let mut authors: Vec<String> = Vec::new();
    let mut comments = 0;
    let mut comment_node = None;
    // Links whose words name one site while they go to another.
    let mut elsewhere: Vec<String> = Vec::new();
    let mut elsewhere_node = None;
    let page_set = pages(data, ctx, &mut budget);
    if !page_set.complete
        && let Some(root) = tree.root()
    {
        let range = tree.get(root).range;
        tree.warning(root, "PDF page tree could not be fully checked", range);
    }
    for (i, page) in page_set.pages.iter().enumerate() {
        let number = i + 1;
        let p = paint(data, ctx, page, &mut budget, &mut invocations, &[]);
        let w = &p.walked;
        for (a, node, _) in p.annots.iter().filter(|(a, ..)| subtype(a) == "Link") {
            let action = match a.get("A") {
                Some(Obj::Ref(n, _)) => top(ctx, *n).map(|r| r.value.clone()),
                Some(d @ Obj::Dict(_)) => Some(d.clone()),
                _ => None,
            };
            let Some(Obj::Str(uri)) = action.as_ref().and_then(|d| d.get("URI")) else {
                continue;
            };
            let (Some(to), Some(area)) = (crate::eml::links::host(&text(uri)), rect(a)) else {
                continue;
            };
            // The words drawn inside the link's box.
            let words: String = w
                .glyphs
                .iter()
                .filter(|g| !unseen(g))
                .filter(|g| {
                    let (x, y) = (
                        (g.area.0[0] + g.area.0[2]) / 2.0,
                        (g.area.0[1] + g.area.0[3]) / 2.0,
                    );
                    x >= area.0[0] && x <= area.0[2] && y >= area.0[1] && y <= area.0[3]
                })
                .map(|g| w.text_of(g))
                .collect();
            if crate::eml::links::says_elsewhere(&words, &to) {
                let line = format!(
                    "page {number}: a link reads “{}” and goes to {to}",
                    words.trim()
                );
                if !elsewhere.contains(&line) {
                    elsewhere.push(line);
                }
                elsewhere_node.get_or_insert(*node);
            }
        }
        // The warnings hang on the page's first content stream.
        let at = p.parts.first().map(|(rec, ..)| {
            (
                rec.node,
                rec.stream.map_or(tree.get(rec.node).range, |s| s.0),
            )
        });

        if !w.complete {
            let (node, range) = at.unwrap_or((page.node, tree.get(page.node).range));
            let warning = tree.warning(
                node,
                "PDF page or form content could not be fully checked",
                range,
            );
            let text =
                format!("page {number}: some page or form content could not be fully checked");
            tree.set_value(warning, Some(Value::Text(text.clone())));
            insert_update(facts, fact("form-incomplete", text, warning));
        }

        let covered = w.pieces(|g| g.covered);
        if let Some((node, range)) = at
            && !covered.is_empty()
        {
            let words = words(&covered);
            let warning = tree.warning(node, "text under a black box", range);
            tree.set_value(warning, Some(Value::Text(words.clone())));
            insert_update(
                facts,
                fact("covered", format!("page {number}: {words}"), warning),
            );
        }
        // Boxes over pictures, less those a copy already blacked out under.
        let over: Vec<Area> = w
            .over_pictures
            .iter()
            .filter(|b| {
                let mut cuts = Vec::new();
                super::pictures::cuts(
                    data,
                    ctx,
                    page.resources.as_ref(),
                    &w.placed,
                    std::slice::from_ref(*b),
                    &mut budget,
                    &mut cuts,
                );
                cuts.iter().any(|c| {
                    c.unit
                        .iter()
                        .any(|u| !super::pictures::already_blacked(ctx, c.num, u))
                })
            })
            .copied()
            .collect();
        let n = over.len();
        if let Some((node, range)) = at
            && n > 0
        {
            let warning = tree.warning(node, "a picture under a black box", range);
            let text = if n == 1 {
                format!(
                    "page {number}: a black box over part of a picture, which still holds what the box hides"
                )
            } else {
                format!(
                    "page {number}: {n} black boxes over parts of a picture, which still holds what they hide"
                )
            };
            tree.set_value(warning, Some(Value::Text(text.clone())));
            insert_update(facts, fact("covered", text, warning));
        }
        if let Some(&(_, node, _)) = p.marks.first() {
            let range = tree.get(node).range;
            let warning = tree.warning(node, "marked for redaction, never redacted", range);
            let n = p.marks.len();
            let what = if n == 1 {
                "an area marked for redaction was".to_string()
            } else {
                format!("{n} areas marked for redaction were")
            };
            let under = w.pieces(|g| g.marked);
            let text = if under.is_empty() {
                format!("page {number}: {what} never applied")
            } else {
                format!("page {number}: {what} never applied: {}", words(&under))
            };
            insert_update(facts, fact("covered", text, warning));
        }
        if (!covered.is_empty() || !p.marks.is_empty() || n > 0) && drawn.len() < MAX_DRAWN_PAGES {
            let blacked = |g: &Glyph| g.covered || g.marked;
            let pieces = w.pieces(blacked);
            let beside = |g: &Glyph| {
                !unseen(g)
                    && pieces.iter().any(|(a, _)| {
                        let h = a.0[3] - a.0[1];
                        (g.area.0[1] - a.0[1]).abs() < 0.3 * h
                    })
            };
            let context = w.pieces(beside);
            let shown = |v: Vec<(Area, String)>| -> Vec<([f64; 4], String)> {
                v.into_iter()
                    .take(MAX_DRAWN)
                    .map(|(a, t)| (a.0, readable(&t)))
                    .collect()
            };
            drawn.push(Blackout {
                page: number,
                media: page.media,
                boxes: w.boxes.iter().chain(&over).map(|a| a.0).collect(),
                texts: shown(pieces),
                context: shown(context)
                    .into_iter()
                    .filter(|(_, t)| !t.is_empty())
                    .collect(),
            });
        }

        // Text no one can see, kind by kind.
        let kinds = [
            (Hidden::Invisible, "text drawn invisibly", "drawn invisibly"),
            (Hidden::White, "text in white on white", "in white on white"),
            (Hidden::OffPage, "text placed off the page", "off the page"),
            (Hidden::Tiny, "text too small to see", "too small to see"),
        ];
        for (kind, label, how) in kinds {
            let pieces = w.pieces(|g| g.hidden == Some(kind) && !g.covered && !g.marked);
            let Some((node, range)) = at else { break };
            if pieces.is_empty() || hidden_facts >= MAX_HIDDEN_FACTS {
                continue;
            }
            hidden_facts += 1;
            let words = words(&pieces);
            let warning = tree.warning(node, label, range);
            tree.set_value(warning, Some(Value::Text(words.clone())));
            insert_update(
                facts,
                fact(
                    "hiddentext",
                    format!("page {number}, {how}: {words}"),
                    warning,
                ),
            );
        }

        for (a, node, _) in &p.annots {
            if COMMENTS.contains(&subtype(a)) {
                comments += 1;
                comment_node.get_or_insert(*node);
                if let Some(Obj::Str(t)) = a.get("T") {
                    authors.push(text(t).trim().to_string());
                }
            }
        }
    }
    if let Some(node) = elsewhere_node {
        let range = tree.get(node).range;
        tree.warning(node, "a link goes somewhere other than it says", range);
        let more = elsewhere.len().saturating_sub(2);
        let mut text = elsewhere[..elsewhere.len().min(2)].join("; ");
        if more > 0 {
            text.push_str(&format!("; and {more} more"));
        }
        insert_update(facts, fact("linkmismatch", text, node));
    }
    if let Some(node) = comment_node {
        let what = format!(
            "{comments} {}",
            if comments == 1 { "comment" } else { "comments" }
        );
        insert_update(facts, fact("comments", by(what, &authors), node));
    }
    drawn
}

/// A domain without its subdomains: `login.example.org` is `example.org`.
fn fact(kind: &'static str, text: String, node: NodeId) -> DocumentFact {
    DocumentFact {
        kind,
        text: cap(text),
        node,
    }
}

/// `3 comments by Olena and Petro`: distinct authors in order of appearance.
fn by(what: String, authors: &[String]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for a in authors {
        if !a.is_empty() && !names.contains(&a.as_str()) {
            names.push(a);
        }
    }
    let more = names.len().saturating_sub(MAX_NAMES);
    names.truncate(MAX_NAMES);
    let mut list = names.join(", ");
    if more > 0 {
        list = format!(
            "{list} and {more} {}",
            if more == 1 { "other" } else { "others" }
        );
    } else if let Some(i) = list.rfind(", ") {
        list.replace_range(i..i + 2, " and ");
    }
    if list.is_empty() {
        what
    } else {
        format!("{what} by {list}")
    }
}

/// What the clean copy writes differently, page by page.
pub(crate) struct ContentRewrite {
    pub(super) page: u32,
    pub(super) object: u32,
    pub(super) bytes: Vec<u8>,
}

pub(crate) struct Rewrites {
    /// Decoded page streams rewritten, identified by page and object.
    pub(super) streams: Vec<ContentRewrite>,
    /// All source-aware text edits, including page and form streams.
    pub(super) source_edits: Vec<StreamEdit>,
    /// Redactions in form streams, with the invocation that reaches each one.
    pub(super) form_edits: Vec<StreamEdit>,
    /// Page streams prefixed with `q\n` before their decoded contents.
    pub(super) prefixed_streams: Vec<(u32, u32)>,
    /// Whether every page-tree entry was resolved and visited.
    pub(super) page_tree_complete: bool,
    /// Pages whose content streams or invoked forms could not be inspected fully.
    pub(super) incomplete_pages: Vec<u32>,
    /// Glyphs taken out.
    pub(super) removed: u64,
    /// Redaction marks applied, by object number: each is now done.
    pub(super) applied: Vec<u32>,
    /// Pictures the marks cover part of: their pixels there go too.
    pub(super) cuts: Vec<super::pictures::Cut>,
    /// Whether a mark covers a picture written into a page's content,
    /// which cannot be edited.
    pub(super) inline: bool,
}

/// The page streams the clean copy rewrites. Covered, marked and hidden
/// glyphs go; every other glyph stays where it was. A page with areas
/// marked for redaction gets them filled in black and the marks retired,
/// as applying the redaction would. `extra` are more areas to black out
/// the same way, by page number counted from 1: what a person chose.
pub(crate) fn rewrites(data: &[u8], ctx: &Ctx, extra: &[(u32, [f64; 4])]) -> Rewrites {
    let mut out: Vec<ContentRewrite> = Vec::new();
    let mut source_edits = Vec::new();
    let mut form_edits = Vec::new();
    let mut prefixed_streams = Vec::new();
    let mut incomplete_pages = Vec::new();
    let mut applied = Vec::new();
    let mut cuts = Vec::new();
    let mut inline = false;
    let mut removed = 0u64;
    let mut budget = BUDGET;
    let mut invocations = 0;
    let page_set = pages(data, ctx, &mut budget);
    let page_tree_complete = page_set.complete;
    for (i, page) in page_set.pages.into_iter().enumerate() {
        let mine: Vec<Area> = extra
            .iter()
            .filter(|(n, _)| *n as usize == i + 1)
            .map(|(_, a)| Area(*a))
            .collect();
        let p = paint(data, ctx, &page, &mut budget, &mut invocations, &mine);
        if !p.walked.complete {
            incomplete_pages.push(page.num);
        }
        // Pictures lose their pixels under the marks, and under dark boxes
        // drawn over them, as text under a box loses its letters.
        if !p.marks.is_empty() || !p.walked.over_pictures.is_empty() {
            let areas: Vec<Area> = p
                .marks
                .iter()
                .map(|m| m.0)
                .chain(p.walked.over_pictures.iter().copied())
                .collect();
            super::pictures::cuts(
                data,
                ctx,
                page.resources.as_ref(),
                &p.walked.placed,
                &areas,
                &mut budget,
                &mut cuts,
            );
            inline |= p
                .walked
                .inline
                .iter()
                .any(|i| areas.iter().any(|a| i.inside(a) > 0.0));
        }
        let edits = p.walked.edits(unseen);
        source_edits.extend(edits.iter().cloned());
        let direct: Vec<_> = edits
            .iter()
            .filter(|edit| edit.target.calls.is_empty())
            .collect();
        form_edits.extend(
            edits
                .iter()
                .filter(|edit| !edit.target.calls.is_empty())
                .cloned(),
        );
        if (edits.is_empty() && p.marks.is_empty()) || p.parts.is_empty() {
            continue;
        }
        removed += p.walked.glyphs.iter().filter(|g| unseen(g)).count() as u64;
        applied.extend(p.marks.iter().filter_map(|m| m.2));
        let last = p.parts.len() - 1;
        for (k, &(rec, start, end)) in p.parts.iter().enumerate() {
            let mine: Vec<(usize, usize, Vec<u8>)> = direct
                .iter()
                .filter(|edit| edit.target.object == rec.num)
                .filter(|edit| edit.end <= end - start)
                .map(|edit| (edit.start, edit.end, edit.replacement.clone()))
                .collect();
            let mut bytes = apply_edits(&p.content[start..end], &mine);
            if !p.marks.is_empty() {
                if k == 0 {
                    bytes.splice(0..0, b"q\n".iter().copied());
                    prefixed_streams.push((page.num, rec.num));
                }
                if k == last {
                    bytes.extend_from_slice(b"\nQ\nq 0 g");
                    for (a, ..) in &p.marks {
                        let [l, b, r, t] = a.0;
                        let n = |v: f64| crate::fixed::fixed(v, 2);
                        bytes.extend_from_slice(
                            format!(" {} {} {} {} re", n(l), n(b), n(r - l), n(t - b)).as_bytes(),
                        );
                    }
                    bytes.extend_from_slice(b" f Q\n");
                }
            }
            let selected_form_call = form_edits.iter().any(|edit| {
                edit.target.page == page.num
                    && edit
                        .target
                        .calls
                        .first()
                        .is_some_and(|site| site.owner == rec.num)
            });
            if !mine.is_empty() || !p.marks.is_empty() || selected_form_call {
                out.retain(|rewrite| !(rewrite.page == page.num && rewrite.object == rec.num));
                out.push(ContentRewrite {
                    page: page.num,
                    object: rec.num,
                    bytes,
                });
            }
        }
    }
    Rewrites {
        streams: out,
        source_edits,
        form_edits,
        prefixed_streams,
        page_tree_complete,
        incomplete_pages,
        removed,
        applied,
        cuts,
        inline,
    }
}

/// The pictures page `number` (from 1) draws, each with the matrix that
/// places its unit square: to show a scanned page while choosing what to
/// black out on it.
pub(crate) fn page_pictures(data: &[u8], ctx: &Ctx, number: u32) -> Vec<(u32, [f64; 6])> {
    let start = number.saturating_sub(1);
    pictures_by_page(data, ctx, start, 1)
        .into_iter()
        .map(|(_, num, m)| (num, m))
        .collect()
}

/// The pictures pages `first + 1` to `first + count` draw: each with its
/// page, counted from 1, its object and where it goes.
pub(crate) fn pictures_by_page(
    data: &[u8],
    ctx: &Ctx,
    first: u32,
    count: u32,
) -> Vec<(u32, u32, [f64; 6])> {
    let mut budget = BUDGET;
    let mut out = Vec::new();
    let mut invocations = 0;
    let page_set = pages(data, ctx, &mut budget);
    for (i, page) in page_set
        .pages
        .iter()
        .enumerate()
        .skip(first as usize)
        .take(count as usize)
    {
        let p = paint(data, ctx, page, &mut budget, &mut invocations, &[]);
        super::pictures::visit(
            data,
            ctx,
            page.resources.as_ref(),
            &p.walked.placed,
            &mut budget,
            0,
            &mut |seen, m| {
                if let super::pictures::Seen::Picture(rec) = seen {
                    out.push((i as u32 + 1, rec.num, *m));
                }
            },
        );
    }
    out
}

/// One page's visible text, glyph by glyph, where each lands: what a
/// person searches to choose what to black out.
#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
    /// Counted from 1.
    pub page: u32,
    /// `[left, bottom, right, top]` in the page's points.
    pub media: [f64; 4],
    /// Whether its content, forms, and page-tree coverage were inspected fully.
    pub complete: bool,
    pub glyphs: Vec<([f64; 4], String)>,
    /// The dark boxes already on the page over text or a picture, and its
    /// marks for redaction: black in the copy too, so shown with it.
    pub boxes: Vec<[f64; 4]>,
}

/// Glyphs read for searching, at most, across the document.
const MAX_SEARCHED: usize = 500_000;

pub(super) fn page_texts(data: &[u8], ctx: &Ctx) -> Vec<PageText> {
    let mut out = Vec::new();
    let mut budget = BUDGET;
    let mut invocations = 0;
    let mut total = 0;
    let page_set = pages(data, ctx, &mut budget);
    let page_tree_complete = page_set.complete;
    for (i, page) in page_set.pages.iter().enumerate() {
        let p = paint(data, ctx, page, &mut budget, &mut invocations, &[]);
        let w = &p.walked;
        let glyphs: Vec<([f64; 4], String)> = w
            .glyphs
            .iter()
            .filter(|g| !unseen(g))
            .take(MAX_SEARCHED.saturating_sub(total))
            .map(|g| (g.area.0, w.text_of(g).to_string()))
            .collect();
        total += glyphs.len();
        out.push(PageText {
            page: i as u32 + 1,
            media: page.media,
            glyphs,
            complete: w.complete && page_tree_complete,
            boxes: w
                .boxes
                .iter()
                .chain(&w.over_pictures)
                .chain(p.marks.iter().map(|m| &m.0))
                .map(|a| a.0)
                .collect(),
        });
    }
    out
}

/// Text an update took off a page that the file still holds: lines an
/// earlier version of a page's content has and its latest version does not.
/// An incremental update (7.5.6) appends the new content and leaves the old
/// where it was, for any reader of the bytes.
pub(super) fn earlier_text(data: &[u8], ctx: &Ctx, facts: &mut Vec<DocumentFact>) {
    let mut budget = BUDGET;
    let crypt = ctx.crypt.as_ref();
    let mut gone: Vec<String> = Vec::new();
    let mut node = None;
    let objects = &ctx.objects;
    let lines = |bytes: &[u8]| -> Vec<String> {
        walk(bytes, &Vec::new(), Area(LETTER), &[])
            .pieces(|_| true)
            .into_iter()
            .map(|(_, t)| readable(&t))
            .filter(|t| !t.is_empty())
            .collect()
    };
    // Each content stream's latest version, and the ones it replaced. Only
    // a number written more than once has earlier versions, and the index
    // says which without a scan; a cap keeps a file of endless content
    // streams from making it slow.
    let contents = objects
        .iter()
        .enumerate()
        .filter(|(_, o)| is_content(o) && ctx.rewritten(o.num))
        .take(MAX_PAGES * 2);
    for (i, latest) in contents {
        if !ctx
            .latest(latest.num)
            .is_some_and(|l| std::ptr::eq(l, latest))
        {
            continue;
        }
        let earlier: Vec<&ObjRec> = objects[..i]
            .iter()
            .filter(|o| o.num == latest.num && is_content(o))
            .collect();
        if earlier.is_empty() {
            continue;
        }
        let Some(now) = decode(data, latest, crypt, &mut budget) else {
            continue;
        };
        let now = lines(&now);
        for old in earlier {
            let Some(bytes) = decode(data, old, crypt, &mut budget) else {
                continue;
            };
            for line in lines(&bytes) {
                if !now.contains(&line) && !gone.contains(&line) && gone.len() < MAX_DRAWN {
                    node.get_or_insert(old.stream.map_or(old.node, |(_, n)| n));
                    gone.push(line);
                }
            }
        }
    }
    if let Some(node) = node {
        insert_update(
            facts,
            fact("earlier", format!("“{}”", gone.join(" … ")), node),
        );
    }
}

/// A stream that looks like page content: no type, no subtype, not a font
/// program (whose dictionaries carry `Length1`), not an image.
fn is_content(rec: &ObjRec) -> bool {
    let v = &rec.value;
    rec.stream.is_some()
        && ["Type", "Subtype", "Length1", "Width"]
            .iter()
            .all(|k| v.get(k).is_none())
}

/// One page's black boxes and the text they cover, in the page's own
/// coordinates (points, the origin at the bottom left), to draw it.
#[derive(Debug, Clone, PartialEq)]
pub struct Blackout {
    /// Counted from 1.
    pub page: usize,
    /// The page: left, bottom, right, top.
    pub media: [f64; 4],
    /// The dark boxes over text or over part of a picture, and marks for
    /// redaction, the same way round.
    pub boxes: Vec<[f64; 4]>,
    /// Each covered piece of text and where it is; the text is empty when
    /// its font's codes do not read as letters.
    pub texts: Vec<([f64; 4], String)>,
    /// Text left showing on the same lines, such as the label before a
    /// covered name.
    pub context: Vec<([f64; 4], String)>,
}

/// The pieces as words, set apart by ` · `, or a count when their font's
/// codes do not read as letters.
fn words(pieces: &[(Area, String)]) -> String {
    let joined = pieces
        .iter()
        .map(|(_, t)| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if !readable(&joined).is_empty() {
        format!("“{joined}”")
    } else {
        let n = pieces.len();
        format!(
            "{n} {} of text, in a font whose codes hexscope does not turn into letters",
            if n == 1 { "piece" } else { "pieces" }
        )
    }
}

/// Dark boxes fewer than this on a page are not a code drawn in boxes.
const MIN_SHAPES: usize = 30;

/// The dark boxes pages `1` to `count` fill, their forms' too, in each
/// page's space; only pages with enough of them to be a QR code drawn in
/// boxes. Each with its page, counted from 1, and its media box.
pub(crate) fn shapes_by_page(
    data: &[u8],
    ctx: &Ctx,
    count: u32,
) -> Vec<(u32, [f64; 4], Vec<[f64; 4]>)> {
    let mut budget = BUDGET;
    let mut out = Vec::new();
    let mut invocations = 0;
    let page_set = pages(data, ctx, &mut budget);
    for (i, page) in page_set.pages.iter().enumerate().take(count as usize) {
        let p = paint(data, ctx, page, &mut budget, &mut invocations, &[]);
        let mut shapes: Vec<[f64; 4]> = p.walked.dark.iter().map(|a| a.0).collect();
        super::pictures::visit(
            data,
            ctx,
            page.resources.as_ref(),
            &p.walked.placed,
            &mut budget,
            0,
            &mut |seen, m| {
                if let super::pictures::Seen::Boxes(boxes) = seen {
                    for b in boxes {
                        let [l, bt, r, t] = b.0;
                        shapes.push(Area::of(m, l, bt, r - l, t - bt).0);
                    }
                }
            },
        );
        if shapes.len() >= MIN_SHAPES {
            out.push((i as u32 + 1, page.media, shapes));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreadable_codes_are_counted_not_shown() {
        let a = Area([0.0; 4]);
        assert_eq!(
            words(&[
                (a, "John  Smith".into()),
                (a, " ".into()),
                (a, "12 Main St ".into())
            ]),
            "“John Smith · 12 Main St”"
        );
        assert_eq!(
            words(&[(a, "\u{FFFD}\u{FFFD}\u{3}".into())]),
            "1 piece of text, in a font whose codes hexscope does not turn into letters"
        );
    }

    #[test]
    fn authors_are_named_once_and_counted_past_four() {
        let authors: Vec<String> = ["A", "B", "A", "C", "D", "E", "F"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            by("7 comments".into(), &authors),
            "7 comments by A, B, C, D and 2 others"
        );
        assert_eq!(by("1 comment".into(), &[]), "1 comment");
        assert_eq!(
            by("2 comments".into(), &authors[..2]),
            "2 comments by A and B"
        );
    }
}
