use crate::crc32::crc32_update;
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
///
/// **Damage ends the walk.** Every error path seeks to end-of-input before
/// returning, so a caller that keeps looping receives `None` on the next call
/// rather than the same error forever. Without this, a file ending in one to
/// three stray bytes would fail the length read without consuming them — a
/// failed read deliberately leaves the position untouched — and spin any naive
/// loop indefinitely. The no-infinite-loop guarantee belongs here, not in the
/// memory of every future caller.
pub fn next_chunk<'a>(r: &mut Reader<'a>) -> Option<Result<Chunk<'a>, ChunkError>> {
    if r.remaining() == 0 {
        return None;
    }
    let start = r.pos();

    let Ok(len) = r.u32_be() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };
    if len > MAX_CHUNK_LEN {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::LengthTooLarge));
    }

    let Ok(kind) = r.array::<4>() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    let data_start = r.pos();
    let Ok(data) = r.bytes(len as usize) else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    let Ok(declared_crc) = r.u32_be() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    Some(Ok(Chunk {
        range: ByteRange::new(start, r.pos() - start),
        kind,
        data,
        data_range: ByteRange::new(data_start, len as u64),
        declared_crc,
        // The CRC covers type then data; streaming it avoids copying a payload
        // that can be megabytes long.
        actual_crc: !crc32_update(crc32_update(0xFFFF_FFFF, &kind), data),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(next_chunk(&mut r), None, "damage must end the walk");
    }

    #[test]
    fn a_truncated_length_field_ends_the_walk() {
        // Three stray trailing bytes: not even enough for the length field.
        // A failed read leaves the position untouched, so unless the walker
        // consumes them itself a looping caller would spin forever here.
        let bytes = [0u8, 0, 0];
        let mut r = Reader::new(&bytes);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::Truncated)));
        assert_eq!(next_chunk(&mut r), None, "the same error must not repeat");
    }

    #[test]
    fn rejects_an_absurd_declared_length() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        bytes.extend_from_slice(b"IDAT");
        let mut r = Reader::new(&bytes);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::LengthTooLarge)));
        assert_eq!(next_chunk(&mut r), None, "damage must end the walk");
    }
}
