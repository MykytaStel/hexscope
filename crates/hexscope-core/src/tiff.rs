//! TIFF: the format scanners and cameras save pictures in, and the one EXIF
//! itself is made of. A TIFF file is a header and a chain of IFDs, tables
//! of tags, with the picture in strips or tiles they point at (TIFF 6.0,
//! Adobe). The reader of a JPEG's EXIF reads it whole, so where it was
//! taken, the camera, its serial number, the owner and the time come out
//! the same way.
//!
//! The clean copy changes bytes in place and keeps every length: the
//! picture's strips are found by absolute offsets, so nothing may move. The
//! tags that say who, where and with what are emptied where they lie, and
//! the EXIF and GPS tables are emptied of every entry.

use crate::clean::{CleanError, Cleaned, Removed};
use crate::exif::{PhotoFacts, parse_tiff};
use crate::model::{ByteRange, NodeKind, ParseTree};

#[derive(Debug)]
pub struct TiffDocument {
    pub tree: ParseTree,
    pub facts: PhotoFacts,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// `II*\0` or `MM\0*`: a classic TIFF, little- or big-endian.
pub fn is_tiff(data: &[u8]) -> bool {
    data.starts_with(b"II\x2A\x00") || data.starts_with(b"MM\x00\x2A")
}

pub fn parse_tiff_file(data: &[u8]) -> TiffDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "TIFF",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let facts = parse_tiff(&mut tree, root, data, 0);
    let ifd0 = Table::new(data).and_then(|t| t.first().map(|at| (t, at)));
    let (width, height) = ifd0.map_or((None, None), |(t, at)| {
        (t.number(at, 0x0100), t.number(at, 0x0101))
    });
    TiffDocument {
        tree,
        facts,
        width,
        height,
    }
}

/// Tags of an image's own IFD that say who, where and with what: emptied
/// in the copy. The picture's own tags — size, strips, colours, Orientation
/// — are left as they are.
const SAYS: [(u16, &str); 14] = [
    (0x010D, "DocumentName"),
    (0x010E, "ImageDescription"),
    (0x010F, "Make"),
    (0x0110, "Model"),
    (0x0131, "Software"),
    (0x0132, "DateTime"),
    (0x013B, "Artist"),
    (0x013C, "HostComputer"),
    (0x02BC, "XMP"),
    (0x8298, "Copyright"),
    (0x83BB, "IPTC"),
    (0x8649, "Photoshop"),
    (0x9C9B, "Windows title"),
    (0xC634, "DNG private data"),
];
/// Windows' own tags (XPComment, XPAuthor, XPKeywords, XPSubject), by range.
const WINDOWS: std::ops::RangeInclusive<u16> = 0x9C9C..=0x9C9F;
/// Tables pointed at from an image's IFD that are about the photo, not the
/// picture: EXIF and GPS, emptied of every entry.
const TABLES: [u16; 2] = [0x8769, 0x8825];
/// SubIFDs: a DNG's full-size and preview images, each an IFD of its own.
const SUB_IFDS: u16 = 0x014A;
/// IFDs visited at most: pages, SubIFDs and the tables they point at.
const MAX_IFDS: usize = 64;

/// A TIFF's tables, read for cleaning: offsets are from the file's start.
struct Table<'a> {
    data: &'a [u8],
    big: bool,
}

/// One 12-byte entry: its tag, and where its value lies.
struct Entry {
    tag: u16,
    /// Value bytes: inline in the entry, or elsewhere in the file.
    value: (usize, usize),
}

impl<'a> Table<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        is_tiff(data).then(|| Table {
            data,
            big: data[0] == b'M',
        })
    }

    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.data.get(at..at + 2)?;
        Some(if self.big {
            u16::from_be_bytes([b[0], b[1]])
        } else {
            u16::from_le_bytes([b[0], b[1]])
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.data.get(at..at + 4)?;
        Some(if self.big {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        })
    }

    fn first(&self) -> Option<usize> {
        self.u32(4).map(|o| o as usize).filter(|&o| o >= 8)
    }

    /// The entries of the IFD at `at`, and where the IFD ends (after its
    /// next-IFD offset). None when the IFD runs past the file.
    fn entries(&self, at: usize) -> Option<(Vec<Entry>, usize)> {
        let count = self.u16(at)? as usize;
        let end = at.checked_add(2 + 12 * count + 4)?;
        if end > self.data.len() {
            return None;
        }
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let e = at + 2 + 12 * i;
            let (tag, typ, n) = (self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)? as usize);
            let size = match typ {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 | 13 => 4,
                5 | 10 | 12 => 8,
                _ => 0,
            } * n;
            let value = if size <= 4 {
                (e + 8, size)
            } else {
                (self.u32(e + 8)? as usize, size)
            };
            out.push(Entry { tag, value });
        }
        Some((out, end))
    }

    /// A one-value SHORT or LONG tag of the IFD at `at`.
    fn number(&self, at: usize, tag: u16) -> Option<u32> {
        let (entries, _) = self.entries(at)?;
        let e = entries.iter().find(|e| e.tag == tag)?;
        match e.value.1 {
            2 => self.u16(e.value.0).map(u32::from),
            4 => self.u32(e.value.0),
            _ => None,
        }
    }

    /// Offsets an entry's value holds: a pointer to an IFD, or several.
    fn offsets(&self, e: &Entry) -> Vec<usize> {
        (0..e.value.1 / 4)
            .filter_map(|i| self.u32(e.value.0 + 4 * i).map(|o| o as usize))
            .collect()
    }
}

/// Zeroes `len` bytes at `at` in `out`, if they lie in the file; how many.
fn blank(out: &mut [u8], at: usize, len: usize) -> u64 {
    match at.checked_add(len).and_then(|end| out.get_mut(at..end)) {
        Some(room) => {
            room.fill(0);
            len as u64
        }
        None => 0,
    }
}

/// The copy without who, where and with what: see the module's comment.
pub(crate) fn clean_tiff(data: &[u8], doc: &TiffDocument) -> Result<Cleaned, CleanError> {
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let t = Table::new(data).ok_or(CleanError::Damaged)?;
    let mut out = data.to_vec();
    let (mut said, mut tables) = (0u64, 0u64);
    let mut images: Vec<usize> = t.first().into_iter().collect();
    let mut seen: Vec<usize> = Vec::new();
    while let Some(at) = images.pop() {
        if seen.contains(&at) || seen.len() >= MAX_IFDS {
            continue;
        }
        seen.push(at);
        let Some((entries, end)) = t.entries(at) else {
            continue;
        };
        for e in &entries {
            if SAYS.iter().any(|(tag, _)| *tag == e.tag) || WINDOWS.contains(&e.tag) {
                said += blank(&mut out, e.value.0, e.value.1);
            } else if TABLES.contains(&e.tag) {
                for table in t.offsets(e) {
                    tables += empty_table(&t, &mut out, table);
                }
            } else if e.tag == SUB_IFDS {
                images.extend(t.offsets(e));
            }
        }
        // The next page, if there is one.
        if let Some(next) = t.u32(end - 4).map(|o| o as usize).filter(|&o| o >= 8) {
            images.push(next);
        }
    }
    let mut removed = Vec::new();
    if tables > 0 {
        removed.push(Removed {
            what: "EXIF and GPS: camera, time, location, serial numbers".into(),
            bytes: tables,
        });
    }
    if said > 0 {
        removed.push(Removed {
            what: "the camera, the software, the owner, the date and notes the picture carried"
                .into(),
            bytes: said,
        });
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: doc.facts.orientation.filter(|&o| (2..=8).contains(&o)),
    })
}

/// Empties the table at `at` in `out`: every value it points at zeroed,
/// then its entries, leaving a table of none. The bytes it took.
fn empty_table(t: &Table, out: &mut [u8], at: usize) -> u64 {
    let Some((entries, end)) = t.entries(at) else {
        return 0;
    };
    let mut bytes = 0;
    for e in entries.iter().filter(|e| e.value.1 > 4) {
        bytes += blank(out, e.value.0, e.value.1);
    }
    // A count of nothing, then zeroes where the entries and the next offset were.
    bytes + blank(out, at, end - at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::summarize;

    fn fixture() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/photo.tif"
        ))
        .unwrap()
    }

    #[test]
    fn a_tiff_says_what_its_exif_says() {
        let data = fixture();
        let doc = parse_tiff_file(&data);
        assert_eq!((doc.width, doc.height), (Some(80), Some(60)));
        let s = summarize(&data);
        assert_eq!(s.format, "tiff");
        assert!(s.complete, "{:?}", s.problems);
        for kind in ["camera", "lens", "serial", "owner", "taken", "software"] {
            assert!(
                s.facts.iter().any(|(k, _)| *k == kind),
                "{kind}: {:?}",
                s.facts
            );
        }
    }

    #[test]
    fn its_copy_keeps_the_picture_and_every_offset() {
        let data = fixture();
        let copy = clean_tiff(&data, &parse_tiff_file(&data)).unwrap();
        assert_eq!(copy.bytes.len(), data.len());
        let after = summarize(&copy.bytes);
        assert!(after.complete, "{:?}", after.problems);
        assert!(after.facts.is_empty(), "{:?}", after.facts);
        // The picture's own tags read the same.
        let (a, b) = (parse_tiff_file(&data), parse_tiff_file(&copy.bytes));
        assert_eq!((a.width, a.height), (b.width, b.height));
        // Nothing of what it said is left anywhere in the bytes.
        for said in [&b"HX-000042"[..], b"Sample Owner", b"Sample Lens"] {
            assert!(!copy.bytes.windows(said.len()).any(|w| w == said));
        }
        assert!(matches!(
            clean_tiff(&copy.bytes, &parse_tiff_file(&copy.bytes)),
            Err(CleanError::NothingToRemove) | Ok(_)
        ));
    }

    #[test]
    fn anything_that_starts_like_a_tiff_is_survivable() {
        for len in 0..64 {
            let mut data = b"MM\x00\x2A\x00\x00\x00\x08".to_vec();
            data.resize(len.max(8), 0xFF);
            let doc = parse_tiff_file(&data);
            let _ = clean_tiff(&data, &doc);
        }
    }
}
