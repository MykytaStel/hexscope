use crate::crc32::crc32;
use crate::model::ByteRange;
use crate::reader::Reader;

pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The PNG spec caps chunk length at 2^31 - 1.
const MAX_CHUNK_LEN: u32 = 0x7FFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkError {
    /// The file ended inside this chunk.
    Truncated,
    /// The declared length exceeds what the format allows.
    LengthTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    /// The whole chunk: length + type + data + CRC.
    pub range: ByteRange,
    pub kind: [u8; 4],
    pub data: &'a [u8],
    pub data_range: ByteRange,
    pub declared_crc: u32,
    pub actual_crc: u32,
}

impl Chunk<'_> {
    pub fn kind_str(&self) -> String {
        self.kind.iter().map(|&b| b as char).collect()
    }

    pub fn crc_ok(&self) -> bool {
        self.declared_crc == self.actual_crc
    }
}

/// Reads the next chunk. Returns `None` at a clean end of input, and
/// `Some(Err(..))` when the file is damaged — in both cases the caller keeps
/// control and decides what to record.
pub fn next_chunk<'a>(r: &mut Reader<'a>) -> Option<Result<Chunk<'a>, ChunkError>> {
    if r.remaining() == 0 {
        return None;
    }
    let start = r.pos();

    let len = match r.u32_be() {
        Ok(v) => v,
        Err(_) => return Some(Err(ChunkError::Truncated)),
    };
    if len > MAX_CHUNK_LEN {
        return Some(Err(ChunkError::LengthTooLarge));
    }

    let kind_bytes = match r.bytes(4) {
        Ok(b) => b,
        Err(_) => return Some(Err(ChunkError::Truncated)),
    };
    let kind = [kind_bytes[0], kind_bytes[1], kind_bytes[2], kind_bytes[3]];

    let data_start = r.pos();
    let data = match r.bytes(len as usize) {
        Ok(d) => d,
        Err(_) => return Some(Err(ChunkError::Truncated)),
    };

    let declared_crc = match r.u32_be() {
        Ok(v) => v,
        Err(_) => return Some(Err(ChunkError::Truncated)),
    };

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(&kind);
    crc_input.extend_from_slice(data);

    Some(Ok(Chunk {
        range: ByteRange::new(start, r.pos() - start),
        kind,
        data,
        data_range: ByteRange::new(data_start, len as u64),
        declared_crc,
        actual_crc: crc32(&crc_input),
    }))
}

#[cfg(test)]
mod tests {
    use super::{next_chunk, ChunkError};
    use crate::crc32::crc32;
    use crate::reader::Reader;

    /// Builds a well-formed chunk: length, type, data, CRC over type+data.
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_input = kind.to_vec();
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        out
    }

    #[test]
    fn walks_a_valid_chunk() {
        let bytes = chunk(b"IHDR", &[1, 2, 3]);
        let mut r = Reader::new(&bytes);
        let c = next_chunk(&mut r).unwrap().unwrap();

        assert_eq!(c.kind_str(), "IHDR");
        assert_eq!(c.data, &[1, 2, 3]);
        assert!(c.crc_ok());
        assert_eq!(c.range.start, 0);
        assert_eq!(c.range.len, 15); // 4 + 4 + 3 + 4
        assert_eq!(c.data_range.start, 8);
        assert_eq!(c.data_range.len, 3);
        assert!(next_chunk(&mut r).is_none());
    }

    #[test]
    fn detects_a_bad_crc_without_failing() {
        let mut bytes = chunk(b"tEXt", b"hi");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let mut r = Reader::new(&bytes);
        let c = next_chunk(&mut r).unwrap().unwrap();

        assert_eq!(c.kind_str(), "tEXt");
        assert!(!c.crc_ok());
        assert_eq!(c.data, b"hi");
    }

    #[test]
    fn reports_truncation_instead_of_panicking() {
        let full = chunk(b"IDAT", &[9; 20]);
        let mut r = Reader::new(&full[..12]);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::Truncated)));
    }

    #[test]
    fn rejects_an_absurd_declared_length() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        bytes.extend_from_slice(b"IDAT");
        let mut r = Reader::new(&bytes);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::LengthTooLarge)));
    }
}
