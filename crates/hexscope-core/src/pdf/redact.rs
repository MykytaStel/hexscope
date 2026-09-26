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
use super::page::{Area, Glyph, Hidden, Walked, apply_edits, num, readable, walk};
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
struct Page {
    dict: Obj,
    node: NodeId,
    media: [f64; 4],
    resources: Option<Obj>,
}

/// The pages in reading order, from the catalog down the page tree (7.7.3),
/// each with the size and resources it may inherit (7.7.3.4).
fn pages(data: &[u8], ctx: &Ctx, budget: &mut u64) -> Vec<Page> {
    let mut out = Vec::new();
    let Some(root) = ctx.trailers.iter().rev().find_map(|t| match t.get("Root") {
        Some(Obj::Ref(n, _)) => Some(*n),
        _ => None,
    }) else {
        return out;
    };
    let catalog = match resolve(data, ctx, root, budget) {
        Some(Found::Top(rec)) => rec.value.clone(),
        Some(Found::Packed(obj, _)) => obj,
        None => return out,
    };
    let mut seen = Vec::new();
    let mut stack = vec![(catalog.get("Pages").cloned(), LETTER, None::<Obj>)];
    while let Some((next, inherited, resources)) = stack.pop() {
        if out.len() >= MAX_PAGES || seen.len() > MAX_PAGES * 4 {
            break;
        }
        let Some(Obj::Ref(n, _)) = next else { continue };
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        let (dict, node) = match resolve(data, ctx, n, budget) {
            Some(Found::Top(rec)) => (rec.value.clone(), rec.node),
            Some(Found::Packed(obj, id)) => (obj, id),
            None => continue,
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
        match dict.get("Kids") {
            Some(Obj::Array(kids)) => {
                // Last first, so the first page comes off the stack first.
                stack.extend(
                    kids.iter()
                        .rev()
                        .map(|k| (Some(k.obj.clone()), media, resources.clone())),
                );
            }
            _ if dict.get("Type").and_then(Obj::name) == Some("Page") => out.push(Page {
                dict,
                node,
                media,
                resources,
            }),
            _ => {}
        }
    }
    out
}

/// A top-level object by number, the latest one.
fn top(ctx: &Ctx, n: u32) -> Option<&ObjRec> {
    ctx.objects.iter().rev().find(|o| o.num == n)
}

/// A page's content, its streams read as one (7.8.2), and where each
/// stream's bytes are in it: `(object number, start, end)`.
fn content<'a>(
    data: &[u8],
    ctx: &'a Ctx,
    page: &Obj,
    budget: &mut u64,
) -> (Vec<u8>, Vec<(&'a ObjRec, usize, usize)>) {
    let refs: Vec<u32> = match page.get("Contents") {
        Some(Obj::Ref(n, _)) => vec![*n],
        Some(Obj::Array(a)) => a
            .iter()
            .filter_map(|i| match i.obj {
                Obj::Ref(n, _) => Some(n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut bytes = Vec::new();
    let mut parts = Vec::new();
    for n in refs {
        let Some(rec) = top(ctx, n) else { continue };
        if let Some(b) = decode(data, rec, ctx.crypt.as_ref(), budget) {
            let start = bytes.len();
            bytes.extend_from_slice(&b);
            parts.push((rec, start, bytes.len()));
            bytes.push(b'\n');
        }
    }
    (bytes, parts)
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

fn paint<'a>(data: &[u8], ctx: &'a Ctx, page: &Page, budget: &mut u64) -> Painted<'a> {
    let fonts: Fonts = page_fonts(data, ctx, page.resources.as_ref(), budget);
    let (content, parts) = content(data, ctx, &page.dict, budget);
    let annots = annotations(ctx, page);
    let marks: Vec<(Area, NodeId, Option<u32>)> = annots
        .iter()
        .filter(|(a, ..)| subtype(a) == "Redact")
        .filter_map(|(a, n, num)| Some((rect(a)?, *n, *num)))
        .collect();
    let areas: Vec<Area> = marks.iter().map(|m| m.0).collect();
    let walked = walk(&content, &fonts, Area(page.media), &areas);
    Painted {
        content,
        parts,
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
    let mut hidden_facts = 0;
    let mut authors: Vec<String> = Vec::new();
    let mut comments = 0;
    let mut comment_node = None;
    for (i, page) in pages(data, ctx, &mut budget).iter().enumerate() {
        let number = i + 1;
        let p = paint(data, ctx, page, &mut budget);
        let w = &p.walked;
        // The warnings hang on the page's first content stream.
        let at = p.parts.first().map(|(rec, ..)| {
            (
                rec.node,
                rec.stream.map_or(tree.get(rec.node).range, |s| s.0),
            )
        });

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
        if (!covered.is_empty() || !p.marks.is_empty()) && drawn.len() < MAX_DRAWN_PAGES {
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
                boxes: w.boxes.iter().map(|a| a.0).collect(),
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
    if let Some(node) = comment_node {
        let what = format!(
            "{comments} {}",
            if comments == 1 { "comment" } else { "comments" }
        );
        insert_update(facts, fact("comments", by(what, &authors), node));
    }
    drawn
}

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
pub(crate) struct Rewrites {
    /// The new content of each page stream rewritten, by object number.
    pub streams: Vec<(u32, Vec<u8>)>,
    /// Glyphs taken out.
    pub removed: u64,
    /// Redaction marks applied, by object number: each is now done.
    pub applied: Vec<u32>,
}

/// The page streams the clean copy rewrites. Covered, marked and hidden
/// glyphs go; every other glyph stays where it was. A page with areas
/// marked for redaction gets them filled in black and the marks retired,
/// as applying the redaction would.
pub(crate) fn rewrites(data: &[u8], ctx: &Ctx) -> Rewrites {
    let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut applied = Vec::new();
    let mut removed = 0u64;
    let mut budget = BUDGET;
    for page in pages(data, ctx, &mut budget) {
        let p = paint(data, ctx, &page, &mut budget);
        let edits = p.walked.edits(&p.content, unseen);
        if (edits.is_empty() && p.marks.is_empty()) || p.parts.is_empty() {
            continue;
        }
        removed += p.walked.glyphs.iter().filter(|g| unseen(g)).count() as u64;
        applied.extend(p.marks.iter().filter_map(|m| m.2));
        let last = p.parts.len() - 1;
        for (k, &(rec, start, end)) in p.parts.iter().enumerate() {
            let mine: Vec<(usize, usize, Vec<u8>)> = edits
                .iter()
                .filter(|e| e.0 >= start && e.1 <= end)
                .map(|(s, e, b)| (s - start, e - start, b.clone()))
                .collect();
            let mut bytes = apply_edits(&p.content[start..end], &mine);
            if !p.marks.is_empty() {
                if k == 0 {
                    bytes.splice(0..0, b"q\n".iter().copied());
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
            if !mine.is_empty() || !p.marks.is_empty() {
                out.retain(|(n, _)| *n != rec.num);
                out.push((rec.num, bytes));
            }
        }
    }
    Rewrites {
        streams: out,
        removed,
        applied,
    }
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
    // Each content stream's latest version, and the ones it replaced. Pages
    // are few beside objects, so a scan per page is cheap; a cap keeps a
    // file of endless content streams from making it slow.
    let contents = objects
        .iter()
        .enumerate()
        .filter(|(_, o)| is_content(o))
        .take(MAX_PAGES * 2);
    for (i, latest) in contents {
        if objects[i + 1..].iter().any(|l| l.num == latest.num) {
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
    /// The dark boxes over text, and marks for redaction, the same way round.
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
