//! A small DEFLATE compressor (RFC 1951) in a zlib wrapper (RFC 1950), for
//! the few things hexscope writes compressed: a picture edited in a PDF. One
//! block of the fixed codes, repeats found greedily through a hash of three
//! bytes — far from the best compression, and plenty for a scanned page,
//! which is mostly runs of the same byte.

use hexscope_inflate::adler32;

const WINDOW: usize = 32 * 1024;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// Earlier places with the same three bytes tried for each repeat.
const CHAIN: usize = 32;
const HASH_BITS: u32 = 15;

/// Lengths 3–258: the first length of each code from 257, and its extra bits.
const LENGTHS: [(u16, u8); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];

/// Distances 1–32768: the first distance of each code, and its extra bits.
const DISTANCES: [(u16, u8); 30] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
];

struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    /// `count` bits of `value`, least significant first.
    fn put(&mut self, value: u32, count: u32) {
        self.acc |= u64::from(value) << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// A Huffman code, which is written most significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }

    fn literal(&mut self, symbol: u16) {
        let s = u32::from(symbol);
        match s {
            0..=143 => self.code(0x30 + s, 8),
            144..=255 => self.code(0x190 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0xC0 + s - 280, 8),
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

/// The code for a value from a table of first values: the last one not past it.
fn bucket(table: &[(u16, u8)], v: usize) -> usize {
    table
        .iter()
        .rposition(|&(first, _)| usize::from(first) <= v)
        .unwrap_or(0)
}

/// `data`, deflated, as a zlib stream.
pub(crate) fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        n: 0,
    };
    // The last block, of the fixed codes.
    bits.put(1, 1);
    bits.put(1, 2);

    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let hash = |at: usize| {
        let v = u32::from(data[at]) << 16 | u32::from(data[at + 1]) << 8 | u32::from(data[at + 2]);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let insert = |at: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if at + MIN_MATCH <= data.len() {
            let h = hash(at);
            prev[at % WINDOW] = head[h];
            head[h] = at;
        }
    };

    let mut at = 0;
    while at < data.len() {
        let mut best = (0, 0);
        if at + MIN_MATCH <= data.len() {
            let mut cand = head[hash(at)];
            let limit = (data.len() - at).min(MAX_MATCH);
            for _ in 0..CHAIN {
                if cand == usize::MAX || at - cand > WINDOW || cand >= at {
                    break;
                }
                let len = data[cand..]
                    .iter()
                    .zip(&data[at..at + limit])
                    .take_while(|(a, b)| a == b)
                    .count();
                if len > best.0 {
                    best = (len, at - cand);
                    if len == limit {
                        break;
                    }
                }
                cand = prev[cand % WINDOW];
            }
        }
        if best.0 >= MIN_MATCH {
            let (len, dist) = best;
            let l = bucket(&LENGTHS, len);
            bits.literal(257 + l as u16);
            let (first, extra) = LENGTHS[l];
            bits.put((len - usize::from(first)) as u32, u32::from(extra));
            let d = bucket(&DISTANCES, dist);
            bits.code(d as u32, 5);
            let (first, extra) = DISTANCES[d];
            bits.put((dist - usize::from(first)) as u32, u32::from(extra));
            for k in at..at + len {
                insert(k, &mut head, &mut prev);
            }
            at += len;
        } else {
            bits.literal(u16::from(data[at]));
            insert(at, &mut head, &mut prev);
            at += 1;
        }
    }
    bits.literal(256);
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexscope_inflate::{NoTrace, zlib_decompress};

    fn round(data: &[u8]) -> Vec<u8> {
        let z = zlib_compress(data);
        let back = zlib_decompress(&z, 1 << 26, &mut NoTrace).unwrap();
        assert_eq!(back, data);
        z
    }

    #[test]
    fn what_goes_in_comes_out() {
        round(b"");
        round(b"a");
        round(b"abcabcabcabcabcabc hello hello hello");
        // Noise, which does not repeat.
        let mut x = 0x1234_5678u32;
        let noise: Vec<u8> = (0..100_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        round(&noise);
        // A page of white with a few dark rows, as a scan is.
        let mut page = vec![0xFFu8; 2_000_000];
        for row in (0..page.len()).step_by(5_000).take(40) {
            page[row..row + 700].fill(0);
        }
        let z = round(&page);
        assert!(z.len() < page.len() / 50, "{}", z.len());
        // And agreed with by another inflater.
        let mut d = flate2::read::ZlibDecoder::new(&z[..]);
        let mut back = Vec::new();
        std::io::Read::read_to_end(&mut d, &mut back).unwrap();
        assert_eq!(back, page);
    }
}
