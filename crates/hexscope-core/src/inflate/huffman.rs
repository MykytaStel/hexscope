use crate::bits::BitReader;
use crate::inflate::InflateError;

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
        // Walks lengths shortest-first: `first` is the smallest code of this
        // length, `index` the offset of that length's symbols in `symbols`.
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;

        for len in 1..=MAX_BITS {
            code |= br.bits(1).map_err(|_| InflateError::Bits)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }

        Err(InflateError::BadHuffmanCode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
