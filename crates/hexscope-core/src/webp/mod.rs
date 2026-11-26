//! WebP: a RIFF file of chunks — the picture in VP8 or VP8L, and around it,
//! when the extended header says so, a colour profile, animation frames,
//! EXIF and XMP (Google's WebP container specification). The EXIF and XMP
//! are read by the same code as a JPEG's.

pub(crate) mod docs;

use crate::exif::{PhotoFacts, parse_tiff};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// Chunks read, at most: an animation has one per frame.
const MAX_CHUNKS: usize = 10_000;

#[derive(Debug)]
pub struct WebpDocument {
    pub tree: ParseTree,
    pub facts: PhotoFacts,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub animated: bool,
}

pub fn is_webp(data: &[u8]) -> bool {
    data.get(..4) == Some(b"RIFF") && data.get(8..12) == Some(b"WEBP")
}

fn le32(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn le24(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 3)
        .map(|b| u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
}

fn field(tree: &mut ParseTree, parent: NodeId, label: &str, at: usize, len: usize, value: Value) {
    tree.add(
        Some(parent),
        label,
        ByteRange::new(at as u64, len as u64),
        NodeKind::Field,
        Some(value),
    );
}

pub fn parse_webp(data: &[u8]) -> WebpDocument {
    let mut tree = ParseTree::new();
    let len = data.len();
    let root = tree.add(
        None,
        "WebP",
        ByteRange::new(0, len as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = PhotoFacts::default();
    let (mut width, mut height, mut animated) = (None, None, false);

    let header = tree.add(
        Some(root),
        "RIFF header",
        ByteRange::new(0, len.min(12) as u64),
        NodeKind::Container,
        None,
    );
    // Calculate declared lengths in u64, including on wasm32.
    let riff = u64::from(le32(data, 4).unwrap_or(0));
    if len >= 8 {
        field(&mut tree, header, "size", 4, 4, Value::U64(riff));
    }
    let declared_end = 8 + riff;
    let end = declared_end.min(len as u64) as usize;
    if len < 12 || riff < 4 {
        tree.error(
            header,
            "incomplete RIFF WebP header",
            ByteRange::new(0, len.min(12) as u64),
        );
    } else if declared_end > len as u64 {
        tree.error(
            header,
            "the file is shorter than its header says",
            ByteRange::new(len as u64, 0),
        );
    } else if declared_end < len as u64 {
        tree.warning(
            root,
            "data after the end of the image",
            ByteRange::new(end as u64, (len - end) as u64),
        );
    }

    let mut at = 12;
    let mut count = 0;
    while end.saturating_sub(at) >= 8 && count < MAX_CHUNKS {
        count += 1;
        let fourcc = data.get(at..at + 4).unwrap_or(b"????");
        let size = u64::from(le32(data, at + 4).unwrap_or(0));
        let body = at + 8;
        // Check the payload and its padding against the RIFF boundary, not
        // the physical file length. Bytes after RIFF are not chunk data.
        let padded = size + (size & 1);
        let remaining = (end - body) as u64;
        let label: String = String::from_utf8_lossy(fourcc).trim_end().to_string();
        let label = if label.chars().all(|c| c.is_ascii_graphic()) && !label.is_empty() {
            label
        } else {
            format!(
                "chunk 0x{:08X}",
                u32::from_be_bytes([fourcc[0], fourcc[1], fourcc[2], fourcc[3]])
            )
        };
        let stop = body + padded.min(remaining) as usize;
        let node = tree.add(
            Some(root),
            label.clone(),
            ByteRange::new(at as u64, (stop - at) as u64),
            NodeKind::Container,
            None,
        );
        field(&mut tree, node, "size", at + 4, 4, Value::U64(size));
        if padded > remaining {
            tree.error(
                node,
                if size > remaining {
                    "chunk runs past the end of RIFF"
                } else {
                    "chunk is missing its RIFF padding byte"
                },
                ByteRange::new(end as u64, 0),
            );
            at = end;
            break;
        }
        let size = size as usize;
        let payload = &data[body..body + size];
        match label.as_str() {
            "VP8X" if payload.len() != 10 => {
                tree.error(
                    node,
                    "VP8X chunk must contain exactly 10 bytes",
                    ByteRange::new(body as u64, size as u64),
                );
            }
            "VP8X" => {
                let flags = payload[0];
                let names: Vec<&str> = [
                    (0x20, "colour profile"),
                    (0x10, "transparency"),
                    (0x08, "EXIF"),
                    (0x04, "XMP"),
                    (0x02, "animation"),
                ]
                .iter()
                .filter(|(bit, _)| flags & bit != 0)
                .map(|(_, n)| *n)
                .collect();
                animated = flags & 0x02 != 0;
                field(
                    &mut tree,
                    node,
                    "flags",
                    body,
                    1,
                    Value::Text(if names.is_empty() {
                        "none".into()
                    } else {
                        names.join(", ")
                    }),
                );
                if let (Some(w), Some(h)) = (le24(payload, 4), le24(payload, 7)) {
                    width = Some(w + 1);
                    height = Some(h + 1);
                    field(
                        &mut tree,
                        node,
                        "canvas width",
                        body + 4,
                        3,
                        Value::U64(u64::from(w) + 1),
                    );
                    field(
                        &mut tree,
                        node,
                        "canvas height",
                        body + 7,
                        3,
                        Value::U64(u64::from(h) + 1),
                    );
                }
            }
            "VP8" => {
                // A key frame's start code, then 14 bits each of width and height.
                if payload.get(3..6) == Some(&[0x9D, 0x01, 0x2A]) {
                    let w = payload
                        .get(6..8)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]) & 0x3FFF);
                    let h = payload
                        .get(8..10)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]) & 0x3FFF);
                    width = width.or(w.map(u32::from));
                    height = height.or(h.map(u32::from));
                }
                field(
                    &mut tree,
                    node,
                    "picture",
                    body,
                    size,
                    Value::Text("lossy, VP8".into()),
                );
            }
            "VP8L" => {
                if payload.first() == Some(&0x2F)
                    && let Some(bits) = le32(payload, 1)
                {
                    width = width.or(Some((bits & 0x3FFF) + 1));
                    height = height.or(Some(((bits >> 14) & 0x3FFF) + 1));
                }
                field(
                    &mut tree,
                    node,
                    "picture",
                    body,
                    size,
                    Value::Text("lossless, VP8L".into()),
                );
            }
            "EXIF" => {
                // Some writers keep JPEG's "Exif\0\0" in front of the TIFF block.
                let skip = if payload.starts_with(b"Exif\0\0") {
                    6
                } else {
                    0
                };
                let tiff = payload.get(skip..).unwrap_or(&[]);
                let found = parse_tiff(&mut tree, node, tiff, (body + skip) as u64);
                facts.fill_from(found);
            }
            "XMP" => {
                let text = String::from_utf8_lossy(payload);
                field(
                    &mut tree,
                    node,
                    "packet",
                    body,
                    size,
                    Value::Text(text.chars().take(300).collect()),
                );
                facts.fill_from(crate::exif::xmp::from_xmp(&text, node));
            }
            "C2PA" => facts.fill_from(crate::exif::c2pa::from_manifest(payload, node)),
            "ICCP" => crate::icc::add_profile(
                &mut tree,
                node,
                payload,
                ByteRange::new(body as u64, size as u64),
                &mut |offset, len| ByteRange::new(body as u64 + offset, len),
            ),
            "ANIM" => {
                if let Some(loops) = payload.get(4..6) {
                    let n = u16::from_le_bytes([loops[0], loops[1]]);
                    field(
                        &mut tree,
                        node,
                        "loops",
                        body + 4,
                        2,
                        Value::Text(if n == 0 {
                            "forever".into()
                        } else {
                            n.to_string()
                        }),
                    );
                }
            }
            "ANMF" => {
                animated = true;
                field(
                    &mut tree,
                    node,
                    "frame",
                    body,
                    size,
                    Value::U64(size as u64),
                );
            }
            "ALPH" => field(
                &mut tree,
                node,
                "transparency",
                body,
                size,
                Value::U64(size as u64),
            ),
            _ => field(&mut tree, node, "data", body, size, Value::U64(size as u64)),
        }
        at = stop;
    }

    if at < end {
        tree.error(
            root,
            if count == MAX_CHUNKS {
                "WebP chunk count exceeds the inspection limit"
            } else {
                "incomplete RIFF chunk header"
            },
            ByteRange::new(at as u64, (end - at) as u64),
        );
    }

    let mut summary = String::from("WebP");
    if let (Some(w), Some(h)) = (width, height) {
        summary.push_str(&format!(" · {w}×{h}"));
    }
    if animated {
        summary.push_str(" · animated");
    }
    tree.set_value(root, Some(Value::Text(summary)));
    WebpDocument {
        tree,
        facts,
        width,
        height,
        animated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WebP of chunks given as they are, with the RIFF size right.
    pub(crate) fn webp(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for (cc, data) in chunks {
            body.extend_from_slice(*cc);
            body.extend_from_slice(&(data.len() as u32).to_le_bytes());
            body.extend_from_slice(data);
            if data.len() % 2 == 1 {
                body.push(0);
            }
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn chunks_size_and_xmp_are_read() {
        let vp8x = [0x04 | 0x08, 0, 0, 0, 99, 0, 0, 49, 0, 0];
        let xmp = br#"<x:xmpmeta><rdf:RDF><rdf:Description xmp:CreatorTool="Squoosh" xmlns:xmp="http://ns.adobe.com/xap/1.0/"/></rdf:RDF></x:xmpmeta>"#;
        let file = webp(&[
            (b"VP8X", &vp8x),
            (b"VP8L", &[0x2F, 0, 0, 0, 0]),
            (b"XMP ", xmp),
        ]);
        assert!(is_webp(&file));
        let doc = parse_webp(&file);
        assert_eq!((doc.width, doc.height), (Some(100), Some(50)));
        assert_eq!(
            doc.facts.software.map(|f| f.text).as_deref(),
            Some("Squoosh")
        );
        let labels: Vec<&str> = doc.tree.nodes().iter().map(|n| n.label.as_str()).collect();
        assert!(
            labels.contains(&"XMP") && labels.contains(&"VP8X"),
            "{labels:?}"
        );
        assert!(
            !doc.tree
                .nodes()
                .iter()
                .any(|n| n.kind != NodeKind::Container && n.kind != NodeKind::Field)
        );
    }

    #[test]
    fn damaged_webps_still_make_a_tree() {
        let mut cut = webp(&[(b"VP8 ", &[0; 20])]);
        cut.truncate(24);
        let doc = parse_webp(&cut);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.label == "the file is shorter than its header says")
        );
        for data in [
            &b"RIFF\0\0\0\0WEBP"[..],
            b"RIFF\xFF\xFF\xFF\xFFWEBPVP8X\xFF\xFF\xFF\xFF",
            b"RIFF",
        ] {
            assert!(!parse_webp(data).tree.is_empty());
        }
    }

    #[test]
    fn riff_boundary_excludes_trailing_chunk_data_and_padding() {
        for declared_size in [12u32, 13] {
            let mut file = webp(&[(b"XMP ", b"x")]);
            file[4..8].copy_from_slice(&declared_size.to_le_bytes());
            let doc = parse_webp(&file);
            assert!(doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
            assert!(!doc.tree.nodes().iter().any(|n| n.label == "packet"));
        }
    }

    #[test]
    fn oversized_riff_and_chunk_lengths_are_errors() {
        let mut file = webp(&[(b"JUNK", b"")]);
        file[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        file[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        let doc = parse_webp(&file);
        assert!(doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
        assert!(
            doc.tree
                .nodes()
                .iter()
                .all(|n| n.range.start + n.range.len <= file.len() as u64)
        );
    }

    #[test]
    fn truncated_riff_and_partial_chunk_header_are_errors() {
        for extra in 1..8 {
            let mut file = webp(&[]);
            file.extend_from_slice(&[0; 7][..extra]);
            let riff_size = (file.len() - 8) as u32;
            file[4..8].copy_from_slice(&riff_size.to_le_bytes());
            assert!(parse_webp(&file).tree.nodes().iter().any(|n| {
                n.kind == NodeKind::Error && n.label == "incomplete RIFF chunk header"
            }));
        }
        let mut file = webp(&[]);
        file[4..8].copy_from_slice(&6u32.to_le_bytes());
        assert!(parse_webp(&file).tree.nodes().iter().any(|n| {
            n.kind == NodeKind::Error && n.label == "the file is shorter than its header says"
        }));
    }

    #[test]
    fn short_vp8x_does_not_borrow_dimensions_from_the_next_chunk() {
        let file = webp(&[(b"VP8X", &[0; 4]), (b"JUNK", &[0; 10])]);
        let doc = parse_webp(&file);
        assert_eq!((doc.width, doc.height), (None, None));
        assert!(doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
        assert!(doc.tree.nodes().iter().any(|n| n.label == "JUNK"));
    }

    #[test]
    fn chunk_limit_marks_unread_riff_bytes() {
        let chunks = vec![(b"JUNK", b"".as_slice()); MAX_CHUNKS + 1];
        let file = webp(&chunks);
        assert!(parse_webp(&file).tree.nodes().iter().any(|n| {
            n.kind == NodeKind::Error && n.label == "WebP chunk count exceeds the inspection limit"
        }));
    }

    #[test]
    fn the_clean_copy_drops_exif_and_xmp_and_their_flags() {
        let vp8x = [0x04 | 0x08 | 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let file = webp(&[
            (b"VP8X", &vp8x),
            (b"VP8L", &[0x2F, 0, 0, 0, 0]),
            (b"EXIF", b"MM\0*\0\0\0\x08\0\0\0\0\0\0"),
            (b"XMP ", b"<x/>"),
        ]);
        let c = crate::clean::clean(&file).unwrap();
        let doc = parse_webp(&c.bytes);
        let labels: Vec<&str> = doc.tree.nodes().iter().map(|n| n.label.as_str()).collect();
        assert!(
            !labels.contains(&"EXIF") && !labels.contains(&"XMP"),
            "{labels:?}"
        );
        assert!(
            !doc.tree
                .nodes()
                .iter()
                .any(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        );
        // Transparency stays flagged; EXIF and XMP do not.
        assert_eq!(c.bytes[20], 0x10);
        assert!(crate::clean::clean(&c.bytes).is_err());
    }
}
