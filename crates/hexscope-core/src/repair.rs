//! A damaged file's copy with what survived put back in order — the
//! answer to "why won't it open" when the damage is of a kind that has one.
//!
//! - A PNG's chunks are copied with their checksums made right; a chunk
//!   whose length is broken is passed over to the next intact one; a file
//!   cut short keeps the image data that arrived and gets its end marker.
//! - A JPEG cut short gets its end marker, so viewers show what arrived.
//! - A ZIP whose end was lost, or whose directory is damaged, is written
//!   anew from the entries that are whole and check out.
//!
//! Nothing is guessed: what is copied is what the file holds. What was done
//! is listed, in words, so a person can judge the copy.

use crate::crc32::{crc32, crc32_update};
use crate::docs::{Concern, describe};
use crate::document::{Document, parse};
use crate::model::NodeKind;
use crate::png::chunks::{ChunkError, PNG_SIGNATURE, next_chunk, resync};
use crate::reader::Reader;
use crate::zip::write::{Part, assemble, header_of};
use crate::zip::{ZipDocument, extract};

/// Entries decompressed to check them, each at most this large; larger
/// ones are kept when their bytes are all there.
const MAX_CHECK: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repaired {
    pub bytes: Vec<u8>,
    /// What was done, in words.
    pub fixed: Vec<String>,
}

/// Why no repaired copy was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairError {
    /// Nothing in it is damaged.
    NothingToRepair,
    /// Damaged in a way hexscope cannot put right.
    CannotRepair,
    /// Not a format hexscope repairs.
    Unsupported,
}

impl RepairError {
    pub fn reason(self) -> &'static str {
        match self {
            RepairError::NothingToRepair => "nothing in it is damaged",
            RepairError::CannotRepair => "its damage is not of a kind hexscope can put right",
            RepairError::Unsupported => {
                "hexscope repairs PNG and JPEG images and ZIP archives only"
            }
        }
    }
}

pub fn repair(data: &[u8]) -> Result<Repaired, RepairError> {
    let doc = parse(data);
    // Damaged the way the verdict says it: broken, or a rule broken in a
    // way that stops readers.
    let tree = doc.tree();
    let damaged = tree.nodes().iter().any(|n| {
        n.kind == NodeKind::Error
            || (n.kind == NodeKind::Warning
                && describe(tree, n.id, doc.format()).and_then(|d| d.concern)
                    == Some(Concern::Damage))
    });
    if !damaged {
        return Err(RepairError::NothingToRepair);
    }
    let r = match doc {
        Document::Png(_) => repair_png(data),
        Document::Jpeg(j) => repair_jpeg(data, &j.tree),
        Document::Zip(z) => repair_zip(data, &z),
        _ => return Err(RepairError::Unsupported),
    }?;
    if r.fixed.is_empty() {
        return Err(RepairError::CannotRepair);
    }
    Ok(r)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&(!crc32_update(crc32_update(0xFFFF_FFFF, kind), body)).to_be_bytes());
}

fn repair_png(data: &[u8]) -> Result<Repaired, RepairError> {
    // The first chunk is IHDR; where its length starts is where chunks do,
    // whatever became of the signature before it.
    let ihdr = data
        .get(..64.min(data.len()))
        .and_then(|h| h.windows(4).position(|w| w == b"IHDR"));
    let Some(start) = ihdr.and_then(|i| i.checked_sub(4)) else {
        return Err(RepairError::CannotRepair);
    };
    let mut fixed = Vec::new();
    if data.get(..8) != Some(&PNG_SIGNATURE[..]) || start != 8 {
        fixed.push("rewrote the signature that says this is a PNG".to_string());
    }
    let mut out = PNG_SIGNATURE.to_vec();
    let mut r = Reader::new(data);
    r.seek(start as u64);
    let (mut crcs, mut skipped) = (0usize, 0u64);
    let mut ended = false;
    loop {
        let at = r.pos();
        match next_chunk(&mut r) {
            None => break,
            Some(Ok(c)) => {
                if !c.crc_ok() {
                    crcs += 1;
                }
                chunk(&mut out, &c.kind, c.data);
                if &c.kind == b"IEND" {
                    ended = true;
                    break;
                }
            }
            Some(Err(ChunkError::LengthTooLarge)) | Some(Err(ChunkError::Truncated))
                if resync(data, at + 1).is_some() =>
            {
                let next = resync(data, at + 1).unwrap_or(at);
                skipped += next - at;
                r.seek(next);
            }
            Some(Err(_)) => {
                // Cut short: a chunk of image data keeps what arrived of it.
                let mut t = Reader::new(data);
                t.seek(at);
                if let (Ok(_), Ok(kind)) = (t.u32_be(), t.array::<4>())
                    && &kind == b"IDAT"
                {
                    let rest = t.rest();
                    if !rest.is_empty() {
                        chunk(&mut out, b"IDAT", rest);
                        fixed.push(format!(
                            "kept the {} of image data that arrived before the file was cut short",
                            plural(rest.len(), "byte", "bytes")
                        ));
                    }
                }
                break;
            }
        }
    }
    if crcs > 0 {
        fixed.push(if crcs == 1 {
            "corrected a checksum that did not match its chunk".to_string()
        } else {
            format!("corrected {crcs} checksums that did not match their chunks")
        });
    }
    if skipped > 0 {
        fixed.push(format!(
            "left out {} of a broken chunk",
            plural(skipped as usize, "byte", "bytes")
        ));
    }
    if !ended {
        chunk(&mut out, b"IEND", &[]);
        fixed.push("added the end marker the file was missing".to_string());
    }
    Ok(Repaired { bytes: out, fixed })
}

fn repair_jpeg(data: &[u8], tree: &crate::model::ParseTree) -> Result<Repaired, RepairError> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Err(RepairError::CannotRepair);
    }
    let mut fixed = Vec::new();
    let mut out = data.to_vec();
    // The parser's word for it, not a search for FF D9: the thumbnail
    // inside a photo has an end marker of its own.
    if tree
        .nodes()
        .iter()
        .any(|n| n.label.starts_with("no EOI marker"))
    {
        out.extend_from_slice(&[0xFF, 0xD9]);
        fixed.push(
            "added the end marker, so viewers show the part of the picture that arrived"
                .to_string(),
        );
    }
    Ok(Repaired { bytes: out, fixed })
}

fn repair_zip(data: &[u8], doc: &ZipDocument) -> Result<Repaired, RepairError> {
    let mut parts = Vec::new();
    let mut lost = 0usize;
    for e in &doc.entries {
        let whole =
            !e.is_encrypted() && e.data.len == e.compressed && e.data.end() <= data.len() as u64;
        let checks = whole && (e.uncompressed > MAX_CHECK || extract(data, e, MAX_CHECK).is_ok());
        let header = header_of(data, &doc.tree, e);
        let body = data.get(e.data.start as usize..e.data.end() as usize);
        match (checks, header, body) {
            (true, Some((name, time, _)), Some(body)) => parts.push(Part {
                name,
                flags: e.flags,
                method: e.method,
                time,
                crc: e.crc32,
                uncompressed: e.uncompressed,
                body,
            }),
            _ => lost += 1,
        }
    }
    if parts.is_empty() {
        return Err(RepairError::CannotRepair);
    }
    let bytes = assemble(&parts).ok_or(RepairError::CannotRepair)?;
    let mut fixed = vec![format!(
        "wrote the archive anew with the {} that {} whole",
        plural(parts.len(), "file", "files"),
        if parts.len() == 1 { "is" } else { "are" }
    )];
    if lost > 0 {
        fixed.push(format!(
            "left out {} that arrived damaged or incomplete",
            plural(lost, "file", "files")
        ));
    }
    // A plain copy with nothing lost and nothing changed repaired nothing.
    if lost == 0 && crc32(&bytes) == crc32(data) {
        fixed.clear();
    }
    Ok(Repaired { bytes, fixed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::testing::{Archive, Entry, build};

    /// Errors, and warnings that stop readers.
    fn problems(bytes: &[u8]) -> Vec<String> {
        let doc = parse(bytes);
        let tree = doc.tree();
        tree.nodes()
            .iter()
            .filter(|n| {
                n.kind == NodeKind::Error
                    || (n.kind == NodeKind::Warning
                        && describe(tree, n.id, doc.format()).and_then(|d| d.concern)
                            == Some(Concern::Damage))
            })
            .map(|n| n.label.clone())
            .collect()
    }

    fn pngsuite(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/pngsuite/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn a_png_with_a_wrong_checksum_is_put_right() {
        let good = pngsuite("basn2c08.png");
        let mut bad = good.clone();
        bad[29] ^= 0xFF; // IHDR's CRC
        assert!(!problems(&bad).is_empty());
        let r = repair(&bad).unwrap();
        assert_eq!(r.bytes, good);
        assert_eq!(
            r.fixed,
            ["corrected a checksum that did not match its chunk"]
        );
        assert_eq!(repair(&good), Err(RepairError::NothingToRepair));
    }

    #[test]
    fn a_png_cut_short_keeps_its_image_data_and_ends() {
        let good = pngsuite("basn2c08.png");
        let cut = &good[..good.len() - 40];
        let r = repair(cut).unwrap();
        assert!(
            problems(&r.bytes).iter().all(|p| p.contains("decompress")),
            "{:?}",
            problems(&r.bytes)
        );
        assert!(
            r.bytes
                .ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82])
        );
        assert!(
            r.fixed.iter().any(|f| f.starts_with("kept the")),
            "{:?}",
            r.fixed
        );
        assert!(
            r.fixed
                .iter()
                .any(|f| f.starts_with("added the end marker")),
            "{:?}",
            r.fixed
        );
    }

    #[test]
    fn a_jpeg_cut_short_gets_its_end() {
        let jpeg = crate::jpeg::testing::jpeg_with_exif(None);
        let cut = &jpeg[..jpeg.len() - 2];
        // A photo whose thumbnail ends, but whose picture does not.
        let photo = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/photo.jpg"
        ))
        .unwrap();
        let r = repair(&photo[..photo.len() - 3000]).unwrap();
        assert!(r.bytes.ends_with(&[0xFF, 0xD9]));
        assert!(!problems(cut).is_empty());
        let r = repair(cut).unwrap();
        assert_eq!(r.bytes, jpeg);
        assert!(problems(&r.bytes).is_empty());
    }

    #[test]
    fn a_zip_cut_short_keeps_its_whole_files() {
        let b = build(&Archive {
            entries: vec![
                Entry::new("a.txt", b"first file, whole", 8),
                Entry::new("b.txt", &[7u8; 5000], 8),
                Entry::new("c.txt", b"the one that is cut", 0),
            ],
            ..Default::default()
        });
        // Cut inside the last file: no central directory, no end record.
        let cut_at = b.bytes.len() - b.bytes.len() / 5;
        let cut = &b.bytes[..cut_at];
        let r = repair(cut).unwrap();
        let fixed = crate::zip::parse_zip(&r.bytes);
        assert!(problems(&r.bytes).is_empty(), "{:?}", problems(&r.bytes));
        let names: Vec<_> = fixed.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.starts_with(&["a.txt"]), "{names:?}");
        for e in &fixed.entries {
            assert!(extract(&r.bytes, e, 1 << 20).is_ok(), "{}", e.name);
        }
        assert!(
            r.fixed[0].starts_with("wrote the archive anew"),
            "{:?}",
            r.fixed
        );
    }

    #[test]
    fn other_formats_and_hopeless_files_say_so() {
        assert_eq!(repair(b"%PDF-1.4 broken"), Err(RepairError::Unsupported));
        assert!(repair(&[0x89, b'P', b'N', b'G', 0, 0, 0]).is_err());
        for cut in 0..60 {
            let good = pngsuite("basn2c08.png");
            let _ = repair(&good[..cut]);
        }
    }
}
