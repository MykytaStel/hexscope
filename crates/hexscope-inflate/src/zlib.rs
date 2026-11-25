use crate::InflateError;
use crate::engine::{Decoder, EventSink, InflateEvent};

const ADLER_MOD: u32 = 65521;

/// Largest run of bytes that can be summed before `b` could overflow a
/// `u32`: 255 * n * (n + 1) / 2 + (n + 1) * (65521 - 1) <= 2^32 - 1.
const NMAX: usize = 5552;

pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    // Reducing once per NMAX bytes instead of twice per byte.
    for block in data.chunks(NMAX) {
        for &byte in block {
            a += byte as u32;
            b += a;
        }
        a %= ADLER_MOD;
        b %= ADLER_MOD;
    }
    (b << 16) | a
}

/// Unwraps a zlib stream (RFC 1950) and inflates its payload. PNG stores IDAT
/// data in exactly this form.
pub fn zlib_decompress(
    data: &[u8],
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<Vec<u8>, InflateError> {
    // Two header bytes, the DEFLATE body, a four-byte Adler-32 trailer.
    let body_len = data
        .len()
        .checked_sub(6)
        .ok_or(InflateError::BadZlibHeader)?;
    let (cmf, flg) = (data[0], data[1]);

    // CM must be DEFLATE, CINFO must describe a window no larger than 32 KiB,
    // and the two header bytes must be a multiple of 31 (RFC 1950).
    if cmf & 0x0F != 8 || cmf >> 4 > 7 || !(((cmf as u16) << 8) | flg as u16).is_multiple_of(31) {
        return Err(InflateError::BadZlibHeader);
    }
    // A preset dictionary is legal zlib but never appears in PNG.
    if flg & 0x20 != 0 {
        return Err(InflateError::BadZlibHeader);
    }

    let body = &data[2..2 + body_len];
    let t = &data[2 + body_len..];
    let stored = u32::from_be_bytes([t[0], t[1], t[2], t[3]]);

    let window = 1u32 << (u32::from(cmf >> 4) + 8);
    let mut decoder = Decoder::new(body, max_output);
    while let Some(step) = decoder.step() {
        let step = step?;
        if let InflateEvent::Match { distance, .. } = step.event
            && u32::from(distance) > window
        {
            return Err(InflateError::BadDistance);
        }
        sink.emit(&step);
    }
    // The final block may end partway through a byte, but complete bytes
    // between that block and the Adler-32 trailer are not part of this stream.
    if decoder.bit_pos().div_ceil(8) != body.len() as u64 {
        return Err(InflateError::BadZlibHeader);
    }
    let out = decoder.into_output();
    if adler32(&out) != stored {
        return Err(InflateError::ChecksumMismatch);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoTrace;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn zlib(input: &[u8]) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn adler_matches_known_vector() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn adler_matches_the_reference_across_block_boundaries() {
        // Longer than NMAX with every byte at its maximum: the case where a
        // deferred modulo would overflow if NMAX were wrong.
        let data = vec![0xFFu8; NMAX * 3 + 17];
        let (mut a, mut b) = (1u64, 0u64);
        for &byte in &data {
            a = (a + byte as u64) % 65521;
            b = (b + a) % 65521;
        }
        assert_eq!(adler32(&data), ((b << 16) | a) as u32);
    }

    #[test]
    fn decompresses_a_zlib_stream() {
        let input = b"hexscope zlib wrapper test, repeated repeated repeated";
        let out = zlib_decompress(&zlib(input), u64::MAX, &mut NoTrace).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn rejects_a_non_deflate_method() {
        // Long enough to get past the length guard, so this really does
        // exercise the CM-nibble check: 0x79 & 0x0F == 9, not 8.
        let bad = [0x79, 0x10, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            zlib_decompress(&bad, u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }

    #[test]
    fn rejects_oversized_window_even_with_valid_header_checksum() {
        let mut stream = zlib(b"payload");
        // CM=8, CINFO=8 (invalid), FCHECK=28, FDICT=0.
        stream[..2].copy_from_slice(&[0x88, 0x1C]);
        assert_eq!(0x881Cu16 % 31, 0);
        assert_eq!(
            zlib_decompress(&stream, u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }

    #[test]
    fn rejects_bytes_between_deflate_and_adler_trailer() {
        let mut stream = zlib(b"payload");
        let trailer = stream.len() - 4;
        stream.insert(trailer, 0);
        assert_eq!(
            zlib_decompress(&stream, u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }

    #[test]
    fn checks_back_references_against_the_declared_window() {
        // A non-final stored block of 257 bytes, followed by a final fixed
        // block containing a length-three, distance-257 match and EOB.
        let mut stream = vec![0x08, 0x1D, 0x00, 0x01, 0x01, 0xFE, 0xFE];
        stream.extend_from_slice(&[b'a'; 257]);
        stream.extend_from_slice(&[0x03, 0x06, 0x00, 0x00]);
        let expected = vec![b'a'; 260];
        stream.extend_from_slice(&adler32(&expected).to_be_bytes());

        // Both headers have valid FCHECK. Only the second window fits.
        assert_eq!(
            zlib_decompress(&stream, 1024, &mut NoTrace),
            Err(InflateError::BadDistance)
        );
        stream[..2].copy_from_slice(&[0x18, 0x19]);
        assert_eq!(zlib_decompress(&stream, 1024, &mut NoTrace), Ok(expected));
    }

    #[test]
    fn rejects_a_bad_header_checksum() {
        // CM is 8 and no preset dictionary, but 0x7800 is not a multiple of 31.
        let bad = [0x78, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            zlib_decompress(&bad, u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }

    #[test]
    fn detects_a_corrupted_checksum() {
        let mut stream = zlib(b"payload");
        let last = stream.len() - 1;
        stream[last] ^= 0xFF;
        assert_eq!(
            zlib_decompress(&stream, u64::MAX, &mut NoTrace),
            Err(InflateError::ChecksumMismatch)
        );
    }

    #[test]
    fn rejects_a_stream_too_short_for_a_header() {
        assert_eq!(
            zlib_decompress(&[0x78], u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }
}
