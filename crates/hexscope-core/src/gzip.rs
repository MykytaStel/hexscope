//! Single-member gzip framing around the shared, bounded DEFLATE decoder.
//! Kept small so formats can add compressed wrappers without bringing in a
//! second inflater or trusting the size in an untrusted trailer.

use crate::inflate::{Decoder, InflateError};
use crate::model::ByteRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GzipError {
    Header,
    Flags,
    Truncated,
    Deflate(InflateError),
    HeaderChecksum,
    Checksum,
    OutputSize,
    TooLarge,
    TrailingData,
}

pub(crate) struct DecodedGzip {
    pub bytes: Vec<u8>,
    /// The DEFLATE stream's source range inside the gzip member.
    pub deflate_range: ByteRange,
}

fn take<'a>(data: &'a [u8], at: &mut usize, len: usize) -> Result<&'a [u8], GzipError> {
    let end = at.checked_add(len).ok_or(GzipError::Truncated)?;
    let bytes = data.get(*at..end).ok_or(GzipError::Truncated)?;
    *at = end;
    Ok(bytes)
}

fn skip_zero_terminated(data: &[u8], at: &mut usize) -> Result<(), GzipError> {
    let tail = data.get(*at..).ok_or(GzipError::Truncated)?;
    let len = tail
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(GzipError::Truncated)?
        + 1;
    *at = at.checked_add(len).ok_or(GzipError::Truncated)?;
    Ok(())
}

/// Decodes one RFC 1952 member, checking both checksums and the caller's
/// output limit. Concatenated members and trailing data are rejected.
pub(crate) fn decode(data: &[u8], max_output: u64) -> Result<DecodedGzip, GzipError> {
    if data.len() < 10 || data.get(..3) != Some(b"\x1F\x8B\x08") {
        return Err(GzipError::Header);
    }
    let flags = data[3];
    if flags & 0xE0 != 0 {
        return Err(GzipError::Flags);
    }

    let mut at = 10;
    if flags & 0x04 != 0 {
        let xlen = u16::from_le_bytes(
            take(data, &mut at, 2)?
                .try_into()
                .map_err(|_| GzipError::Truncated)?,
        ) as usize;
        take(data, &mut at, xlen)?;
    }
    if flags & 0x08 != 0 {
        skip_zero_terminated(data, &mut at)?;
    }
    if flags & 0x10 != 0 {
        skip_zero_terminated(data, &mut at)?;
    }
    if flags & 0x02 != 0 {
        let expected = u16::from_le_bytes(
            take(data, &mut at, 2)?
                .try_into()
                .map_err(|_| GzipError::Truncated)?,
        );
        if crate::crc32::crc32(&data[..at - 2]) as u16 != expected {
            return Err(GzipError::HeaderChecksum);
        }
    }

    let deflate_start = at;
    let mut decoder = Decoder::new(
        data.get(deflate_start..).ok_or(GzipError::Truncated)?,
        max_output,
    );
    while let Some(step) = decoder.step() {
        step.map_err(|error| match error {
            InflateError::OutputTooLarge => GzipError::TooLarge,
            other => GzipError::Deflate(other),
        })?;
    }
    let deflate_len =
        usize::try_from(decoder.bit_pos().div_ceil(8)).map_err(|_| GzipError::Truncated)?;
    let deflate_end = deflate_start
        .checked_add(deflate_len)
        .ok_or(GzipError::Truncated)?;
    let trailer_end = deflate_end.checked_add(8).ok_or(GzipError::Truncated)?;
    if trailer_end != data.len() {
        return Err(GzipError::TrailingData);
    }
    let trailer = data
        .get(deflate_end..trailer_end)
        .ok_or(GzipError::Truncated)?;
    let output = decoder.into_output();
    let expected_crc =
        u32::from_le_bytes(trailer[..4].try_into().map_err(|_| GzipError::Truncated)?);
    let expected_size =
        u32::from_le_bytes(trailer[4..].try_into().map_err(|_| GzipError::Truncated)?);
    if crate::crc32::crc32(&output) != expected_crc {
        return Err(GzipError::Checksum);
    }
    if output.len() as u32 != expected_size {
        return Err(GzipError::OutputSize);
    }
    Ok(DecodedGzip {
        bytes: output,
        deflate_range: ByteRange::new(deflate_start as u64, deflate_len as u64),
    })
}

/// Writes a deterministic gzip member using the core's fixed-Huffman
/// compressor, then records the standard CRC-32 and input size trailer.
pub(crate) fn encode(data: &[u8]) -> Vec<u8> {
    let zlib = crate::deflate::zlib_compress(data);
    let raw_deflate = &zlib[2..zlib.len() - 4];
    let mut output = Vec::with_capacity(10 + raw_deflate.len() + 8);
    output.extend_from_slice(b"\x1F\x8B\x08\0\0\0\0\0\0\xFF");
    output.extend_from_slice(raw_deflate);
    output.extend_from_slice(&crate::crc32::crc32(data).to_le_bytes());
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn reference(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn reads_and_writes_gzip_members_with_checked_source_ranges() {
        let input = b"an SVG with repeated metadata metadata metadata";
        let encoded = reference(input);
        let decoded = decode(&encoded, 1024).unwrap();
        assert_eq!(decoded.bytes, input);
        assert_eq!(
            &encoded[decoded.deflate_range.start as usize..decoded.deflate_range.end() as usize],
            &encoded[10..encoded.len() - 8]
        );

        let rewritten = encode(input);
        assert_eq!(decode(&rewritten, 1024).unwrap().bytes, input);
    }

    #[test]
    fn reads_optional_filename_and_header_checksum() {
        let mut member = reference(b"optional gzip header");
        let name = b"private-drawing.svgz\0";
        member[3] = 0x0A; // FNAME and FHCRC.
        member.splice(10..10, name.iter().copied());
        let checksum_at = 10 + name.len();
        let header_crc = crate::crc32::crc32(&member[..checksum_at]) as u16;
        member.splice(checksum_at..checksum_at, header_crc.to_le_bytes());

        assert_eq!(
            decode(&member, 1024).unwrap().bytes,
            b"optional gzip header"
        );
        let bad_crc_at = checksum_at;
        member[bad_crc_at] ^= 1;
        assert!(matches!(
            decode(&member, 1024),
            Err(GzipError::HeaderChecksum)
        ));
    }

    #[test]
    fn checks_the_expansion_limit_and_member_integrity() {
        let encoded = reference(&vec![b'x'; 4096]);
        assert!(matches!(decode(&encoded, 32), Err(GzipError::TooLarge)));

        let mut bad_crc = reference(b"hello");
        let last = bad_crc.len() - 8;
        bad_crc[last] ^= 1;
        assert!(matches!(decode(&bad_crc, 32), Err(GzipError::Checksum)));
    }

    #[test]
    fn rejects_reserved_flags_and_trailing_members() {
        let mut reserved = reference(b"hello");
        reserved[3] = 0x20;
        assert!(matches!(decode(&reserved, 32), Err(GzipError::Flags)));

        let mut concatenated = reference(b"hello");
        concatenated.extend_from_slice(&reference(b"again"));
        assert!(matches!(
            decode(&concatenated, 32),
            Err(GzipError::TrailingData)
        ));
    }
}
