//! One entry's bytes, decompressed and checked against its CRC-32. Only on
//! demand: opening an archive decompresses nothing.

use super::ZipEntry;
use crate::crc32::crc32;
use crate::inflate::{InflateError, NoTrace, inflate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractError {
    /// A compression method this tool does not decompress.
    Unsupported(u16),
    Encrypted,
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
    if entry.is_encrypted() {
        return Err(ExtractError::Encrypted);
    }
    if entry.data.len != entry.compressed {
        return Err(ExtractError::OutOfRange);
    }
    let start = usize::try_from(entry.data.start).map_err(|_| ExtractError::OutOfRange)?;
    let end = usize::try_from(entry.data.end()).map_err(|_| ExtractError::OutOfRange)?;
    let bytes = data.get(start..end).ok_or(ExtractError::OutOfRange)?;

    extract_compressed(bytes, entry, limit)
}

/// Decompresses one entry from exactly its compressed bytes, read separately
/// from the archive's original file. The central record supplies the expected
/// compressed length, uncompressed length and CRC.
pub fn extract_compressed(
    bytes: &[u8],
    entry: &ZipEntry,
    limit: u64,
) -> Result<Vec<u8>, ExtractError> {
    if entry.is_encrypted() {
        return Err(ExtractError::Encrypted);
    }
    if entry.data.len != entry.compressed || bytes.len() as u64 != entry.compressed {
        return Err(ExtractError::OutOfRange);
    }

    let out = match entry.method {
        0 if bytes.len() as u64 > limit => return Err(ExtractError::TooLarge),
        0 => bytes.to_vec(),
        8 => inflate(bytes, limit, &mut NoTrace).map_err(|e| match e {
            InflateError::OutputTooLarge => ExtractError::TooLarge,
            e => ExtractError::Inflate(e),
        })?,
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
        a.entries[1].method = 12;
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            extract(&b.bytes, &doc.entries[0], u64::MAX),
            Err(ExtractError::Encrypted)
        );
        assert_eq!(
            extract(&b.bytes, &doc.entries[1], u64::MAX),
            Err(ExtractError::Unsupported(12))
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
