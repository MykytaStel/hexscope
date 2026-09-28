//! Pictures under what is blacked out. A box over part of a scanned page
//! hides the words on screen, and the picture keeps them: every pixel is
//! still in the file, for anyone who takes the picture out. So where a box
//! lies, the picture's own pixels are set to zero — black, or no ink — and
//! the picture is written again.
//!
//! Pictures kept with Flate, or not compressed at all, are edited here. A
//! JPEG needs a JPEG codec, which the caller has (the browser does): it is
//! given where to black out, and hands back the JPEG painted. JBIG2, CCITT
//! and JPEG 2000 pictures are not edited, and a copy that would keep one
//! whole under a box is not made.

use super::fonts::deref;
use super::lexer::Obj;
use super::page::{Area, walk};
use super::{Ctx, ObjRec, facts};
use hexscope_inflate::{NoTrace, inflate, zlib_decompress};

/// A picture's pixels, decompressed, at most.
const MAX_PICTURE: u64 = 256 * 1024 * 1024;
/// Forms followed inside forms, at most.
const MAX_DEPTH: u32 = 4;

/// A picture a box covers part of: its object, and each part covered, in
/// its unit square — `[left, bottom, right, top]` from 0 to 1, as the page
/// places it (8.9.5).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cut {
    pub num: u32,
    pub unit: Vec<[f64; 4]>,
}

fn invert(m: &[f64; 6]) -> Option<[f64; 6]> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    Some([a, b, c, d, -(m[4] * a + m[5] * c), -(m[4] * b + m[5] * d)])
}

/// `m` then `n`, as the page applies them.
fn then(m: &[f64; 6], n: &[f64; 6]) -> [f64; 6] {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

/// The part of the unit square that `m` places under `box_`, if any.
fn under(m: &[f64; 6], box_: &Area) -> Option<[f64; 4]> {
    let inv = invert(m)?;
    let [l, b, r, t] = box_.0;
    let (mut u0, mut v0, mut u1, mut v1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in [(l, b), (r, b), (l, t), (r, t)] {
        let u = inv[0] * x + inv[2] * y + inv[4];
        let v = inv[1] * x + inv[3] * y + inv[5];
        u0 = u0.min(u);
        v0 = v0.min(v);
        u1 = u1.max(u);
        v1 = v1.max(v);
    }
    let (u0, v0, u1, v1) = (u0.max(0.0), v0.max(0.0), u1.min(1.0), v1.min(1.0));
    (u0 < u1 && v0 < v1).then_some([u0, v0, u1, v1])
}

fn add(out: &mut Vec<Cut>, num: u32, unit: [f64; 4]) {
    match out.iter_mut().find(|c| c.num == num) {
        Some(c) => {
            if !c.unit.contains(&unit) {
                c.unit.push(unit);
            }
        }
        None => out.push(Cut {
            num,
            unit: vec![unit],
        }),
    }
}

/// The top-level object `num`, when it is a stream.
fn stream(ctx: &Ctx, num: u32) -> Option<&ObjRec> {
    ctx.objects
        .iter()
        .rev()
        .find(|o| o.num == num)
        .filter(|o| o.stream.is_some())
}

/// Calls `f` with each picture drawn by `placed` through `resources`, and
/// the matrix that places it — following forms into their own pictures.
pub(super) fn visit(
    data: &[u8],
    ctx: &Ctx,
    resources: Option<&Obj>,
    placed: &[(String, [f64; 6])],
    budget: &mut u64,
    depth: u32,
    f: &mut dyn FnMut(&ObjRec, &[f64; 6]),
) {
    let xobjects = resources
        .and_then(|r| deref(data, ctx, r, budget))
        .and_then(|r| r.get("XObject").and_then(|x| deref(data, ctx, x, budget)));
    let Some(xobjects) = xobjects else { return };
    for (name, m) in placed {
        let Some(&Obj::Ref(num, _)) = xobjects.get(name) else {
            continue;
        };
        let Some(rec) = stream(ctx, num) else {
            continue;
        };
        match rec.value.get("Subtype").and_then(Obj::name) {
            Some("Image") => f(rec, m),
            // A form is as large as its box, not the unit square: its own
            // pictures are each looked at.
            Some("Form") if depth < MAX_DEPTH => {
                let Some(content) = facts::decode(data, rec, ctx.crypt.as_ref(), budget) else {
                    continue;
                };
                let matrix = match rec.value.get("Matrix") {
                    Some(Obj::Array(a)) if a.len() == 6 => {
                        let v: Vec<f64> =
                            a.iter().filter_map(|i| super::page::num(&i.obj)).collect();
                        <[f64; 6]>::try_from(v).unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
                    }
                    _ => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                };
                let inner = walk(&content, &Vec::new(), Area([0.0, 0.0, 0.0, 0.0]), &[]);
                let placed: Vec<(String, [f64; 6])> = inner
                    .placed
                    .into_iter()
                    .map(|(n, cm)| (n, then(&then(&cm, &matrix), m)))
                    .collect();
                let own = rec.value.get("Resources").cloned();
                visit(
                    data,
                    ctx,
                    own.as_ref().or(resources),
                    &placed,
                    budget,
                    depth + 1,
                    f,
                );
            }
            _ => {}
        }
    }
}

/// Every picture that `marks` cover part of, among those drawn by `placed`
/// through `resources`, with its masks, which carry the same shapes.
pub(super) fn cuts(
    data: &[u8],
    ctx: &Ctx,
    resources: Option<&Obj>,
    placed: &[(String, [f64; 6])],
    marks: &[Area],
    budget: &mut u64,
    out: &mut Vec<Cut>,
) {
    visit(data, ctx, resources, placed, budget, 0, &mut |rec, m| {
        let masks: Vec<u32> = ["SMask", "Mask"]
            .iter()
            .filter_map(|k| match rec.value.get(k) {
                Some(&Obj::Ref(n, _)) => stream(ctx, n).map(|_| n),
                _ => None,
            })
            .collect();
        for h in marks.iter().filter_map(|b| under(m, b)) {
            add(out, rec.num, h);
            for &n in &masks {
                add(out, n, h);
            }
        }
    });
}

/// What became of a picture under a box.
pub(crate) enum Edited {
    /// Written again: its dictionary and its data.
    Written(Vec<u8>, Vec<u8>),
    /// A JPEG: the caller paints it.
    Jpeg,
    /// Kept in a way this cannot edit: JBIG2, CCITT, JPEG 2000, an
    /// unusual colour space or depth, damaged, or too large.
    Cannot,
}

/// The pixel rectangles, `[x0, y0, x1, y1)` counted from the top left, that
/// the unit-square parts `unit` cover in a `width` by `height` picture: a
/// pixel more all round, so nothing half covered stays.
pub(crate) fn pixels(unit: &[[f64; 4]], width: u32, height: u32) -> Vec<[u32; 4]> {
    let (w, h) = (f64::from(width), f64::from(height));
    // Not `clamp`: its check for a NaN bound keeps float formatting in the
    // wasm build, 20 KB of it.
    let fit = |v: f64, max: f64| v.max(0.0).min(max) as u32;
    unit.iter()
        .map(|&[u0, v0, u1, v1]| {
            let x0 = fit((u0 * w).floor() - 1.0, w);
            let x1 = fit((u1 * w).ceil() + 1.0, w);
            let y0 = fit(((1.0 - v1) * h).floor() - 1.0, h);
            let y1 = fit(((1.0 - v0) * h).ceil() + 1.0, h);
            [x0, y0, x1, y1]
        })
        .filter(|r| r[0] < r[2] && r[1] < r[3])
        .collect()
}

/// Colour components per pixel of a colour space (8.6).
fn components(data: &[u8], ctx: &Ctx, cs: &Obj, budget: &mut u64) -> Option<usize> {
    let cs = deref(data, ctx, cs, budget)?;
    let family = match &cs {
        Obj::Name(n) => n.as_str(),
        Obj::Array(a) => a.first()?.obj.name()?,
        _ => return None,
    };
    Some(match family {
        "DeviceGray" | "CalGray" | "G" | "Indexed" | "I" | "Separation" => 1,
        "DeviceRGB" | "CalRGB" | "RGB" | "Lab" => 3,
        "DeviceCMYK" | "CMYK" => 4,
        "ICCBased" => {
            let Obj::Array(a) = &cs else { return None };
            let Obj::Ref(n, _) = a.get(1)?.obj else {
                return None;
            };
            let n = stream(ctx, n)?.value.get("N")?.int()?;
            usize::try_from(n).ok().filter(|n| (1..=4).contains(n))?
        }
        "DeviceN" => match &cs {
            Obj::Array(a) => match &a.get(1)?.obj {
                Obj::Array(names) => names.len(),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    })
}

/// How a picture is kept, as far as editing it goes.
pub(crate) enum Kind {
    Jpeg,
    Other,
}

pub(crate) fn edit_kind(rec: &ObjRec) -> Kind {
    match filter(&rec.value) {
        Ok(Some("DCTDecode" | "DCT")) => Kind::Jpeg,
        _ => Kind::Other,
    }
}

/// A stream's only filter, or none.
fn filter(dict: &Obj) -> Result<Option<&str>, ()> {
    match dict.get("Filter") {
        None => Ok(None),
        Some(Obj::Name(n)) => Ok(Some(n)),
        Some(Obj::Array(a)) if a.is_empty() => Ok(None),
        Some(Obj::Array(a)) if a.len() == 1 => a[0].obj.name().map(Some).ok_or(()),
        Some(_) => Err(()),
    }
}

/// The dictionary of `rec` without the keys in `drop`, and with `add`.
pub(crate) fn dict_without(data: &[u8], rec: &ObjRec, drop: &[&str], add: &str) -> Vec<u8> {
    let mut out = b"<<".to_vec();
    for e in rec.value.entries() {
        if drop.contains(&e.key.as_str()) {
            continue;
        }
        let r = e.range;
        if let Some(bytes) = data.get(r.start as usize..r.end() as usize) {
            out.push(b' ');
            out.extend_from_slice(bytes);
        }
    }
    out.push(b' ');
    out.extend_from_slice(add.as_bytes());
    out.extend_from_slice(b" >>");
    out
}

/// Rows undone of a PNG predictor (7.4.4.4): each a filter byte, then the
/// row, each byte predicted from the one a pixel before, the one above, or
/// both.
fn unpredict(raw: &[u8], colors: usize, bpc: usize, columns: usize) -> Option<Vec<u8>> {
    let bpp = (colors * bpc).div_ceil(8).max(1);
    let row = (colors * bpc * columns).div_ceil(8);
    if row == 0 {
        return None;
    }
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut above = vec![0u8; row];
    for line in raw.chunks(row + 1) {
        let (&kind, bytes) = line.split_first()?;
        let mut cur = bytes.to_vec();
        cur.resize(row, 0);
        for i in 0..row {
            let left = if i >= bpp { cur[i - bpp] } else { 0 };
            let up = above[i];
            let up_left = if i >= bpp { above[i - bpp] } else { 0 };
            let guess = match kind {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => {
                    let p = i16::from(left) + i16::from(up) - i16::from(up_left);
                    let (pa, pb, pc) = (
                        (p - i16::from(left)).abs(),
                        (p - i16::from(up)).abs(),
                        (p - i16::from(up_left)).abs(),
                    );
                    if pa <= pb && pa <= pc {
                        left
                    } else if pb <= pc {
                        up
                    } else {
                        up_left
                    }
                }
                _ => return None,
            };
            cur[i] = cur[i].wrapping_add(guess);
        }
        out.extend_from_slice(&cur);
        above = cur;
    }
    Some(out)
}

/// Clears bits `from..to` of `row`, the first bit the high one.
fn clear_bits(row: &mut [u8], mut from: usize, to: usize) {
    while from < to {
        let Some(byte) = row.get_mut(from / 8) else {
            return;
        };
        if from.is_multiple_of(8) && to - from >= 8 {
            *byte = 0;
            from += 8;
        } else {
            *byte &= !(0x80 >> (from % 8));
            from += 1;
        }
    }
}

/// Keys a picture written again no longer has, or has anew.
const REWRITTEN: [&str; 6] = ["Filter", "DecodeParms", "Length", "DL", "FFilter", BLACKED];

/// Where a picture written again says it was blacked out: rectangles of
/// its unit square, four numbers each. A black box over one of them hides
/// nothing any more, and the check says so by leaving it be.
const BLACKED: &str = "HexscopeBlackedOut";

/// The parts of picture `rec` blacked out by an earlier copy.
fn blacked(rec: &ObjRec) -> Vec<[f64; 4]> {
    let Some(Obj::Array(a)) = rec.value.get(BLACKED) else {
        return Vec::new();
    };
    let v: Vec<f64> = a.iter().filter_map(|i| super::page::num(&i.obj)).collect();
    v.as_chunks::<4>().0.to_vec()
}

/// The key that says so, for the parts already blacked out and `unit`.
fn blacked_key(rec: &ObjRec, unit: &[[f64; 4]]) -> String {
    let mut all = blacked(rec);
    for u in unit {
        if !all.contains(u) {
            all.push(*u);
        }
    }
    let nums: Vec<String> = all
        .iter()
        .flatten()
        .map(|&v| crate::fixed::fixed(v, 4))
        .collect();
    format!("/{BLACKED} [{}]", nums.join(" "))
}

/// Whether the part `unit` of picture `num` was blacked out by an earlier
/// copy, to within rounding.
pub(super) fn already_blacked(ctx: &Ctx, num: u32, unit: &[f64; 4]) -> bool {
    let Some(rec) = stream(ctx, num) else {
        return false;
    };
    blacked(rec).iter().any(|b| {
        b[0] <= unit[0] + 1e-3
            && b[1] <= unit[1] + 1e-3
            && b[2] >= unit[2] - 1e-3
            && b[3] >= unit[3] - 1e-3
    })
}

/// A picture's samples, undone of Flate and any predictor: rows from the
/// top, each `row` bytes, `bpc` bits for each of `comps` components.
pub(crate) struct Samples {
    pub width: u32,
    pub height: u32,
    pub bpc: usize,
    pub comps: usize,
    pub row: usize,
    /// A stencil (8.9.6.2): a one-bit shape painted in the fill colour.
    pub mask: bool,
    pub data: Vec<u8>,
}

/// Picture `rec`'s samples, or what keeps them from being read here.
pub(crate) fn samples(data: &[u8], ctx: &Ctx, rec: &ObjRec) -> Result<Samples, Edited> {
    let dict = &rec.value;
    let mut budget = MAX_PICTURE;
    let size = |k: &str| {
        dict.get(k)
            .and_then(Obj::int)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (1..=1 << 16).contains(v))
    };
    let (Some(width), Some(height)) = (size("Width"), size("Height")) else {
        return Err(Edited::Cannot);
    };
    // JBIG2, CCITT, JPEG 2000 and the rest: not read here.
    let f = match filter(dict) {
        Ok(f @ (None | Some("FlateDecode" | "Fl"))) => f,
        Ok(Some("DCTDecode" | "DCT")) => return Err(Edited::Jpeg),
        _ => return Err(Edited::Cannot),
    };
    let mask = matches!(dict.get("ImageMask"), Some(Obj::Bool(true)));
    let (bpc, comps) = if mask {
        (1, 1)
    } else {
        let bpc = dict.get("BitsPerComponent").and_then(Obj::int).unwrap_or(8);
        let Some(comps) = dict
            .get("ColorSpace")
            .and_then(|cs| components(data, ctx, cs, &mut budget))
        else {
            return Err(Edited::Cannot);
        };
        (bpc as usize, comps)
    };
    if ![1, 2, 4, 8, 16].contains(&bpc) {
        return Err(Edited::Cannot);
    }
    let row = (width as usize * bpc * comps).div_ceil(8);
    let need = row as u64 * u64::from(height);
    if need > MAX_PICTURE {
        return Err(Edited::Cannot);
    }
    let Some(stored) = rec
        .stream
        .and_then(|(r, _)| data.get(r.start as usize..r.end() as usize))
    else {
        return Err(Edited::Cannot);
    };
    let mut out = if f.is_some() {
        let Some(raw) = zlib_decompress(stored, MAX_PICTURE + u64::from(height), &mut NoTrace)
            .ok()
            .or_else(|| inflate(stored.get(2..).unwrap_or(&[]), MAX_PICTURE, &mut NoTrace).ok())
        else {
            return Err(Edited::Cannot);
        };
        let parms = dict.get("DecodeParms").and_then(|p| match p {
            Obj::Array(a) => a.first().map(|i| i.obj.clone()),
            other => Some(other.clone()),
        });
        let get = |k: &str, d: i64| {
            parms
                .as_ref()
                .and_then(|p| p.get(k))
                .and_then(Obj::int)
                .unwrap_or(d)
        };
        match get("Predictor", 1) {
            1 => raw,
            10..=15 => {
                let colors = get("Colors", 1).clamp(1, 32) as usize;
                let pbpc = get("BitsPerComponent", 8).clamp(1, 16) as usize;
                let columns = get("Columns", 1).clamp(1, 1 << 16) as usize;
                unpredict(&raw, colors, pbpc, columns).ok_or(Edited::Cannot)?
            }
            _ => return Err(Edited::Cannot),
        }
    } else {
        stored.to_vec()
    };
    if (out.len() as u64) < need {
        return Err(Edited::Cannot);
    }
    out.truncate(need as usize);
    Ok(Samples {
        width,
        height,
        bpc,
        comps,
        row,
        mask,
        data: out,
    })
}

/// Picture `rec`, the parts `unit` of it zeroed and written again.
pub(crate) fn edit(data: &[u8], ctx: &Ctx, rec: &ObjRec, unit: &[[f64; 4]]) -> Edited {
    let mut s = match samples(data, ctx, rec) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let bits = s.bpc * s.comps;
    for [x0, y0, x1, y1] in pixels(unit, s.width, s.height) {
        for y in y0..y1 {
            let at = y as usize * s.row;
            if let Some(line) = s.data.get_mut(at..at + s.row) {
                clear_bits(line, x0 as usize * bits, x1 as usize * bits);
            }
        }
    }
    let packed = crate::deflate::zlib_compress(&s.data);
    let dict = dict_without(
        data,
        rec,
        &REWRITTEN,
        &format!(
            "/Filter /FlateDecode /Length {} {}",
            packed.len(),
            blacked_key(rec, unit)
        ),
    );
    Edited::Written(dict, packed)
}

/// Pixels shown, at most, for a picture drawn to choose what to black out.
const MAX_SHOWN: u64 = 16 * 1024 * 1024;

/// Picture `rec` as RGBA, to be seen: gray, RGB and CMYK pictures and
/// stencils — not indexed or spot colours, whose samples are not colours.
pub(crate) fn rgba(data: &[u8], ctx: &Ctx, rec: &ObjRec) -> Option<(u32, u32, Vec<u8>)> {
    let s = samples(data, ctx, rec).ok()?;
    if u64::from(s.width) * u64::from(s.height) > MAX_SHOWN {
        return None;
    }
    if !s.mask {
        let mut budget = MAX_PICTURE;
        let cs = rec
            .value
            .get("ColorSpace")
            .and_then(|c| deref(data, ctx, c, &mut budget))?;
        let family = match &cs {
            Obj::Name(n) => n.as_str(),
            Obj::Array(a) => a.first()?.obj.name()?,
            _ => return None,
        };
        if matches!(family, "Indexed" | "I" | "Separation" | "DeviceN" | "Lab") {
            return None;
        }
    }
    let inverted = matches!(
        rec.value.get("Decode"),
        Some(Obj::Array(a)) if a.first().and_then(|i| i.obj.int()) == Some(1)
    );
    let bpc = s.bpc;
    let sample = |line: &[u8], i: usize| -> u8 {
        let bit = i * bpc;
        let byte = line.get(bit / 8).copied().unwrap_or(0);
        match bpc {
            8 => byte,
            16 => byte,
            _ => {
                let v = (byte >> (8 - bpc - bit % 8)) & ((1 << bpc) - 1);
                (u16::from(v) * 255 / ((1 << bpc) - 1)) as u8
            }
        }
    };
    let (w, h) = (s.width as usize, s.height as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let line = s.data.get(y * s.row..(y + 1) * s.row).unwrap_or(&[]);
        for x in 0..w {
            let c = |k: usize| {
                let v = sample(line, x * s.comps + k);
                if inverted { 255 - v } else { v }
            };
            let px = if s.mask {
                // Zero paints; one leaves the page.
                if c(0) == 0 {
                    [0, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                }
            } else {
                match s.comps {
                    1 => [c(0), c(0), c(0), 255],
                    3 => [c(0), c(1), c(2), 255],
                    4 => {
                        let k = 255 - u16::from(c(3));
                        let ink = |v: u8| ((255 - u16::from(v)) * k / 255) as u8;
                        [ink(c(0)), ink(c(1)), ink(c(2)), 255]
                    }
                    _ => return None,
                }
            };
            out.extend_from_slice(&px);
        }
    }
    Some((s.width, s.height, out))
}

/// A JPEG picture `rec` replaced by `jpeg`, the caller's painted copy of it:
/// in colour, eight bits, whatever it was before.
pub(crate) fn jpeg_dict(data: &[u8], rec: &ObjRec, jpeg: &[u8], unit: &[[f64; 4]]) -> Vec<u8> {
    dict_without(
        data,
        rec,
        &[
            "Filter",
            "DecodeParms",
            "Length",
            "DL",
            "FFilter",
            "ColorSpace",
            "BitsPerComponent",
            "Decode",
            BLACKED,
        ],
        &format!(
            "/Filter /DCTDecode /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} {}",
            jpeg.len(),
            blacked_key(rec, unit)
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_maps_into_a_pictures_pixels() {
        // A 100 by 50 point picture at (10, 20), and a box over its top left quarter.
        let m = [100.0, 0.0, 0.0, 50.0, 10.0, 20.0];
        let hit = under(&m, &Area([10.0, 45.0, 60.0, 70.0])).unwrap();
        assert_eq!(hit, [0.0, 0.5, 0.5, 1.0]);
        // Rows count from the top.
        assert_eq!(pixels(&[hit], 200, 100), [[0, 0, 101, 51]]);
        // Beside it, nothing.
        assert!(under(&m, &Area([200.0, 0.0, 300.0, 10.0])).is_none());
        // Turned a quarter, the same box lands on another part.
        let turned = [0.0, 50.0, -100.0, 0.0, 110.0, 20.0];
        assert!(under(&turned, &Area([10.0, 20.0, 20.0, 30.0])).is_some());
    }

    #[test]
    fn predicted_rows_come_back() {
        // Two rows of three gray pixels: Sub, then Up.
        let raw = [1, 10, 5, 5, 2, 1, 1, 1];
        assert_eq!(unpredict(&raw, 1, 8, 3).unwrap(), [10, 15, 20, 11, 16, 21]);
        let mut row = [0xFFu8; 3];
        clear_bits(&mut row, 3, 13);
        assert_eq!(row, [0b1110_0000, 0b0000_0111, 0xFF]);
    }
}
