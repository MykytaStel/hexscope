//! A stream's filters undone, one after another (7.4): Flate, LZW, ASCII
//! hex and base-85, run lengths, and the PNG and TIFF predictors Flate and
//! LZW may apply first. Picture codecs — DCT, JPX, JBIG2, CCITT — are
//! pictures' business, not text's, and stop the chain.

use super::lexer::Obj;
use super::page::num;
use hexscope_inflate::{NoTrace, inflate, zlib_decompress};

/// A picture codec: its data is the picture, left as it is.
pub(super) fn is_picture(filter: &str) -> bool {
    matches!(
        filter,
        "DCTDecode" | "DCT" | "JPXDecode" | "JBIG2Decode" | "CCITTFaxDecode" | "CCF"
    )
}

/// Undoes `filter` on `data`, output at most `limit` bytes; `None` for a
/// filter this does not undo, or data it cannot.
pub(super) fn undo(filter: &str, parms: Option<&Obj>, data: &[u8], limit: u64) -> Option<Vec<u8>> {
    let out = match filter {
        "FlateDecode" | "Fl" => zlib_decompress(data, limit, &mut NoTrace)
            .ok()
            // Some writers get the Adler-32 wrong; the data is still good.
            .or_else(|| inflate(data.get(2..)?, limit, &mut NoTrace).ok())?,
        "LZWDecode" | "LZW" => lzw(data, limit, parm(parms, "EarlyChange").unwrap_or(1) != 0),
        "ASCIIHexDecode" | "AHx" => hex(data, limit),
        "ASCII85Decode" | "A85" => base85(data, limit)?,
        "RunLengthDecode" | "RL" => run_length(data, limit),
        _ => return None,
    };
    match filter {
        "FlateDecode" | "Fl" | "LZWDecode" | "LZW" => predict(out, parms),
        _ => Some(out),
    }
}

fn parm(parms: Option<&Obj>, key: &str) -> Option<u64> {
    num(parms?.get(key)?).map(|v| v as u64)
}

fn hex(data: &[u8], limit: u64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut high: Option<u8> = None;
    for &c in data {
        if c == b'>' || out.len() as u64 >= limit {
            break;
        }
        let Some(v) = (c as char).to_digit(16) else {
            continue;
        };
        match high.take() {
            Some(h) => out.push(h << 4 | v as u8),
            None => high = Some(v as u8),
        }
    }
    // An odd digit at the end is followed by a 0.
    if let Some(h) = high {
        out.push(h << 4);
    }
    out
}

fn base85(data: &[u8], limit: u64) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut group = [0u8; 5];
    let mut n = 0;
    let body = data.strip_prefix(b"<~").unwrap_or(data);
    for &c in body {
        if c == b'~' || out.len() as u64 >= limit {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        if c == b'z' && n == 0 {
            out.extend([0; 4]);
            continue;
        }
        if !(b'!'..=b'u').contains(&c) {
            return None;
        }
        group[n] = c - b'!';
        n += 1;
        if n == 5 {
            out.extend(word(&group).to_be_bytes());
            n = 0;
        }
    }
    // A last group of n characters stands for n - 1 bytes, padded with 'u'.
    if n > 1 {
        group[n..].fill(84);
        out.extend(&word(&group).to_be_bytes()[..n - 1]);
    }
    Some(out)
}

fn word(g: &[u8; 5]) -> u32 {
    g.iter()
        .fold(0u32, |v, &d| v.wrapping_mul(85).wrapping_add(d as u32))
}

fn run_length(data: &[u8], limit: u64) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() && (out.len() as u64) < limit {
        let n = data[i] as usize;
        i += 1;
        match n {
            128 => break,
            0..=127 => {
                let end = (i + n + 1).min(data.len());
                out.extend_from_slice(&data[i..end]);
                i = end;
            }
            _ => {
                let Some(&b) = data.get(i) else { break };
                out.extend(std::iter::repeat_n(b, 257 - n));
                i += 1;
            }
        }
    }
    out
}

/// LZW as PDF uses it (7.4.4): codes from 9 to 12 bits, 256 to clear the
/// table, 257 to end; with early change, a code grows a word sooner.
fn lzw(data: &[u8], limit: u64, early: bool) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    // Each entry: where its bytes start in `out`, and how many.
    let mut table: Vec<(usize, usize)> = Vec::with_capacity(4096);
    let mut width = 9;
    let mut previous: Option<(usize, usize)> = None;
    let (mut acc, mut bits, mut at) = (0u32, 0, 0);
    loop {
        while bits < width {
            let Some(&b) = data.get(at) else { return out };
            at += 1;
            acc = (acc << 8) | b as u32;
            bits += 8;
        }
        bits -= width;
        let code = ((acc >> bits) & ((1 << width) - 1)) as usize;
        match code {
            256 => {
                table.clear();
                width = 9;
                previous = None;
                continue;
            }
            257 => return out,
            _ => {}
        }
        let start = out.len();
        let entry = match code {
            0..=255 => {
                out.push(code as u8);
                (start, 1)
            }
            c if c - 258 < table.len() => {
                let (s, n) = table[c - 258];
                out.extend_from_within(s..s + n);
                (start, n)
            }
            // The code being defined: the last word and its first byte.
            c if c - 258 == table.len() => {
                let Some((s, n)) = previous else { return out };
                out.extend_from_within(s..s + n);
                out.push(out[s]);
                (start, n + 1)
            }
            _ => return out,
        };
        if let Some((s, n)) = previous
            && table.len() < 4096 - 258
        {
            // The last word and this one's first byte, which now follows it.
            table.push((s, n + 1));
        }
        previous = Some(entry);
        let next = table.len() + 258 + usize::from(early);
        width = if next < 512 {
            9
        } else if next < 1024 {
            10
        } else if next < 2048 {
            11
        } else {
            12
        };
        if out.len() as u64 >= limit {
            out.truncate(limit as usize);
            return out;
        }
    }
}

/// Undoes the predictor a stream's parameters name: TIFF's (2) or PNG's
/// (10 and up), each row led by its filter type.
fn predict(data: Vec<u8>, parms: Option<&Obj>) -> Option<Vec<u8>> {
    let predictor = parm(parms, "Predictor").unwrap_or(1);
    if predictor < 2 {
        return Some(data);
    }
    let colors = parm(parms, "Colors").unwrap_or(1).clamp(1, 32) as usize;
    let bpc = parm(parms, "BitsPerComponent").unwrap_or(8).clamp(1, 16) as usize;
    let columns = parm(parms, "Columns").unwrap_or(1).clamp(1, 1 << 20) as usize;
    let pixel = (colors * bpc).div_ceil(8);
    let row = (colors * bpc * columns).div_ceil(8);
    if predictor == 2 {
        if bpc != 8 {
            return Some(data);
        }
        let mut out = data;
        for r in out.chunks_mut(row) {
            for i in pixel..r.len() {
                r[i] = r[i].wrapping_add(r[i - pixel]);
            }
        }
        return Some(out);
    }
    let mut out = Vec::with_capacity(data.len());
    let mut prior = vec![0u8; row];
    for line in data.chunks(row + 1) {
        let (&kind, bytes) = line.split_first()?;
        let mut cur = bytes.to_vec();
        cur.resize(row, 0);
        for i in 0..row {
            let a = if i >= pixel { cur[i - pixel] } else { 0 };
            let b = prior[i];
            let c = if i >= pixel { prior[i - pixel] } else { 0 };
            cur[i] = cur[i].wrapping_add(match kind {
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => {
                    let p = a as i16 + b as i16 - c as i16;
                    let (pa, pb, pc) = (
                        (p - a as i16).abs(),
                        (p - b as i16).abs(),
                        (p - c as i16).abs(),
                    );
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => 0,
            });
        }
        out.extend_from_slice(&cur[..bytes.len().min(row)]);
        prior = cur;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_filter_undone() {
        assert_eq!(hex(b"48 65 6C6c6F>", 100), b"Hello");
        assert_eq!(hex(b"7", 100), [0x70]);
        assert_eq!(base85(b"<~87cURD]i,\"Ebo7~>", 100).unwrap(), b"Hello World");
        assert_eq!(base85(b"z~>", 100).unwrap(), [0; 4]);
        assert_eq!(
            run_length(&[2, b'a', b'b', b'c', 254, b'x', 128], 100),
            b"abcxxx"
        );
        // "-----A---B" in LZW, with early change: the specification's example (7.4.4.2).
        let lzw_example = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        assert_eq!(lzw(&lzw_example, 100, true), b"-----A---B");
    }

    #[test]
    fn png_predictors_are_undone_row_by_row() {
        // Two rows of three bytes: Up, then Sub.
        let data = vec![2, 1, 2, 3, 1, 4, 1, 1];
        let parms = super::super::lexer::Lexer::new(b"<< /Predictor 12 /Columns 3 >>", 0)
            .value()
            .map(|i| i.obj);
        assert_eq!(predict(data, parms.as_ref()).unwrap(), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn broken_data_never_panics() {
        for n in 0..64u8 {
            let junk: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
            let _ = lzw(&junk, 1000, true);
            let _ = lzw(&junk, 1000, false);
            let _ = base85(&junk, 1000);
            let _ = run_length(&junk, 1000);
            let _ = hex(&junk, 1000);
        }
    }
}
