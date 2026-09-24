//! A copy of a file without what it reveals about the people behind it.
//!
//! Nothing is re-encoded. A JPEG loses whole segments and keeps its image
//! data byte for byte. An Office document gets empty property files, and
//! every other entry is copied as it was stored. Cutting at the positions the
//! parser found means the copy differs from the original only where it must.

use crate::crc32::crc32;
use crate::document::{Document, parse};
use crate::jpeg::JpegDocument;
use crate::model::{ByteRange, NodeKind};
use crate::zip::ZipDocument;

/// The copy, and what was taken out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
    pub bytes: Vec<u8>,
    pub removed: Vec<Removed>,
    /// The orientation written back into a minimal EXIF block, so the
    /// picture stays upright.
    pub orientation_kept: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub what: String,
    pub bytes: u64,
}

/// Why no copy was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanError {
    /// The file reveals nothing hexscope knows how to remove.
    NothingToRemove,
    /// An archive entry is encrypted; rewriting its header could make it
    /// impossible to decrypt.
    Encrypted,
    /// The archive needs ZIP64, which this writer does not produce.
    Zip64,
    /// Parts of the file could not be read, so a copy could lose more.
    Damaged,
    /// Not a format this can clean.
    Unsupported,
}

impl CleanError {
    pub fn reason(self) -> &'static str {
        match self {
            CleanError::NothingToRemove => {
                "there is nothing in it that hexscope knows how to remove"
            }
            CleanError::Encrypted => {
                "it holds encrypted files, which a rewrite could make unreadable"
            }
            CleanError::Zip64 => "it is too large an archive for hexscope to rewrite",
            CleanError::Damaged => "parts of it are damaged, and a copy could lose more",
            CleanError::Unsupported => "hexscope cleans JPEG photos and Office documents only",
        }
    }
}

pub fn clean(data: &[u8]) -> Result<Cleaned, CleanError> {
    match parse(data) {
        Document::Jpeg(doc) => clean_jpeg(data, &doc),
        Document::Zip(doc) => clean_zip(data, &doc),
        _ => Err(CleanError::Unsupported),
    }
}

// --- JPEG ------------------------------------------------------------------

/// What a removed segment held, in words, from its label.
fn jpeg_what(label: &str) -> String {
    let kind = label.split(" · ").nth(1).unwrap_or("");
    match (label, kind) {
        (_, "EXIF") => "EXIF: camera, time, location, serial numbers, thumbnail".into(),
        (_, "XMP") => "XMP: editing history and author".into(),
        (_, "Photoshop") => "Photoshop and IPTC: captions, keywords, credits".into(),
        (_, "MPF") => "an index of extra pictures stored in the file".into(),
        (_, "JFXX") => "a JFIF thumbnail".into(),
        ("COM", _) => "a comment".into(),
        ("data after the end of the image", _) => "data after the end of the picture".into(),
        _ => format!("{label}: application data"),
    }
}

/// Segments that say how to show the picture, not who took it.
fn jpeg_keeps(label: &str) -> bool {
    !(label.starts_with("APP") || label == "COM" || label == "data after the end of the image")
        || label == "APP0 · JFIF"
        || label.ends_with("· ICC")
        || label.ends_with("· Adobe")
}

fn clean_jpeg(data: &[u8], doc: &JpegDocument) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let root = tree.root().ok_or(CleanError::Damaged)?;
    let top = &tree.get(root).children;
    if top.first().map(|&c| tree.get(c).label.as_str()) != Some("SOI") {
        return Err(CleanError::Damaged);
    }

    let mut cut: Vec<ByteRange> = Vec::new();
    let mut removed = Vec::new();
    for &id in top {
        let n = tree.get(id);
        if n.range.len > 0 && !jpeg_keeps(&n.label) {
            cut.push(n.range);
            removed.push(Removed {
                what: jpeg_what(&n.label),
                bytes: n.range.len,
            });
        }
    }
    if cut.is_empty() {
        return Err(CleanError::NothingToRemove);
    }

    let orientation = doc.facts.orientation.filter(|&o| (2..=8).contains(&o));
    // After APP0 JFIF when there is one, as JFIF requires, else after SOI.
    let insert_at = top
        .get(1)
        .map(|&c| tree.get(c))
        .filter(|n| n.label == "APP0 · JFIF")
        .map_or(2, |n| n.range.end());

    let mut out = Vec::with_capacity(data.len());
    let mut at = 0u64;
    let mut inserted = orientation.is_none();
    for r in &cut {
        copy(
            &mut out,
            data,
            at,
            r.start,
            insert_at,
            &mut inserted,
            orientation,
        );
        at = r.end();
    }
    copy(
        &mut out,
        data,
        at,
        data.len() as u64,
        insert_at,
        &mut inserted,
        orientation,
    );

    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: orientation,
    })
}

/// Copies `data[from..to]`, writing the orientation block when the copy
/// passes `insert_at`.
fn copy(
    out: &mut Vec<u8>,
    data: &[u8],
    from: u64,
    to: u64,
    insert_at: u64,
    inserted: &mut bool,
    orientation: Option<u16>,
) {
    // `get`, not indexing: ranges come from the parser, which clips them to
    // the file, but a copy must never be the thing that panics.
    let bytes = |a: usize, b: usize| data.get(a..b).unwrap_or(&[]);
    let (from, to, at) = (from.min(to) as usize, to as usize, insert_at as usize);
    if !*inserted && (from..=to).contains(&at) {
        out.extend_from_slice(bytes(from, at));
        out.extend(orientation_segment(orientation.unwrap_or(1)));
        out.extend_from_slice(bytes(at, to));
        *inserted = true;
    } else {
        out.extend_from_slice(bytes(from, to));
    }
}

/// An APP1 EXIF segment holding one tag, Orientation, and nothing else.
fn orientation_segment(orientation: u16) -> Vec<u8> {
    let mut s = vec![0xFF, 0xE1, 0x00, 0x22];
    s.extend_from_slice(b"Exif\0\0");
    s.extend_from_slice(b"MM\x00\x2A\x00\x00\x00\x08");
    s.extend_from_slice(&[0x00, 0x01]); // one entry
    s.extend_from_slice(&[0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01]); // Orientation, SHORT, 1
    s.extend_from_slice(&orientation.to_be_bytes());
    s.extend_from_slice(&[0x00, 0x00]);
    s.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // no next IFD
    s
}

// --- Office documents --------------------------------------------------------

const EMPTY_CORE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"/>"#;
const EMPTY_APP: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#;
const EMPTY_CUSTOM: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#;

fn replacement(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "docProps/core.xml" => Some((
            EMPTY_CORE,
            "document properties: title, author, last editor, dates",
        )),
        "docProps/app.xml" => Some((
            EMPTY_APP,
            "application properties: company, application, template, editing time",
        )),
        "docProps/custom.xml" => Some((EMPTY_CUSTOM, "custom properties")),
        _ => None,
    }
}

fn le16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn clean_zip(data: &[u8], doc: &ZipDocument) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    if !doc.entries.iter().any(|e| replacement(&e.name).is_some()) {
        return Err(CleanError::NothingToRemove);
    }
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.entries.iter().any(|e| e.is_encrypted()) {
        return Err(CleanError::Encrypted);
    }
    let limit = u32::MAX as u64;
    if doc.entries.len() >= 0xFFFF
        || doc
            .entries
            .iter()
            .any(|e| e.compressed >= limit || e.uncompressed >= limit)
    {
        return Err(CleanError::Zip64);
    }

    struct Record {
        flags: u16,
        method: u16,
        time: [u8; 4],
        crc: u32,
        comp: u32,
        uncomp: u32,
        name: Vec<u8>,
        offset: u32,
    }

    let mut out = Vec::with_capacity(data.len());
    let mut records = Vec::new();
    let mut removed = Vec::new();
    let mut extras = 0u64;
    for e in &doc.entries {
        if e.data.len != e.compressed {
            return Err(CleanError::Damaged);
        }
        // The original local header: its times and its exact name bytes.
        let at = tree.get(e.node).range.start as usize;
        let header = data.get(at..at + 30).ok_or(CleanError::Damaged)?;
        let name_len = u16::from_le_bytes([header[26], header[27]]) as usize;
        extras += u16::from_le_bytes([header[28], header[29]]) as u64;
        let name = data
            .get(at + 30..at + 30 + name_len)
            .ok_or(CleanError::Damaged)?
            .to_vec();
        let time = [header[10], header[11], header[12], header[13]];

        let original = data
            .get(e.data.start as usize..e.data.end() as usize)
            .ok_or(CleanError::Damaged)?;
        let (method, body, crc, uncomp): (u16, &[u8], u32, u64) = match replacement(&e.name) {
            Some((xml, what)) => {
                removed.push(Removed {
                    what: format!("{}: {what}", e.name),
                    bytes: e.uncompressed,
                });
                (0, xml.as_bytes(), crc32(xml.as_bytes()), xml.len() as u64)
            }
            None => (e.method, original, e.crc32, e.uncompressed),
        };
        let offset = u32::try_from(out.len()).map_err(|_| CleanError::Zip64)?;
        let r = Record {
            // No data descriptor: the sizes are known and go in the header.
            flags: e.flags & !(1 << 3),
            method,
            time,
            crc,
            comp: body.len() as u32,
            uncomp: uncomp as u32,
            name,
            offset,
        };
        le32(&mut out, 0x0403_4B50);
        le16(&mut out, 20);
        le16(&mut out, r.flags);
        le16(&mut out, r.method);
        out.extend_from_slice(&r.time);
        le32(&mut out, r.crc);
        le32(&mut out, r.comp);
        le32(&mut out, r.uncomp);
        le16(&mut out, r.name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(&r.name);
        out.extend_from_slice(body);
        records.push(r);
    }

    let cd_start = u32::try_from(out.len()).map_err(|_| CleanError::Zip64)?;
    for r in &records {
        le32(&mut out, 0x0201_4B50);
        le16(&mut out, 20);
        le16(&mut out, 20);
        le16(&mut out, r.flags);
        le16(&mut out, r.method);
        out.extend_from_slice(&r.time);
        le32(&mut out, r.crc);
        le32(&mut out, r.comp);
        le32(&mut out, r.uncomp);
        le16(&mut out, r.name.len() as u16);
        le16(&mut out, 0); // extra
        le16(&mut out, 0); // comment
        le16(&mut out, 0); // disk
        le16(&mut out, 0); // internal attributes
        le32(&mut out, 0); // external attributes
        le32(&mut out, r.offset);
        out.extend_from_slice(&r.name);
    }
    let cd_size = u32::try_from(out.len()).map_err(|_| CleanError::Zip64)? - cd_start;
    le32(&mut out, 0x0605_4B50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, records.len() as u16);
    le16(&mut out, records.len() as u16);
    le32(&mut out, cd_size);
    le32(&mut out, cd_start);
    le16(&mut out, 0);

    if extras > 0 {
        removed.push(Removed {
            what: "extra fields: file owners and precise times".into(),
            bytes: extras,
        });
    }
    let hidden: u64 = tree
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Warning && n.parent == tree.root())
        .filter(|n| {
            n.label.starts_with("data before")
                || n.label.ends_with("nothing points at")
                || n.label.ends_with("after the end of the archive")
        })
        .map(|n| n.range.len)
        .sum();
    if hidden > 0 {
        removed.push(Removed {
            what: "data hidden outside the files".into(),
            bytes: hidden,
        });
    }

    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::ByteOrder;
    use crate::exif::testing::{Spec, V, build};
    use crate::jpeg::parse_jpeg;
    use crate::jpeg::testing::jpeg_with_exif;
    use crate::zip::{extract, parse_zip};

    fn scan(bytes: &[u8]) -> Vec<u8> {
        let doc = parse_jpeg(bytes);
        let n = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.label == "scan data")
            .unwrap();
        bytes[n.range.start as usize..n.range.end() as usize].to_vec()
    }

    fn problems(tree: &crate::model::ParseTree) -> Vec<String> {
        tree.nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .map(|n| n.label.clone())
            .collect()
    }

    fn revealing(orientation: u16) -> Vec<u8> {
        let mut s = Spec::new(ByteOrder::Little);
        s.ifd0 = vec![
            (0x010F, V::Ascii("Canon")),
            (0x0110, V::Ascii("Canon EOS R5")),
            (0x0112, V::Short(vec![orientation])),
        ];
        s.exif = vec![(0xA431, V::Ascii("HX-000042")), (0xA430, V::Ascii("Olena"))];
        s.gps = vec![
            (0x01, V::Ascii("N")),
            (0x02, V::Rational(vec![(48, 1), (51, 1), (30, 1)])),
            (0x03, V::Ascii("E")),
            (0x04, V::Rational(vec![(2, 1), (17, 1), (40, 1)])),
        ];
        let mut jpeg = jpeg_with_exif(Some(&build(s)));
        jpeg.extend_from_slice(b"a second picture hidden after the end");
        jpeg
    }

    #[test]
    fn a_photo_loses_what_it_reveals_and_keeps_its_picture() {
        let original = revealing(1);
        let cleaned = clean(&original).unwrap();
        let doc = parse_jpeg(&cleaned.bytes);
        assert_eq!(doc.facts, Default::default(), "nothing left to reveal");
        assert_eq!(problems(&doc.tree), Vec::<String>::new());
        assert_eq!(
            scan(&cleaned.bytes),
            scan(&original),
            "picture copied byte for byte"
        );
        assert_eq!(cleaned.orientation_kept, None);
        let what: Vec<_> = cleaned.removed.iter().map(|r| r.what.as_str()).collect();
        assert!(what[0].starts_with("EXIF"), "{what:?}");
        assert!(
            what.contains(&"data after the end of the picture"),
            "{what:?}"
        );
    }

    #[test]
    fn a_turned_photo_stays_upright() {
        let cleaned = clean(&revealing(6)).unwrap();
        assert_eq!(cleaned.orientation_kept, Some(6));
        let doc = parse_jpeg(&cleaned.bytes);
        assert_eq!(doc.facts.orientation, Some(6));
        assert_eq!(doc.facts.camera, None);
        assert_eq!(doc.facts.location, None);
        assert_eq!(problems(&doc.tree), Vec::<String>::new());
        let tags: Vec<_> = doc
            .tree
            .nodes()
            .iter()
            .filter(|n| n.parent.is_some_and(|p| doc.tree.get(p).label == "IFD0"))
            .map(|n| n.label.clone())
            .collect();
        assert_eq!(tags, ["entryCount", "Orientation", "nextIFDOffset"]);
    }

    #[test]
    fn a_photo_with_nothing_to_hide_is_left_alone() {
        assert_eq!(
            clean(&jpeg_with_exif(None)),
            Err(CleanError::NothingToRemove)
        );
        assert_eq!(
            clean(b"not a file hexscope cleans"),
            Err(CleanError::Unsupported)
        );
    }

    #[test]
    fn a_document_loses_its_properties_and_keeps_its_content() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/report.docx");
        let original = std::fs::read(path).unwrap();
        let cleaned = clean(&original).unwrap();
        let before = parse_zip(&original);
        let after = parse_zip(&cleaned.bytes);
        assert!(after.facts.is_empty(), "{:?}", after.facts);
        assert_eq!(problems(&after.tree), Vec::<String>::new());
        assert_eq!(after.entries.len(), before.entries.len());
        for (a, b) in before.entries.iter().zip(&after.entries) {
            assert_eq!(a.name, b.name);
            if replacement(&a.name).is_none() {
                assert_eq!(
                    extract(&original, a, 1 << 20).unwrap(),
                    extract(&cleaned.bytes, b, 1 << 20).unwrap(),
                    "{}",
                    a.name
                );
            }
        }
        assert!(cleaned.removed[0].what.starts_with("docProps/core.xml"));
    }

    #[test]
    fn cleaning_anything_is_survivable() {
        for (name, bytes) in crate::docs::tests::fixtures()
            .into_iter()
            .chain(crate::docs::tests::damaged())
        {
            if let Ok(c) = clean(&bytes) {
                let doc = parse(&c.bytes);
                let errors = doc
                    .tree()
                    .nodes()
                    .iter()
                    .filter(|n| n.kind == NodeKind::Error)
                    .count();
                assert_eq!(errors, 0, "{name}: the copy has errors");
            }
        }
    }
}
