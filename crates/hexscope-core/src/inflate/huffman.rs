use crate::bits::BitReader;
use crate::inflate::InflateError;
use crate::inflate::explain::CodeGroup;

pub const MAX_BITS: usize = 15;

/// A canonical Huffman code stored as symbol counts per length plus symbols in
/// canonical order — the decode walks lengths instead of building a tree, which
/// keeps construction cheap and allocation-free per symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Huffman {
    counts: [u16; MAX_BITS + 1],
    symbols: Vec<u16>,
}

impl Huffman {
    pub fn from_lengths(lengths: &[u8]) -> Result<Self, InflateError> {
        let mut counts = [0u16; MAX_BITS + 1];
        for &len in lengths {
            if len as usize > MAX_BITS {
                return Err(InflateError::BadCodeLengths);
            }
            counts[len as usize] += 1;
        }
        counts[0] = 0;

        // Kraft inequality: a code is valid when it is not oversubscribed.
        let mut left = 1i32;
        for &count in &counts[1..=MAX_BITS] {
            left <<= 1;
            left -= count as i32;
            if left < 0 {
                return Err(InflateError::BadCodeLengths);
            }
        }

        // `cursor[len]` is where the next symbol of that length goes, so
        // symbols end up grouped by length in canonical order.
        let mut cursor = [0u16; MAX_BITS + 1];
        for len in 2..=MAX_BITS {
            cursor[len] = cursor[len - 1] + counts[len - 1];
        }

        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &len) in lengths.iter().enumerate() {
            if len != 0 {
                let slot = cursor[len as usize] as usize;
                symbols[slot] = symbol as u16;
                cursor[len as usize] += 1;
            }
        }

        Ok(Self { counts, symbols })
    }

    pub fn decode(&self, br: &mut BitReader) -> Result<u16, InflateError> {
        self.decode_traced(br).map(|(symbol, _, _)| symbol)
    }

    /// Like [`decode`](Self::decode), also returning the code as the loop
    /// assembled it — most significant bit first — and its length.
    pub fn decode_traced(&self, br: &mut BitReader) -> Result<(u16, u16, u8), InflateError> {
        // Walks lengths shortest-first: `first` is the smallest code of this
        // length, `index` the offset of that length's symbols in `symbols`.
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;

        for len in 1..=MAX_BITS {
            code |= br.bits(1).map_err(|_| InflateError::Bits)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                let symbol = self.symbols[(index + (code - first)) as usize];
                return Ok((symbol, code as u16, len as u8));
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }

        Err(InflateError::BadHuffmanCode)
    }

    /// The code grouped by length, each with its first canonical code — the
    /// same ranges `decode` walks, so what is shown is what decoding does.
    pub fn groups(&self) -> Vec<CodeGroup> {
        let mut out = Vec::new();
        let mut code = 0u32;
        let mut index = 0usize;
        for len in 1..=MAX_BITS {
            code = (code + self.counts[len - 1] as u32) << 1;
            let n = self.counts[len] as usize;
            if n > 0 {
                out.push(CodeGroup {
                    len: len as u8,
                    first_code: code as u16,
                    symbols: self.symbols[index..index + n].to_vec(),
                });
            }
            index += n;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::inflate::tables::fixed_literal_lengths;

    /// Packs Huffman codes, each most significant bit first, into DEFLATE's
    /// least-significant-bit-first byte order.
    fn pack(codes: &[(u16, u8)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut n = 0usize;
        for &(code, len) in codes {
            for i in (0..len).rev() {
                if n.is_multiple_of(8) {
                    out.push(0);
                }
                let bit = ((code >> i) & 1) as u8;
                *out.last_mut().unwrap() |= bit << (n % 8);
                n += 1;
            }
        }
        out
    }

    #[test]
    fn groups_reproduce_the_rfc_example() {
        // RFC 1951 §3.2.2: A..H with lengths 3,3,3,3,3,2,4,4 give
        // F=00, A=010, B=011, C=100, D=101, E=110, G=1110, H=1111.
        let h = Huffman::from_lengths(&[3, 3, 3, 3, 3, 2, 4, 4]).unwrap();
        let groups: Vec<_> = h
            .groups()
            .into_iter()
            .map(|g| (g.len, g.first_code, g.symbols))
            .collect();
        assert_eq!(
            groups,
            vec![
                (2, 0b00, vec![5]),
                (3, 0b010, vec![0, 1, 2, 3, 4]),
                (4, 0b1110, vec![6, 7])
            ]
        );
    }

    #[test]
    fn groups_of_the_fixed_literal_table() {
        let h = Huffman::from_lengths(&fixed_literal_lengths()).unwrap();
        let g = h.groups();
        let shape: Vec<_> = g
            .iter()
            .map(|g| (g.len, g.first_code, g.symbols.len()))
            .collect();
        assert_eq!(
            shape,
            vec![
                (7, 0b000_0000, 24),
                (8, 0b0011_0000, 152),
                (9, 0b1_1001_0000, 112)
            ]
        );
        // Within a length, symbols keep their order: 0..=143, then 280..=287.
        assert_eq!(g[1].symbols[0], 0);
        assert_eq!(g[1].symbols[144], 280);
    }

    #[test]
    fn a_traced_code_sits_at_its_symbol_in_its_group() {
        let h = Huffman::from_lengths(&fixed_literal_lengths()).unwrap();
        for g in h.groups() {
            for (k, &symbol) in g.symbols.iter().enumerate() {
                let code = g.first_code + k as u16;
                let data = pack(&[(code, g.len)]);
                let mut br = BitReader::new(&data);
                assert_eq!(h.decode_traced(&mut br), Ok((symbol, code, g.len)));
            }
        }
    }

    #[test]
    fn decodes_a_three_symbol_canonical_code() {
        // Lengths [1, 2, 2] give codes A=0, B=10, C=11 (MSB-first on the wire).
        let h = Huffman::from_lengths(&[1, 2, 2]).unwrap();

        // Bits are emitted LSB-first, so "0" then "10" then "11" is
        // 0, 0,1, 1,1 => 0b11010 = 0x1A in the first byte.
        let data = [0b0001_1010];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Ok(0));
        assert_eq!(h.decode(&mut br), Ok(1));
        assert_eq!(h.decode(&mut br), Ok(2));
    }

    #[test]
    fn ignores_symbols_with_zero_length() {
        let h = Huffman::from_lengths(&[0, 1, 0, 1]).unwrap();
        let data = [0b0000_0010];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Ok(1));
        assert_eq!(h.decode(&mut br), Ok(3));
    }

    #[test]
    fn rejects_an_oversubscribed_code() {
        // Three symbols of length 1 cannot fit in a binary tree.
        assert_eq!(
            Huffman::from_lengths(&[1, 1, 1]),
            Err(InflateError::BadCodeLengths)
        );
    }

    #[test]
    fn reports_an_invalid_code_instead_of_looping() {
        let h = Huffman::from_lengths(&[1, 2, 2]).unwrap();
        let data = [];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Err(InflateError::Bits));
    }
}
