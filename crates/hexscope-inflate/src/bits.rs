#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    Eof,
    /// More than 32 bits asked for at once; they would not fit the result.
    TooWide,
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

    /// Bits left before end of input.
    pub fn remaining_bits(&self) -> u64 {
        self.data.len() as u64 * 8 - self.bit_pos
    }

    /// Reads the next nine DEFLATE code bits as a most-significant-first
    /// lookup-table index. The stream stores each byte's bits low-first, so
    /// reverse only the requested prefix after loading at most two bytes.
    pub(crate) fn peek_huffman_9(&self) -> Option<usize> {
        if self.remaining_bits() < 9 {
            return None;
        }
        let byte_pos = self.byte_pos();
        let bit_offset = (self.bit_pos % 8) as u32;
        let word = u16::from_le_bytes([self.data[byte_pos], self.data[byte_pos + 1]]);
        let low_first = (word >> bit_offset) & 0x01ff;
        Some((low_first.reverse_bits() >> 7) as usize)
    }

    /// Advances by a Huffman code length after `peek_huffman_9` proved that
    /// at least nine input bits remain.
    pub(crate) fn advance_huffman(&mut self, bits: u8) {
        self.bit_pos += bits as u64;
    }

    pub fn bits(&mut self, n: u32) -> Result<u32, BitError> {
        if n > 32 {
            return Err(BitError::TooWide);
        }
        // Checked up front so a failed read leaves the position untouched —
        // the same contract `Reader` gives, letting a caller recover and try
        // something smaller instead of silently skipping the bits it consumed.
        if self.remaining_bits() < n as u64 {
            return Err(BitError::Eof);
        }
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

    /// Reads whole bytes, starting at the next byte boundary.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], BitError> {
        // The aligned start is computed without committing to it, so a failed
        // read does not consume the alignment padding.
        let start = self.bit_pos.div_ceil(8) as usize;
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
    fn asking_for_more_than_32_bits_is_an_error() {
        let data = [0xFFu8; 8];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(33), Err(BitError::TooWide));
        assert_eq!(br.bit_pos(), 0);
        assert_eq!(br.bits(32), Ok(u32::MAX));
    }

    #[test]
    fn a_failed_read_leaves_the_position_untouched() {
        // One byte, asking for nine bits: the eight readable bits must not be
        // consumed, or a caller retrying with a smaller width silently skips them.
        let data = [0b1010_1010u8];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(9), Err(BitError::Eof));
        assert_eq!(br.bit_pos(), 0, "a failed read must not consume input");
        assert_eq!(br.bits(8), Ok(0b1010_1010));
    }

    #[test]
    fn bytes_reads_whole_bytes_and_reports_eof_without_consuming() {
        let data = [0xAAu8, 0xBB, 0xCC];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(3), Ok(0b010));
        // Skips to the byte boundary, then takes two whole bytes.
        assert_eq!(br.bytes(2), Ok(&data[1..3]));
        assert_eq!(br.bit_pos(), 24);

        let mut br = BitReader::new(&data);
        assert_eq!(br.bytes(9), Err(BitError::Eof));
        assert_eq!(br.bit_pos(), 0, "a failed read must not consume alignment");
    }

    #[test]
    fn align_is_a_no_op_on_a_byte_boundary() {
        let data = [0xFFu8, 0x00];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(8), Ok(0xFF));
        br.align();
        assert_eq!(br.bit_pos(), 8, "already aligned: must not skip a byte");
    }

    #[test]
    fn seek_bits_clamps_past_the_end() {
        let data = [1u8, 2];
        let mut br = BitReader::new(&data);
        br.seek_bits(u64::MAX);
        assert_eq!(br.bit_pos(), 16);
        assert_eq!(br.remaining_bits(), 0);
        assert_eq!(br.bits(1), Err(BitError::Eof));
    }

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
