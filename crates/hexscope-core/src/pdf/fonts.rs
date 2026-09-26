//! What a page's fonts say about their text: how wide each glyph is, so a
//! string's letters can be placed one by one, and which characters its
//! codes stand for, so the text can be read.
//!
//! Widths come from a simple font's `/Widths` (9.6.2), a composite font's
//! `/W` (9.7.4.3), or, for the standard fonts that carry none, the widths
//! Adobe publishes for Helvetica, Times and Courier (Arial is metrically
//! Helvetica). Characters come from `/ToUnicode` (9.10.3) when there is
//! one; otherwise a simple font's codes are read as PDFDocEncoding, which
//! is right for the Latin text most such fonts hold, and a composite
//! font's codes, being glyph numbers, are not read at all.

use super::Ctx;
use super::facts::{Found, decode, resolve};
use super::lexer::{Lexer, Obj};

/// Font resources read per page, at most.
const MAX_FONTS: usize = 64;
/// Mappings kept from one ToUnicode map.
const MAX_MAPPINGS: usize = 65_536;

/// Widths of `space` to `asciitilde` (32–126) in the standard fonts, in
/// thousandths of the font size, from Adobe's AFM files.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];
const TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611,
    556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722,
    722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500,
    278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

/// One font as the page uses it.
#[derive(Debug, Clone, Default)]
pub(super) struct Font {
    /// Codes are two bytes: a composite font (Type0).
    pub two_byte: bool,
    /// A simple font's widths from `first`, or a composite font's ranges.
    first: u32,
    widths: Vec<f64>,
    ranges: Vec<(u32, u32, f64)>,
    /// The width of a code nothing lists.
    default: f64,
    /// A standard font's table, when the font lists no widths.
    standard: Option<&'static [u16; 95]>,
    /// Code ranges to text: `(first, last, text of first)`, the last
    /// character counting up through the range.
    unicode: Vec<(u32, u32, Vec<u16>)>,
}

impl Font {
    /// A font nothing is known of: half the size per code, codes read as
    /// PDFDocEncoding.
    pub fn estimated() -> Font {
        Font {
            default: 500.0,
            ..Font::default()
        }
    }

    /// The codes in a string, one or two bytes each.
    pub fn codes(&self, bytes: &[u8]) -> Vec<(u32, usize)> {
        if self.two_byte {
            bytes
                .chunks(2)
                .map(|c| {
                    (
                        c.iter().fold(0u32, |v, &b| (v << 8) | u32::from(b)),
                        c.len(),
                    )
                })
                .collect()
        } else {
            bytes.iter().map(|&b| (u32::from(b), 1)).collect()
        }
    }

    /// A code's width in thousandths of the font size.
    pub fn width(&self, code: u32) -> f64 {
        if self.two_byte {
            return self
                .ranges
                .iter()
                .find(|&&(lo, hi, _)| lo <= code && code <= hi)
                .map_or(self.default, |r| r.2);
        }
        if let Some(w) = code
            .checked_sub(self.first)
            .and_then(|i| self.widths.get(i as usize))
        {
            return *w;
        }
        match self.standard {
            // Courier is fixed: every glyph the same.
            Some(t) if (32..=126).contains(&code) => f64::from(t[(code - 32) as usize]),
            _ => self.default,
        }
    }

    /// The text a code stands for, when the font says.
    pub fn unicode(&self, code: u32) -> Option<String> {
        let (lo, _, first) = self
            .unicode
            .iter()
            .find(|&&(lo, hi, _)| lo <= code && code <= hi)?;
        let mut units = first.clone();
        if let Some(last) = units.last_mut() {
            *last = last.wrapping_add((code - lo) as u16);
        }
        Some(String::from_utf16_lossy(&units))
    }
}

/// A page's fonts by resource name.
pub(super) type Fonts = Vec<(String, Font)>;

/// Resolves an indirect object, or returns a direct one as it is.
fn deref(data: &[u8], ctx: &Ctx, obj: &Obj, budget: &mut u64) -> Option<Obj> {
    match obj {
        Obj::Ref(n, _) => match resolve(data, ctx, *n, budget)? {
            Found::Top(rec) => Some(rec.value.clone()),
            Found::Packed(o, _) => Some(o),
        },
        other => Some(other.clone()),
    }
}

fn num(o: &Obj) -> Option<f64> {
    match o {
        Obj::Int(i) => Some(*i as f64),
        Obj::Real(r) => super::page::real(r),
        _ => None,
    }
}

/// The fonts a page's resources name.
pub(super) fn page_fonts(
    data: &[u8],
    ctx: &Ctx,
    resources: Option<&Obj>,
    budget: &mut u64,
) -> Fonts {
    let mut out = Vec::new();
    let Some(res) = resources.and_then(|r| deref(data, ctx, r, budget)) else {
        return out;
    };
    let Some(fonts) = res.get("Font").and_then(|f| deref(data, ctx, f, budget)) else {
        return out;
    };
    for e in fonts.entries().iter().take(MAX_FONTS) {
        if let Some(dict) = deref(data, ctx, &e.value.obj, budget) {
            out.push((e.key.clone(), font(data, ctx, &dict, budget)));
        }
    }
    out
}

fn font(data: &[u8], ctx: &Ctx, dict: &Obj, budget: &mut u64) -> Font {
    let mut f = Font::estimated();
    let base = dict.get("BaseFont").and_then(Obj::name).unwrap_or("");
    if dict.get("Subtype").and_then(Obj::name) == Some("Type0") {
        f.two_byte = true;
        f.default = 1000.0;
        let cid = dict
            .get("DescendantFonts")
            .and_then(|d| deref(data, ctx, d, budget))
            .and_then(|d| match d {
                Obj::Array(a) => a.first().and_then(|i| deref(data, ctx, &i.obj, budget)),
                _ => None,
            });
        if let Some(cid) = cid {
            if let Some(dw) = cid.get("DW").and_then(num) {
                f.default = dw;
            }
            if let Some(Obj::Array(w)) = cid.get("W").and_then(|w| deref(data, ctx, w, budget)) {
                f.ranges = cid_widths(&w.iter().map(|i| i.obj.clone()).collect::<Vec<_>>());
            }
        }
    } else {
        f.first = dict
            .get("FirstChar")
            .and_then(num)
            .map_or(0, |v| v.max(0.0) as u32);
        if let Some(Obj::Array(w)) = dict.get("Widths").and_then(|w| deref(data, ctx, w, budget)) {
            f.widths = w.iter().map(|i| num(&i.obj).unwrap_or(0.0)).collect();
        }
        let plain = base.rsplit('+').next().unwrap_or(base);
        f.standard = if plain.starts_with("Courier") {
            f.default = 600.0;
            None
        } else if plain.starts_with("Times") {
            Some(&TIMES)
        } else if plain.contains("Bold")
            && (plain.starts_with("Helvetica") || plain.starts_with("Arial"))
        {
            Some(&HELVETICA_BOLD)
        } else if plain.starts_with("Helvetica") || plain.starts_with("Arial") {
            Some(&HELVETICA)
        } else {
            None
        };
    }
    if let Some(Obj::Ref(n, _)) = dict.get("ToUnicode")
        && let Some(rec) = ctx.objects.iter().rev().find(|o| o.num == *n)
        && let Some(bytes) = decode(data, rec, ctx.crypt.as_ref(), budget)
    {
        f.unicode = to_unicode(&bytes);
    }
    f
}

/// A composite font's `/W`: `c [w1 w2 …]` gives consecutive codes from c,
/// `c1 c2 w` one width for a range.
fn cid_widths(w: &[Obj]) -> Vec<(u32, u32, f64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < w.len() && out.len() < MAX_MAPPINGS {
        let Some(c) = num(&w[i]).map(|c| c.max(0.0) as u32) else {
            break;
        };
        match &w[i + 1] {
            Obj::Array(list) => {
                for (k, item) in list.iter().enumerate() {
                    if let Some(width) = num(&item.obj) {
                        let code = c.saturating_add(k as u32);
                        out.push((code, code, width));
                    }
                }
                i += 2;
            }
            last => {
                let (Some(hi), Some(width)) = (num(last), w.get(i + 2).and_then(num)) else {
                    break;
                };
                out.push((c, hi.max(0.0) as u32, width));
                i += 3;
            }
        }
    }
    out
}

/// A ToUnicode CMap's `bfchar` and `bfrange` mappings (9.10.3).
pub(super) fn to_unicode(cmap: &[u8]) -> Vec<(u32, u32, Vec<u16>)> {
    let mut out = Vec::new();
    let mut lx = Lexer::new(cmap, 0);
    let hex = |o: &Obj| match o {
        Obj::Str(b) => Some(b.clone()),
        _ => None,
    };
    let code = |b: &[u8]| b.iter().take(4).fold(0u32, |v, &x| (v << 8) | u32::from(x));
    let units = |b: &[u8]| -> Vec<u16> {
        b.chunks(2)
            .map(|c| c.iter().fold(0u16, |v, &x| (v << 8) | u16::from(x)))
            .collect()
    };
    let mut ops: Vec<Obj> = Vec::new();
    let mut section = "";
    while lx.pos < cmap.len() && out.len() < MAX_MAPPINGS {
        lx.skip_ws();
        let start = lx.pos;
        if let Some(item) = lx.value() {
            ops.push(item.obj);
            if section == "bfchar" && ops.len() == 2 {
                if let (Some(src), Some(dst)) = (hex(&ops[0]), hex(&ops[1])) {
                    let c = code(&src);
                    out.push((c, c, units(&dst)));
                }
                ops.clear();
            } else if section == "bfrange" && ops.len() == 3 {
                if let (Some(lo), Some(hi)) = (hex(&ops[0]), hex(&ops[1])) {
                    let (lo, hi) = (code(&lo), code(&hi));
                    match &ops[2] {
                        Obj::Str(dst) => out.push((lo, hi.max(lo), units(dst))),
                        // One text per code, listed.
                        Obj::Array(list) => {
                            for (k, item) in list.iter().enumerate() {
                                if let Some(dst) = hex(&item.obj) {
                                    let c = lo.saturating_add(k as u32);
                                    out.push((c, c, units(&dst)));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                ops.clear();
            }
            continue;
        }
        lx.pos = start;
        let word = lx.word();
        if word.is_empty() {
            lx.pos += 1;
            continue;
        }
        section = match word {
            b"beginbfchar" => "bfchar",
            b"beginbfrange" => "bfrange",
            _ => "",
        };
        ops.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_to_unicode_map_reads_chars_ranges_and_lists() {
        let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap
            2 beginbfchar <0003> <0020> <0011> <041E> endbfchar
            2 beginbfrange <0024> <0026> <0041> <0030> <0031> [<0444> <D83DDE00>] endbfrange
            endcmap end end";
        let f = Font {
            two_byte: true,
            unicode: to_unicode(cmap),
            ..Font::default()
        };
        let read = |c| f.unicode(c).unwrap_or_default();
        assert_eq!(read(0x03), " ");
        assert_eq!(read(0x11), "О");
        assert_eq!([read(0x24), read(0x25), read(0x26)], ["A", "B", "C"]);
        assert_eq!(read(0x30), "ф");
        assert_eq!(read(0x31), "😀");
        assert_eq!(f.unicode(0x99), None);
    }

    #[test]
    fn widths_come_from_the_font_then_the_standard_tables() {
        let simple = Font {
            first: 65,
            widths: vec![600.0, 700.0],
            standard: Some(&HELVETICA),
            default: 500.0,
            ..Font::default()
        };
        assert_eq!(simple.width(65), 600.0);
        assert_eq!(simple.width(66), 700.0);
        // Past its list: Helvetica's own "z", then the default.
        assert_eq!(simple.width(b'z' as u32), 500.0);
        assert_eq!(simple.width(200), 500.0);
        let cid = Font {
            two_byte: true,
            ranges: cid_widths(&[
                Obj::Int(1),
                Obj::Array(vec![]),
                Obj::Int(3),
                Obj::Int(5),
                Obj::Int(250),
            ]),
            default: 1000.0,
            ..Font::default()
        };
        assert_eq!(cid.width(4), 250.0);
        assert_eq!(cid.width(9), 1000.0);
        assert_eq!(cid.codes(&[0, 3, 0, 4, 7]), [(3, 2), (4, 2), (7, 1)]);
    }

    #[test]
    fn broken_maps_give_what_they_can() {
        for bad in [
            &b"beginbfchar <01> endbfchar"[..],
            b"beginbfrange <01> [<41> endbfrange",
            b"<<",
            b")",
        ] {
            let _ = to_unicode(bad);
        }
        let _ = cid_widths(&[Obj::Int(1), Obj::Int(2)]);
    }
}
