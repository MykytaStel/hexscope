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
use super::facts::{Found, decode, resolve};
use super::fonts::{Font, Fonts};
use super::lexer::{Item, Lexer, Obj};
use super::{Ctx, ObjRec};
use crate::fixed::fixed;

/// Glyphs kept for one page.
const MAX_GLYPHS: usize = 200_000;
/// Operands kept for one operator; real ones take at most a handful.
const MAX_OPERANDS: usize = 32;
/// Glyph-against-box comparisons made for one page, at most.
const MAX_WORK: u64 = 50_000_000;
/// Boxes kept to draw a page.
const MAX_BOXES: usize = 200;
/// Dark shapes kept per page: a code drawn in boxes is a few thousand.
const MAX_SHAPES: usize = 20_000;
/// The darkest a fill can be and still hide nothing: 0 is black, 1 white.
const DARK: f64 = 0.25;
/// The lightest a fill can be and still show on white paper.
const WHITE: f64 = 0.95;
/// How opaque a fill must be to hide what is under it.
const OPAQUE: f64 = 0.9;
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
    pub(super) fn of(m: &Matrix, x: f64, y: f64, w: f64, h: f64) -> Self {
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

    /// The part the two have in common, or an empty area.
    fn and(&self, other: &Area) -> Self {
        let [l, b, r, t] = self.0;
        let [ol, ob, or, ot] = other.0;
        Area([l.max(ol), b.max(ob), r.min(or), t.min(ot)])
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
    /// Past an edge, in a word whose other letters are seen: cut off.
    pub cut: bool,
}

/// One exact XObject name operand in the stream that invokes a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FormSite {
    pub owner: u32,
    pub start: usize,
    pub end: usize,
}

/// A glyph's stream and the form invocations needed to reach it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StreamPath {
    pub calls: Vec<FormSite>,
    pub object: u32,
    /// The page object that selected this invocation; content streams can be shared.
    pub page: u32,
}

#[derive(Debug, Clone)]
pub(super) struct StreamEdit {
    pub target: StreamPath,
    pub start: usize,
    pub end: usize,
    pub replacement: Vec<u8>,
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
    shown: Shown,
    glyphs: (usize, usize),
    source: StreamPath,
    local_start: usize,
    local_end: usize,
    quote_values: Option<(Vec<u8>, Vec<u8>)>,
}

/// A page, painted.
#[derive(Debug)]
pub(super) struct Walked {
    pub glyphs: Vec<Glyph>,
    runs: Vec<Run>,
    text: String,
    /// The dark boxes that cover some text, and the marks.
    pub boxes: Vec<Area>,
    /// Each XObject drawn — a picture or a form — by its name in the
    /// page's resources, with the matrix that places its unit square.
    pub placed: Vec<(String, [f64; 6])>,
    /// Where each picture written into the content itself (8.9.7) lands.
    pub inline: Vec<Area>,
    /// Dark boxes filled over part of a large picture drawn before them:
    /// a box over a scanned page, hiding what the picture still holds.
    pub over_pictures: Vec<Area>,
    /// Every dark box filled: what a QR code drawn in boxes is made of.
    pub dark: Vec<Area>,
    pub complete: bool,
}

/// Reads the content filters used by a page or form. The general stream
/// decoder intentionally stops at picture codecs; that is not complete text.
pub(super) fn decode_content_stream(
    data: &[u8],
    rec: &ObjRec,
    ctx: &Ctx,
    budget: &mut u64,
) -> Option<Vec<u8>> {
    decode(data, rec, ctx.crypt.as_ref(), budget, true)
}

impl Default for Walked {
    fn default() -> Self {
        Self {
            glyphs: Vec::new(),
            runs: Vec::new(),
            text: String::new(),
            boxes: Vec::new(),
            placed: Vec::new(),
            inline: Vec::new(),
            over_pictures: Vec::new(),
            dark: Vec::new(),
            complete: true,
        }
    }
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

    /// The edits that remove selected glyphs, tied to the decoded stream
    /// that owns each byte range. A string loses its chosen glyphs to TJ
    /// adjustments that move the pen as far as they did.
    pub fn edits(&self, remove: impl Fn(&Glyph) -> bool) -> Vec<StreamEdit> {
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
            let replacement = match run.shown {
                Shown::Item => inner,
                Shown::Tj => format!("[{inner}] TJ"),
                Shown::Quote => format!("T* [{inner}] TJ"),
                Shown::DoubleQuote { .. } => run.quote_values.as_ref().map_or_else(
                    || format!("T* [{inner}] TJ"),
                    |(aw, ac)| {
                        format!(
                            "{} Tw {} Tc T* [{inner}] TJ",
                            String::from_utf8_lossy(aw),
                            String::from_utf8_lossy(ac)
                        )
                    },
                ),
            };
            out.push(StreamEdit {
                target: run.source.clone(),
                start: run.local_start,
                end: run.local_end,
                replacement: replacement.into_bytes(),
            });
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
    font: Option<Font>,
    size: f64,
    mode: u8,
    /// The bounds of the clipping path the page set with `W`, if any: what
    /// a shading paints (8.5.4).
    clip: Option<Area>,
    /// How opaque fills are, from the graphics state's `ca` (11.6.4.4): a
    /// box drawn see-through hides nothing, and text drawn at 0 is not seen.
    alpha: f64,
}

struct PaintState {
    painted: Vec<Area>,
    fills: Vec<(Area, bool)>,
    checked: std::cell::Cell<u64>,
    images: Vec<Area>,
    work: u64,
}

impl Default for PaintState {
    fn default() -> Self {
        Self {
            painted: Vec::new(),
            fills: Vec::new(),
            checked: std::cell::Cell::new(0),
            images: Vec::new(),
            work: 0,
        }
    }
}

struct FormWalk<'a> {
    data: &'a [u8],
    ctx: &'a Ctx,
    page: u32,
    page_resources: Option<Obj>,
    version: Option<String>,
    budget: &'a mut u64,
    invocations: &'a mut usize,
    calls: Vec<FormSite>,
    active: Vec<u32>,
}

#[derive(Clone, Copy)]
struct FormClip {
    matrix: Matrix,
    bbox: [f64; 4],
    bounds: Area,
}

impl FormClip {
    fn new(matrix: Matrix, bbox: [f64; 4]) -> Self {
        Self {
            matrix,
            bbox,
            bounds: Area::of(
                &matrix,
                bbox[0],
                bbox[1],
                bbox[2] - bbox[0],
                bbox[3] - bbox[1],
            ),
        }
    }

    /// A glyph's bounding rectangle must fit inside the transformed BBox.
    /// This is conservative for rotated glyphs: if its axis-aligned area
    /// crosses the clip edge, the whole glyph is treated as clipped.
    fn contains(&self, area: Area) -> bool {
        let det = self.matrix[0] * self.matrix[3] - self.matrix[1] * self.matrix[2];
        if !det.is_finite() || det.abs() < f64::EPSILON {
            return false;
        }
        [
            (area.0[0], area.0[1]),
            (area.0[2], area.0[1]),
            (area.0[2], area.0[3]),
            (area.0[0], area.0[3]),
        ]
        .into_iter()
        .all(|(x, y)| {
            let dx = x - self.matrix[4];
            let dy = y - self.matrix[5];
            let local_x = (self.matrix[3] * dx - self.matrix[2] * dy) / det;
            let local_y = (-self.matrix[1] * dx + self.matrix[0] * dy) / det;
            local_x >= self.bbox[0] - 1e-8
                && local_x <= self.bbox[2] + 1e-8
                && local_y >= self.bbox[1] - 1e-8
                && local_y <= self.bbox[3] + 1e-8
        })
    }
}

fn source_span(
    calls: &[FormSite],
    page: u32,
    source_owner: Option<u32>,
    source_parts: &[(u32, usize, usize)],
    start: usize,
    end: usize,
) -> Option<(StreamPath, usize, usize)> {
    if let Some(object) = source_owner {
        return Some((
            StreamPath {
                calls: calls.to_vec(),
                object,
                page,
            },
            start,
            end,
        ));
    }
    let &(object, part_start, part_end) = source_parts
        .iter()
        .find(|(_, lo, hi)| start >= *lo && end <= *hi)?;
    Some((
        StreamPath {
            calls: calls.to_vec(),
            object,
            page,
        },
        start - part_start,
        end.min(part_end) - part_start,
    ))
}

fn deref_obj(data: &[u8], ctx: &Ctx, obj: &Obj, budget: &mut u64) -> Option<Obj> {
    match obj {
        Obj::Ref(n, _) => match resolve(data, ctx, *n, budget)? {
            Found::Top(rec) => Some(rec.value.clone()),
            Found::Packed(value, _) => Some(value),
        },
        other => Some(other.clone()),
    }
}

/// The entry `name` in the resources' `category` (XObject, ExtGState).
fn resource(
    data: &[u8],
    ctx: &Ctx,
    resources: Option<&Obj>,
    fallback: Option<&Obj>,
    category: &str,
    name: &str,
    budget: &mut u64,
) -> Option<Obj> {
    for scope in [resources, fallback].into_iter().flatten() {
        let resources = deref_obj(data, ctx, scope, budget)?;
        let Some(entries) = resources.get(category) else {
            continue;
        };
        let Some(entries) = deref_obj(data, ctx, entries, budget) else {
            continue;
        };
        if let Some(entry) = entries.entries().iter().rev().find(|e| e.key == name) {
            return Some(entry.value.obj.clone());
        }
    }
    None
}

fn form_matrix(form: &Obj) -> Option<Matrix> {
    match form.get("Matrix") {
        None => Some(IDENTITY),
        Some(Obj::Array(a)) if a.len() == 6 => {
            let mut out = [0.0; 6];
            for (n, item) in out.iter_mut().zip(a) {
                *n = super::page::num(&item.obj)?;
            }
            out.iter().all(|n| n.is_finite()).then_some(out)
        }
        _ => None,
    }
}

fn form_bbox(form: &Obj) -> Option<[f64; 4]> {
    let Obj::Array(a) = form.get("BBox")? else {
        return None;
    };
    if a.len() != 4 {
        return None;
    }
    let [x0, y0, x1, y1] = [
        super::page::num(&a[0].obj)?,
        super::page::num(&a[1].obj)?,
        super::page::num(&a[2].obj)?,
        super::page::num(&a[3].obj)?,
    ];
    (x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()).then_some([
        x0.min(x1),
        y0.min(y1),
        x0.max(x1),
        y0.max(y1),
    ])
}

const MAX_FORM_DEPTH: usize = 4;
const MAX_FORM_INVOCATIONS: usize = 10_000;

/// What a `Do` drew.
enum Placed {
    /// A form, walked like the page.
    Form,
    /// A picture. One seen through — a soft mask, a mask, a stencil — is a
    /// shadow or an effect a drawing program made, never a scan.
    Picture { see_through: bool },
}

#[allow(clippy::too_many_arguments)]
fn follow_form(
    name_item: &Item,
    source_owner: Option<u32>,
    source_parts: &[(u32, usize, usize)],
    resources: Option<&Obj>,
    legacy_fallback: Option<&Obj>,
    caller: &Graphics,
    media: Area,
    marks: &[Area],
    clips: &[FormClip],
    forms: &mut FormWalk<'_>,
    w: &mut Walked,
    paint: &mut PaintState,
) -> Option<Placed> {
    let name = name_item.obj.name()?;
    let scope = resources.or(forms.page_resources.as_ref());
    let legacy = forms
        .version
        .as_deref()
        .is_some_and(|v| v == "1.0" || v == "1.1");
    let page_fallback = legacy.then_some(forms.page_resources.as_ref()).flatten();
    let xobject = resource(
        forms.data,
        forms.ctx,
        scope,
        legacy_fallback.or(page_fallback),
        "XObject",
        name,
        forms.budget,
    )?;
    let Obj::Ref(num, _) = xobject else {
        return None;
    };
    let rec = forms.ctx.latest(num)?;
    match rec.value.get("Subtype").and_then(Obj::name) {
        Some("Image") => {
            let see_through = rec.value.get("SMask").is_some()
                || rec.value.get("Mask").is_some()
                || matches!(rec.value.get("ImageMask"), Some(Obj::Bool(true)));
            return Some(Placed::Picture { see_through });
        }
        Some("Form") => {}
        _ => return None,
    }
    if *forms.invocations >= MAX_FORM_INVOCATIONS
        || forms.active.len() >= MAX_FORM_DEPTH
        || forms.active.contains(&num)
    {
        return None;
    }
    *forms.invocations += 1;

    let bbox = form_bbox(&rec.value)?;
    let matrix = form_matrix(&rec.value)?;
    let bytes = decode_content_stream(forms.data, rec, forms.ctx, forms.budget)?;
    let child_resources = match rec.value.get("Resources") {
        Some(resources) => Some(deref_obj(forms.data, forms.ctx, resources, forms.budget)?),
        None => resources.cloned().or_else(|| forms.page_resources.clone()),
    };
    let fallback = legacy.then(|| forms.page_resources.clone()).flatten();
    let child_fonts = super::fonts::page_fonts_with_fallback(
        forms.data,
        forms.ctx,
        child_resources.as_ref(),
        fallback.as_ref(),
        forms.budget,
    );
    let span = source_span(
        &forms.calls,
        forms.page,
        source_owner,
        source_parts,
        name_item.range.start as usize,
        name_item.range.end() as usize,
    )?;
    let site = FormSite {
        owner: span.0.object,
        start: span.1,
        end: span.2,
    };
    let mut child_initial = caller.clone();
    child_initial.ctm = mul(&matrix, &caller.ctm);
    let clip = FormClip::new(child_initial.ctm, bbox);
    let mut child_clips = clips.to_vec();
    child_clips.push(clip);
    forms.calls.push(site);
    forms.active.push(num);
    walk_stream(
        &bytes,
        &child_fonts,
        media,
        marks,
        child_initial,
        child_resources,
        fallback,
        Some(num),
        &[],
        &child_clips,
        Some(forms),
        w,
        paint,
    );
    forms.active.pop();
    forms.calls.pop();
    Some(Placed::Form)
}

/// Paints a page's content. `media` is the page; `marks`, areas marked for
/// redaction, cover what is under them once everything is painted.
pub(super) fn walk(content: &[u8], fonts: &Fonts, media: Area, marks: &[Area]) -> Walked {
    let mut w = Walked::default();
    let mut paint = PaintState::default();
    walk_stream(
        content,
        fonts,
        media,
        marks,
        Graphics {
            ctm: IDENTITY,
            fill: Some(0.0),
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            leading: 0.0,
            font: None,
            size: 0.0,
            mode: 0,
            clip: None,
            alpha: 1.0,
        },
        None,
        None,
        Some(0),
        &[],
        &[],
        None,
        &mut w,
        &mut paint,
    );
    finish_walk(&mut w, marks, media, &paint.images);
    w
}

/// A page's content, with its resource scope and the document objects needed
/// to follow form XObjects. `parts` maps the concatenated page content back to
/// the streams that own its bytes.
#[allow(clippy::too_many_arguments)]
pub(super) fn walk_page(
    content: &[u8],
    fonts: &Fonts,
    media: Area,
    marks: &[Area],
    data: &[u8],
    ctx: &Ctx,
    resources: Option<Obj>,
    version: Option<String>,
    page: u32,
    budget: &mut u64,
    invocations: &mut usize,
    parts: &[(u32, usize, usize)],
) -> Walked {
    let mut w = Walked::default();
    let mut paint = PaintState::default();
    let page_resources = resources.clone();
    let mut forms = FormWalk {
        data,
        ctx,
        page,
        page_resources,
        version,
        budget,
        invocations,
        calls: Vec::new(),
        active: Vec::new(),
    };
    walk_stream(
        content,
        fonts,
        media,
        marks,
        Graphics {
            ctm: IDENTITY,
            fill: Some(0.0),
            tc: 0.0,
            tw: 0.0,
            th: 1.0,
            leading: 0.0,
            font: None,
            size: 0.0,
            mode: 0,
            clip: None,
            alpha: 1.0,
        },
        resources,
        None,
        None,
        parts,
        &[],
        Some(&mut forms),
        &mut w,
        &mut paint,
    );
    finish_walk(&mut w, marks, media, &paint.images);
    w
}

fn finish_walk(w: &mut Walked, marks: &[Area], media: Area, images: &[Area]) {
    // Marks for redaction cover what is under them, whenever it was drawn.
    for m in marks {
        for g in &mut w.glyphs {
            g.marked |= g.area.inside(m) >= COVERED;
        }
        if w.boxes.len() < MAX_BOXES {
            w.boxes.push(*m);
        }
    }
    // A word an edge cuts through — the page's, or a clip's — is cut off,
    // not hidden: its unseen letters are not reported as text placed away.
    let text = &w.text;
    let mut rest = &mut w.glyphs[..];
    while !rest.is_empty() {
        let mut len = 1;
        while len < rest.len() && same_word(text, &rest[len - 1], &rest[len]) {
            len += 1;
        }
        let (word, after) = rest.split_at_mut(len);
        if word.iter().any(|g| g.hidden.is_none()) {
            for g in word
                .iter_mut()
                .filter(|g| g.hidden == Some(Hidden::OffPage))
            {
                g.cut = true;
            }
        }
        rest = after;
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
}

/// Grows `extent` to take in a point.
fn reach(extent: &mut Option<Area>, (x, y): (f64, f64)) {
    let [l, b, r, t] = extent.map_or([x, y, x, y], |e| e.0);
    *extent = Some(Area([l.min(x), b.min(y), r.max(x), t.max(y)]));
}

/// Whether `b` goes on the word `a` is in: a letter, not a space, beside it
/// on the same line.
fn same_word(text: &str, a: &Glyph, b: &Glyph) -> bool {
    let letter = |g: &Glyph| {
        text.get(g.text.0 as usize..g.text.1 as usize)
            .is_some_and(|t| !t.bytes().all(|b| b.is_ascii_whitespace()))
    };
    let h = a.area.height().abs().max(b.area.height().abs());
    letter(a)
        && letter(b)
        && (b.area.0[0] - a.area.0[2]).abs() < 0.5 * h
        && (b.area.0[1] - a.area.0[1]).abs() < 0.3 * h
}

#[allow(clippy::too_many_arguments)]
fn walk_stream(
    content: &[u8],
    fonts: &Fonts,
    media: Area,
    marks: &[Area],
    initial: Graphics,
    resources: Option<Obj>,
    legacy_fallback: Option<Obj>,
    source_owner: Option<u32>,
    source_parts: &[(u32, usize, usize)],
    clips: &[FormClip],
    mut forms: Option<&mut FormWalk<'_>>,
    w: &mut Walked,
    paint: &mut PaintState,
) {
    let mut lx = Lexer::new(content, 0);
    let mut ops: Vec<Item> = Vec::new();
    let mut gs = initial;
    let unknown = Font::estimated();
    let mut saved: Vec<Graphics> = Vec::new();
    // The path being built: its rectangles, and the corners of any other
    // shape, which count when they make a rectangle too.
    let mut rects: Vec<Area> = Vec::new();
    let mut points: Vec<(f64, f64)> = Vec::new();
    // Everything the path reaches, its curves' control points too, which
    // bound the curve (8.5.2.2); and whether `W` made it the clip.
    let mut extent: Option<Area> = None;
    let mut clipping = false;
    let (mut tm, mut tlm) = (IDENTITY, IDENTITY);
    let calls = forms
        .as_deref()
        .map_or_else(Vec::new, |forms| forms.calls.clone());
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
            let font = gs.font.as_ref().unwrap_or(&unknown);
            let first = w.glyphs.len();
            let white = gs.fill.is_some_and(|l| l >= WHITE);
            let dark_text = gs.fill.is_some_and(|l| l <= DARK) && gs.mode != 3 && gs.mode != 7;
            let page = forms.as_deref().map_or(0, |forms| forms.page);
            let source = source_span(&calls, page, source_owner, source_parts, at.0, at.1);
            let (source, local_start, local_end) = match source {
                Some(source) => source,
                None => {
                    w.complete = false;
                    (
                        StreamPath {
                            calls: calls.clone(),
                            object: source_owner.unwrap_or(0),
                            page,
                        },
                        at.0,
                        at.1,
                    )
                }
            };
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
                let mut clipped_out = false;
                for clip in clips {
                    if !area.meets(&clip.bounds) || !clip.contains(area) {
                        clipped_out = true;
                        break;
                    }
                }
                let from = w.text.len() as u32;
                match font.unicode(code) {
                    Some(t) => w.text.push_str(&t),
                    None if !font.two_byte => w.text.push_str(&pdf_doc(&[code as u8])),
                    None => w.text.push('\u{FFFD}'),
                }
                // On a dark box, and nothing lighter painted over it since.
                let on_dark = if dark_text && paint.checked.get() < MAX_WORK {
                    paint
                        .fills
                        .iter()
                        .rev()
                        .find(|(f, _)| {
                            paint.checked.set(paint.checked.get() + 1);
                            area.inside(f) >= COVERED
                        })
                        .filter(|(_, dark)| *dark)
                        .map(|(f, _)| *f)
                } else {
                    None
                };
                let hidden = if clipped_out {
                    Some(Hidden::OffPage)
                } else if gs.mode == 3 || gs.mode == 7 || gs.alpha <= 0.01 {
                    Some(Hidden::Invisible)
                } else if !area.meets(&media) {
                    Some(Hidden::OffPage)
                } else if area.height().abs() < TINY {
                    Some(Hidden::Tiny)
                } else if white && !paint.painted.iter().any(|p| area.inside(p) >= COVERED) {
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
                    covered: on_dark.is_some() && hidden.is_none(),
                    marked: false,
                    hidden,
                    cut: false,
                });
                if let Some(b) = on_dark
                    && hidden.is_none()
                    && w.boxes.len() < MAX_BOXES
                    && !w.boxes.contains(&b)
                {
                    w.boxes.push(b);
                }
                *tm = translate(advance, tm);
            }
            let quote_values = match shown {
                Shown::DoubleQuote { aw, ac } => Some((
                    content.get(aw.0..aw.1).unwrap_or_default().to_vec(),
                    content.get(ac.0..ac.1).unwrap_or_default().to_vec(),
                )),
                _ => None,
            };
            w.runs.push(Run {
                shown,
                glyphs: (first, w.glyphs.len()),
                source,
                local_start,
                local_end,
                quote_values,
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
                    let a = Area::of(&gs.ctm, x, y, wd, h);
                    rects.push(a);
                    reach(&mut extent, (a.0[0], a.0[1]));
                    reach(&mut extent, (a.0[2], a.0[3]));
                }
            }
            b"m" | b"l" => {
                if let Some([x, y]) = nums::<2>(&ops) {
                    if op == b"m" {
                        points.clear();
                    }
                    let p = apply(&gs.ctm, x, y);
                    points.push(p);
                    reach(&mut extent, p);
                }
            }
            // A curve makes the shape something other than a box.
            b"c" | b"v" | b"y" => {
                points.push((f64::NAN, f64::NAN));
                let xy: Vec<f64> = ops.iter().filter_map(|o| num(&o.obj)).collect();
                for &[x, y] in xy.as_chunks::<2>().0 {
                    reach(&mut extent, apply(&gs.ctm, x, y));
                }
            }
            b"W" | b"W*" => clipping = true,
            b"gs" => {
                if let (Some(forms), Some(name)) =
                    (forms.as_deref_mut(), ops.last().and_then(|o| o.obj.name()))
                {
                    let scope = resources.as_ref().or(forms.page_resources.as_ref());
                    let state = resource(
                        forms.data,
                        forms.ctx,
                        scope,
                        legacy_fallback.as_ref(),
                        "ExtGState",
                        name,
                        forms.budget,
                    )
                    .and_then(|o| deref_obj(forms.data, forms.ctx, &o, forms.budget));
                    if let Some(ca) = state.as_ref().and_then(|s| s.get("ca")).and_then(num) {
                        gs.alpha = ca;
                    }
                }
            }
            // A shading paints all of the clip, or the page.
            b"sh" => {
                let a = gs.clip.unwrap_or(media);
                paint.painted.push(a);
                if paint.fills.len() < MAX_GLYPHS {
                    paint.fills.push((a, false));
                }
            }
            b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" => {
                if let Some(a) = rectangle(&points) {
                    rects.push(a);
                }
                // Any other shape is something white text can show against.
                // Only its bounds are known, so it is never taken to hide anything.
                if rects.is_empty()
                    && let Some(a) = extent
                {
                    if gs.fill.is_none_or(|l| l < WHITE) && paint.painted.len() < MAX_GLYPHS {
                        paint.painted.push(a);
                    }
                    if paint.fills.len() < MAX_GLYPHS {
                        paint.fills.push((a, false));
                    }
                }
                if clipping {
                    gs.clip = extent.map(|e| gs.clip.map_or(e, |c| c.and(&e)));
                    clipping = false;
                }
                let dark = gs.alpha >= OPAQUE && gs.fill.is_some_and(|l| l <= DARK);
                for area in &rects {
                    if gs.fill.is_none_or(|l| l < WHITE) && paint.painted.len() < MAX_GLYPHS {
                        paint.painted.push(*area);
                    }
                    if paint.fills.len() < MAX_GLYPHS {
                        paint.fills.push((*area, dark));
                    }
                    if dark && w.dark.len() < MAX_SHAPES {
                        w.dark.push(*area);
                    }
                    if dark
                        && w.over_pictures.len() < MAX_BOXES
                        && over_picture(area, &paint.images, &media)
                    {
                        w.over_pictures.push(*area);
                    }
                    if !dark || paint.work > MAX_WORK {
                        continue;
                    }
                    paint.work += w.glyphs.len() as u64;
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
                extent = None;
            }
            b"n" | b"S" | b"s" => {
                if clipping {
                    gs.clip = extent.map(|e| gs.clip.map_or(e, |c| c.and(&e)));
                    clipping = false;
                }
                rects.clear();
                points.clear();
                extent = None;
            }
            // An image, or a form: what white text could show against.
            b"Do" => {
                let a = Area::of(&gs.ctm, 0.0, 0.0, 1.0, 1.0);
                if let Some(name) = ops.last().and_then(|o| o.obj.name())
                    && w.placed.len() < MAX_BOXES
                {
                    w.placed.push((name.to_string(), gs.ctm));
                }
                let placed =
                    if let (Some(forms), Some(name_item)) = (forms.as_deref_mut(), ops.last()) {
                        let placed = follow_form(
                            name_item,
                            source_owner,
                            source_parts,
                            resources.as_ref(),
                            legacy_fallback.as_ref(),
                            &gs,
                            media,
                            marks,
                            clips,
                            forms,
                            w,
                            paint,
                        );
                        if placed.is_none() {
                            w.complete = false;
                        }
                        placed
                    } else {
                        None
                    };
                if !matches!(placed, Some(Placed::Form)) {
                    paint.painted.push(a);
                    if !matches!(placed, Some(Placed::Picture { see_through: true })) {
                        paint.images.push(a);
                    }
                    if paint.fills.len() < MAX_GLYPHS {
                        paint.fills.push((a, false));
                    }
                }
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
                gs.font =
                    name.and_then(|n| fonts.iter().find(|(k, _)| k == n).map(|(_, f)| f.clone()));
                if source_owner.is_some() && name.is_some() && gs.font.is_none() {
                    w.complete = false;
                }
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
                    show(w, &mut tm, &gs, s, (from, op_end), shown);
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
                            Obj::Str(s) => show(w, &mut tm, &gs, s, range(i), Shown::Item),
                            other => {
                                if let Some(n) = num(other) {
                                    tm = translate(-n / 1000.0 * gs.size * gs.th, &tm);
                                }
                            }
                        }
                    }
                }
            }
            b"BI" => {
                if w.inline.len() < MAX_BOXES {
                    w.inline.push(Area::of(&gs.ctm, 0.0, 0.0, 1.0, 1.0));
                }
                skip_inline_image(&mut lx);
            }
            _ => {}
        }
        ops.clear();
    }
}

/// Whether a dark box hides part of a picture: it lies on one drawn before
/// it that fills a good part of the page, and covers only some of it — a
/// box over a name on a scan, not a frame, a rule, or a dark background.
fn over_picture(area: &Area, images: &[Area], media: &Area) -> bool {
    let [l, b, r, t] = area.0;
    if r - l < 8.0 || t - b < 4.0 {
        return false;
    }
    images.iter().any(|i| {
        i.size() >= 0.1 * media.size() && area.inside(i) >= 0.9 && area.size() <= 0.3 * i.size()
    })
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
            for (_, t) in w.pieces(|g| g.hidden == Some(kind) && !g.cut) {
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
        // Drawn first, a box is a background: text in a colour that shows
        // on it is seen.
        assert!(
            covered("0 g 70 695 100 16 re f BT 1 g /F1 12 Tf 72 700 Td (Visible) Tj ET").is_empty()
        );
        assert!(
            covered("0.9 g 70 695 100 16 re f BT 0 g /F1 12 Tf 72 700 Td (Visible) Tj ET")
                .is_empty()
        );
    }

    #[test]
    fn dark_text_on_a_dark_box_is_covered() {
        // A name "blacked out" by a black highlight: box first, then the
        // text in black on it.
        let page = "0 g 70 695 100 16 re f BT /F1 12 Tf 72 700 Td (Olena) Tj 0 -40 Td (Kept) Tj ET";
        assert_eq!(covered(page), ["Olena"]);
        // Something lighter painted on the box since is what the text shows against.
        let page =
            "0 g 70 695 100 16 re f 1 g 70 695 100 16 re f BT 0 g /F1 12 Tf 72 700 Td (Seen) Tj ET";
        assert!(covered(page).is_empty());
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
    fn a_word_the_edge_cuts_is_not_hidden_but_one_past_it_is() {
        // "Velocity" runs past the right edge at 612: its last letters are
        // cut off, and not reported.
        let cut = "BT /F1 12 Tf 570 700 Td (Velocity) Tj ET";
        assert!(hidden(cut).is_empty(), "{:?}", hidden(cut));
        let past = "BT /F1 12 Tf 570 700 Td (Velo) Tj 60 0 Td (secret) Tj ET";
        assert_eq!(hidden(past), [(Hidden::OffPage, "secret".to_string())]);
    }

    #[test]
    fn white_text_on_a_rounded_panel_or_a_gradient_is_seen() {
        // A device's panel with rounded corners, drawn with curves, and a
        // label in white on it.
        let panel = "0.25 0.28 0.3 rg 60 640 m 260 640 l 270 640 270 650 270 650 c 270 690 l \
                     270 700 260 700 260 700 c 60 700 l h f \
                     1 g BT /F1 12 Tf 72 660 Td (POWER) Tj ET";
        assert!(hidden(panel).is_empty(), "{:?}", hidden(panel));
        // A gradient fills the clip it is given.
        let gradient = "q 60 640 200 60 re W n /Sh1 sh Q 1 g BT /F1 12 Tf 72 660 Td (PANIC) Tj ET";
        assert!(hidden(gradient).is_empty(), "{:?}", hidden(gradient));
        // On the paper beside them, white text is still not seen.
        let beside = format!("{panel} 1 g BT /F1 12 Tf 400 300 Td (hidden) Tj ET");
        assert_eq!(hidden(&beside), [(Hidden::White, "hidden".to_string())]);
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
        let edits = local_edits(w.edits(|g| g.covered));
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
            &local_edits(w.edits(|g| g.covered)),
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
                &local_edits(w.edits(|g| g.hidden.is_some() || g.covered)),
            );
        }
    }

    fn local_edits(edits: Vec<StreamEdit>) -> Vec<(usize, usize, Vec<u8>)> {
        edits
            .into_iter()
            .map(|edit| (edit.start, edit.end, edit.replacement))
            .collect()
    }
}
