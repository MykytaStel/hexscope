#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnfilterError {
    /// The decompressed data is smaller than width × height requires.
    ShortData,
    BadFilterType(u8),
    BadDimensions,
}

/// PNG's Paeth predictor (RFC 2083 §6.6).
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Reverses the per-scanline filters PNG applies before compression. `raw` is
/// the decompressed IDAT payload: one filter-type byte per row, then the row.
pub fn unfilter(raw: &[u8], width: u32, height: u32, bpp: usize) -> Result<Vec<u8>, UnfilterError> {
    if width == 0 || height == 0 || bpp == 0 {
        return Err(UnfilterError::BadDimensions);
    }

    let row_len = (width as usize)
        .checked_mul(bpp)
        .ok_or(UnfilterError::BadDimensions)?;
    // Every step is checked: `usize` is 32 bits on wasm32, the target this
    // crate compiles to, so a crafted width really can reach the top of the
    // range. `row_len + 1` is the stride including the filter-type byte.
    let needed = row_len
        .checked_add(1)
        .and_then(|stride| stride.checked_mul(height as usize))
        .ok_or(UnfilterError::BadDimensions)?;
    if raw.len() < needed {
        return Err(UnfilterError::ShortData);
    }

    let out_len = row_len
        .checked_mul(height as usize)
        .ok_or(UnfilterError::BadDimensions)?;
    let mut out = vec![0u8; out_len];

    for y in 0..height as usize {
        let filter = raw[y * (row_len + 1)];
        let src = &raw[y * (row_len + 1) + 1..y * (row_len + 1) + 1 + row_len];

        for x in 0..row_len {
            let left = if x >= bpp {
                out[y * row_len + x - bpp]
            } else {
                0
            };
            let up = if y > 0 { out[(y - 1) * row_len + x] } else { 0 };
            let up_left = if y > 0 && x >= bpp {
                out[(y - 1) * row_len + x - bpp]
            } else {
                0
            };

            let value = match filter {
                0 => src[x],
                1 => src[x].wrapping_add(left),
                2 => src[x].wrapping_add(up),
                3 => src[x].wrapping_add(((left as u16 + up as u16) / 2) as u8),
                4 => src[x].wrapping_add(paeth(left, up, up_left)),
                other => return Err(UnfilterError::BadFilterType(other)),
            };
            out[y * row_len + x] = value;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_none_passes_bytes_through() {
        // One row, 2 px, 1 byte per pixel, filter type 0.
        let raw = [0, 10, 20];
        assert_eq!(unfilter(&raw, 2, 1, 1).unwrap(), vec![10, 20]);
    }

    #[test]
    fn filter_sub_adds_the_pixel_to_the_left() {
        // filter 1: each byte is a delta from the byte one pixel back.
        let raw = [1, 10, 5, 5];
        assert_eq!(unfilter(&raw, 3, 1, 1).unwrap(), vec![10, 15, 20]);
    }

    #[test]
    fn filter_up_adds_the_row_above() {
        let raw = [0, 10, 20, 2, 5, 5];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![10, 20, 15, 25]);
    }

    #[test]
    fn filter_average_reconstructs_from_left_and_up() {
        // 2 px wide, 2 rows, 1 byte per pixel.
        // Row 0, filter 0: [8, 16].
        // Row 1, filter 3: out = delta + floor((left + up) / 2)
        //   x=0: left=0 (no pixel to the left), up=8  -> 4 + 4  = 8
        //   x=1: left=8,                        up=16 -> 4 + 12 = 16
        let raw = [0, 8, 16, 3, 4, 4];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![8, 16, 8, 16]);
    }

    #[test]
    fn filter_paeth_picks_the_closest_predictor() {
        // Row 0, filter 0: [8, 16].
        // Row 1, filter 4, deltas [0, 0]:
        //   x=0: paeth(left=0, up=8, upleft=0)  -> 8  -> out 8
        //   x=1: paeth(left=8, up=16, upleft=8) -> 16 -> out 16
        let raw = [0, 8, 16, 4, 0, 0];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![8, 16, 8, 16]);
    }

    #[test]
    fn paeth_predictor_matches_the_spec_examples() {
        assert_eq!(paeth(0, 0, 0), 0);
        assert_eq!(paeth(10, 20, 30), 10); // p=0:  pa=10, pb=20, pc=30
        assert_eq!(paeth(30, 20, 10), 30); // p=40: pa=10, pb=20, pc=30
        // p = 1 + 200 - 100 = 101; pa=100, pb=99, pc=1 -> upper-left wins.
        assert_eq!(paeth(1, 200, 100), 100);
    }

    #[test]
    fn rejects_an_unknown_filter_type() {
        let raw = [9, 1, 2];
        assert_eq!(
            unfilter(&raw, 2, 1, 1),
            Err(UnfilterError::BadFilterType(9))
        );
    }

    #[test]
    fn reports_short_data_instead_of_panicking() {
        let raw = [0, 1];
        assert_eq!(unfilter(&raw, 5, 1, 1), Err(UnfilterError::ShortData));
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(unfilter(&[], 0, 1, 1), Err(UnfilterError::BadDimensions));
    }
}
