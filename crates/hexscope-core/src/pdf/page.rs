//! A page's content, painted the way a viewer paints it (ISO 32000-1 §8,
//! §9), keeping each glyph: where it lands, what it says, whether a dark
//! box is filled over it afterwards, and whether anyone could see it at all
//! — drawn invisibly, in white where nothing is behind it, off the page, or
//! too small. Each glyph also remembers which string in the content it came
//! from, so a copy can be written with chosen glyphs taken out and the rest
//! left exactly where they were.
//!
//! Glyphs are placed with the font's widths when it has them (see
//! [`super::fonts`]), and estimated at half the font size when not. Content
//! drawn by form XObjects is not followed.

use super::facts::text as pdf_doc;
use super::fonts::{Font, Fonts};
use super::lexer::{Item, Lexer, Obj};
use crate::fixed::fixed;

/// Glyphs kept for one page.
const MAX_GLYPHS: usize = 200_000;
/// Operands kept for one operator; real ones take at most a handful.
const MAX_OPERANDS: usize = 32;
/// Glyph-against-box comparisons made for one page, at most.
const MAX_WORK: u64 = 50_000_000;
/// Boxes kept to draw a page.
const MAX_BOXES: usize = 200;
/// The darkest a fill can be and still hide nothing: 0 is black, 1 white.
const DARK: f64 = 0.25;
/// The lightest a fill can be and still show on white paper.
const WHITE: f64 = 0.95;
/// How much of a glyph a box must cover to hide it.
const COVERED: f64 = 0.5;
/// Glyphs shorter than this on the page, in points, cannot be read.
const TINY: f64 = 1.0;

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

fn translate(tx: f64, m: &Matrix) -> Matrix {
    mul(&[1.0, 0.0, 0.0, 1.0, tx, 0.0], m)
}

/// An axis-aligned area on the page: left, bottom, right, top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Area(pub [f64; 4]);

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

    fn height(&self) -> f64 {
        self.0[3] - self.0[1]
    }

    /// How much of `self` lies inside `other`, from 0 to 1.
    pub fn inside(&self, other: &Area) -> f64 {
        let [l, b, r, t] = self.0;
        let [ol, ob, or, ot] = other.0;
        let w = (r.min(or) - l.max(ol)).max(0.0);
        let h = (t.min(ot) - b.max(ob)).max(0.0);
        let size = self.size();
        if size > 0.0 { w * h / size } else { 0.0 }
    }

    /// Whether the two meet at all.
    fn meets(&self, other: &Area) -> bool {
        self.0[0] <= other.0[2]
            && other.0[0] <= self.0[2]
            && self.0[1] <= other.0[3]
            && other.0[1] <= self.0[3]
    }
}

/// Why a glyph cannot be seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hidden {
    /// Text render mode 3 or 7 (9.3.6): drawn with no ink.
    Invisible,
    /// In white, with nothing painted behind it but the paper.
    White,
    /// Placed wholly outside the page.
    OffPage,
    /// Less than a point high.
    Tiny,
}

/// One glyph as painted.
#[derive(Debug, Clone)]
pub(super) struct Glyph {
    pub area: Area,
    code: u32,
    len: u8,
    /// The TJ number that moves the pen as far as this glyph does.
    adjust: f64,
    /// Its text in [`Walked::text`], as a byte range.
    text: (u32, u32),
    /// A dark box was filled over it afterwards.
    pub covered: bool,
    /// An area marked for redaction lies over it.
    pub marked: bool,
    pub hidden: Option<Hidden>,
}

/// How a string came to be shown, so it can be written again.
#[derive(Debug, Clone, Copy)]
enum Shown {
    /// An element of a TJ array: only the string itself is replaced.
    Item,
    /// `(…) Tj`, from the string to the end of the operator.
    Tj,
    /// `(…) '`: move to the next line, then show.
    Quote,
    /// `aw ac (…) "`: set the spacing, move to the next line, show.
    DoubleQuote {
        aw: (usize, usize),
        ac: (usize, usize),
    },
}

/// One string in the content and its glyphs.
#[derive(Debug, Clone)]
struct Run {
    /// The bytes a rewrite replaces.
    start: usize,
    end: usize,
    shown: Shown,
    glyphs: (usize, usize),
}

/// A page, painted.
#[derive(Debug, Default)]
pub(super) struct Walked {
    pub glyphs: Vec<Glyph>,
    runs: Vec<Run>,
    text: String,
    /// The dark boxes that cover some text, and the marks.
    pub boxes: Vec<Area>,
}

impl Walked {
    /// A glyph's text.
    pub fn text_of(&self, g: &Glyph) -> &str {
        self.text
            .get(g.text.0 as usize..g.text.1 as usize)
            .unwrap_or("")
    }

    /// The glyphs `keep` picks, a line's run of them as one piece of text:
    /// glyphs that follow on from each other on a line belong together; a
    /// little further along the line starts a new word; anywhere else, a
    /// new piece.
    pub fn pieces(&self, keep: impl Fn(&Glyph) -> bool) -> Vec<(Area, String)> {
        let mut out: Vec<(Area, String)> = Vec::new();
        for g in self.glyphs.iter().filter(|g| keep(g)) {
            let t = self.text_of(g);
            if let Some((last, text)) = out.last_mut() {
                let h = last.height().abs().max(0.01);
                let gap = g.area.0[0] - last.0[2];
                if (g.area.0[1] - last.0[1]).abs() < 0.3 * h && gap > -0.3 * h && gap < 1.5 * h {
                    if gap >= 0.2 * h && !text.ends_with(' ') && !t.starts_with(' ') {
                        text.push(' ');
                    }
                    last.0[2] = last.0[2].max(g.area.0[2]);
                    last.0[1] = last.0[1].min(g.area.0[1]);
                    last.0[3] = last.0[3].max(g.area.0[3]);
                    text.push_str(t);
                    continue;
                }
            }
            out.push((g.area, t.to_string()));
        }
        out
    }

    /// The edits that take out the glyphs `remove` picks and leave every
    /// other glyph where it was: `(start, end, replacement)` in the content,
    /// in order. A string loses its chosen glyphs to TJ adjustments that
    /// move the pen as far as they did.
    pub fn edits(
        &self,
        content: &[u8],
        remove: impl Fn(&Glyph) -> bool,
    ) -> Vec<(usize, usize, Vec<u8>)> {
        let mut out = Vec::new();
        for run in &self.runs {
            let glyphs = &self.glyphs[run.glyphs.0..run.glyphs.1];
            if !glyphs.iter().any(&remove) {
                continue;
            }
            // Runs of kept glyphs as hex strings, of removed ones as one number.
            let mut parts: Vec<String> = Vec::new();
            let mut hex = String::new();
            let mut gap = 0.0;
            let flush_gap = |parts: &mut Vec<String>, gap: &mut f64| {
                if *gap != 0.0 {
                    parts.push(number(*gap));
                    *gap = 0.0;
                }
            };
            for g in glyphs {
                if remove(g) {
                    if !hex.is_empty() {
                        parts.push(format!("<{hex}>"));
                        hex.clear();
                    }
                    gap += g.adjust;
                } else {
                    flush_gap(&mut parts, &mut gap);
                    for k in (0..g.len).rev() {
                        hex.push_str(&format!("{:02X}", (g.code >> (8 * k)) & 0xFF));
                    }
                }
            }
            if !hex.is_empty() {
                parts.push(format!("<{hex}>"));
            }
            flush_gap(&mut parts, &mut gap);
            let inner = parts.join(" ");
            let bytes =
                |r: (usize, usize)| String::from_utf8_lossy(&content[r.0..r.1]).into_owned();
            let replacement = match run.shown {
                Shown::Item => inner,
                Shown::Tj => format!("[{inner}] TJ"),
                Shown::Quote => format!("T* [{inner}] TJ"),
                Shown::DoubleQuote { aw, ac } => {
                    format!("{} Tw {} Tc T* [{inner}] TJ", bytes(aw), bytes(ac))
                }
            };
            out.push((run.start, run.end, replacement.into_bytes()));
        }
        // Runs are met in the order of the content: the edits are in order.
        out
    }
}

/// A TJ number, short: `-2780`, `-417.5`.
fn number(v: f64) -> String {
    let s = fixed(v, 3);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Reads a number operand.
pub(super) fn num(o: &Obj) -> Option<f64> {
    match o {
        Obj::Int(i) => Some(*i as f64),
        Obj::Real(r) => real(r),
        _ => None,
    }
}

/// A PDF real (7.3.3): a sign, digits and at most one point, no exponent.
/// Read by hand: the standard library's float parser would add a fifth to
/// the size of the WebAssembly build.
pub(super) fn real(s: &str) -> Option<f64> {
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
fn nums<const N: usize>(ops: &[Item]) -> Option<[f64; N]> {
    let start = ops.len().checked_sub(N)?;
    let mut out = [0.0; N];
    for (o, v) in out.iter_mut().zip(&ops[start..]) {
        *o = num(&v.obj)?;
    }
    Some(out)
}

/// How light a colour given as gray, RGB or CMYK components is, 0 to 1.
fn lightness(c: &[f64]) -> Option<f64> {
    Some(match *c {
        [g] => g,
        [r, g, b] => r.max(g).max(b),
        [c, m, y, k] => ((1.0 - c.min(m).min(y)) * (1.0 - k)).max(0.0),
        _ => return None,
    })
}

#[derive(Clone)]
struct Graphics {
    ctm: Matrix,
    /// How light the fill colour is; black is where every page starts
    /// (8.4.1). `None` for a pattern, or a colour not read.
    fill: Option<f64>,
    /// The text state (9.3): spacing, scale, leading, font, size, mode.
    tc: f64,
    tw: f64,
    th: f64,
    leading: f64,
    font: Option<usize>,
    size: f64,
    mode: u8,
}

/// Paints a page's content. `media` is the page; `marks`, areas marked for
/// redaction, cover what is under them once everything is painted.
pub(super) fn walk(content: &[u8], fonts: &Fonts, media: Area, marks: &[Area]) -> Walked {
    let mut w = Walked::default();
    let mut lx = Lexer::new(content, 0);
    let mut ops: Vec<Item> = Vec::new();
    let mut gs = Graphics {
        ctm: IDENTITY,
        fill: Some(0.0),
        tc: 0.0,
        tw: 0.0,
        th: 1.0,
        leading: 0.0,
        font: None,
        size: 0.0,
        mode: 0,
    };
    let unknown = Font::estimated();
    let mut saved: Vec<Graphics> = Vec::new();
    // The path being built: its rectangles, and the corners of any other
    // shape, which count when they make a rectangle too.
    let mut rects: Vec<Area> = Vec::new();
    let mut points: Vec<(f64, f64)> = Vec::new();
    let (mut tm, mut tlm) = (IDENTITY, IDENTITY);
    // What is painted that is not white: what white text shows against.
    let mut painted: Vec<Area> = Vec::new();
    let mut images: Vec<Area> = Vec::new();
    let mut work = 0u64;

    loop {
        lx.skip_ws();
        if lx.pos >= content.len() {
            break;
        }
        let start = lx.pos;
        if let Some(item) = lx.value() {
            if ops.len() < MAX_OPERANDS {
                ops.push(item);
            }
            continue;
        }
        lx.pos = start;
        let op = lx.word();
        let op_end = lx.pos;
        if op.is_empty() {
            // A stray delimiter: nothing to read here.
            lx.pos += 1;
            ops.clear();
            continue;
        }
        // Shows a string: each glyph where it lands, the pen past it.
        let show = |w: &mut Walked,
                    tm: &mut Matrix,
                    gs: &Graphics,
                    bytes: &[u8],
                    at: (usize, usize),
                    shown: Shown| {
            let font = gs
                .font
                .and_then(|i| fonts.get(i))
                .map_or(&unknown, |f| &f.1);
            let first = w.glyphs.len();
            let white = gs.fill.is_some_and(|l| l >= WHITE);
            for (code, len) in font.codes(bytes) {
                if w.glyphs.len() >= MAX_GLYPHS {
                    break;
                }
                let width = font.width(code);
                let space = if len == 1 && code == 32 { gs.tw } else { 0.0 };
                let advance = (width / 1000.0 * gs.size + gs.tc + space) * gs.th;
                let area = Area::of(
                    &mul(tm, &gs.ctm),
                    0.0,
                    -0.2 * gs.size,
                    width / 1000.0 * gs.size * gs.th,
                    gs.size,
                );
                let from = w.text.len() as u32;
                match font.unicode(code) {
                    Some(t) => w.text.push_str(&t),
                    None if !font.two_byte => w.text.push_str(&pdf_doc(&[code as u8])),
                    None => w.text.push('\u{FFFD}'),
                }
                let hidden = if gs.mode == 3 || gs.mode == 7 {
                    Some(Hidden::Invisible)
                } else if !area.meets(&media) {
                    Some(Hidden::OffPage)
                } else if area.height().abs() < TINY {
                    Some(Hidden::Tiny)
                } else if white && !painted.iter().any(|p| area.inside(p) >= COVERED) {
                    Some(Hidden::White)
                } else {
                    None
                };
                w.glyphs.push(Glyph {
                    area,
                    code,
                    len: len as u8,
                    adjust: if gs.size != 0.0 {
                        -(width + (gs.tc + space) * 1000.0 / gs.size)
                    } else {
                        0.0
                    },
                    text: (from, w.text.len() as u32),
                    covered: false,
                    marked: false,
                    hidden,
                });
                *tm = translate(advance, tm);
            }
            w.runs.push(Run {
                start: at.0,
                end: at.1,
                shown,
                glyphs: (first, w.glyphs.len()),
            });
        };
        let next_line = |tm: &mut Matrix, tlm: &mut Matrix, tx: f64, ty: f64| {
            *tlm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], tlm);
            *tm = *tlm;
        };
        let range = |i: &Item| (i.range.start as usize, i.range.end() as usize);
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
                // A pattern or a named colour is not taken for any colour.
                let c: Vec<f64> = ops.iter().filter_map(|o| num(&o.obj)).collect();
                gs.fill = if c.len() == ops.len() {
                    lightness(&c)
                } else {
                    None
                };
            }
            // A new colour space starts at black, except for patterns.
            b"cs" => {
                gs.fill = (ops.last().and_then(|o| o.obj.name()) != Some("Pattern")).then_some(0.0)
            }
            b"re" => {
                if let Some([x, y, wd, h]) = nums::<4>(&ops) {
                    rects.push(Area::of(&gs.ctm, x, y, wd, h));
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
                let dark = gs.fill.is_some_and(|l| l <= DARK);
                for area in &rects {
                    if gs.fill.is_none_or(|l| l < WHITE) && painted.len() < MAX_GLYPHS {
                        painted.push(*area);
                    }
                    if !dark || work > MAX_WORK {
                        continue;
                    }
                    work += w.glyphs.len() as u64;
                    let mut any = false;
                    for g in w.glyphs.iter_mut().filter(|g| !g.covered) {
                        g.covered = g.area.inside(area) >= COVERED;
                        any |= g.covered;
                    }
                    if any && w.boxes.len() < MAX_BOXES {
                        w.boxes.push(*area);
                    }
                }
                rects.clear();
                points.clear();
            }
            b"n" | b"S" | b"s" => {
                rects.clear();
                points.clear();
            }
            // An image, or a form: what white text could show against.
            b"Do" => {
                let a = Area::of(&gs.ctm, 0.0, 0.0, 1.0, 1.0);
                painted.push(a);
                images.push(a);
            }
            b"BT" => {
                tm = IDENTITY;
                tlm = IDENTITY;
            }
            b"Tf" => {
                if let Some(s) = ops.last().and_then(|o| num(&o.obj)) {
                    gs.size = s;
                }
                let name = ops.len().checked_sub(2).and_then(|i| ops[i].obj.name());
                gs.font = name.and_then(|n| fonts.iter().position(|(k, _)| k == n));
            }
            b"Tc" => gs.tc = ops.last().and_then(|o| num(&o.obj)).unwrap_or(gs.tc),
            b"Tw" => gs.tw = ops.last().and_then(|o| num(&o.obj)).unwrap_or(gs.tw),
            b"TL" => gs.leading = ops.last().and_then(|o| num(&o.obj)).unwrap_or(gs.leading),
            b"Tz" => {
                gs.th = ops
                    .last()
                    .and_then(|o| num(&o.obj))
                    .map_or(gs.th, |z| z / 100.0)
            }
            b"Tr" => {
                gs.mode = ops
                    .last()
                    .and_then(|o| num(&o.obj))
                    .map_or(gs.mode, |m| m as u8)
            }
            b"Td" | b"TD" => {
                if let Some([tx, ty]) = nums::<2>(&ops) {
                    if op == b"TD" {
                        gs.leading = -ty;
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
            b"T*" => next_line(&mut tm, &mut tlm, 0.0, -gs.leading),
            b"Tj" | b"'" | b"\"" => {
                if op == b"\""
                    && let [.., aw, ac, _] = &ops[..]
                {
                    gs.tw = num(&aw.obj).unwrap_or(gs.tw);
                    gs.tc = num(&ac.obj).unwrap_or(gs.tc);
                }
                if op != b"Tj" {
                    next_line(&mut tm, &mut tlm, 0.0, -gs.leading);
                }
                if let Some(
                    item @ Item {
                        obj: Obj::Str(s), ..
                    },
                ) = ops.last()
                {
                    let (from, shown) = match (op, &ops[..]) {
                        (b"Tj", _) => (range(item).0, Shown::Tj),
                        (b"'", _) => (range(item).0, Shown::Quote),
                        (_, [.., aw, ac, _]) => (
                            range(aw).0,
                            Shown::DoubleQuote {
                                aw: range(aw),
                                ac: range(ac),
                            },
                        ),
                        _ => (range(item).0, Shown::Quote),
                    };
                    show(&mut w, &mut tm, &gs, s, (from, op_end), shown);
                }
            }
            b"TJ" => {
                if let Some(Item {
                    obj: Obj::Array(items),
                    ..
                }) = ops.last()
                {
                    for i in items {
                        match &i.obj {
                            Obj::Str(s) => show(&mut w, &mut tm, &gs, s, range(i), Shown::Item),
                            other => {
                                if let Some(n) = num(other) {
                                    tm = translate(-n / 1000.0 * gs.size * gs.th, &tm);
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

    // Marks for redaction cover what is under them, whenever it was drawn.
    for m in marks {
        for g in w.glyphs.iter_mut() {
            g.marked |= g.area.inside(m) >= COVERED;
        }
        if w.boxes.len() < MAX_BOXES {
            w.boxes.push(*m);
        }
    }
    // A scanned page: an image over most of it, and its text laid out
    // invisibly under the picture of the words, for search. That is OCR,
    // not hiding.
    let scanned = images
        .iter()
        .any(|i| i.inside(&media).max(media.inside(i)) >= 0.5 && i.size() >= 0.5 * media.size());
    if scanned {
        for g in w
            .glyphs
            .iter_mut()
            .filter(|g| g.hidden == Some(Hidden::Invisible))
        {
            g.hidden = None;
        }
    }
    w
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

/// Applies edits, in order and apart, to bytes.
pub(super) fn apply_edits(bytes: &[u8], edits: &[(usize, usize, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    for (start, end, with) in edits {
        if *start < at || *end > bytes.len() {
            continue;
        }
        out.extend_from_slice(&bytes[at..*start]);
        out.extend_from_slice(with);
        at = *end;
    }
    out.extend_from_slice(&bytes[at..]);
    out
}

/// The text with its spaces tidied, or nothing when under four in five of
/// its characters are letters, digits, punctuation or spaces: codes that
/// are not letters read as noise.
pub(super) fn readable(t: &str) -> String {
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

    const PAGE: Area = Area([0.0, 0.0, 612.0, 792.0]);

    fn walked(content: &str) -> Walked {
        walk(content.as_bytes(), &Vec::new(), PAGE, &[])
    }

    fn covered(content: &str) -> Vec<String> {
        walked(content)
            .pieces(|g| g.covered)
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }

    fn hidden(content: &str) -> Vec<(Hidden, String)> {
        let w = walked(content);
        let mut out = Vec::new();
        for kind in [
            Hidden::Invisible,
            Hidden::White,
            Hidden::OffPage,
            Hidden::Tiny,
        ] {
            for (_, t) in w.pieces(|g| g.hidden == Some(kind)) {
                out.push((kind, t));
            }
        }
        out
    }

    #[test]
    fn reals_read_as_pdf_writes_them() {
        assert_eq!(real("-3.5"), Some(-3.5));
        assert_eq!(real(".25"), Some(0.25));
        assert_eq!(real("+12."), Some(12.0));
        for bad in ["", "-", ".", "1.2.3", "1e5", "nan"] {
            assert_eq!(real(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_black_box_drawn_over_text_covers_it() {
        let page =
            "BT /F1 12 Tf 72 700 Td (Salary: 4200) Tj 0 -20 Td (Kept) Tj ET 0 g 70 695 100 16 re f";
        assert_eq!(covered(page), ["Salary: 4200"]);
        // Drawn first, a box is a background.
        assert!(
            covered("0 g 70 695 100 16 re f BT /F1 12 Tf 72 700 Td (Visible) Tj ET").is_empty()
        );
    }

    #[test]
    fn only_the_letters_under_the_box_are_covered() {
        // "Name: " is 6 glyphs of 6 points: the box starts after it.
        let page = "BT /F1 12 Tf 72 700 Td (Name: Olena) Tj ET 0 g 108 695 40 16 re f";
        assert_eq!(covered(page), ["Olena"]);
    }

    #[test]
    fn light_boxes_boxes_beside_and_strokes_hide_nothing() {
        let text = "BT /F1 12 Tf 72 700 Td (Name) Tj ET";
        for after in [
            "1 g 70 695 100 16 re f",
            "0.9 0.9 0.9 rg 70 695 100 16 re f",
            "0 g 300 695 100 16 re f",
            "0 g 70 695 100 16 re S",
        ] {
            assert!(covered(&format!("{text} {after}")).is_empty(), "{after}");
        }
    }

    #[test]
    fn transforms_colours_and_shapes_are_followed() {
        let page = "q 2 0 0 2 0 0 cm BT /F1 6 Tf 1 0 0 1 36 350 Tm [(Olena) -250 (Koval)] TJ ET Q \
                    0 0 0 1 k 70 695 m 170 695 l 170 711 l 70 711 l h f";
        assert_eq!(covered(page), ["Olena Koval"]);
        let page = "BT /F1 12 Tf 72 700 Td (A) Tj ET q 0 g Q 1 g 70 695 100 16 re f";
        assert!(covered(page).is_empty());
    }

    #[test]
    fn text_no_one_can_see_is_found() {
        let page = "BT /F1 12 Tf 72 700 Td 3 Tr (invisible) Tj 0 Tr ET \
                    1 g BT /F1 12 Tf 72 680 Td (white) Tj ET \
                    0.2 g 60 640 200 30 re f 1 g BT /F1 12 Tf 72 650 Td (on dark) Tj ET \
                    0 g BT /F1 12 Tf 900 700 Td (off the page) Tj ET \
                    BT /F1 0.5 Tf 72 600 Td (tiny) Tj ET BT /F1 12 Tf 72 580 Td (plain) Tj ET";
        assert_eq!(
            hidden(page),
            [
                (Hidden::Invisible, "invisible".to_string()),
                (Hidden::White, "white".to_string()),
                (Hidden::OffPage, "off the page".to_string()),
                (Hidden::Tiny, "tiny".to_string()),
            ]
        );
    }

    #[test]
    fn a_scanned_page_has_an_invisible_text_layer_that_is_not_hiding() {
        let page =
            "q 612 0 0 792 0 0 cm /Im0 Do Q BT 3 Tr /F1 12 Tf 72 700 Td (scanned words) Tj ET";
        assert!(hidden(page).is_empty());
    }

    #[test]
    fn a_rewrite_takes_out_the_covered_glyphs_and_keeps_the_rest_in_place() {
        let page = "BT /F1 12 Tf 72 700 Td (Name: Olena) Tj [(Hi) -120 (there)] TJ 0 -14 Td 1 2 (x) \" ET 0 g 108 695 29 16 re f";
        let w = walked(page);
        let edits = w.edits(page.as_bytes(), |g| g.covered);
        let out = String::from_utf8(apply_edits(page.as_bytes(), &edits)).unwrap();
        // Five glyphs of 6 points at 12 points: -500 each, one number.
        assert!(out.contains("[<4E616D653A20> -2500] TJ"), "{out}");
        assert!(out.contains("[(Hi) -120 (there)] TJ"), "{out}");
        // The rewritten page shows the same glyphs at the same places, less the covered ones.
        let again = walked(&out);
        let before: Vec<_> = w
            .glyphs
            .iter()
            .filter(|g| !g.covered)
            .map(|g| (g.area, w.text_of(g).to_string()))
            .collect();
        let after: Vec<_> = again
            .glyphs
            .iter()
            .map(|g| (g.area, again.text_of(g).to_string()))
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn quotes_are_rewritten_with_their_spacing() {
        let page = "BT /F1 10 Tf 12 TL 72 700 Td 2 1 (ab) \" (cd) ' ET 0 g 0 0 612 792 re f";
        let w = walked(page);
        let out = String::from_utf8(apply_edits(
            page.as_bytes(),
            &w.edits(page.as_bytes(), |g| g.covered),
        ))
        .unwrap();
        assert_eq!(
            out,
            "BT /F1 10 Tf 12 TL 72 700 Td 2 Tw 1 Tc T* [-1200] TJ T* [-1200] TJ ET 0 g 0 0 612 792 re f"
        );
    }

    #[test]
    fn marks_cover_what_is_under_them() {
        let w = walk(
            b"BT /F1 12 Tf 72 700 Td (Petro) Tj ET",
            &Vec::new(),
            PAGE,
            &[Area([70.0, 695.0, 110.0, 715.0])],
        );
        assert_eq!(w.pieces(|g| g.marked)[0].1, "Petro");
    }

    #[test]
    fn inline_images_and_junk_are_passed_over() {
        let page = "BT /F1 12 Tf 72 700 Td (Hi) Tj ET BI /W 1 /H 1 ID \x00) re f EI 0 g 70 695 100 16 re f ] } )";
        assert_eq!(covered(page), ["Hi"]);
        assert!(covered("BI /W 1 ID no end").is_empty());
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
            "/F1 ",
            "Td ",
            "TD ",
            "Tm ",
            "T* ",
            "(x) ",
            "<00ff> ",
            "Tj ",
            "[(a) 5] ",
            "TJ ",
            "' ",
            "\" ",
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
            "Tr ",
            "Tc ",
            "Tw ",
            "Tz ",
            "Do ",
        ];
        for _ in 0..3000 {
            let s: String = (0..next() % 40)
                .map(|_| pieces[(next() % pieces.len() as u64) as usize])
                .collect();
            let w = walked(&s);
            let _ = w.pieces(|g| g.covered || g.hidden.is_some());
            let _ = apply_edits(
                s.as_bytes(),
                &w.edits(s.as_bytes(), |g| g.hidden.is_some() || g.covered),
            );
        }
    }
}
