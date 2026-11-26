//! One entry's bytes, decompressed and checked against its CRC-32. Only on
//! demand: opening an archive decompresses nothing.

use super::ZipEntry;
use crate::crc32::crc32;
use crate::inflate::{InflateError, NoTrace, inflate};
use std::borrow::Cow;
#[cfg(any(feature = "zip-bzip2", feature = "zip-zstd"))]
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractError {
    /// A compression method this tool does not decompress.
    Unsupported(u16),
    Encrypted,
    /// The BZIP2 stream is malformed or truncated.
    Bzip2,
    /// The Zstandard stream is malformed, truncated, or exceeds its window limit.
    Zstd,
    /// The entry's data runs past the end of the file.
    OutOfRange,
    Inflate(InflateError),
    Crc {
        stored: u32,
        actual: u32,
    },
    /// The decompressed bytes disagree with the central directory.
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    /// The output would exceed the caller's limit.
    TooLarge,
}

/// Decompresses `entry` from `data`, the whole file it was parsed from.
pub fn extract(data: &[u8], entry: &ZipEntry, limit: u64) -> Result<Vec<u8>, ExtractError> {
    extract_view(data, entry, limit).map(Cow::into_owned)
}

/// Internal read-only consumers can borrow stored entries. All admission,
/// size and CRC checks are identical to the owned extraction API.
pub(super) fn extract_view<'a>(
    data: &'a [u8],
    entry: &ZipEntry,
    limit: u64,
) -> Result<Cow<'a, [u8]>, ExtractError> {
    if entry.is_encrypted() {
        return Err(ExtractError::Encrypted);
    }
    if entry.data.len != entry.compressed {
        return Err(ExtractError::OutOfRange);
    }
    let start = usize::try_from(entry.data.start).map_err(|_| ExtractError::OutOfRange)?;
    let end = entry.data.checked_end().ok_or(ExtractError::OutOfRange)?;
    let end = usize::try_from(end).map_err(|_| ExtractError::OutOfRange)?;
    let bytes = data.get(start..end).ok_or(ExtractError::OutOfRange)?;

    extract_compressed_view(bytes, entry, limit)
}

/// Decompresses one entry from exactly its compressed bytes, read separately
/// from the archive's original file. The central record supplies the expected
/// compressed length, uncompressed length and CRC.
pub fn extract_compressed(
    bytes: &[u8],
    entry: &ZipEntry,
    limit: u64,
) -> Result<Vec<u8>, ExtractError> {
    extract_compressed_view(bytes, entry, limit).map(Cow::into_owned)
}

fn extract_compressed_view<'a>(
    bytes: &'a [u8],
    entry: &ZipEntry,
    limit: u64,
) -> Result<Cow<'a, [u8]>, ExtractError> {
    if entry.is_encrypted() {
        return Err(ExtractError::Encrypted);
    }
    if entry.data.len != entry.compressed || bytes.len() as u64 != entry.compressed {
        return Err(ExtractError::OutOfRange);
    }

    // A claimed output larger than the caller's budget cannot produce a
    // valid result within that budget. Reject it before allocating or decoding.
    // The decoder's own limit and the final size check still protect against
    // understated or otherwise false directory sizes.
    if entry.uncompressed > limit {
        return Err(ExtractError::TooLarge);
    }

    let out = match entry.method {
        0 if bytes.len() as u64 > limit => return Err(ExtractError::TooLarge),
        0 => Cow::Borrowed(bytes),
        8 => Cow::Owned(inflate(bytes, limit, &mut NoTrace).map_err(|e| match e {
            InflateError::OutputTooLarge => ExtractError::TooLarge,
            e => ExtractError::Inflate(e),
        })?),
        #[cfg(feature = "zip-bzip2")]
        12 => Cow::Owned(inflate_bzip2(bytes, limit)?),
        #[cfg(feature = "zip-zstd")]
        93 => Cow::Owned(inflate_zstd(bytes, limit)?),
        m => return Err(ExtractError::Unsupported(m)),
    };
    let actual_size = out.len() as u64;
    if actual_size != entry.uncompressed {
        return Err(ExtractError::SizeMismatch {
            expected: entry.uncompressed,
            actual: actual_size,
        });
    }
    let actual = crc32(&out);
    if actual != entry.crc32 {
        return Err(ExtractError::Crc {
            stored: entry.crc32,
            actual,
        });
    }
    Ok(out)
}

#[cfg(feature = "zip-bzip2")]
fn inflate_bzip2(bytes: &[u8], limit: u64) -> Result<Vec<u8>, ExtractError> {
    let mut decoder = bzip2_rs::DecoderReader::new(bytes);
    let mut out = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let n = decoder.read(&mut chunk).map_err(|_| ExtractError::Bzip2)?;
        if n == 0 {
            break;
        }
        if (out.len() as u64)
            .checked_add(n as u64)
            .is_none_or(|next| next > limit)
        {
            return Err(ExtractError::TooLarge);
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

#[cfg(feature = "zip-zstd")]
fn inflate_zstd(bytes: &[u8], limit: u64) -> Result<Vec<u8>, ExtractError> {
    use ruzstd::decoding::{
        BlockDecodingStrategy, FrameDecoder,
        errors::{FrameDecoderError, ReadFrameHeaderError},
    };
    use ruzstd::io::Read as _;
    use std::io::Cursor;

    if bytes.is_empty() {
        return Err(ExtractError::Zstd);
    }

    let mut source = Cursor::new(bytes);
    let mut decoder = FrameDecoder::new();
    let mut out = Vec::new();
    let mut chunk = [0; 8192];
    let mut saw_frame = false;

    while source.position() < bytes.len() as u64 {
        match decoder.reset(&mut source) {
            Ok(()) => saw_frame = true,
            Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                length,
                ..
            })) => {
                let end = source
                    .position()
                    .checked_add(u64::from(length))
                    .filter(|end| *end <= bytes.len() as u64)
                    .ok_or(ExtractError::Zstd)?;
                source.set_position(end);
                continue;
            }
            Err(_) => return Err(ExtractError::Zstd),
        }

        loop {
            let position_before = source.position();
            let blocks_before = decoder.blocks_decoded();
            if !decoder.is_finished() {
                decoder
                    .decode_blocks(&mut source, BlockDecodingStrategy::UptoBlocks(1))
                    .map_err(|_| ExtractError::Zstd)?;
            }

            let n = decoder.read(&mut chunk).map_err(|_| ExtractError::Zstd)?;
            if n > 0 {
                if (out.len() as u64)
                    .checked_add(n as u64)
                    .is_none_or(|next| next > limit)
                {
                    return Err(ExtractError::TooLarge);
                }
                out.extend_from_slice(&chunk[..n]);
            }

            if decoder.is_finished() && decoder.can_collect() == 0 {
                break;
            }
            if n == 0
                && !decoder.is_finished()
                && source.position() == position_before
                && decoder.blocks_decoded() == blocks_before
            {
                return Err(ExtractError::Zstd);
            }
        }
    }

    if !saw_frame {
        return Err(ExtractError::Zstd);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::parse_zip;
    use crate::zip::testing::{Archive, Descriptor, Entry, build};

    fn archive() -> Archive {
        let mut zip64 = Entry::new("big.bin", &b"0123456789".repeat(300), 8);
        zip64.zip64 = true;
        let mut deferred = Entry::new("later.txt", b"sizes after the data", 8);
        deferred.descriptor = Descriptor::WithSignature;
        Archive {
            entries: vec![
                Entry::new("a.txt", b"stored as it is", 0),
                Entry::new("b.txt", &b"deflated, deflated, deflated ".repeat(20), 8),
                zip64,
                deferred,
                Entry::new("empty/", b"", 0),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn borrowed_and_owned_extraction_share_validation() {
        let a = archive();
        let mut b = build(&a);
        let doc = parse_zip(&b.bytes);
        let stored = &doc.entries[0];
        let compressed = &doc.entries[1];
        let view = extract_view(&b.bytes, stored, u64::MAX).unwrap();
        assert!(matches!(view, Cow::Borrowed(_)));
        assert_eq!(
            view.as_ptr(),
            b.bytes[stored.data.start as usize..].as_ptr()
        );
        assert_eq!(view.as_ref(), a.entries[0].data.as_slice());
        let view = extract_view(&b.bytes, compressed, u64::MAX).unwrap();
        assert!(matches!(view, Cow::Owned(_)));
        assert_eq!(view.as_ref(), a.entries[1].data.as_slice());
        assert_eq!(
            extract_view(&b.bytes, stored, 1),
            Err(ExtractError::TooLarge)
        );
        let mut invalid = stored.clone();
        invalid.uncompressed += 1;
        assert!(matches!(
            extract_view(&b.bytes, &invalid, u64::MAX),
            Err(ExtractError::SizeMismatch { .. })
        ));
        invalid = stored.clone();
        invalid.flags |= 1;
        assert_eq!(
            extract_view(&b.bytes, &invalid, u64::MAX),
            Err(ExtractError::Encrypted)
        );
        b.bytes[stored.data.start as usize] ^= 1;
        assert!(matches!(
            extract_view(&b.bytes, stored, u64::MAX),
            Err(ExtractError::Crc { .. })
        ));
    }

    #[test]
    fn every_entry_extracts_to_what_went_in() {
        let a = archive();
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        assert_eq!(doc.entries.len(), a.entries.len());
        for (e, want) in doc.entries.iter().zip(&a.entries) {
            assert_eq!(
                extract(&b.bytes, e, u64::MAX),
                Ok(want.data.clone()),
                "{}",
                e.name
            );
        }
        assert!(doc.entries[4].is_dir());
    }

    #[test]
    fn extracts_an_entry_from_only_its_compressed_range() {
        let a = archive();
        let b = build(&a);
        let doc = parse_zip(&b.bytes);

        for (entry, want) in doc.entries.iter().zip(&a.entries) {
            let start = entry.data.start as usize;
            let end = entry.data.end() as usize;
            let compressed = &b.bytes[start..end];

            assert_eq!(
                extract_compressed(compressed, entry, u64::MAX),
                Ok(want.data.clone()),
                "{}",
                entry.name
            );
        }
    }

    #[test]
    fn refuses_an_overflowing_source_range() {
        let b = build(&archive());
        let doc = parse_zip(&b.bytes);
        let mut entry = doc.entries[0].clone();
        entry.data.start = u64::MAX;
        assert_eq!(
            extract(&b.bytes, &entry, u64::MAX),
            Err(ExtractError::OutOfRange)
        );
    }

    #[test]
    fn refuses_a_truncated_compressed_range() {
        let a = archive();
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        let entry = &doc.entries[1];
        let start = entry.data.start as usize;
        let end = entry.data.end() as usize;

        assert_eq!(
            extract_compressed(&b.bytes[start..end - 1], entry, u64::MAX),
            Err(ExtractError::OutOfRange)
        );
    }

    #[test]
    fn refuses_what_it_cannot_or_should_not_extract() {
        let mut a = archive();
        a.entries[0].flags = 1;
        a.entries[1].method = 14;
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            extract(&b.bytes, &doc.entries[0], u64::MAX),
            Err(ExtractError::Encrypted)
        );
        assert_eq!(
            extract(&b.bytes, &doc.entries[1], u64::MAX),
            Err(ExtractError::Unsupported(14))
        );
        assert_eq!(
            extract(&b.bytes, &doc.entries[2], 100),
            Err(ExtractError::TooLarge)
        );
    }

    #[test]
    fn damage_is_caught() {
        let a = archive();
        let mut b = build(&a);
        let doc = parse_zip(&b.bytes);
        // A flipped byte in stored data: the CRC no longer matches.
        let at = doc.entries[0].data.start as usize;
        b.bytes[at] ^= 0x20;
        assert!(matches!(
            extract(&b.bytes, &doc.entries[0], u64::MAX),
            Err(ExtractError::Crc { .. })
        ));

        // Data clipped by truncation.
        let cut = &b.bytes[..doc.entries[1].data.start as usize + 3];
        let doc = parse_zip(cut);
        let e = doc.entries.iter().find(|e| e.name == "b.txt").unwrap();
        assert_eq!(extract(cut, e, u64::MAX), Err(ExtractError::OutOfRange));
    }

    #[test]
    fn rejects_over_budget_declared_size_before_decoding() {
        let archive = Archive {
            entries: vec![Entry::new("payload.bin", b"payload", 8)],
            ..Default::default()
        };
        let bytes = build(&archive).bytes;
        let doc = parse_zip(&bytes);
        let mut entry = doc.entries[0].clone();
        entry.uncompressed = 1024;
        // A deliberately invalid DEFLATE block: the budget check must win.
        let invalid = vec![0xFF; entry.compressed as usize];
        assert_eq!(
            extract_compressed(&invalid, &entry, 32),
            Err(ExtractError::TooLarge)
        );
    }

    #[test]
    fn rejects_output_with_a_false_declared_size() {
        let payload = b"declared ZIP size must match extracted bytes";
        for method in [0, 8] {
            let archive = Archive {
                entries: vec![Entry::new("payload.bin", payload, method)],
                ..Default::default()
            };
            let mut bytes = build(&archive).bytes;
            let central = bytes
                .windows(4)
                .position(|window| window == b"PK\x01\x02")
                .expect("the archive has one central record");
            let declared = (payload.len() as u32) - 1;
            bytes[central + 24..central + 28].copy_from_slice(&declared.to_le_bytes());

            let doc = parse_zip(&bytes);
            let entry = doc.entries.first().expect("the local entry is preserved");
            assert_eq!(
                extract(&bytes, entry, u64::MAX),
                Err(ExtractError::SizeMismatch {
                    expected: u64::from(declared),
                    actual: payload.len() as u64,
                }),
                "method {method} must reject output with a false declared size"
            );
        }
    }
}
