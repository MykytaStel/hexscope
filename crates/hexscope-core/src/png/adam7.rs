//! Adam7, PNG's interlacing (PNG §8.2). The picture is stored in seven
//! passes, each a smaller picture of every 8th, 4th or 2nd pixel, so a
//! viewer can show a coarse version early and fill it in. Each pass is
//! filtered as a picture of its own; a pass with no pixels has no bytes at
//! all, not even filter bytes.

use super::fields::Ihdr;
use super::unfilter::{UnfilterError, unfilter};

/// Where each pass starts on the 8×8 grid, and its steps, across and down.
const ORIGIN: [(u32, u32); 7] = [(0, 0), (4, 0), (0, 4), (2, 0), (0, 2), (1, 0), (0, 1)];
const STEP: [(u32, u32); 7] = [(8, 8), (8, 8), (4, 8), (4, 4), (2, 4), (2, 2), (1, 2)];

/// One pass: which pixels it holds, and where its scanlines are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pass {
    /// Its first pixel, and the steps between its pixels.
    pub x0: u32,
    pub y0: u32,
    pub dx: u32,
    pub dy: u32,
    /// Its size as a picture of its own.
    pub width: u32,
    pub height: u32,
    /// Bytes in one of its scanlines, the filter byte not counted.
    pub stride: usize,
    /// Where its first filter byte is in the decompressed data.
    pub start: usize,
}

/// The seven passes of an interlaced picture; empty ones are included, with
/// no rows. `None` when the sizes overflow.
pub fn passes(ihdr: &Ihdr) -> Option<[Pass; 7]> {
    let bits = ihdr.channels() * ihdr.bit_depth as usize;
    let mut start = 0usize;
    let mut out = [Pass {
        x0: 0,
        y0: 0,
        dx: 1,
        dy: 1,
        width: 0,
        height: 0,
        stride: 0,
        start: 0,
    }; 7];
    for (i, pass) in out.iter_mut().enumerate() {
        let (x0, y0) = ORIGIN[i];
        let (dx, dy) = STEP[i];
        let width = ihdr.width.saturating_sub(x0).div_ceil(dx);
        let height = ihdr.height.saturating_sub(y0).div_ceil(dy);
        let stride = (width as usize).checked_mul(bits)?.div_ceil(8);
        *pass = Pass {
            x0,
            y0,
            dx,
            dy,
            width,
            height,
            stride,
            start,
        };
        if width > 0 && height > 0 {
            start = start.checked_add(stride.checked_add(1)?.checked_mul(height as usize)?)?;
        }
    }
    Some(out)
}

/// The picture's scanlines, unfiltered and put back in order: the same
/// layout as a picture that was never interlaced.
pub fn deinterlace(raw: &[u8], ihdr: &Ihdr) -> Result<Vec<u8>, UnfilterError> {
    let passes = passes(ihdr).ok_or(UnfilterError::BadDimensions)?;
    let stride = ihdr.stride().ok_or(UnfilterError::BadDimensions)?;
    let bits = ihdr.channels() * ihdr.bit_depth as usize;
    let mut out = vec![
        0u8;
        stride
            .checked_mul(ihdr.height as usize)
            .ok_or(UnfilterError::BadDimensions)?
    ];
    for p in passes {
        if p.width == 0 || p.height == 0 {
            continue;
        }
        let data = raw.get(p.start..).ok_or(UnfilterError::ShortData)?;
        let pixels = unfilter(data, p.stride, p.height, ihdr.filter_distance())?;
        for r in 0..p.height as usize {
            let y = p.y0 as usize + r * p.dy as usize;
            let src = &pixels[r * p.stride..(r + 1) * p.stride];
            let dst = &mut out[y * stride..(y + 1) * stride];
            for c in 0..p.width as usize {
                let x = p.x0 as usize + c * p.dx as usize;
                if bits >= 8 {
                    let n = bits / 8;
                    dst[x * n..(x + 1) * n].copy_from_slice(&src[c * n..(c + 1) * n]);
                } else {
                    // 1, 2 or 4 bits: moved within their bytes.
                    let mask = (1u8 << bits) - 1;
                    let from = c * bits;
                    let v = (src[from / 8] >> (8 - bits - from % 8)) & mask;
                    let to = x * bits;
                    let shift = 8 - bits - to % 8;
                    dst[to / 8] = (dst[to / 8] & !(mask << shift)) | (v << shift);
                }
            }
        }
    }
    Ok(out)
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

    #[test]
    fn the_passes_of_an_8_by_8_picture_hold_1_1_2_4_8_16_32_pixels() {
        let ihdr = Ihdr {
            width: 8,
            height: 8,
            bit_depth: 8,
            color_type: 0,
            interlace: 1,
        };
        let p = passes(&ihdr).unwrap();
        let counts: Vec<u32> = p.iter().map(|p| p.width * p.height).collect();
        assert_eq!(counts, [1, 1, 2, 4, 8, 16, 32]);
        // Each pass's rows follow the last's: a filter byte and the row.
        assert_eq!(p[1].start, 2);
        assert_eq!(p[6].start, 2 + 2 + 3 + 2 * 3 + 2 * 5 + 4 * 5);
    }

    #[test]
    fn a_one_pixel_picture_has_one_pass() {
        let ihdr = Ihdr {
            width: 1,
            height: 1,
            bit_depth: 1,
            color_type: 0,
            interlace: 1,
        };
        let p = passes(&ihdr).unwrap();
        assert_eq!((p[0].width, p[0].height), (1, 1));
        assert!(p[1..].iter().all(|p| p.width * p.height == 0));
    }

    #[test]
    fn interlaced_pictures_come_out_as_their_plain_twins() {
        let pairs = [
            "basi0g01", "basi0g02", "basi0g04", "basi0g08", "basi0g16", "basi2c08", "basi2c16",
            "basi3p01", "basi3p02", "basi3p04", "basi3p08", "basi4a08", "basi4a16", "basi6a08",
            "basi6a16", "s01i3p01", "s02i3p01", "s03i3p01", "s04i3p01", "s05i3p02", "s06i3p02",
            "s07i3p02", "s08i3p02", "s09i3p02", "s32i3p04", "s33i3p04", "s34i3p04", "s35i3p04",
            "s36i3p04", "s37i3p04", "s38i3p04", "s39i3p04", "s40i3p04",
        ];
        for name in pairs {
            let twin = if let Some(rest) = name.strip_prefix("basi") {
                format!("basn{rest}")
            } else {
                name.replacen('i', "n", 1)
            };
            let interlaced = parse_png(&suite(&format!("{name}.png")));
            let plain = parse_png(&suite(&format!("{twin}.png")));
            assert_eq!(interlaced.ihdr.unwrap().interlace, 1, "{name}");
            assert!(interlaced.pixels.is_some(), "{name} has pixels");
            assert_eq!(interlaced.pixels, plain.pixels, "{name} and {twin}");
        }
    }
}
