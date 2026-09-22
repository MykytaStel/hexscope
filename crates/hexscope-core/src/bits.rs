#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    Eof,
}

/// Reads bits low-to-high within each byte, which is the order DEFLATE uses.
/// The position is tracked in bits so the UI can point at an exact bit offset.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: u64,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    pub fn bit_pos(&self) -> u64 {
        self.bit_pos
    }

    pub fn byte_pos(&self) -> usize {
        (self.bit_pos / 8) as usize
    }

    /// Used to resume decoding from a checkpoint.
    pub fn seek_bits(&mut self, bit_pos: u64) {
        self.bit_pos = bit_pos.min(self.data.len() as u64 * 8);
    }

    pub fn bits(&mut self, n: u32) -> Result<u32, BitError> {
        debug_assert!(n <= 32);
        let mut out = 0u32;
        for i in 0..n {
            let byte = self
                .data
                .get((self.bit_pos / 8) as usize)
                .ok_or(BitError::Eof)?;
            let bit = (byte >> (self.bit_pos % 8)) & 1;
            out |= (bit as u32) << i;
            self.bit_pos += 1;
        }
        Ok(out)
    }

    /// Discards bits up to the next byte boundary (used before stored blocks).
    pub fn align(&mut self) {
        self.bit_pos = self.bit_pos.div_ceil(8) * 8;
    }

    /// Reads whole bytes from the current (aligned) position.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], BitError> {
        self.align();
        let start = self.byte_pos();
        let end = start.checked_add(n).ok_or(BitError::Eof)?;
        if end > self.data.len() {
            return Err(BitError::Eof);
        }
        self.bit_pos = end as u64 * 8;
        Ok(&self.data[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bits_least_significant_first() {
        // 0b1011_0101 — DEFLATE consumes from the low end.
        let data = [0b1011_0101];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(1), Ok(1));
        assert_eq!(br.bits(2), Ok(0b10));
        assert_eq!(br.bits(5), Ok(0b1_0110));
        assert_eq!(br.bit_pos(), 8);
    }

    #[test]
    fn spans_byte_boundaries() {
        let data = [0xFF, 0x01];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(10), Ok(0b01_1111_1111));
    }

    #[test]
    fn zero_bits_is_always_zero() {
        let data = [0xAB];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(0), Ok(0));
        assert_eq!(br.bit_pos(), 0);
    }

    #[test]
    fn align_moves_to_the_next_byte() {
        let data = [0xFF, 0xAA];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(3), Ok(0b111));
        br.align();
        assert_eq!(br.byte_pos(), 1);
        assert_eq!(br.bits(8), Ok(0xAA));
    }

    #[test]
    fn reports_eof_instead_of_panicking() {
        let data = [0x01];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(9), Err(BitError::Eof));
    }
}
