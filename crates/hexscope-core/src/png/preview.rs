//! The picture itself, as 8-bit RGBA for a screen, scaled down when large.
//!
//! PNG stores samples at 1 to 16 bits, as grey, RGB, palette indices, with
//! or without alpha, and tRNS can make one colour or some palette entries
//! transparent (PNG §11.2.1, §11.3.2.1). Every combination comes out the
//! same way here: four bytes per pixel. No gamma or colour profile is
//! applied; the point is to see where bytes land, not to match a viewer.

use super::PngDocument;
use crate::model::ByteRange;

pub struct Preview {
    pub width: u32,
    pub height: u32,
    /// Four bytes per pixel, row by row.
    pub rgba: Vec<u8>,
}

/// A chunk's data, found by its label among the top-level nodes.
fn chunk_data<'a>(data: &'a [u8], doc: &PngDocument, label: &str) -> Option<&'a [u8]> {
    let tree = &doc.tree;
    let node = tree
        .get(tree.root()?)
        .children
        .iter()
        .map(|&c| tree.get(c))
        .find(|n| n.label == label)?;
    let ByteRange { start, len } = node.range;
    // Length and type before the data, the CRC after.
    data.get(start as usize + 8..(start + len) as usize - 4)
}

/// The picture scaled so its longer side is at most `max_side`, or `None`
/// when there are no pixels to show (interlaced, damaged, or absent).
pub fn preview(data: &[u8], doc: &PngDocument, max_side: u32) -> Option<Preview> {
    let ihdr = doc.ihdr?;
    let pixels = doc.pixels.as_ref()?;
    let (w, h) = (ihdr.width, ihdr.height);
    if w == 0 || h == 0 || max_side == 0 {
        return None;
    }
    let stride = ihdr.stride()?;
    let depth = ihdr.bit_depth as usize;
    let channels = ihdr.channels();
    if pixels.len() < stride.checked_mul(h as usize)? || !matches!(depth, 1 | 2 | 4 | 8 | 16) {
        return None;
    }
    let palette = chunk_data(data, doc, "PLTE").unwrap_or_default();
    let trns = chunk_data(data, doc, "tRNS").unwrap_or_default();

    let longest = w.max(h);
    let (pw, ph) = if longest <= max_side {
        (w, h)
    } else {
        (
            ((w as u64 * max_side as u64 / longest as u64) as u32).max(1),
            ((h as u64 * max_side as u64 / longest as u64) as u32).max(1),
        )
    };

    // Sample `c` of pixel `x` in row `y`, at the file's depth.
    let sample = |x: usize, y: usize, c: usize| -> u16 {
        let row = &pixels[y * stride..(y + 1) * stride];
        let bit = (x * channels + c) * depth;
        match depth {
            16 => u16::from_be_bytes([row[bit / 8], row[bit / 8 + 1]]),
            8 => row[bit / 8] as u16,
            _ => {
                let shift = 8 - depth - bit % 8;
                ((row[bit / 8] >> shift) & ((1u8 << depth) - 1)) as u16
            }
        }
    };
    let max = (1u32 << depth) - 1;
    let to8 = |v: u16| -> u8 {
        match depth {
            16 => (v >> 8) as u8,
            8 => v as u8,
            _ => (v as u32 * 255 / max) as u8,
        }
    };
    // tRNS for grey and RGB: the one colour that is transparent, compared at
    // the file's depth.
    let key = |i: usize| -> Option<u16> {
        trns.get(i * 2..i * 2 + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) & max as u16)
    };

    let mut rgba = Vec::with_capacity(pw as usize * ph as usize * 4);
    for py in 0..ph {
        let y = (py as u64 * h as u64 / ph as u64) as usize;
        for px in 0..pw {
            let x = (px as u64 * w as u64 / pw as u64) as usize;
            let s = |c| sample(x, y, c);
            let pixel = match ihdr.color_type {
                0 => {
                    let g = s(0);
                    let a = if key(0) == Some(g) { 0 } else { 255 };
                    [to8(g), to8(g), to8(g), a]
                }
                2 => {
                    let (r, g, b) = (s(0), s(1), s(2));
                    let clear = key(0) == Some(r) && key(1) == Some(g) && key(2) == Some(b);
                    [to8(r), to8(g), to8(b), if clear { 0 } else { 255 }]
                }
                3 => {
                    let i = s(0) as usize;
                    let rgb = palette.get(i * 3..i * 3 + 3).unwrap_or(&[0, 0, 0]);
                    [rgb[0], rgb[1], rgb[2], trns.get(i).copied().unwrap_or(255)]
                }
                4 => {
                    let g = to8(s(0));
                    [g, g, g, to8(s(1))]
                }
                _ => [to8(s(0)), to8(s(1)), to8(s(2)), to8(s(3))],
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    Some(Preview {
        width: pw,
        height: ph,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png::parse_png;

    fn suite(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/pngsuite/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    fn pixel(p: &Preview, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * p.width + x) * 4) as usize;
        p.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn every_colour_type_and_depth_comes_out_as_rgba() {
        for name in [
            "basn0g01.png",
            "basn0g02.png",
            "basn0g04.png",
            "basn0g08.png",
            "basn0g16.png",
            "basn2c08.png",
            "basn2c16.png",
            "basn3p01.png",
            "basn3p02.png",
            "basn3p04.png",
            "basn3p08.png",
            "basn4a08.png",
            "basn4a16.png",
            "basn6a08.png",
            "basn6a16.png",
        ] {
            let data = suite(name);
            let p = preview(&data, &parse_png(&data), 1024).unwrap();
            assert_eq!((p.width, p.height), (32, 32), "{name}");
            assert_eq!(p.rgba.len(), 32 * 32 * 4, "{name}");
        }
        // A 1-bit greyscale pixel is black or white, nothing between.
        let data = suite("basn0g01.png");
        let p = preview(&data, &parse_png(&data), 1024).unwrap();
        assert!(
            p.rgba
                .chunks(4)
                .all(|c| c == [0, 0, 0, 255] || c == [255, 255, 255, 255])
        );
    }

    #[test]
    fn a_large_picture_is_scaled_to_fit() {
        let data = suite("basn2c08.png");
        let p = preview(&data, &parse_png(&data), 8).unwrap();
        assert_eq!((p.width, p.height), (8, 8));
        let full = preview(&data, &parse_png(&data), 32).unwrap();
        // Nearest neighbour: the scaled pixel is one of the original's.
        assert_eq!(pixel(&p, 1, 1), pixel(&full, 4, 4));
    }

    #[test]
    fn an_interlaced_picture_has_no_preview() {
        let data = suite("basi0g08.png");
        assert!(preview(&data, &parse_png(&data), 64).is_none());
    }
}
