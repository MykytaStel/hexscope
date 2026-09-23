#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    Eof { needed: usize, available: usize },
}

/// A cursor over a byte slice. Every read is bounds-checked and a failed read
/// leaves the position untouched, so a caller can recover and try something
/// smaller.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn pos(&self) -> u64 {
        self.pos as u64
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Clamps to end-of-input rather than failing; subsequent reads report EOF.
    pub fn seek(&mut self, pos: u64) {
        self.pos = usize::try_from(pos)
            .unwrap_or(usize::MAX)
            .min(self.data.len());
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::Eof {
                needed: n,
                available: self.remaining(),
            });
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16_be(&mut self) -> Result<u16, ReadError> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn u32_be(&mut self) -> Result<u32, ReadError> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Little-endian reads, for formats such as TIFF that declare their byte
    /// order per file.
    pub fn u16_le(&mut self) -> Result<u16, ReadError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    pub fn u32_le(&mut self) -> Result<u32, ReadError> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    /// Reads up to the next occurrence of `delim` and consumes the delimiter.
    /// Returns `None` when the delimiter is absent, leaving the position
    /// untouched so the caller can record the damage and move on.
    pub fn bytes_until(&mut self, delim: u8) -> Option<&'a [u8]> {
        let offset = self.data[self.pos..].iter().position(|&b| b == delim)?;
        let out = self.bytes(offset).ok()?;
        // `position` already proved the delimiter is the next byte.
        self.pos += 1;
        Some(out)
    }

    /// Consumes and returns everything left, which may be empty.
    pub fn rest(&mut self) -> &'a [u8] {
        let out = &self.data[self.pos..];
        self.pos = self.data.len();
        out
    }

    /// Reads a fixed-size field as an owned array. Parsers use this for things
    /// like a four-byte chunk type so they never index a slice by hand.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], ReadError> {
        let available = self.remaining();
        let slice = self.bytes(N)?;
        // `bytes` already guaranteed exactly N bytes, so this conversion cannot
        // fail; writing it as a fallible conversion keeps the function total
        // and leaves no panicking path in the crate's read layer.
        slice.try_into().map_err(|_| ReadError::Eof {
            needed: N,
            available,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_big_endian_integers() {
        let data = [0x00, 0x00, 0x07, 0x80, 0x12, 0x34];
        let mut r = Reader::new(&data);
        assert_eq!(r.u32_be(), Ok(1920));
        assert_eq!(r.u16_be(), Ok(0x1234));
        assert_eq!(r.pos(), 6);
    }

    #[test]
    fn reports_eof_instead_of_panicking() {
        let data = [0x01, 0x02];
        let mut r = Reader::new(&data);
        assert_eq!(
            r.u32_be(),
            Err(ReadError::Eof {
                needed: 4,
                available: 2
            })
        );
        // A failed read must not consume anything.
        assert_eq!(r.pos(), 0);
        assert_eq!(r.u8(), Ok(0x01));
    }

    #[test]
    fn seek_past_end_clamps_and_reports_eof() {
        let data = [0x01, 0x02];
        let mut r = Reader::new(&data);
        r.seek(9999);
        assert_eq!(r.remaining(), 0);
        assert_eq!(
            r.u8(),
            Err(ReadError::Eof {
                needed: 1,
                available: 0
            })
        );
    }

    #[test]
    fn bytes_borrows_without_copying() {
        let data = [1, 2, 3, 4, 5];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes(3), Ok(&data[0..3]));
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn array_reads_a_fixed_size_field() {
        let data = *b"IHDRxx";
        let mut r = Reader::new(&data);
        assert_eq!(r.array::<4>(), Ok(*b"IHDR"));
        assert_eq!(r.pos(), 4);
        // Too few bytes left: reports EOF and stays put, like every other read.
        assert_eq!(
            r.array::<4>(),
            Err(ReadError::Eof {
                needed: 4,
                available: 2
            })
        );
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn reads_little_endian_integers() {
        let data = [0x80, 0x07, 0x00, 0x00, 0x34, 0x12];
        let mut r = Reader::new(&data);
        assert_eq!(r.u32_le(), Ok(1920));
        assert_eq!(r.u16_le(), Ok(0x1234));
        assert_eq!(
            r.u16_le(),
            Err(ReadError::Eof {
                needed: 2,
                available: 0
            })
        );
    }

    #[test]
    fn bytes_until_splits_on_the_delimiter() {
        let data = *b"Author\0Ada";
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), Some(&b"Author"[..]));
        // The delimiter itself is consumed.
        assert_eq!(r.pos(), 7);
        assert_eq!(r.rest(), &b"Ada"[..]);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn bytes_until_reports_a_missing_delimiter_without_moving() {
        let data = *b"no-null-here";
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), None);
        assert_eq!(r.pos(), 0, "a failed search must not consume input");
    }

    #[test]
    fn bytes_until_handles_an_empty_leading_field() {
        let data = [0u8, b'x'];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), Some(&[][..]));
        assert_eq!(r.rest(), b"x");
    }

    #[test]
    fn rest_on_exhausted_input_is_empty() {
        let data = [1u8];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes(1), Ok(&data[..]));
        assert_eq!(r.rest(), &[][..]);
    }
}
