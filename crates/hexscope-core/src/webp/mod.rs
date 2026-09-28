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
    let riff = le32(data, 4).unwrap_or(0) as usize;
    field(&mut tree, header, "size", 4, 4, Value::U64(riff as u64));
    // The RIFF size counts from after itself.
    let end = (8 + riff).min(len);
    if 8 + riff > len {
        tree.warning(
            header,
            "the file is shorter than its header says",
            ByteRange::new(len as u64, 0),
        );
    } else if 8 + riff < len {
        tree.warning(
            root,
            "data after the end of the image",
            ByteRange::new(end as u64, (len - end) as u64),
        );
    }

    let mut at = 12;
    let mut count = 0;
    while at + 8 <= end && count < MAX_CHUNKS {
        count += 1;
        let fourcc = data.get(at..at + 4).unwrap_or(b"????");
        let size = le32(data, at + 4).unwrap_or(0) as usize;
        let body = at + 8;
        // Chunks are padded to an even length.
        let padded = size + (size & 1);
        let label: String = String::from_utf8_lossy(fourcc).trim_end().to_string();
        let label = if label.chars().all(|c| c.is_ascii_graphic()) && !label.is_empty() {
            label
        } else {
            format!(
                "chunk 0x{:08X}",
                u32::from_be_bytes([fourcc[0], fourcc[1], fourcc[2], fourcc[3]])
            )
        };
        let stop = (body + padded).min(len);
        let node = tree.add(
            Some(root),
            label.clone(),
            ByteRange::new(at as u64, (stop - at) as u64),
            NodeKind::Container,
            None,
        );
        field(&mut tree, node, "size", at + 4, 4, Value::U64(size as u64));
        if body + size > len {
            tree.error(
                node,
                "chunk runs past the end of the file",
                ByteRange::new(len as u64, 0),
            );
            break;
        }
        let payload = data.get(body..body + size).unwrap_or(&[]);
        match label.as_str() {
            "VP8X" => {
                let flags = payload.first().copied().unwrap_or(0);
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
                if let (Some(w), Some(h)) = (le24(data, body + 4), le24(data, body + 7)) {
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
            "ICCP" => field(
                &mut tree,
                node,
                "profile",
                body,
                size,
                Value::U64(size as u64),
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
        at = body + padded;
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
    fn the_clean_copy_drops_exif_and_xmp_and_their_flags() {
        let vp8x = [0x04 | 0x08 | 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let file = webp(&[
            (b"VP8X", &vp8x),
            (b"VP8L", &[0x2F, 0, 0, 0, 0]),
            (b"EXIF", b"MM\0*\0\0\0\x08\0\0"),
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
