//! Text a black box covers but does not remove.
//!
//! Drawing a filled rectangle over a name hides it on screen and on paper,
//! and nothing else: the name is still in the page's content, for anyone to
//! select, copy or search. So each page's content stream is walked in the
//! order a viewer paints it (ISO 32000-1 §8, §9), keeping where each piece
//! of text went; a dark rectangle filled after it, over it, covers it.
//!
//! Positions are estimated, not typeset: a string's width is taken from its
//! length and the font size, since the font's widths are not read. That is
//! enough to tell text under a box from text beside it. Content drawn by
//! form XObjects is not followed.
//!
//! An area marked for redaction (a Redact annotation, 12.5.6.23) is not yet
//! redacted either: until a tool applies it, what it marks is all still there.

use super::facts::{Found, cap, decode, insert_update, resolve, text};
use super::lexer::{Lexer, Obj};
use super::{Ctx, ObjRec};
use crate::model::{NodeId, ParseTree, Value};
use crate::zip::DocumentFact;

/// Pages walked, at most.
const MAX_PAGES: usize = 2000;
/// Pieces of text remembered on one page, and boxes checked against them.
const MAX_TEXTS: usize = 20_000;
const MAX_BOXES: usize = 2000;
/// Boxes and pieces of text kept to draw a page, at most.
const MAX_DRAWN: usize = 200;
/// Pages drawn, at most.
const MAX_DRAWN_PAGES: usize = 50;
/// Operands kept for one operator; real ones take at most a handful.
const MAX_OPERANDS: usize = 32;
/// Content decompressed for this check, in all.
const BUDGET: u64 = 64 * 1024 * 1024;
/// The darkest a fill can be and still hide nothing: 0 is black, 1 white.
const DARK: f64 = 0.25;
/// How much of a piece of text a box must cover to hide it.
const COVERED: f64 = 0.5;

/// An affine matrix `[a b c d e f]` (8.3.4).
type Matrix = [f64; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` then `n`: a point goes through `m` first.
fn mul(m: &Matrix, n: &Matrix) -> Matrix {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn apply(m: &Matrix, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// An axis-aligned area on the page: left, bottom, right, top.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Area([f64; 4]);

impl Area {
    /// The area a rectangle covers once `m` has placed it.
    fn of(m: &Matrix, x: f64, y: f64, w: f64, h: f64) -> Self {
        let corners = [
            apply(m, x, y),
            apply(m, x + w, y),
            apply(m, x, y + h),
            apply(m, x + w, y + h),
        ];
        let mut a = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for (px, py) in corners {
            a[0] = a[0].min(px);
            a[1] = a[1].min(py);
            a[2] = a[2].max(px);
            a[3] = a[3].max(py);
        }
        Area(a)
    }

    fn size(&self) -> f64 {
        (self.0[2] - self.0[0]).max(0.0) * (self.0[3] - self.0[1]).max(0.0)
    }

    /// How much of `self` lies inside `other`, from 0 to 1.
    fn inside(&self, other: &Area) -> f64 {
        let [l, b, r, t] = self.0;
        let [ol, ob, or, ot] = other.0;
        let w = (r.min(or) - l.max(ol)).max(0.0);
        let h = (t.min(ot) - b.max(ob)).max(0.0);
        let size = self.size();
        if size > 0.0 { w * h / size } else { 0.0 }
    }
}

/// A piece of text as painted: where, and its bytes.
struct Shown {
    area: Area,
    bytes: Vec<u8>,
    covered: bool,
}

#[derive(Clone)]
struct Graphics {
    ctm: Matrix,
    /// The fill colour is dark; black is where every page starts (8.4.1).
    dark: bool,
}

/// Reads a number operand.
fn num(o: &Obj) -> Option<f64> {
    match o {
        Obj::Int(i) => Some(*i as f64),
        Obj::Real(r) => real(r),
        _ => None,
    }
}

/// A PDF real (7.3.3): a sign, digits and at most one point, no exponent.
/// Read by hand: the standard library's float parser would add a fifth to
/// the size of the WebAssembly build.
fn real(s: &str) -> Option<f64> {
    let (neg, digits) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let (mut value, mut scale, mut point, mut any) = (0.0f64, 1.0f64, false, false);
    for b in digits.bytes() {
        match b {
            b'0'..=b'9' => {
                any = true;
                let d = f64::from(b - b'0');
                if point {
                    scale /= 10.0;
                    value += d * scale;
                } else {
                    value = value * 10.0 + d;
                }
            }
            b'.' if !point => point = true,
            _ => return None,
        }
    }
    any.then_some(if neg { -value } else { value })
}

/// The last `n` operands as numbers, when there are that many and all are.
fn nums<const N: usize>(ops: &[Obj]) -> Option<[f64; N]> {
    let start = ops.len().checked_sub(N)?;
    let mut out = [0.0; N];
    for (o, v) in out.iter_mut().zip(&ops[start..]) {
        *o = num(v)?;
    }
    Some(out)
}

/// Whether a colour given as gray, RGB or CMYK components is dark.
fn dark(c: &[f64]) -> bool {
    let light = match *c {
        [g] => g,
        [r, g, b] => r.max(g).max(b),
        [c, m, y, k] => ((1.0 - c.min(m).min(y)) * (1.0 - k)).max(0.0),
        _ => return false,
    };
    light <= DARK
}

/// Walks one page's content and returns the text that ends up under a dark
/// box, in the order it was drawn.
fn covered_text(content: &[u8]) -> Painted {
    let mut lx = Lexer::new(content, 0);
    let mut ops: Vec<Obj> = Vec::new();
    let mut gs = Graphics {
        ctm: IDENTITY,
        dark: true,
    };
    let mut saved: Vec<Graphics> = Vec::new();
    // The path being built: its rectangles, and the corners of any other
    // shape, which count when they make a rectangle too.
    let mut rects: Vec<Area> = Vec::new();
    let mut points: Vec<(f64, f64)> = Vec::new();
    let (mut tm, mut tlm) = (IDENTITY, IDENTITY);
    let (mut size, mut leading, mut scale) = (0.0f64, 0.0f64, 1.0f64);
    let mut shown: Vec<Shown> = Vec::new();
    let mut boxes = 0;
    // The boxes that cover something, for drawing the page.
    let mut covering: Vec<Area> = Vec::new();

    loop {
        lx.skip_ws();
        if lx.pos >= content.len() {
            break;
        }
        let start = lx.pos;
        if let Some(item) = lx.value() {
            if ops.len() < MAX_OPERANDS {
                ops.push(item.obj);
            }
            continue;
        }
        lx.pos = start;
        let op = lx.word();
        if op.is_empty() {
            // A stray delimiter: nothing to read here.
            lx.pos += 1;
            ops.clear();
            continue;
        }
        // Shows a string at the text position, and moves past it.
        let mut show = |tm: &mut Matrix, bytes: &[u8], ctm: &Matrix| {
            let width = bytes.len() as f64 * size * 0.5 * scale;
            let at = mul(tm, ctm);
            let area = Area::of(&at, 0.0, -0.2 * size, width, size);
            if shown.len() < MAX_TEXTS && !bytes.is_empty() {
                shown.push(Shown {
                    area,
                    bytes: bytes.to_vec(),
                    covered: false,
                });
            }
            *tm = mul(&[1.0, 0.0, 0.0, 1.0, width, 0.0], tm);
        };
        let next_line = |tm: &mut Matrix, tlm: &mut Matrix, tx: f64, ty: f64| {
            *tlm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], tlm);
            *tm = *tlm;
        };
        match op {
            b"q" => saved.push(gs.clone()),
            b"Q" => {
                if let Some(g) = saved.pop() {
                    gs = g;
                }
            }
            b"cm" => {
                if let Some(m) = nums::<6>(&ops) {
                    gs.ctm = mul(&m, &gs.ctm);
                }
            }
            b"g" | b"rg" | b"k" | b"sc" | b"scn" => {
                // A pattern or a named colour is not taken for black.
                let c: Vec<f64> = ops.iter().filter_map(num).collect();
                gs.dark = ops.iter().all(|o| num(o).is_some()) && dark(&c);
            }
            // A new colour space starts at black, except for patterns.
            b"cs" => gs.dark = ops.last().and_then(Obj::name) != Some("Pattern"),
            b"re" => {
                if let Some([x, y, w, h]) = nums::<4>(&ops) {
                    rects.push(Area::of(&gs.ctm, x, y, w, h));
                }
            }
            b"m" | b"l" => {
                if let Some([x, y]) = nums::<2>(&ops) {
                    if op == b"m" {
                        points.clear();
                    }
                    points.push(apply(&gs.ctm, x, y));
                }
            }
            // A curve makes the shape something other than a box.
            b"c" | b"v" | b"y" => points.push((f64::NAN, f64::NAN)),
            b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" => {
                if let Some(a) = rectangle(&points) {
                    rects.push(a);
                }
                if gs.dark {
                    for area in rects.iter().take(MAX_BOXES.saturating_sub(boxes)) {
                        boxes += 1;
                        let mut any = false;
                        for s in shown.iter_mut().filter(|s| !s.covered) {
                            s.covered = s.area.inside(area) >= COVERED;
                            any |= s.covered;
                        }
                        if any && covering.len() < MAX_DRAWN {
                            covering.push(*area);
                        }
                    }
                }
                rects.clear();
                points.clear();
            }
            b"n" | b"S" | b"s" => {
                rects.clear();
                points.clear();
            }
            b"BT" => {
                tm = IDENTITY;
                tlm = IDENTITY;
            }
            b"Tf" => {
                if let Some(s) = ops.last().and_then(num) {
                    size = s;
                }
            }
            b"TL" => leading = ops.last().and_then(num).unwrap_or(leading),
            b"Tz" => scale = ops.last().and_then(num).map_or(scale, |z| z / 100.0),
            b"Td" | b"TD" => {
                if let Some([tx, ty]) = nums::<2>(&ops) {
                    if op == b"TD" {
                        leading = -ty;
                    }
                    next_line(&mut tm, &mut tlm, tx, ty);
                }
            }
            b"Tm" => {
                if let Some(m) = nums::<6>(&ops) {
                    tm = m;
                    tlm = m;
                }
            }
            b"T*" => next_line(&mut tm, &mut tlm, 0.0, -leading),
            b"Tj" | b"'" | b"\"" => {
                if op != b"Tj" {
                    next_line(&mut tm, &mut tlm, 0.0, -leading);
                }
                if let Some(Obj::Str(s)) = ops.last() {
                    show(&mut tm, s, &gs.ctm);
                }
            }
            b"TJ" => {
                if let Some(Obj::Array(items)) = ops.last() {
                    for i in items {
                        match &i.obj {
                            Obj::Str(s) => show(&mut tm, s, &gs.ctm),
                            other => {
                                if let Some(n) = num(other) {
                                    let dx = -n / 1000.0 * size * scale;
                                    tm = mul(&[1.0, 0.0, 0.0, 1.0, dx, 0.0], &tm);
                                }
                            }
                        }
                    }
                }
            }
            b"BI" => skip_inline_image(&mut lx),
            _ => {}
        }
        ops.clear();
    }
    let (covered, rest): (Vec<Shown>, Vec<Shown>) = shown.into_iter().partition(|s| s.covered);
    let pieces = merge(covered);
    // What is written on the same lines, and left showing: what the covered
    // text was, a name after "Claimant:".
    let beside = |s: &Shown| {
        pieces.iter().any(|(a, _)| {
            let h = a.0[3] - a.0[1];
            (s.area.0[1] - a.0[1]).abs() < 0.3 * h
        })
    };
    let context = if pieces.is_empty() {
        Vec::new()
    } else {
        merge(rest.into_iter().filter(|s| beside(s)).collect())
    };
    Painted {
        pieces,
        context,
        boxes: covering,
    }
}

/// Pieces that run on from each other on one line are one word: a TJ array
/// splits words for kerning. A little further along the line is the next
/// word; anywhere else is another piece of text.
fn merge(shown: Vec<Shown>) -> Vec<(Area, Vec<u8>)> {
    let mut out: Vec<(Area, Vec<u8>)> = Vec::new();
    for s in shown {
        if let Some((last, bytes)) = out.last_mut() {
            let h = (last.0[3] - last.0[1]).abs();
            let gap = s.area.0[0] - last.0[2];
            if (s.area.0[1] - last.0[1]).abs() < 0.3 * h && gap > -0.1 * h && gap < 1.5 * h {
                if gap >= 0.2 * h {
                    bytes.push(b' ');
                }
                last.0[2] = s.area.0[2];
                bytes.extend_from_slice(&s.bytes);
                continue;
            }
        }
        out.push((s.area, s.bytes));
    }
    out.truncate(MAX_DRAWN);
    out
}

/// What a page's dark boxes cover: each piece of text, where it is, and the
/// boxes over it.
struct Painted {
    pieces: Vec<(Area, Vec<u8>)>,
    /// Text left showing on the same lines.
    context: Vec<(Area, Vec<u8>)>,
    boxes: Vec<Area>,
}

/// The box a path of straight lines makes, when it makes one: four corners
/// (and perhaps the first again), each on the edge of their bounds.
fn rectangle(points: &[(f64, f64)]) -> Option<Area> {
    if !(4..=5).contains(&points.len()) || points.iter().any(|p| p.0.is_nan()) {
        return None;
    }
    let (mut l, mut b, mut r, mut t) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in points {
        l = l.min(x);
        b = b.min(y);
        r = r.max(x);
        t = t.max(y);
    }
    let near = |a: f64, v: f64| (a - v).abs() < 0.5;
    points
        .iter()
        .all(|&(x, y)| (near(x, l) || near(x, r)) && (near(y, b) || near(y, t)))
        .then_some(Area([l, b, r, t]))
}

/// Past an inline image's data (8.9.7): from `ID` to the `EI` that ends it.
fn skip_inline_image(lx: &mut Lexer) {
    let data = lx.data;
    lx.pos = find_word(data, lx.pos, b"ID")
        .and_then(|id| find_word(data, id + 3, b"EI"))
        .map_or(data.len(), |ei| ei + 2);
}

/// Where `word` next stands alone, between whitespace or the ends.
fn find_word(data: &[u8], from: usize, word: &[u8]) -> Option<usize> {
    let ws = |b: Option<&u8>| b.is_none_or(|b| b" \t\r\n\x0C\0".contains(b));
    let mut at = from;
    while at + word.len() <= data.len() {
        let i = at + super::find(&data[at..], word)?;
        if (i == 0 || ws(data.get(i - 1))) && ws(data.get(i + word.len())) {
            return Some(i);
        }
        at = i + 1;
    }
    None
}

/// The pages in reading order, from the catalog down the page tree (7.7.3).
/// Each comes with its MediaBox, which a page may inherit (7.7.3.4).
fn pages(data: &[u8], ctx: &Ctx, root: u32, budget: &mut u64) -> Vec<(Obj, NodeId, [f64; 4])> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    let catalog = match resolve(data, ctx, root, budget) {
        Some(Found::Top(rec)) => rec.value.clone(),
        Some(Found::Packed(obj, _)) => obj,
        None => return out,
    };
    // US Letter, when nothing says otherwise.
    let letter = [0.0, 0.0, 612.0, 792.0];
    let mut stack = vec![(catalog.get("Pages").cloned(), letter)];
    while let Some((next, inherited)) = stack.pop() {
        if out.len() >= MAX_PAGES || seen.len() > MAX_PAGES * 4 {
            break;
        }
        let Some(Obj::Ref(n, _)) = next else { continue };
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        let (node, id) = match resolve(data, ctx, n, budget) {
            Some(Found::Top(rec)) => (rec.value.clone(), rec.node),
            Some(Found::Packed(obj, id)) => (obj, id),
            None => continue,
        };
        let media = match node.get("MediaBox") {
            Some(Obj::Array(a)) if a.len() == 4 => {
                let v: Vec<f64> = a.iter().filter_map(|i| num(&i.obj)).collect();
                match v[..] {
                    [l, b, r, t] if r != l && t != b => [l.min(r), b.min(t), l.max(r), b.max(t)],
                    _ => inherited,
                }
            }
            _ => inherited,
        };
        match node.get("Kids") {
            Some(Obj::Array(kids)) => {
                // Last first, so the first page comes off the stack first.
                stack.extend(kids.iter().rev().map(|k| (Some(k.obj.clone()), media)));
            }
            _ if node.get("Type").and_then(Obj::name) == Some("Page") => {
                out.push((node, id, media))
            }
            _ => {}
        }
    }
    out
}

/// A top-level object by number, the latest one.
fn top(ctx: &Ctx, n: u32) -> Option<&ObjRec> {
    ctx.objects.iter().rev().find(|o| o.num == n)
}

/// Text under black boxes, and areas marked for redaction that were never
/// redacted, as a warning on the page's content and a fact for each page.
pub(super) fn check(
    data: &[u8],
    tree: &mut ParseTree,
    ctx: &Ctx,
    facts: &mut Vec<DocumentFact>,
) -> Vec<Blackout> {
    let mut drawn = Vec::new();
    let Some(root) = ctx.trailers.iter().rev().find_map(|t| match t.get("Root") {
        Some(Obj::Ref(n, _)) => Some(*n),
        _ => None,
    }) else {
        return drawn;
    };
    let mut budget = BUDGET;
    let crypt = ctx.crypt.as_ref();
    for (i, (page, page_node, media)) in pages(data, ctx, root, &mut budget).into_iter().enumerate()
    {
        let number = i + 1;
        // The page's content, one stream or several read as one (7.8.2).
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
        let mut content = Vec::new();
        let mut first = None;
        for n in refs {
            let Some(rec) = top(ctx, n) else { continue };
            if let Some(bytes) = decode(data, rec, crypt, &mut budget) {
                first.get_or_insert(rec);
                content.extend_from_slice(&bytes);
                content.push(b'\n');
            }
        }
        let painted = covered_text(&content);
        let hidden: Vec<Vec<u8>> = painted.pieces.iter().map(|(_, b)| b.clone()).collect();
        if let Some(rec) = first
            && !hidden.is_empty()
        {
            let words = words(&hidden);
            if drawn.len() < MAX_DRAWN_PAGES {
                drawn.push(Blackout {
                    page: number,
                    media,
                    boxes: painted.boxes.iter().map(|a| a.0).collect(),
                    texts: painted
                        .pieces
                        .iter()
                        .map(|(a, b)| (a.0, readable(&text(b))))
                        .collect(),
                    context: painted
                        .context
                        .iter()
                        .map(|(a, b)| (a.0, readable(&text(b))))
                        .filter(|(_, t)| !t.is_empty())
                        .collect(),
                });
            }
            let (range, _) = rec.stream.unwrap_or((tree.get(rec.node).range, rec.node));
            let node = tree.warning(rec.node, "text under a black box", range);
            tree.set_value(node, Some(Value::Text(words.clone())));
            insert_update(
                facts,
                DocumentFact {
                    kind: "covered",
                    text: cap(format!("page {number}: {words}")),
                    node,
                },
            );
        }

        // Areas marked for redaction and never redacted.
        let Some(Obj::Array(annots)) = page.get("Annots") else {
            continue;
        };
        let marks = annots
            .iter()
            .filter_map(|a| match &a.obj {
                Obj::Ref(n, _) => top(ctx, *n).map(|r| (r.value.clone(), r.node)),
                d @ Obj::Dict(_) => Some((d.clone(), page_node)),
                _ => None,
            })
            .filter(|(a, _)| a.get("Subtype").and_then(Obj::name) == Some("Redact"))
            .collect::<Vec<_>>();
        if let Some(&(_, at)) = marks.first() {
            let range = tree.get(at).range;
            let node = tree.warning(at, "marked for redaction, never redacted", range);
            let n = marks.len();
            let what = if n == 1 {
                "an area marked for redaction was".to_string()
            } else {
                format!("{n} areas marked for redaction were")
            };
            insert_update(
                facts,
                DocumentFact {
                    kind: "covered",
                    text: format!("page {number}: {what} never applied"),
                    node,
                },
            );
        }
    }
    drawn
}

/// One page's black boxes and the text they cover, in the page's own
/// coordinates (points, the origin at the bottom left), to draw it.
#[derive(Debug, Clone, PartialEq)]
pub struct Blackout {
    /// Counted from 1.
    pub page: usize,
    /// The page: left, bottom, right, top.
    pub media: [f64; 4],
    /// The dark boxes over text, the same way round.
    pub boxes: Vec<[f64; 4]>,
    /// Each covered piece of text and where it is; the text is empty when
    /// its font's codes do not read as letters.
    pub texts: Vec<([f64; 4], String)>,
    /// Text left showing on the same lines, such as the label before a
    /// covered name.
    pub context: Vec<([f64; 4], String)>,
}

/// The hidden pieces as words, set apart by ` · `, or a count when their
/// font's codes do not read as letters.
fn words(pieces: &[Vec<u8>]) -> String {
    let joined: String = pieces
        .iter()
        .map(|p| text(p).split_whitespace().collect::<Vec<_>>().join(" "))
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

/// The text with its spaces tidied, or nothing when under four in five of
/// its characters are letters, digits, punctuation or spaces: codes that
/// are not letters read as noise.
fn readable(t: &str) -> String {
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = t.chars().count();
    let good = t
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_ascii_punctuation() || *c == ' ' || *c == '·')
        .count();
    if chars > 0 && good * 10 >= chars * 8 {
        t
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hidden(content: &str) -> Vec<String> {
        covered_text(content.as_bytes())
            .pieces
            .iter()
            .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
            .collect()
    }

    #[test]
    fn reals_read_as_pdf_writes_them() {
        assert_eq!(real("-3.5"), Some(-3.5));
        assert_eq!(real(".25"), Some(0.25));
        assert_eq!(real("+12."), Some(12.0));
        assert_eq!(real("0.1"), Some(0.1));
        for bad in ["", "-", ".", "1.2.3", "1e5", "nan"] {
            assert_eq!(real(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_black_box_drawn_over_text_covers_it() {
        let page =
            "BT /F1 12 Tf 72 700 Td (Salary: 4200) Tj 0 -20 Td (Kept) Tj ET 0 g 70 695 100 16 re f";
        assert_eq!(hidden(page), ["Salary: 4200"]);
    }

    #[test]
    fn a_box_drawn_first_is_a_background() {
        let page = "0 g 70 695 100 16 re f BT /F1 12 Tf 72 700 Td (Visible) Tj ET";
        assert!(hidden(page).is_empty());
    }

    #[test]
    fn a_light_box_or_one_beside_the_text_hides_nothing() {
        let text = "BT /F1 12 Tf 72 700 Td (Name) Tj ET";
        assert!(hidden(&format!("{text} 1 g 70 695 100 16 re f")).is_empty());
        assert!(hidden(&format!("{text} 0.9 0.9 0.9 rg 70 695 100 16 re f")).is_empty());
        assert!(hidden(&format!("{text} 0 g 300 695 100 16 re f")).is_empty());
        assert!(hidden(&format!("{text} 0 g 70 695 100 16 re S")).is_empty());
    }

    #[test]
    fn transforms_colours_and_shapes_are_followed() {
        // A scaled page, a CMYK black, a box drawn as four lines, TJ arrays.
        let page = "q 2 0 0 2 0 0 cm BT /F1 6 Tf 1 0 0 1 36 350 Tm [(Olena) -250 (Koval)] TJ ET Q \
                    0 0 0 1 k 70 695 m 170 695 l 170 711 l 70 711 l h f";
        assert_eq!(hidden(page), ["Olena Koval"]);
        // The same box in a restored state that is white again.
        let page = "BT /F1 12 Tf 72 700 Td (A) Tj ET q 0 g Q 1 g 70 695 100 16 re f";
        assert!(hidden(page).is_empty());
    }

    #[test]
    fn kerned_pieces_make_one_word_and_lines_stay_apart() {
        let page = "BT /F1 10 Tf 72 700 Td [(Ol) 20 (ena)] TJ 0 -14 Td (Koval) Tj ET 0 g 60 680 100 40 re f";
        assert_eq!(hidden(page), ["Olena", "Koval"]);
    }

    #[test]
    fn inline_images_and_junk_are_passed_over() {
        let page = "BT /F1 12 Tf 72 700 Td (Hi) Tj ET BI /W 1 /H 1 ID \x00) re f EI 0 g 70 695 100 16 re f ] } )";
        assert_eq!(hidden(page), ["Hi"]);
        assert!(hidden("BI /W 1 ID no end").is_empty());
    }

    #[test]
    fn unreadable_codes_are_counted_not_shown() {
        assert_eq!(
            words(&[
                b"John  Smith".to_vec(),
                b" ".to_vec(),
                b"12 Main St ".to_vec()
            ]),
            "“John Smith · 12 Main St”"
        );
        assert_eq!(
            words(&[vec![0, 3, 0, 17, 0, 9]]),
            "1 piece of text, in a font whose codes hexscope does not turn into letters"
        );
    }

    #[test]
    fn random_content_never_panics() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let pieces = [
            "BT ",
            "ET ",
            "q ",
            "Q ",
            "1 ",
            "-3.5 ",
            "0 ",
            "cm ",
            "re ",
            "f ",
            "g ",
            "k ",
            "Tf ",
            "Td ",
            "TD ",
            "Tm ",
            "T* ",
            "(x) ",
            "Tj ",
            "[(a) 5] ",
            "TJ ",
            "' ",
            "BI ",
            "ID ",
            "EI ",
            "<< /A 1 >> ",
            "m ",
            "l ",
            "c ",
            "h ",
            ") ",
            "] ",
            "/P ",
            "cs ",
            "scn ",
        ];
        for _ in 0..3000 {
            let s: String = (0..next() % 40)
                .map(|_| pieces[(next() % pieces.len() as u64) as usize])
                .collect();
            let _ = covered_text(s.as_bytes());
        }
    }
}
