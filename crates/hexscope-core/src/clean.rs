//! A copy of a file without what it reveals about the people behind it.
//!
//! Nothing is re-encoded. A JPEG loses whole segments and keeps its image
//! data byte for byte. An Office document gets empty property files, and
//! every other entry is copied as it was stored. Cutting at the positions the
//! parser found means the copy differs from the original only where it must.

use crate::crc32::crc32;
use crate::document::{Document, parse};
use crate::heif::HeifDocument;
use crate::jpeg::JpegDocument;
use crate::model::{ByteRange, NodeKind};
use crate::png::PngDocument;
use crate::video::gapped::Shrunk;
use crate::video::{Gap, Scrub, VideoDocument, parse_video};
use crate::zip::ZipDocument;
use crate::zip::write::{Part, assemble, header_of};

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
    /// The whole document is encrypted.
    Locked,
    /// Parts of it are compressed in a way this does not read.
    Unreadable,
    /// Not a format this can clean.
    Unsupported,
    /// A box covers part of a picture kept in a way this cannot edit —
    /// JBIG2, CCITT, JPEG 2000 — which would still show what is under it.
    PictureUnderBox,
    /// A box covers part of a JPEG picture, and no painted copy of it was
    /// given: only a caller with a JPEG codec can black one out — the
    /// browser can, the command line cannot.
    JpegUnderBox,
    /// Legacy name retained for API compatibility: a PDF page, its content, or
    /// its page tree could not be inspected completely.
    FormContentIncomplete,
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
            CleanError::Locked => "it is encrypted, so hexscope cannot rewrite it",
            CleanError::Unreadable => {
                "parts of it are compressed in a way hexscope does not read, and a copy could lose them"
            }
            CleanError::Unsupported => {
                "hexscope cleans PNG, JPEG, HEIC, AVIF, WebP, GIF and TIFF images, SVG drawings, MP3 tags, MP4 and QuickTime videos, PDFs, Office and LibreOffice documents, RTF, old Word, Excel and PowerPoint files, and WebAssembly modules only"
            }
            CleanError::PictureUnderBox => {
                "a box covers part of a picture on the page that hexscope cannot edit — a JBIG2, CCITT or JPEG 2000 scan, or one too large — and the picture would still show what is under the box"
            }
            CleanError::JpegUnderBox => {
                "a box covers part of a JPEG picture on the page that was not redrawn, so the copy would still hold what is under the box — hexscope in the browser redraws it"
            }
            CleanError::FormContentIncomplete => {
                "no copy was made because PDF checks are incomplete"
            }
        }
    }
}

/// What a clean copy may take out beyond what it always does.
#[derive(Debug, Clone, Copy, Default)]
pub struct CleanOptions {
    /// Empty Excel's and PowerPoint's comments and speaker notes, and the
    /// names of who wrote them. They are part of the document, so only when
    /// asked.
    pub comments_and_notes: bool,
}

pub fn clean(data: &[u8]) -> Result<Cleaned, CleanError> {
    clean_with(data, CleanOptions::default())
}

pub fn clean_with(data: &[u8], options: CleanOptions) -> Result<Cleaned, CleanError> {
    let mut cleaned = match parse(data) {
        Document::Png(doc) => clean_png(data, &doc),
        Document::Jpeg(doc) => clean_jpeg(data, &doc),
        Document::Heif(doc) => clean_heif(data, &doc),
        Document::Webp(doc) => clean_webp(data, &doc),
        Document::Gif(doc) => clean_gif(data, &doc),
        Document::Tiff(doc) => crate::tiff::clean_tiff(data, &doc),
        Document::Zip(doc) if cfg!(feature = "office") => clean_zip(data, &doc, options, false),
        Document::Pdf(_) if cfg!(feature = "pdf") => crate::pdf::clean::clean_pdf(data),
        Document::Video(doc) => clean_video(data, &doc),
        #[cfg(feature = "audio")]
        Document::Audio(doc) if doc.format == crate::audio::AudioFormat::Mp3 => clean_mp3(data),
        #[cfg(feature = "audio")]
        Document::Audio(doc) if doc.format == crate::audio::AudioFormat::Flac => {
            clean_flac(data, &doc)
        }
        Document::Wasm(doc) if cfg!(feature = "module") => clean_wasm(data, &doc),
        Document::Cfb(doc) if cfg!(feature = "office") => clean_cfb(data, &doc),
        Document::Rtf(doc) if cfg!(feature = "office") => clean_rtf(data, &doc),
        Document::Svg(doc) if cfg!(feature = "svg") => crate::svg::clean_svg(data, &doc),
        _ => Err(CleanError::Unsupported),
    }?;
    cleaned.removed = merged(cleaned.removed);
    Ok(cleaned)
}

/// Removes only the bounded ID3v2, APE and ID3v1 tags at an MP3's edges.
/// MPEG frames are copied byte for byte and never decoded or re-encoded.
#[cfg(feature = "audio")]
fn clean_mp3(data: &[u8]) -> Result<Cleaned, CleanError> {
    let ranges = crate::audio::removable_tag_ranges(data).ok_or(CleanError::Damaged)?;
    if ranges.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut bytes = Vec::with_capacity(data.len());
    let mut removed = Vec::with_capacity(ranges.len());
    let mut at = 0u64;
    for (range, what) in ranges {
        let end = range.end();
        if range.start < at || end > data.len() as u64 {
            return Err(CleanError::Damaged);
        }
        bytes.extend_from_slice(
            data.get(at as usize..range.start as usize)
                .ok_or(CleanError::Damaged)?,
        );
        removed.push(Removed {
            what: what.into(),
            bytes: range.len,
        });
        at = end;
    }
    bytes.extend_from_slice(data.get(at as usize..).ok_or(CleanError::Damaged)?);
    Ok(Cleaned {
        bytes,
        removed,
        orientation_kept: None,
    })
}

/// Removes FLAC Vorbis comments and complete picture blocks while leaving
/// every retained metadata block and compressed audio frame byte for byte intact.
#[cfg(feature = "audio")]
fn clean_flac(data: &[u8], doc: &crate::audio::AudioDocument) -> Result<Cleaned, CleanError> {
    use crate::audio::AudioFormat;

    if doc.format != AudioFormat::Flac || !data.starts_with(b"fLaC") {
        return Err(CleanError::Damaged);
    }
    if doc
        .tree
        .nodes()
        .iter()
        .any(|node| node.kind == NodeKind::Error)
    {
        return Err(CleanError::Damaged);
    }
    if !doc.metadata_complete {
        return Err(CleanError::Unsupported);
    }
    let root = doc.tree.root().ok_or(CleanError::Damaged)?;
    let children = &doc.tree.get(root).children;
    let blocks = children
        .iter()
        .map(|&id| doc.tree.get(id))
        .filter(|node| node.label.starts_with("FLAC ") && node.label.ends_with(" block"))
        .collect::<Vec<_>>();
    let comments = blocks
        .iter()
        .filter(|node| node.label == "FLAC Vorbis comments block")
        .count();
    let pictures = blocks
        .iter()
        .filter(|node| node.label == "FLAC picture block")
        .count();
    if comments == 0 && pictures == 0 {
        return Err(CleanError::NothingToRemove);
    }
    if comments > 1 {
        return Err(CleanError::Damaged);
    }

    let frame = children
        .iter()
        .map(|&id| doc.tree.get(id))
        .find(|node| node.label == "audio frames (not decoded)")
        .ok_or(CleanError::Damaged)?;
    if frame.range.len == 0 || frame.range.end() != data.len() as u64 {
        return Err(CleanError::Damaged);
    }

    let mut cursor = 4u64;
    for block in &blocks {
        if block.range.start != cursor
            || block.range.len < 4
            || block.range.end() > frame.range.start
        {
            return Err(CleanError::Damaged);
        }
        cursor = block.range.end();
    }
    if cursor != frame.range.start {
        return Err(CleanError::Damaged);
    }

    let retained = blocks
        .iter()
        .copied()
        .filter(|node| {
            node.label != "FLAC Vorbis comments block" && node.label != "FLAC picture block"
        })
        .collect::<Vec<_>>();
    if retained.is_empty() || retained[0].label != "FLAC STREAMINFO block" {
        return Err(CleanError::Damaged);
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(b"fLaC");
    for (index, block) in retained.iter().enumerate() {
        let start = usize::try_from(block.range.start).map_err(|_| CleanError::Damaged)?;
        let end = usize::try_from(block.range.end()).map_err(|_| CleanError::Damaged)?;
        let bytes = data.get(start..end).ok_or(CleanError::Damaged)?;
        let mut copied = bytes.to_vec();
        copied[0] = (copied[0] & 0x7f) | if index + 1 == retained.len() { 0x80 } else { 0 };
        out.extend_from_slice(&copied);
    }
    out.extend_from_slice(
        data.get(frame.range.start as usize..)
            .ok_or(CleanError::Damaged)?,
    );

    let mut removed = Vec::new();
    if comments == 1 {
        removed.push(Removed {
            what: "FLAC Vorbis comments".into(),
            bytes: blocks
                .iter()
                .find(|node| node.label == "FLAC Vorbis comments block")
                .ok_or(CleanError::Damaged)?
                .range
                .len,
        });
    }
    if pictures > 0 {
        removed.push(Removed {
            what: "FLAC pictures".into(),
            bytes: blocks
                .iter()
                .filter(|node| node.label == "FLAC picture block")
                .map(|node| node.range.len)
                .sum(),
        });
    }

    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

/// Two blocks of the same kind, such as a second EXIF segment, are one line.
fn merged(removed: Vec<Removed>) -> Vec<Removed> {
    let mut out: Vec<Removed> = Vec::new();
    for r in removed {
        match out.iter_mut().find(|m| m.what == r.what) {
            Some(m) => m.bytes += r.bytes,
            None => out.push(r),
        }
    }
    out
}

/// A clean copy of a movie given without the bodies of the boxes in `gaps`,
/// as [`crate::video::parse_video_gapped`] reads it. The copy is of the bytes
/// given; each body goes back after its header unchanged.
pub fn clean_video_gapped(data: &[u8], gaps: &[Gap]) -> Result<Cleaned, CleanError> {
    let shrunk = Shrunk::new(data, gaps).ok_or(CleanError::Damaged)?;
    let doc = parse_video(&shrunk.bytes);
    let mut cleaned = clean_video(&shrunk.bytes, &doc)?;
    cleaned.bytes = shrunk.unshrink(&cleaned.bytes).ok_or(CleanError::Damaged)?;
    cleaned.removed = merged(cleaned.removed);
    Ok(cleaned)
}

/// A clean copy of a PDF with `areas` blacked out as well: each a page
/// number counted from 1 and `[left, bottom, right, top]` in its points.
/// What is under them is taken out of the page, not only covered, and a
/// black box drawn in its place.
///
/// Pictures under them lose their pixels there too. A JPEG picture needs a
/// JPEG codec, so it is painted by the caller: [`crate::pdf::pictures_under`]
/// says which and where, and `painted` gives each back, by object number.
pub fn redact(
    data: &[u8],
    areas: &[(u32, [f64; 4])],
    painted: &[(u32, Vec<u8>)],
) -> Result<Cleaned, CleanError> {
    if !crate::pdf::is_pdf(data) {
        return Err(CleanError::Unsupported);
    }
    crate::pdf::clean::clean_pdf_with(data, areas, painted)
}

// --- WebP and GIF --------------------------------------------------------------

/// A WebP without its EXIF and XMP chunks, the extended header's flags for
/// them cleared, and the RIFF size made right.
fn clean_webp(data: &[u8], doc: &crate::webp::WebpDocument) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let root = tree.root().ok_or(CleanError::Damaged)?;
    let mut body = b"WEBP".to_vec();
    let mut removed = Vec::new();
    for &id in &tree.get(root).children {
        let n = tree.get(id);
        let bytes = data
            .get(n.range.start as usize..n.range.end() as usize)
            .ok_or(CleanError::Damaged)?;
        match n.label.as_str() {
            "RIFF header" => {}
            "EXIF" => removed.push(Removed {
                what: "EXIF: camera, time, location, serial numbers".into(),
                bytes: n.range.len,
            }),
            "XMP" => removed.push(Removed {
                what: "XMP: editing history, author and place names".into(),
                bytes: n.range.len,
            }),
            "C2PA" => removed.push(Removed {
                what: "Content Credentials: who or what made it, and how it was edited".into(),
                bytes: n.range.len,
            }),
            "data after the end of the image" => removed.push(Removed {
                what: "data after the end of the image".into(),
                bytes: n.range.len,
            }),
            _ if n.kind == NodeKind::Warning => {}
            "VP8X" => {
                let mut chunk = bytes.to_vec();
                // Bit 3 says EXIF, bit 2 XMP: neither is there now.
                if let Some(flags) = chunk.get_mut(8) {
                    *flags &= !0x0C;
                }
                body.extend_from_slice(&chunk);
            }
            _ => body.extend_from_slice(bytes),
        }
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let size = u32::try_from(body.len()).map_err(|_| CleanError::Damaged)?;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

/// A GIF without its comments and XMP; timing, looping and colour profiles
/// stay, and every frame is copied byte for byte.
fn clean_gif(data: &[u8], doc: &crate::gif::GifDocument) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let root = tree.root().ok_or(CleanError::Damaged)?;
    let mut out = Vec::with_capacity(data.len());
    let mut removed = Vec::new();
    for &id in &tree.get(root).children {
        let n = tree.get(id);
        let bytes = data
            .get(n.range.start as usize..n.range.end() as usize)
            .ok_or(CleanError::Damaged)?;
        match n.label.as_str() {
            "comment" => removed.push(Removed {
                what: "comments".into(),
                bytes: n.range.len,
            }),
            l if l.starts_with("application · XMP") => removed.push(Removed {
                what: "XMP: editing history, author and place names".into(),
                bytes: n.range.len,
            }),
            "data after the end of the image" => removed.push(Removed {
                what: "data after the end of the image".into(),
                bytes: n.range.len,
            }),
            _ if n.kind == NodeKind::Warning => {}
            // The screen descriptor's node covers only its fields; its
            // colour table and the header are nodes of their own.
            _ => out.extend_from_slice(bytes),
        }
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

// --- WebAssembly -------------------------------------------------------------

/// A module without the custom sections that say who built it and how:
/// names, producers, source map and debug info links, DWARF. Engines ignore
/// custom sections, and nothing else refers to their positions, so each is
/// cut out whole and the rest copied byte for byte.
fn clean_wasm(data: &[u8], doc: &crate::wasm::WasmDocument) -> Result<Cleaned, CleanError> {
    if doc.component {
        return Err(CleanError::Unsupported);
    }
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.strip.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut bytes = Vec::with_capacity(data.len());
    let mut at = 0usize;
    let mut removed = Vec::new();
    for s in &doc.strip {
        let (start, end) = (s.range.start as usize, s.range.end() as usize);
        bytes.extend_from_slice(&data[at..start]);
        at = end;
        removed.push(Removed {
            what: s.what.to_string(),
            bytes: s.range.len,
        });
    }
    bytes.extend_from_slice(&data[at..]);
    Ok(Cleaned {
        bytes,
        removed,
        orientation_kept: None,
    })
}

// --- JPEG ------------------------------------------------------------------

/// What a removed segment held, in words, from its label.
fn jpeg_what(label: &str) -> String {
    let kind = label.split(" · ").nth(1).unwrap_or("");
    match (label, kind) {
        (_, "EXIF") => "EXIF: camera, time, location, serial numbers, thumbnail".into(),
        (_, "XMP") => "XMP: editing history, author and place names".into(),
        (_, "Photoshop") => "Photoshop and IPTC: captions, keywords, credits".into(),
        (_, "MPF") => "an index of extra pictures stored in the file".into(),
        (_, "JUMBF") => "Content Credentials: who or what made it, and how it was edited".into(),
        (_, "JFXX") => "a JFIF thumbnail".into(),
        ("COM", _) => "a comment".into(),
        ("data after the end of the image", _) => "data after the end of the picture".into(),
        (crate::jpeg::LISTED_PICTURE, _) => {
            "a second picture for HDR screens or depth: on an HDR screen the copy shows less bright highlights, elsewhere the same".into()
        }
        _ => format!("{label}: application data"),
    }
}

/// Segments that say how to show the picture, not who took it.
fn jpeg_keeps(label: &str) -> bool {
    !(label.starts_with("APP")
        || label == "COM"
        || label == "data after the end of the image"
        || label == crate::jpeg::LISTED_PICTURE)
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
    if top
        .iter()
        .any(|&id| tree.get(id).label == crate::jpeg::LISTED_PICTURE)
    {
        #[cfg(feature = "hdr")]
        if top.iter().any(|&id| is_hdr_gain_map(tree, id)) {
            return clean_hdr_jpeg(data, doc, root, top);
        }
        // A listed secondary image is part of the photo. Without recognized
        // gain-map metadata, do not turn it into blank bytes or lose its role.
        return Err(CleanError::Unsupported);
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

#[cfg(feature = "hdr")]
const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
#[cfg(feature = "hdr")]
const ISO_GAIN_MAP_ID: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

#[cfg(feature = "hdr")]
fn is_hdr_gain_map(tree: &crate::model::ParseTree, id: u32) -> bool {
    let node = tree.get(id);
    node.label == crate::jpeg::LISTED_PICTURE
        && matches!(node.value, Some(crate::model::Value::Text(ref kind)) if kind == "HDR gain map")
}

#[cfg(feature = "hdr")]
fn xmp_info(
    data: &[u8],
    range: ByteRange,
    expected_map_len: Option<u64>,
) -> crate::jpeg::HdrXmpInfo {
    let Some(segment) = data.get(range.start as usize..range.end() as usize) else {
        return crate::jpeg::HdrXmpInfo::default();
    };
    let Some(payload) = segment.get(4..) else {
        return crate::jpeg::HdrXmpInfo::default();
    };
    let Some(packet) = payload.strip_prefix(XMP_ID) else {
        return crate::jpeg::HdrXmpInfo::default();
    };
    crate::jpeg::inspect_hdr_xmp(packet, expected_map_len)
}

#[cfg(feature = "hdr")]
fn has_container_directory(data: &[u8], range: ByteRange) -> bool {
    let needle = b"Container:Directory";
    data.get(range.start as usize..range.end() as usize)
        .is_some_and(|segment| segment.windows(needle.len()).any(|w| w == needle))
}

#[cfg(feature = "hdr")]
fn has_hdr_gain_map_fields(data: &[u8], range: ByteRange) -> bool {
    let Some(segment) = data.get(range.start as usize..range.end() as usize) else {
        return false;
    };
    [
        b"HDRGainMap".as_slice(),
        b"hdrgm:".as_slice(),
        b"urn:iso:std:iso:ts:21496".as_slice(),
    ]
    .iter()
    .any(|needle| {
        segment
            .windows(needle.len())
            .any(|window| window == *needle)
    })
}

/// ISO 21496-1's primary-image marker contains only the version fields; the
/// gain-map JPEG carries those fields and the encoded gain-map parameters.
#[cfg(feature = "hdr")]
fn valid_iso_gain_map_segment(data: &[u8], range: ByteRange, require_values: bool) -> bool {
    let Some(segment) = data.get(range.start as usize..range.end() as usize) else {
        return false;
    };
    let Some(payload) = segment.get(4..) else {
        return false;
    };
    let Some(metadata) = payload.strip_prefix(ISO_GAIN_MAP_ID) else {
        return false;
    };
    valid_iso_gain_map_metadata(metadata, require_values)
}

/// ISO 21496-1 version 0 metadata has an exact layout. The primary image's
/// marker contains only the two version fields; the gain-map image carries a
/// flags byte followed by one or three channels of rational values.
#[cfg(feature = "hdr")]
fn valid_iso_gain_map_metadata(metadata: &[u8], require_values: bool) -> bool {
    if metadata.len() == 4 {
        return !require_values && metadata == [0, 0, 0, 0];
    }
    if !require_values || metadata.len() < 5 || metadata[..4] != [0, 0, 0, 0] {
        return false;
    }
    let flags = metadata[4];
    // Version 0 has four defined flag bits. The direction bit is defined but
    // unsupported by the decoder; reserved bits and future versions fail closed.
    if flags & 0xF0 != 0 || flags & 0x04 != 0 {
        return false;
    }
    let channels = if flags & 0x01 != 0 { 3 } else { 1 };
    let common_denominator = flags & 0x08 != 0;
    let fields_start = if common_denominator { 17 } else { 21 };
    let field_bytes = if common_denominator { 20 } else { 40 };
    if metadata.len() != fields_start + channels * field_bytes {
        return false;
    }
    if common_denominator {
        return nonzero_be_u32(metadata, 5);
    }
    for channel in 0..channels {
        let start = fields_start + channel * field_bytes;
        for denominator in [start + 4, start + 12, start + 20, start + 28, start + 36] {
            if !nonzero_be_u32(metadata, denominator) {
                return false;
            }
        }
    }
    nonzero_be_u32(metadata, 9) && nonzero_be_u32(metadata, 17)
}

#[cfg(feature = "hdr")]
fn nonzero_be_u32(bytes: &[u8], at: usize) -> bool {
    bytes.get(at..at + 4).is_some_and(|raw| raw != [0, 0, 0, 0])
}

/// HDR JPEGs depend on offsets from their MPF marker to the appended gain map.
/// Keep each segment's length fixed, blank personal metadata in place, and
/// preserve only technical gain-map metadata that passed the strict XMP gate.
#[cfg(feature = "hdr")]
fn clean_hdr_jpeg(
    data: &[u8],
    doc: &JpegDocument,
    _root: u32,
    top: &[u32],
) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    let maps: Vec<_> = top
        .iter()
        .map(|&id| tree.get(id))
        .filter(|node| node.label == crate::jpeg::LISTED_PICTURE)
        .collect();
    let mpf_count = top
        .iter()
        .filter(|&&id| tree.get(id).label == "APP2 · MPF")
        .count();
    if maps.len() != 1 || mpf_count != 1 || doc.pictures.len() != 1 {
        return Err(CleanError::Unsupported);
    }
    let map_node = maps[0];
    let map_range = map_node.range;
    if !is_hdr_gain_map(tree, map_node.id)
        || doc.pictures[0] != map_range
        || map_range.end() > data.len() as u64
        || top
            .iter()
            .any(|&id| tree.get(id).label == "data after the end of the image")
    {
        return Err(CleanError::Unsupported);
    }
    let map_bytes = data
        .get(map_range.start as usize..map_range.end() as usize)
        .ok_or(CleanError::Damaged)?;
    if map_bytes.len() as u64 > 64 * 1024 * 1024 {
        return Err(CleanError::Unsupported);
    }
    let map_doc = crate::jpeg::parse_jpeg(map_bytes);
    if map_doc.tree.nodes().iter().any(|node| {
        node.kind == NodeKind::Error
            || node.kind == NodeKind::Warning
            || node.label == crate::jpeg::LISTED_PICTURE
            || node.label == "data after the end of the image"
    }) || !map_doc.pictures.is_empty()
    {
        return Err(CleanError::Damaged);
    }
    let map_root = map_doc.tree.root().ok_or(CleanError::Damaged)?;
    let map_top = &map_doc.tree.get(map_root).children;
    let has_map_iso = map_top.iter().any(|&id| {
        let node = map_doc.tree.get(id);
        node.label == "APP2 · ISO gain map"
            && valid_iso_gain_map_segment(map_bytes, node.range, true)
    });
    let map_xmps: Vec<_> = map_top
        .iter()
        .map(|&id| map_doc.tree.get(id))
        .filter(|node| node.label == "APP1 · XMP")
        .collect();
    let map_has_safe_xmp = map_xmps.iter().any(|node| {
        let info = xmp_info(map_bytes, node.range, None);
        info.safe && info.has_gain_map_metadata
    });
    if !has_map_iso && !map_has_safe_xmp {
        return Err(CleanError::Unsupported);
    }
    for node in &map_xmps {
        let info = xmp_info(map_bytes, node.range, None);
        if !info.safe && has_container_directory(map_bytes, node.range) {
            return Err(CleanError::Unsupported);
        }
        if !info.safe && !has_map_iso {
            return Err(CleanError::Unsupported);
        }
    }

    let primary_xmps: Vec<_> = top
        .iter()
        .map(|&id| tree.get(id))
        .filter(|node| node.label == "APP1 · XMP")
        .collect();
    for node in &primary_xmps {
        let info = xmp_info(data, node.range, Some(map_range.len));
        if !info.safe
            && (has_container_directory(data, node.range)
                || has_hdr_gain_map_fields(data, node.range))
        {
            return Err(CleanError::Unsupported);
        }
    }

    let orientation = doc.facts.orientation.filter(|&o| (2..=8).contains(&o));
    let minimal_orientation = orientation.map(orientation_segment);
    if let (Some(segment), Some(range)) = (
        &minimal_orientation,
        top.iter()
            .find_map(|&id| (tree.get(id).label == "APP1 · EXIF").then_some(tree.get(id).range)),
    ) && range.len < segment.len() as u64
    {
        return Err(CleanError::Unsupported);
    }

    let mut out = data.to_vec();
    let mut removed = Vec::new();
    for &id in top {
        let node = tree.get(id);
        if node.id == map_node.id {
            let (sanitized, map_removed) = clean_hdr_gain_map(map_bytes, &map_doc, has_map_iso)?;
            if sanitized.len() != map_bytes.len() {
                return Err(CleanError::Damaged);
            }
            out.get_mut(map_range.start as usize..map_range.end() as usize)
                .ok_or(CleanError::Damaged)?
                .copy_from_slice(&sanitized);
            removed.extend(map_removed);
            continue;
        }
        if node.range.len == 0
            || node.label == "SOI"
            || node.label == "EOI"
            || node.label == "APP2 · MPF"
            || node.label == "APP2 · ISO gain map"
            || jpeg_keeps(&node.label)
        {
            if node.label == "APP2 · ISO gain map"
                && !valid_iso_gain_map_segment(data, node.range, false)
            {
                blank_jpeg_segment(&mut out, node.range)?;
                removed.push(Removed {
                    what: jpeg_what(&node.label),
                    bytes: node.range.len,
                });
            }
            continue;
        }
        if node.label == "data after the end of the image" {
            return Err(CleanError::Unsupported);
        }
        if node.label == "APP1 · XMP" {
            let info = xmp_info(data, node.range, Some(map_range.len));
            if info.safe && (info.has_container_directory || info.has_gain_map_metadata) {
                continue;
            }
        }
        if node.label == "APP1 · EXIF" {
            if let Some(segment) = &minimal_orientation {
                write_orientation_in_segment(&mut out, data, node.range, segment)?;
            } else {
                blank_jpeg_segment(&mut out, node.range)?;
            }
        } else {
            blank_jpeg_segment(&mut out, node.range)?;
        }
        removed.push(Removed {
            what: jpeg_what(&node.label),
            bytes: node.range.len,
        });
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: orientation,
    })
}

#[cfg(feature = "hdr")]
fn clean_hdr_gain_map(
    data: &[u8],
    doc: &JpegDocument,
    has_iso: bool,
) -> Result<(Vec<u8>, Vec<Removed>), CleanError> {
    let tree = &doc.tree;
    let root = tree.root().ok_or(CleanError::Damaged)?;
    let mut out = data.to_vec();
    let mut removed = Vec::new();
    for &id in &tree.get(root).children {
        let node = tree.get(id);
        if node.range.len == 0 || node.label == "SOI" || node.label == "EOI" {
            continue;
        }
        if node.label == "APP2 · ISO gain map" {
            if valid_iso_gain_map_segment(data, node.range, true) {
                continue;
            }
        } else if node.label == "APP1 · XMP" {
            let info = xmp_info(data, node.range, None);
            if info.safe && info.has_gain_map_metadata {
                continue;
            }
            if has_container_directory(data, node.range) || !has_iso {
                return Err(CleanError::Unsupported);
            }
        } else if jpeg_keeps(&node.label) {
            continue;
        }
        if node.label == "data after the end of the image"
            || node.label == crate::jpeg::LISTED_PICTURE
        {
            return Err(CleanError::Unsupported);
        }
        blank_jpeg_segment(&mut out, node.range)?;
        removed.push(Removed {
            what: jpeg_what(&node.label),
            bytes: node.range.len,
        });
    }
    Ok((out, removed))
}

#[cfg(feature = "hdr")]
fn blank_jpeg_segment(out: &mut [u8], range: ByteRange) -> Result<(), CleanError> {
    let start = usize::try_from(range.start).map_err(|_| CleanError::Damaged)?;
    let end = usize::try_from(range.end()).map_err(|_| CleanError::Damaged)?;
    let payload = start.checked_add(4).ok_or(CleanError::Damaged)?;
    out.get_mut(payload..end)
        .ok_or(CleanError::Damaged)?
        .fill(0);
    Ok(())
}

#[cfg(feature = "hdr")]
fn write_orientation_in_segment(
    out: &mut [u8],
    original: &[u8],
    range: ByteRange,
    orientation: &[u8],
) -> Result<(), CleanError> {
    let start = usize::try_from(range.start).map_err(|_| CleanError::Damaged)?;
    let end = usize::try_from(range.end()).map_err(|_| CleanError::Damaged)?;
    let room = out.get_mut(start..end).ok_or(CleanError::Damaged)?;
    let source = original.get(start..start + 4).ok_or(CleanError::Damaged)?;
    let length = [source[2], source[3]];
    room.fill(0);
    let minimal = room
        .get_mut(..orientation.len())
        .ok_or(CleanError::Damaged)?;
    minimal.copy_from_slice(orientation);
    minimal[2..4].copy_from_slice(&length);
    Ok(())
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

// --- PNG ---------------------------------------------------------------------

/// Text notes, EXIF and the modification time say who made the image, with
/// what and when; every other chunk is about how to show it, and stays.
fn clean_png(data: &[u8], doc: &PngDocument) -> Result<Cleaned, CleanError> {
    let tree = &doc.tree;
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let root = tree.root().ok_or(CleanError::Damaged)?;
    let top = &tree.get(root).children;

    let orientation = doc.facts.orientation.filter(|&o| (2..=8).contains(&o));
    let mut out = data.get(..8).ok_or(CleanError::Damaged)?.to_vec();
    let mut text: Vec<String> = Vec::new();
    let (mut text_bytes, mut removed) = (0u64, Vec::new());
    for &id in top {
        let n = tree.get(id);
        let bytes = data
            .get(n.range.start as usize..n.range.end() as usize)
            .ok_or(CleanError::Damaged)?;
        match n.label.as_str() {
            "signature" | "damaged PNG signature" => {}
            "tEXt" | "zTXt" | "iTXt" => {
                let keyword = n
                    .children
                    .iter()
                    .map(|&c| tree.get(c))
                    .find(|c| c.label == "keyword")
                    .and_then(|c| match &c.value {
                        Some(crate::model::Value::Text(t)) => Some(t.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                if keyword == "XML:com.adobe.xmp" {
                    removed.push(Removed {
                        what: "XMP: editing history, author and place names".into(),
                        bytes: n.range.len,
                    });
                } else {
                    if !keyword.is_empty() && !text.contains(&keyword) {
                        text.push(keyword);
                    }
                    text_bytes += n.range.len;
                }
            }
            "eXIf" => removed.push(Removed {
                what: "EXIF: camera, time, location, serial numbers".into(),
                bytes: n.range.len,
            }),
            "tIME" => removed.push(Removed {
                what: "the time it was last changed".into(),
                bytes: n.range.len,
            }),
            "caBX" => removed.push(Removed {
                what: "Content Credentials: who or what made it, and how it was edited".into(),
                bytes: n.range.len,
            }),
            "data after the end of the image" => removed.push(Removed {
                what: "data after the end of the image".into(),
                bytes: n.range.len,
            }),
            // Problems inside the file other than damage are kept as they
            // are; they are not chunks of their own.
            _ if n.kind == NodeKind::Warning => {}
            label => {
                out.extend_from_slice(bytes);
                // The orientation goes back right after the header, so the
                // picture stays upright.
                if label == "IHDR"
                    && let Some(o) = orientation
                {
                    let tiff = minimal_tiff(Some(o));
                    let mut chunk = (tiff.len() as u32).to_be_bytes().to_vec();
                    let start = chunk.len();
                    chunk.extend_from_slice(b"eXIf");
                    chunk.extend_from_slice(&tiff);
                    let crc = crc32(&chunk[start..]);
                    chunk.extend_from_slice(&crc.to_be_bytes());
                    out.extend_from_slice(&chunk);
                }
            }
        }
    }
    if text_bytes > 0 {
        let what = if text.is_empty() {
            "text notes".to_string()
        } else {
            format!("text notes: {}", text.join(", "))
        };
        removed.insert(
            0,
            Removed {
                what,
                bytes: text_bytes,
            },
        );
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: orientation,
    })
}

// --- MP4 and QuickTime --------------------------------------------------------

/// A movie's chunks are found by absolute offset, so the copy changes bytes
/// in place: each box that describes becomes a `free` box of the same size
/// with nothing in it, and the times in the headers become zero.
fn clean_video(data: &[u8], doc: &VideoDocument) -> Result<Cleaned, CleanError> {
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.scrub.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut out = data.to_vec();
    let mut removed: Vec<Removed> = Vec::new();
    for s in &doc.scrub {
        let (what, bytes) = match *s {
            Scrub::Free {
                at,
                len,
                header,
                what,
            } => {
                let (at, len, header) = (at as usize, len as usize, header as usize);
                let range = out.get_mut(at..at + len).ok_or(CleanError::Damaged)?;
                if len < header {
                    return Err(CleanError::Damaged);
                }
                range[4..8].copy_from_slice(b"free");
                range[header..].fill(0);
                (what, len as u64)
            }
            Scrub::Zero { range, what } => {
                let r = out
                    .get_mut(range.start as usize..range.end() as usize)
                    .ok_or(CleanError::Damaged)?;
                r.fill(0);
                (what, range.len)
            }
        };
        // Lines of the same kind are merged by `clean`.
        removed.push(Removed {
            what: what.into(),
            bytes,
        });
    }
    // The place first: it is what people most need gone.
    if let Some(i) = removed.iter().position(|r| r.what == "the location") {
        let place = removed.remove(i);
        removed.insert(0, place);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

// --- Office 97–2003 -------------------------------------------------------------

/// A Word, Excel or PowerPoint 97–2003 file with its properties and unused
/// sectors overwritten with zeros where they are, so nothing else moves.
fn clean_cfb(data: &[u8], doc: &crate::cfb::CfbDocument) -> Result<Cleaned, CleanError> {
    if !doc.kind.is_office() {
        return Err(CleanError::Unsupported);
    }
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    let blanks = crate::cfb::blanks(data);
    if blanks.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut out = data.to_vec();
    let mut removed = Vec::new();
    for (what, spans) in blanks {
        let mut bytes = 0;
        for (at, len) in spans {
            out[at as usize..(at + len) as usize].fill(0);
            bytes += len;
        }
        removed.push(Removed {
            what: what.into(),
            bytes,
        });
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

// --- HEIF (HEIC, AVIF) -------------------------------------------------------

/// A minimal big-endian TIFF block: IFD0 with Orientation when there is one,
/// otherwise with no entries.
fn minimal_tiff(orientation: Option<u16>) -> Vec<u8> {
    let mut t = b"MM\x00\x2A\x00\x00\x00\x08".to_vec();
    match orientation {
        Some(o) => {
            t.extend_from_slice(&[0x00, 0x01, 0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01]);
            t.extend_from_slice(&o.to_be_bytes());
            t.extend_from_slice(&[0x00, 0x00]);
        }
        None => t.extend_from_slice(&[0x00, 0x00]),
    }
    t.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    t
}

/// An XMP packet with nothing in it, exactly `len` bytes long: the padding
/// goes inside the packet, where XMP expects it.
fn empty_xmp(len: usize) -> Option<Vec<u8>> {
    let head = "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>";
    let tail = "<?xpacket end=\"w\"?>";
    let pad = len.checked_sub(head.len() + tail.len())?;
    let mut out = head.as_bytes().to_vec();
    out.resize(out.len() + pad, b' ');
    out.extend_from_slice(tail.as_bytes());
    Some(out)
}

/// HEIF points at items by absolute offset, so the copy changes bytes in
/// place and keeps every length: no offset anywhere has to move.
fn clean_heif(data: &[u8], doc: &HeifDocument) -> Result<Cleaned, CleanError> {
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.exif.is_none() && doc.xmp.is_none() {
        return Err(CleanError::NothingToRemove);
    }
    let mut out = data.to_vec();
    let mut removed = Vec::new();
    let orientation = doc.facts.orientation.filter(|&o| (2..=8).contains(&o));
    if let Some(exif) = doc.exif {
        let (start, end) = (exif.tiff_start as usize, exif.data.end() as usize);
        let tiff = minimal_tiff(orientation);
        let room = out.get_mut(start..end).ok_or(CleanError::Damaged)?;
        if room.len() < tiff.len() {
            return Err(CleanError::Damaged);
        }
        room.fill(0);
        room[..tiff.len()].copy_from_slice(&tiff);
        removed.push(Removed {
            what: "EXIF: camera, time, location, serial numbers".into(),
            bytes: exif.data.len,
        });
    }
    if let Some(xmp) = doc.xmp {
        let (start, end) = (xmp.start as usize, xmp.end() as usize);
        let room = out.get_mut(start..end).ok_or(CleanError::Damaged)?;
        match empty_xmp(room.len()) {
            Some(packet) => room.copy_from_slice(&packet),
            // Too small for even an empty packet: blank it instead.
            None => room.fill(b' '),
        }
        removed.push(Removed {
            what: "XMP: editing history, author and place names".into(),
            bytes: xmp.len,
        });
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: orientation.filter(|_| doc.exif.is_some()),
    })
}

// --- RTF -----------------------------------------------------------------------

/// An RTF document without its information group, the program and template
/// that made it, its custom properties and editing-session numbers, and the
/// names of who changed and commented on it. The text is copied as it is:
/// tracked changes and comments stay, without their authors.
fn clean_rtf(data: &[u8], doc: &crate::rtf::RtfDocument) -> Result<Cleaned, CleanError> {
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    apply(data, &doc.edits)
}

/// A text document's copy: each edit's bytes replaced by what it puts
/// there, in one pass, and the edits named once each by what they took.
pub(crate) fn apply(data: &[u8], edits: &[crate::rtf::Edit]) -> Result<Cleaned, CleanError> {
    if edits.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0usize;
    let mut removed: Vec<Removed> = Vec::new();
    for e in edits {
        let (start, end) = (e.range.start as usize, e.range.end() as usize);
        // An edit inside one already made — a comment's author in a removed group — is part of it.
        if start < at {
            continue;
        }
        out.extend_from_slice(data.get(at..start).ok_or(CleanError::Damaged)?);
        out.extend_from_slice(&e.with);
        at = end;
        let bytes = (end - start).saturating_sub(e.with.len()) as u64;
        match removed.iter_mut().find(|r| r.what == e.what) {
            Some(r) => r.bytes += bytes,
            None => removed.push(Removed {
                what: e.what.into(),
                bytes,
            }),
        }
    }
    out.extend_from_slice(data.get(at..).ok_or(CleanError::Damaged)?);
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
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
            "document properties: title, subject, author, last editor, keywords, description, dates",
        )),
        "docProps/app.xml" => Some((
            EMPTY_APP,
            "application properties: company, manager, application, template, editing time",
        )),
        "docProps/custom.xml" => Some((EMPTY_CUSTOM, "custom properties and sensitivity labels")),
        _ => None,
    }
}

/// A Word part with its tracked changes accepted and its comments' marks
/// gone, or a part of comments emptied, and what that did; `None` when the
/// part is neither, or has nothing to take out.
fn revise(
    data: &[u8],
    e: &crate::zip::ZipEntry,
    options: CleanOptions,
    odf: bool,
) -> Option<(Vec<u8>, String)> {
    use crate::zip::office::MAX_PART;
    // An OpenDocument file's own parts: its properties, settings, thumbnail and text.
    if odf && let Some(change) = crate::zip::odf::revise(data, e) {
        return Some(change);
    }
    use crate::zip::revise::{accept, blank_notes, emptied, is_comments, is_story};
    if e.uncompressed > MAX_PART {
        return None;
    }
    // What Office says about the computer: the folder, the template's path, the printer.
    if !odf && let Some(change) = crate::zip::traces::revise(data, e) {
        return Some(change);
    }
    // Excel's and PowerPoint's comments and notes, when asked for.
    if options.comments_and_notes && blank_notes(&e.name, "").is_some() {
        let bytes = crate::zip::extract(data, e, MAX_PART).ok()?;
        let xml = std::str::from_utf8(&bytes).ok()?;
        let (out, n) = blank_notes(&e.name, xml)?;
        let what = if e.name.starts_with("ppt/notesSlides/") {
            "the speaker's notes"
        } else if e.name.contains("uthor") || e.name.contains("person") {
            "who wrote the comments"
        } else {
            "comments"
        };
        return (n > 0).then(|| (out.into_bytes(), format!("{what}, emptied")));
    }
    if !(is_story(&e.name) || is_comments(&e.name)) {
        return None;
    }
    let bytes = crate::zip::extract(data, e, MAX_PART).ok()?;
    let xml = std::str::from_utf8(&bytes).ok()?;
    if is_comments(&e.name) {
        let empty = emptied(xml)?;
        return (empty.len() < xml.len()).then(|| {
            (
                empty.into_bytes(),
                "comments and who wrote them".to_string(),
            )
        });
    }
    let (out, n) = accept(xml);
    // Text no reader sees — hidden, white, too small — goes too.
    let (out, hidden) = crate::zip::revise::drop_hidden(&out);
    // The editing-session numbers that tie it to other documents go too.
    let (out, sessions) = crate::zip::traces::drop_sessions(&out);
    let mut said = Vec::new();
    if n > 0 {
        said.push(if n == 1 {
            "1 tracked change or comment mark, accepted or removed".to_string()
        } else {
            format!("{n} tracked changes and comment marks, accepted or removed")
        });
    }
    if hidden > 0 {
        said.push(if hidden == 1 {
            "1 piece of text no one can see".to_string()
        } else {
            format!("{hidden} pieces of text no one can see")
        });
    }
    if sessions > 0 {
        said.push("editing-session numbers".to_string());
    }
    (!said.is_empty()).then(|| (out.into_bytes(), said.join("; ")))
}

/// A photo placed in an Office document, cleaned like one opened on its
/// own; `None` when it is not a JPEG or PNG, or has nothing to remove.
fn clean_media(data: &[u8], e: &crate::zip::ZipEntry) -> Option<Cleaned> {
    use crate::zip::office::MAX_PART;
    if e.uncompressed > MAX_PART {
        return None;
    }
    let bytes = crate::zip::extract(data, e, MAX_PART).ok()?;
    // Only pictures: an archive inside is not opened again.
    if !bytes.starts_with(&[0xFF, 0xD8]) && !bytes.starts_with(b"\x89PNG") {
        return None;
    }
    clean(&bytes).ok()
}

/// A Word, Excel or PowerPoint file kept inside a document, such as the
/// workbook behind a chart, cleaned as a file of its own. `inside` is set
/// for it, so a file inside that one is left as it is: a document that
/// holds itself cannot send this round for ever.
fn clean_embedded(data: &[u8], e: &crate::zip::ZipEntry, options: CleanOptions) -> Option<Cleaned> {
    use crate::zip::office::{MAX_PART, is_embedded};
    if !is_embedded(&e.name) || e.uncompressed > MAX_PART {
        return None;
    }
    let bytes = crate::zip::extract(data, e, MAX_PART).ok()?;
    if !bytes.starts_with(&crate::zip::MAGIC) {
        return None;
    }
    clean_zip(&bytes, &crate::zip::parse_zip(&bytes), options, true).ok()
}

fn clean_zip(
    data: &[u8],
    doc: &ZipDocument,
    options: CleanOptions,
    inside: bool,
) -> Result<Cleaned, CleanError> {
    use crate::zip::office::{MAX_EMBEDDED, MAX_PHOTOS, is_embedded};
    let tree = &doc.tree;
    if doc
        .entries
        .iter()
        .any(|e| !crate::zip::write::headers_agree(data, tree, e))
    {
        return Err(CleanError::Damaged);
    }
    // Where the document keeps its photos: Office's media folders, an OpenDocument file's Pictures.
    let placed = |name: &str| {
        crate::zip::office::is_media(name) || (doc.odf && crate::zip::odf::is_picture(name))
    };
    let (mut photos, mut files) = (0, 0);
    let media: Vec<Option<Cleaned>> = doc
        .entries
        .iter()
        .map(|e| {
            if is_embedded(&e.name) {
                if inside || files >= MAX_EMBEDDED {
                    return None;
                }
                files += 1;
                return clean_embedded(data, e, options);
            }
            if photos >= MAX_PHOTOS || !placed(&e.name) {
                return None;
            }
            photos += 1;
            clean_media(data, e)
        })
        .collect();
    let revised: Vec<Option<(Vec<u8>, String)>> = doc
        .entries
        .iter()
        .map(|e| revise(data, e, options, doc.odf))
        .collect();
    use crate::zip::apple::is_apple_double;
    if !doc
        .entries
        .iter()
        .any(|e| replacement(&e.name).is_some() || is_apple_double(&e.name))
        && media.iter().all(Option::is_none)
        && revised.iter().all(Option::is_none)
    {
        return Err(CleanError::NothingToRemove);
    }
    if tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.entries.iter().any(|e| e.is_encrypted()) {
        return Err(CleanError::Encrypted);
    }
    let mut parts = Vec::with_capacity(doc.entries.len());
    let mut removed = Vec::new();
    let mut extras = 0u64;
    let mut mac = 0u64;
    for ((e, photo), change) in doc.entries.iter().zip(&media).zip(&revised) {
        if e.data.len != e.compressed {
            return Err(CleanError::Damaged);
        }
        // A Mac's notes about the files are not files: the copy leaves them out.
        if is_apple_double(&e.name) {
            mac += e.uncompressed.max(1);
            continue;
        }
        // The original local header: its times and its exact name bytes.
        let (name, time, extra) = header_of(data, tree, e).ok_or(CleanError::Damaged)?;
        extras += u64::from(extra);
        let original = data
            .get(e.data.start as usize..e.data.end() as usize)
            .ok_or(CleanError::Damaged)?;
        let (method, body, crc, uncompressed): (u16, &[u8], u32, u64) = match replacement(&e.name) {
            Some((xml, what)) => {
                removed.push(Removed {
                    what: format!("{}: {what}", e.name),
                    bytes: e.uncompressed,
                });
                (0, xml.as_bytes(), crc32(xml.as_bytes()), xml.len() as u64)
            }
            // A photo is stored as it is: JPEG and PNG are compressed already.
            None => match (photo, change) {
                (_, Some((xml, what))) => {
                    removed.push(Removed {
                        what: format!("{}: {what}", e.name),
                        bytes: e.uncompressed.saturating_sub(xml.len() as u64),
                    });
                    (0, &xml[..], crc32(xml), xml.len() as u64)
                }
                (Some(c), None) => {
                    let what = if placed(&e.name) {
                        "what the photo reveals"
                    } else {
                        "who made it, and what else it reveals"
                    };
                    removed.push(Removed {
                        what: format!("{}: {what}", e.name),
                        bytes: c.removed.iter().map(|r| r.bytes).sum(),
                    });
                    (0, &c.bytes[..], crc32(&c.bytes), c.bytes.len() as u64)
                }
                (None, None) => (e.method, original, e.crc32, e.uncompressed),
            },
        };
        parts.push(Part {
            name,
            flags: e.flags,
            method,
            time,
            crc,
            uncompressed,
            body,
        });
    }
    let out = assemble(&parts).ok_or(CleanError::Zip64)?;

    if mac > 0 {
        removed.push(Removed {
            what: "__MACOSX: what the Mac noted about each file — where it was downloaded from, with which app, its tags".into(),
            bytes: mac,
        });
    }

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
    use crate::exif::PhotoFacts;
    use crate::exif::testing::{Spec, V, build};
    use crate::jpeg::parse_jpeg;
    use crate::jpeg::testing::jpeg_with_exif;
    use crate::zip::{extract, parse_zip};

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    fn scan(bytes: &[u8]) -> Vec<u8> {
        let doc = parse_jpeg(bytes);
        let root = doc.tree.root().unwrap();
        let n = doc
            .tree
            .get(root)
            .children
            .iter()
            .map(|&id| doc.tree.get(id))
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

    #[cfg(feature = "audio")]
    fn flac_with_metadata(
        fields: Option<&[&[u8]]>,
        extra: &[(u8, &[u8])],
        frames: &[u8],
    ) -> Vec<u8> {
        let mut blocks = vec![(0u8, vec![0; 34])];
        if let Some(fields) = fields {
            let mut comments = Vec::new();
            comments.extend_from_slice(&12u32.to_le_bytes());
            comments.extend_from_slice(b"test encoder");
            comments.extend_from_slice(&(fields.len() as u32).to_le_bytes());
            for field in fields {
                comments.extend_from_slice(&(field.len() as u32).to_le_bytes());
                comments.extend_from_slice(field);
            }
            blocks.push((4u8, comments));
        }
        blocks.extend(extra.iter().map(|(kind, body)| (*kind, body.to_vec())));
        let mut out = b"fLaC".to_vec();
        for (index, (kind, body)) in blocks.iter().enumerate() {
            let last = if index + 1 == blocks.len() { 0x80 } else { 0 };
            out.push(last | *kind);
            out.extend_from_slice(&[
                ((body.len() >> 16) & 0xff) as u8,
                ((body.len() >> 8) & 0xff) as u8,
                (body.len() & 0xff) as u8,
            ]);
            out.extend_from_slice(body);
        }
        out.extend_from_slice(frames);
        out
    }

    #[cfg(feature = "hdr")]
    fn add_iso_gain_map_segment(bytes: &mut Vec<u8>, metadata: &[u8]) {
        let doc = parse_jpeg(bytes);
        let range = doc.pictures[0];
        let mpf = doc
            .tree
            .nodes()
            .iter()
            .find(|node| node.label == "APP2 · MPF")
            .unwrap()
            .range;
        let mut segment = vec![0xFF, 0xE2];
        let payload_len = ISO_GAIN_MAP_ID.len() + metadata.len();
        segment.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes());
        segment.extend_from_slice(ISO_GAIN_MAP_ID);
        segment.extend_from_slice(metadata);

        let old_size = (range.len as u32).to_be_bytes();
        let mpf_bytes = &bytes[mpf.start as usize..mpf.end() as usize];
        let matches: Vec<_> = mpf_bytes
            .windows(old_size.len())
            .enumerate()
            .filter_map(|(at, window)| (window == old_size).then_some(at))
            .collect();
        assert_eq!(matches.len(), 1, "fixture has one MPF size entry");
        let size_at = mpf.start as usize + matches[0];
        let new_size = range.len as u32 + segment.len() as u32;
        bytes[size_at..size_at + 4].copy_from_slice(&new_size.to_be_bytes());
        bytes.splice(range.start as usize + 2..range.start as usize + 2, segment);
    }

    #[cfg(feature = "hdr")]
    #[test]
    fn iso_gain_map_metadata_requires_a_complete_known_layout() {
        assert!(valid_iso_gain_map_metadata(&[0, 0, 0, 0], false));
        assert!(!valid_iso_gain_map_metadata(&[0, 0, 0, 0, 0], false));
        assert!(!valid_iso_gain_map_metadata(&[0, 0, 0, 1], false));

        let mut single_channel = vec![0; 61];
        for denominator in [9, 17, 25, 33, 41, 49, 57] {
            single_channel[denominator] = 1;
        }
        assert!(valid_iso_gain_map_metadata(&single_channel, true));
        assert!(!valid_iso_gain_map_metadata(&single_channel[..60], true));
        single_channel.push(0);
        assert!(!valid_iso_gain_map_metadata(&single_channel, true));

        let mut multi_channel = vec![0; 141];
        multi_channel[4] = 1; // Three channel pairs.
        for channel in 0..3 {
            let start = 21 + channel * 40;
            for denominator in [start + 4, start + 12, start + 20, start + 28, start + 36] {
                multi_channel[denominator] = 1;
            }
        }
        multi_channel[9] = 1;
        multi_channel[17] = 1;
        assert!(valid_iso_gain_map_metadata(&multi_channel, true));

        let mut common_denominator = vec![0; 37];
        common_denominator[4] = 0x08;
        common_denominator[8] = 1;
        assert!(valid_iso_gain_map_metadata(&common_denominator, true));
        common_denominator[8] = 0;
        assert!(!valid_iso_gain_map_metadata(&common_denominator, true));

        let mut unknown_direction = vec![0; 61];
        unknown_direction[4] = 0x04;
        assert!(!valid_iso_gain_map_metadata(&unknown_direction, true));
        let mut reserved_flag = vec![0; 61];
        reserved_flag[4] = 0x80;
        assert!(!valid_iso_gain_map_metadata(&reserved_flag, true));
    }

    #[cfg(feature = "audio")]
    fn mp3_title_tag(title: &[u8]) -> Vec<u8> {
        let mut body = b"TIT2".to_vec();
        body.extend_from_slice(&((title.len() + 1) as u32).to_be_bytes());
        body.extend_from_slice(&[0, 0, 0]);
        body.extend_from_slice(title);
        let mut tag = b"ID3\x03\0\0".to_vec();
        let size = body.len();
        tag.extend_from_slice(&[
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        tag.extend_from_slice(&body);
        tag
    }

    #[cfg(feature = "audio")]
    fn ape_text_item(key: &[u8], value: &[u8]) -> Vec<u8> {
        let mut item = Vec::new();
        item.extend_from_slice(&(value.len() as u32).to_le_bytes());
        item.extend_from_slice(&0u32.to_le_bytes());
        item.extend_from_slice(key);
        item.push(0);
        item.extend_from_slice(value);
        item
    }

    #[cfg(feature = "audio")]
    fn ape_footer(size: usize, count: u32) -> Vec<u8> {
        let mut footer = vec![0; 32];
        footer[..8].copy_from_slice(b"APETAGEX");
        footer[8..12].copy_from_slice(&2000u32.to_le_bytes());
        footer[12..16].copy_from_slice(&(size as u32).to_le_bytes());
        footer[16..20].copy_from_slice(&count.to_le_bytes());
        footer
    }

    #[cfg(feature = "audio")]
    fn ape_header(size: usize, count: u32) -> Vec<u8> {
        let mut header = ape_footer(size, count);
        header[20..24].copy_from_slice(&0xa000_0000u32.to_le_bytes());
        header
    }

    #[cfg(feature = "audio")]
    fn id3v1(title: &[u8]) -> Vec<u8> {
        let mut tag = vec![0; 128];
        tag[..3].copy_from_slice(b"TAG");
        let len = title.len().min(30);
        tag[3..3 + len].copy_from_slice(&title[..len]);
        tag
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

    #[cfg(feature = "audio")]
    #[test]
    fn mp3_copy_removes_id3v2_and_confirms_its_title_without_changing_audio() {
        let audio = [0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4];
        let mut original = mp3_title_tag(b"Sample song");
        original.extend_from_slice(&audio);

        let cleaned = clean(&original).unwrap();
        assert_eq!(cleaned.bytes, audio);
        assert_eq!(cleaned.removed[0].what, "ID3v2 metadata tags");

        let before = crate::verification::snapshot(&crate::summary::summarize(&original));
        let after = crate::verification::snapshot(&crate::summary::summarize(&cleaned.bytes));
        let report = crate::verification::compare(&before, &after);
        assert!(report.removed.iter().any(|item| item.kind == "title"));
        assert!(report.unchecked.is_empty());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn mp3_copy_removes_ape_id3v1_plus_and_id3v1_but_keeps_partial_findings_unconfirmed() {
        let audio = [0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4];
        let item = ape_text_item(b"Artist", b"Alice");
        let mut original = audio.to_vec();
        original.extend_from_slice(&item);
        original.extend_from_slice(&ape_footer(item.len() + 32, 1));
        let mut plus = vec![0; 227];
        plus[..4].copy_from_slice(b"TAG+");
        original.extend_from_slice(&plus);
        original.extend_from_slice(&id3v1(b"Old title"));

        let cleaned = clean(&original).unwrap();
        assert_eq!(cleaned.bytes, audio);
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "APE metadata tags")
        );
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "ID3v1 Plus extension")
        );
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "ID3v1 metadata tag")
        );

        let before = crate::verification::snapshot(&crate::summary::summarize(&original));
        let after = crate::verification::snapshot(&crate::summary::summarize(&cleaned.bytes));
        let report = crate::verification::compare(&before, &after);
        assert!(report.removed.is_empty());
        assert!(report.unchecked.iter().any(|item| item.kind == "artist"));
        assert!(report.unchecked.iter().any(|item| item.kind == "title"));
    }

    #[cfg(feature = "audio")]
    #[test]
    fn mp3_copy_refuses_an_ape_footer_with_inconsistent_items() {
        let mut original = vec![0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4];
        original.extend_from_slice(&ape_footer(32, 1));
        assert_eq!(clean(&original), Err(CleanError::Damaged));
    }

    #[cfg(feature = "audio")]
    #[test]
    fn mp3_copy_removes_a_consistent_ape_header_and_footer() {
        let audio = [0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4];
        let item = ape_text_item(b"Artist", b"Alice");
        let mut original = audio.to_vec();
        original.extend_from_slice(&ape_header(item.len() + 32, 1));
        original.extend_from_slice(&item);
        original.extend_from_slice(&{
            let mut footer = ape_footer(item.len() + 32, 1);
            footer[20..24].copy_from_slice(&0x8000_0000u32.to_le_bytes());
            footer
        });

        let cleaned = clean(&original).unwrap();
        assert_eq!(cleaned.bytes, audio);
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "APE metadata tags")
        );
    }

    #[cfg(feature = "audio")]
    #[test]
    fn mp3_copy_refuses_stacked_id3v2_until_both_tags_are_inspected() {
        let mut original = mp3_title_tag(b"First title");
        original.extend_from_slice(&mp3_title_tag(b"Second title"));
        original.extend_from_slice(&[0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4]);

        assert_eq!(clean(&original), Err(CleanError::Damaged));
        let doc = crate::audio::parse_mp3(&original);
        assert!(!doc.metadata_complete);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|node| { node.label == "stacked ID3v2 tag is not inspected" })
        );
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

    #[cfg(feature = "hdr")]
    #[test]
    fn an_hdr_photo_keeps_its_gain_map_and_cleans_primary_metadata() {
        let original = include_bytes!("../tests/fixtures/iphone-hdr.jpg");
        let before_doc = parse_jpeg(original);
        assert_eq!(before_doc.pictures.len(), 1);
        assert!(before_doc.facts.camera.is_some());
        assert!(before_doc.facts.owner.is_some());
        let map_before = before_doc.pictures[0];

        let cleaned = clean(original).unwrap();
        assert_eq!(
            cleaned.bytes.len(),
            original.len(),
            "MPF offsets stay fixed"
        );
        assert_eq!(
            scan(&cleaned.bytes),
            scan(original),
            "primary pixels stay fixed"
        );
        let after_doc = parse_jpeg(&cleaned.bytes);
        assert_eq!(after_doc.pictures, [map_before]);
        assert_eq!(
            scan(&cleaned.bytes[map_before.start as usize..map_before.end() as usize]),
            scan(&original[map_before.start as usize..map_before.end() as usize]),
            "gain-map pixels stay fixed"
        );
        assert_eq!(after_doc.facts.camera, None);
        assert_eq!(after_doc.facts.owner, None);
        assert_eq!(after_doc.facts.serial, None);
        assert_eq!(after_doc.facts.location, None);
        assert!(after_doc.metadata_complete);

        let picture = after_doc
            .tree
            .nodes()
            .iter()
            .find(|node| node.label == crate::jpeg::LISTED_PICTURE)
            .unwrap();
        assert_eq!(
            picture.value,
            Some(crate::model::Value::Text("HDR gain map".into()))
        );
        let map = &cleaned.bytes[map_before.start as usize..map_before.end() as usize];
        assert!(
            map.windows(b"HDRGainMapVersion".len())
                .any(|w| w == b"HDRGainMapVersion")
        );
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what.starts_with("EXIF"))
        );

        let before = crate::verification::snapshot(&crate::summary::summarize(original));
        let after = crate::verification::snapshot(&crate::summary::summarize(&cleaned.bytes));
        let report = crate::verification::compare(&before, &after);
        assert!(report.removed.iter().any(|item| item.kind == "camera"));
        assert!(report.removed.iter().any(|item| item.kind == "owner"));
        assert!(report.removed.iter().any(|item| item.kind == "location"));
        assert!(report.unchecked.is_empty());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn flac_clean_copy_removes_comments_updates_last_flag_and_keeps_audio_bytes() {
        let frames = [0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4];
        let original = flac_with_metadata(
            Some(&[b"TITLE=Song", b"LOCATION=Kyiv"]),
            &[(1, &[0, 0, 0])],
            &frames,
        );
        let before = crate::summary::summarize(&original);
        assert_eq!(before.format, "flac");
        assert!(before.complete);

        let cleaned = clean(&original).unwrap();
        assert!(cleaned.bytes.len() < original.len());
        assert!(
            cleaned.bytes.ends_with(&frames),
            "FLAC frames stay byte for byte"
        );
        assert_eq!(cleaned.removed[0].what, "FLAC Vorbis comments");

        let after = crate::audio::parse(&cleaned.bytes).unwrap();
        assert!(after.metadata_complete);
        assert!(after.facts.is_empty());
        let root = after.tree.root().unwrap();
        let padding = after
            .tree
            .get(root)
            .children
            .iter()
            .map(|&id| after.tree.get(id))
            .find(|node| node.label == "FLAC padding block")
            .unwrap();
        assert_ne!(cleaned.bytes[padding.range.start as usize] & 0x80, 0);

        let read_back = crate::summary::summarize(&cleaned.bytes);
        let report = crate::verification::compare(
            &crate::verification::snapshot(&before),
            &crate::verification::snapshot(&read_back),
        );
        assert!(report.removed.iter().any(|item| item.kind == "title"));
        assert!(report.removed.iter().any(|item| item.kind == "location"));
        assert!(report.unchecked.is_empty());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn flac_clean_copy_refuses_uninspected_blocks_and_missing_audio_frames() {
        let frames = [0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4];
        let unknown = flac_with_metadata(Some(&[b"TITLE=Song"]), &[(2, &[1, 2, 3])], &frames);
        assert_eq!(clean(&unknown), Err(CleanError::Unsupported));

        let no_frames = flac_with_metadata(Some(&[b"TITLE=Song"]), &[], &[]);
        assert_eq!(clean(&no_frames), Err(CleanError::Damaged));

        let empty_comment = flac_with_metadata(Some(&[]), &[(1, &[0, 0, 0])], &frames);
        let parsed = crate::audio::parse(&empty_comment).unwrap();
        assert!(parsed.metadata_complete);
        // Even an empty comment block is an explicitly removable tag block.
        assert!(clean(&empty_comment).is_ok());
    }

    #[cfg(feature = "audio")]
    #[test]
    fn flac_clean_copy_removes_cover_art_whole_and_supports_art_only_files() {
        let frames = [0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4];
        let picture_data = [0, 0, 0, 3, b'P', b'N', b'G', 0, 0, 0, 0, 1, 2, 3];
        let with_art = flac_with_metadata(Some(&[b"TITLE=Song"]), &[(6, &picture_data)], &frames);
        let before = crate::summary::summarize(&with_art);
        assert!(before.complete);

        let cleaned = clean(&with_art).unwrap();
        assert!(cleaned.bytes.ends_with(&frames));
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "FLAC Vorbis comments")
        );
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what == "FLAC pictures")
        );
        let after = crate::audio::parse(&cleaned.bytes).unwrap();
        assert!(after.metadata_complete);
        assert!(after.facts.is_empty());
        assert!(
            after
                .tree
                .nodes()
                .iter()
                .all(|node| node.label != "FLAC picture block")
        );

        let art_only = flac_with_metadata(None, &[(6, &picture_data)], &frames);
        assert!(crate::summary::summarize(&art_only).complete);
        let art_only_cleaned = clean(&art_only).unwrap();
        assert_eq!(art_only_cleaned.removed.len(), 1);
        assert_eq!(art_only_cleaned.removed[0].what, "FLAC pictures");
        assert!(art_only_cleaned.bytes.ends_with(&frames));
    }

    #[cfg(feature = "hdr")]
    #[test]
    fn an_hdr_copy_refuses_unknown_gain_map_xmp_without_iso_fallback() {
        let mut bytes = include_bytes!("../tests/fixtures/iphone-hdr.jpg").to_vec();
        let field = b"HDRGainMapVersion";
        let replacement = b"PrivateMapField!!";
        assert_eq!(field.len(), replacement.len());
        let at = bytes
            .windows(field.len())
            .position(|window| window == field)
            .unwrap();
        bytes[at..at + field.len()].copy_from_slice(replacement);
        assert_eq!(clean(&bytes), Err(CleanError::Unsupported));
    }

    #[test]
    fn a_photo_with_an_unrecognized_secondary_picture_is_not_blanked() {
        let mut bytes = include_bytes!("../tests/fixtures/iphone-hdr.jpg").to_vec();
        let old = b"HDRGainMap";
        let new = b"UnknownMap";
        assert_eq!(old.len(), new.len());
        let mut at = 0;
        while let Some(relative) = bytes[at..]
            .windows(old.len())
            .position(|window| window == old)
        {
            let start = at + relative;
            bytes[start..start + old.len()].copy_from_slice(new);
            at = start + new.len();
        }

        let doc = parse_jpeg(&bytes);
        assert_eq!(doc.pictures.len(), 1);
        assert_eq!(
            doc.tree
                .nodes()
                .iter()
                .find(|node| node.label == crate::jpeg::LISTED_PICTURE)
                .and_then(|node| node.value.as_ref()),
            Some(&crate::model::Value::Text("picture".into()))
        );
        assert_eq!(clean(&bytes), Err(CleanError::Unsupported));
    }

    #[cfg(feature = "hdr")]
    #[test]
    fn a_valid_iso_gain_map_allows_unknown_map_xmp_to_be_removed() {
        let mut bytes = include_bytes!("../tests/fixtures/iphone-hdr.jpg").to_vec();
        let field = b"HDRGainMapVersion";
        let replacement = b"PrivateMapField!!";
        assert_eq!(field.len(), replacement.len());
        let at = bytes
            .windows(field.len())
            .position(|window| window == field)
            .unwrap();
        bytes[at..at + field.len()].copy_from_slice(replacement);

        let mut metadata = vec![0; 61];
        for denominator in [9, 17, 25, 33, 41, 49, 57] {
            metadata[denominator] = 1;
        }
        add_iso_gain_map_segment(&mut bytes, &metadata);
        let before = crate::summary::summarize(&bytes);
        assert!(before.complete);

        let cleaned = clean(&bytes).unwrap();
        let map = parse_jpeg(&cleaned.bytes).pictures[0];
        let map_bytes = &cleaned.bytes[map.start as usize..map.end() as usize];
        assert!(
            map_bytes
                .windows(ISO_GAIN_MAP_ID.len())
                .any(|w| w == ISO_GAIN_MAP_ID)
        );
        assert!(
            !map_bytes
                .windows(replacement.len())
                .any(|w| w == replacement)
        );
        assert!(
            cleaned
                .removed
                .iter()
                .any(|item| item.what.starts_with("XMP:"))
        );
        assert!(crate::summary::summarize(&cleaned.bytes).complete);
    }

    #[test]
    fn an_editors_record_goes_with_the_rest() {
        let mut jpeg = jpeg_with_exif(None);
        let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta photoshop:City=\"Lviv\" xmpMM:PreservedFileName=\"IMG_1.CR3\"/>";
        let mut segment = vec![0xFF, 0xE1];
        segment.extend(((xmp.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(xmp);
        jpeg.splice(2..2, segment);
        let before = parse_jpeg(&jpeg).facts;
        assert_eq!(before.place.map(|f| f.text).as_deref(), Some("Lviv"));
        assert_eq!(
            before.original.map(|f| f.text).as_deref(),
            Some("IMG_1.CR3")
        );
        let cleaned = clean(&jpeg).unwrap();
        assert_eq!(parse_jpeg(&cleaned.bytes).facts, Default::default());
        assert_eq!(
            cleaned.removed[0].what,
            "XMP: editing history, author and place names"
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
    fn refuses_office_copy_when_zip_entry_headers_disagree() {
        use crate::zip::testing::{Archive, Entry, build};
        let archive = Archive {
            entries: vec![
                Entry::new("docProps/core.xml", b"<creator>private</creator>", 0),
                Entry::new("word/document.xml", b"document", 0),
            ],
            ..Default::default()
        };
        let built = build(&archive);
        assert!(clean(&built.bytes).is_ok());
        for offset in [30usize, 6, 8, 14, 18, 22] {
            let mut bytes = built.bytes.clone();
            bytes[built.local[0] as usize + offset] ^= 1;
            assert_eq!(clean(&bytes), Err(CleanError::Damaged), "offset {offset}");
        }
    }

    #[test]
    fn office_copy_accepts_zip64_and_data_descriptors() {
        use crate::zip::testing::{Archive, Descriptor, Entry, build};
        let mut properties = Entry::new("docProps/core.xml", b"<creator>private</creator>", 8);
        properties.zip64 = true;
        let mut document = Entry::new("word/document.xml", b"document", 8);
        document.descriptor = Descriptor::WithSignature;
        let built = build(&Archive {
            entries: vec![properties, document],
            ..Default::default()
        });
        assert!(clean(&built.bytes).is_ok());
    }

    #[test]
    fn a_photo_in_a_document_loses_what_it_reveals() {
        use crate::zip::testing::{Archive, Entry, build as zip};
        let photo = revealing(1);
        let text = b"<w:document><w:body/></w:document>";
        let b = zip(&Archive {
            entries: vec![
                Entry::new("word/document.xml", text, 8),
                Entry::new("word/media/image1.jpeg", &photo, 8),
                Entry::new("word/media/plain.jpeg", &jpeg_with_exif(None), 8),
            ],
            ..Default::default()
        });
        let cleaned = clean(&b.bytes).unwrap();
        let after = parse_zip(&cleaned.bytes);
        assert!(after.facts.is_empty(), "{:?}", after.facts);
        assert_eq!(problems(&after.tree), Vec::<String>::new());
        let image = extract(&cleaned.bytes, &after.entries[1], 1 << 20).unwrap();
        assert_eq!(scan(&image), scan(&photo), "picture copied byte for byte");
        assert_eq!(after.entries[1].method, 0);
        // Nothing to take out of the others: copied as they were.
        assert_eq!(after.entries[0].method, 8);
        assert_eq!(after.entries[2].method, 8);
        let what: Vec<_> = cleaned.removed.iter().map(|r| r.what.as_str()).collect();
        assert_eq!(what, ["word/media/image1.jpeg: what the photo reveals"]);
    }

    fn pngsuite(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/pngsuite/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn a_png_loses_its_notes_and_keeps_its_pixels() {
        for name in [
            "ct1n0g04.png",
            "ctzn0g04.png",
            "cten0g04.png",
            "exif2c08.png",
        ] {
            let original = pngsuite(name);
            let before = crate::png::parse_png(&original);
            assert!(
                before.facts.software.is_some()
                    || before.facts.owner.is_some()
                    || name.starts_with("exif"),
                "{name}"
            );
            let cleaned = clean(&original).unwrap();
            let after = crate::png::parse_png(&cleaned.bytes);
            assert_eq!(problems(&after.tree), Vec::<String>::new(), "{name}");
            assert_eq!(after.facts, PhotoFacts::default(), "{name}");
            assert_eq!(after.pixels, before.pixels, "{name}: pixels unchanged");
            let labels: Vec<_> = after
                .tree
                .get(0)
                .children
                .iter()
                .map(|&c| after.tree.get(c).label.as_str())
                .collect();
            assert!(
                !labels
                    .iter()
                    .any(|l| ["tEXt", "zTXt", "iTXt", "eXIf", "tIME"].contains(l)),
                "{name}: {labels:?}"
            );
        }
        let what = clean(&pngsuite("ct1n0g04.png")).unwrap().removed;
        assert_eq!(
            what[0].what,
            "text notes: Title, Author, Copyright, Description, Software, Disclaimer"
        );
    }

    #[test]
    fn a_photo_saved_as_png_loses_its_location_twice_over() {
        let original = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/photo.png"
        ))
        .unwrap();
        let before = crate::png::parse_png(&original);
        assert!(before.facts.location.is_some());
        assert_eq!(
            before.facts.camera.as_ref().unwrap().text,
            "hexscope Sample Camera X1"
        );
        let cleaned = clean(&original).unwrap();
        let what: Vec<_> = cleaned.removed.iter().map(|r| r.what.as_str()).collect();
        assert_eq!(
            what,
            [
                "EXIF: camera, time, location, serial numbers",
                "XMP: editing history, author and place names"
            ]
        );
        // The XMP packet repeats the location in words of its own.
        assert!(find(&original, b"GPSLatitude").is_some());
        assert!(find(&cleaned.bytes, b"GPSLatitude").is_none());
        assert!(find(&cleaned.bytes, b"HX-000042").is_none());
        let after = crate::png::parse_png(&cleaned.bytes);
        assert_eq!(after.facts, PhotoFacts::default());
        assert_eq!(after.pixels, before.pixels);
    }

    #[test]
    fn a_png_taken_sideways_stays_sideways() {
        let original = pngsuite("exif2c08.png");
        let mut turned = original.clone();
        // IFD0's first entry is Orientation = 1; make it 6, and fix the CRC.
        let at = find(&turned, b"\x01\x12\x00\x03\x00\x00\x00\x01\x00\x01").unwrap();
        turned[at + 9] = 6;
        let chunk = find(&turned, b"eXIf").unwrap();
        let len = u32::from_be_bytes(turned[chunk - 4..chunk].try_into().unwrap()) as usize;
        let crc = crc32(&turned[chunk..chunk + 4 + len]);
        turned[chunk + 4 + len..chunk + 8 + len].copy_from_slice(&crc.to_be_bytes());
        let cleaned = clean(&turned).unwrap();
        assert_eq!(cleaned.orientation_kept, Some(6));
        let after = crate::png::parse_png(&cleaned.bytes);
        assert_eq!(problems(&after.tree), Vec::<String>::new());
        assert_eq!(after.facts.orientation, Some(6));
        assert_eq!(after.facts.camera, None);
    }

    #[test]
    fn data_after_a_png_is_removed() {
        let mut original = pngsuite("basn2c08.png");
        original.extend_from_slice(b"PK\x03\x04 a hidden archive");
        let cleaned = clean(&original).unwrap();
        assert_eq!(cleaned.removed[0].what, "data after the end of the image");
        assert_eq!(cleaned.bytes, pngsuite("basn2c08.png"));
        assert_eq!(
            clean(&pngsuite("basn2c08.png")),
            Err(CleanError::NothingToRemove)
        );
    }

    #[test]
    fn a_heif_photo_is_cleaned_in_place() {
        for name in ["photo.heic", "photo-grid.heic", "photo.avif"] {
            let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
            let original = std::fs::read(path).unwrap();
            let cleaned = clean(&original).unwrap();
            assert_eq!(
                cleaned.bytes.len(),
                original.len(),
                "{name}: every length kept"
            );
            let doc = crate::heif::parse_heif(&cleaned.bytes);
            assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
            assert_eq!(doc.facts.camera, None, "{name}");
            assert_eq!(doc.facts.location, None, "{name}");
            assert_eq!(doc.facts.serial, None, "{name}");
            let xmp = doc.xmp.unwrap();
            let text =
                String::from_utf8_lossy(&cleaned.bytes[xmp.start as usize..xmp.end() as usize]);
            assert!(
                text.starts_with("<?xpacket begin") && text.ends_with("<?xpacket end=\"w\"?>"),
                "{name}"
            );
        }
    }

    /// A deck exported by Keynote: its speaker notes stay in the clean copy
    /// unless asked for, and then are emptied, with the deck still whole.
    #[test]
    fn keynote_notes_go_only_when_asked() {
        let deck = std::fs::read(format!(
            "{}/tests/fixtures/keynote.pptx",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let has = |b: &[u8]| {
            crate::zip::parse_zip(b)
                .facts
                .iter()
                .any(|f| f.kind == "notes")
        };
        assert!(has(&deck));
        let kept = clean(&deck);
        // Nothing else to take out of this deck: the notes are its content.
        assert!(kept.is_err() || has(&kept.unwrap().bytes));
        let gone = clean_with(
            &deck,
            CleanOptions {
                comments_and_notes: true,
            },
        )
        .unwrap();
        assert!(!has(&gone.bytes), "{:?}", gone.removed);
        assert!(
            gone.removed
                .iter()
                .any(|r| r.what.contains("speaker's notes, emptied"))
        );
        assert!(
            !crate::zip::parse_zip(&gone.bytes)
                .tree
                .nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Error)
        );
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
