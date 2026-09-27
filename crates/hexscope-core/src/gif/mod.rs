//! GIF: a screen size and colour table, then blocks — frames, each with its
//! LZW-compressed pixels in sub-blocks, and extensions between them: timing,
//! looping, comments any program may leave, and XMP (GIF89a).

pub(crate) mod docs;

use crate::exif::PhotoFacts;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// Blocks read, at most: an animation has a few per frame.
const MAX_BLOCKS: usize = 20_000;
/// Characters of a comment shown.
const MAX_SHOWN: usize = 300;

#[derive(Debug)]
pub struct GifDocument {
    pub tree: ParseTree,
    pub facts: PhotoFacts,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frames: u32,
}

pub fn is_gif(data: &[u8]) -> bool {
    data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")
}

fn le16(data: &[u8], at: usize) -> Option<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
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

/// The sub-blocks from `at`: each a length byte and that many bytes, until a
/// zero. Returns their bytes joined and where they end, or `None` when the
/// file ends first.
fn sub_blocks(data: &[u8], mut at: usize) -> (Vec<u8>, usize, bool) {
    let mut out = Vec::new();
    loop {
        let Some(&n) = data.get(at) else {
            return (out, data.len(), false);
        };
        at += 1;
        if n == 0 {
            return (out, at, true);
        }
        let Some(chunk) = data.get(at..at + n as usize) else {
            return (out, data.len(), false);
        };
        out.extend_from_slice(chunk);
        at += n as usize;
    }
}

pub fn parse_gif(data: &[u8]) -> GifDocument {
    let mut tree = ParseTree::new();
    let len = data.len();
    let root = tree.add(
        None,
        "GIF",
        ByteRange::new(0, len as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = PhotoFacts::default();
    let version = String::from_utf8_lossy(data.get(..6).unwrap_or(&[])).to_string();
    field(
        &mut tree,
        root,
        "header",
        0,
        len.min(6),
        Value::Text(version),
    );

    let (width, height) = (le16(data, 6), le16(data, 8));
    let packed = data.get(10).copied().unwrap_or(0);
    let screen = tree.add(
        Some(root),
        "logical screen descriptor",
        ByteRange::new(6, (len.min(13).saturating_sub(6)) as u64),
        NodeKind::Container,
        None,
    );
    if let (Some(w), Some(h)) = (width, height) {
        field(&mut tree, screen, "width", 6, 2, Value::U64(u64::from(w)));
        field(&mut tree, screen, "height", 8, 2, Value::U64(u64::from(h)));
    } else {
        tree.error(
            screen,
            "the file ends before its screen size",
            ByteRange::new(len as u64, 0),
        );
    }
    let mut at = 13;
    // A colour table follows when its flag is set: 3 bytes per colour.
    let table =
        |at: usize, packed: u8, tree: &mut ParseTree, parent: NodeId, label: &str| -> usize {
            if packed & 0x80 == 0 {
                return at;
            }
            let size = 3 * (2usize << (packed & 0x07));
            let (start, end) = (at.min(len), (at + size).min(len));
            field(
                tree,
                parent,
                label,
                start,
                end - start,
                Value::U64((size / 3) as u64),
            );
            at + size
        };
    at = table(at, packed, &mut tree, root, "global colour table");

    let mut frames = 0u32;
    let mut comments: Vec<String> = Vec::new();
    let mut comment_node = None;
    let mut ended = false;
    let mut count = 0;
    while at < len && count < MAX_BLOCKS {
        count += 1;
        let start = at;
        match data.get(at).copied() {
            Some(0x21) => {
                let kind = data.get(at + 1).copied().unwrap_or(0);
                let (label, body_at) = match kind {
                    0xF9 => ("graphic control", at + 2),
                    0xFE => ("comment", at + 2),
                    0x01 => ("plain text", at + 2),
                    0xFF => ("application", at + 2),
                    _ => ("extension", at + 2),
                };
                // An application's name is its first sub-block: 8 bytes and a 3-byte code.
                let app = if kind == 0xFF {
                    data.get(at + 3..at + 14)
                        .map(|n| String::from_utf8_lossy(n).to_string())
                } else {
                    None
                };
                let (bytes, end, closed) = sub_blocks(data, body_at);
                let name = match &app {
                    Some(a) => format!("application · {}", a.trim_end_matches('\0')),
                    None => label.to_string(),
                };
                let node = tree.add(
                    Some(root),
                    name.clone(),
                    ByteRange::new(start as u64, (end - start) as u64),
                    NodeKind::Container,
                    None,
                );
                match kind {
                    0xFE => {
                        let text = String::from_utf8_lossy(&bytes).trim().to_string();
                        field(
                            &mut tree,
                            node,
                            "text",
                            body_at,
                            end.saturating_sub(body_at + 1),
                            Value::Text(text.chars().take(MAX_SHOWN).collect()),
                        );
                        if !text.is_empty() {
                            comments.push(text);
                            comment_node = comment_node.or(Some(node));
                        }
                    }
                    0xFF if app.as_deref().is_some_and(|a| a.starts_with("XMP Data")) => {
                        // XMP is written raw, not in sub-blocks; the bytes to the
                        // packet's end are the packet.
                        let raw = data.get(at + 14..end).unwrap_or(&[]);
                        let text = String::from_utf8_lossy(raw);
                        let packet = text.find("<x:xmpmeta").map_or(&text[..0], |i| {
                            let e = text[i..]
                                .find("</x:xmpmeta>")
                                .map_or(text.len(), |j| i + j + 12);
                            &text[i..e]
                        });
                        field(
                            &mut tree,
                            node,
                            "packet",
                            at + 14,
                            raw.len(),
                            Value::Text(packet.chars().take(MAX_SHOWN).collect()),
                        );
                        facts.fill_from(crate::exif::xmp::from_xmp(packet, node));
                    }
                    0xFF if app.as_deref().is_some_and(|a| {
                        a.starts_with("NETSCAPE") || a.starts_with("ANIMEXTS")
                    }) =>
                    {
                        let loops = bytes.get(12..14).map(|b| u16::from_le_bytes([b[0], b[1]]));
                        if let Some(n) = loops {
                            field(
                                &mut tree,
                                node,
                                "loops",
                                start + 16,
                                2,
                                Value::Text(if n == 0 {
                                    "forever".into()
                                } else {
                                    n.to_string()
                                }),
                            );
                        }
                    }
                    0xF9 => {
                        if let Some(d) = bytes.get(1..3) {
                            let cs = u16::from_le_bytes([d[0], d[1]]);
                            field(
                                &mut tree,
                                node,
                                "delay",
                                at + 4,
                                2,
                                Value::Text(format!("{} ms", u32::from(cs) * 10)),
                            );
                        }
                    }
                    _ => {}
                }
                if !closed {
                    tree.error(
                        node,
                        "the file ends inside a block",
                        ByteRange::new(len as u64, 0),
                    );
                    break;
                }
                at = end;
            }
            Some(0x2C) => {
                frames += 1;
                let node = tree.add(
                    Some(root),
                    format!("frame {frames}"),
                    ByteRange::new(start as u64, 0),
                    NodeKind::Container,
                    None,
                );
                let (Some(w), Some(h), Some(&p)) =
                    (le16(data, at + 5), le16(data, at + 7), data.get(at + 9))
                else {
                    tree.error(
                        node,
                        "the file ends inside a block",
                        ByteRange::new(len as u64, 0),
                    );
                    break;
                };
                field(
                    &mut tree,
                    node,
                    "size",
                    at + 5,
                    4,
                    Value::Text(format!("{w}×{h}")),
                );
                let mut k = table(at + 10, p, &mut tree, node, "local colour table");
                // The LZW code size, then the pixels in sub-blocks.
                let lzw = k;
                k += 1;
                let (bytes, end, closed) = sub_blocks(data, k);
                field(
                    &mut tree,
                    node,
                    "pixels",
                    lzw,
                    end.saturating_sub(lzw),
                    Value::U64(bytes.len() as u64),
                );
                tree.set_range(
                    node,
                    ByteRange::new(start as u64, (end.min(len) - start) as u64),
                );
                if !closed {
                    tree.error(
                        node,
                        "the file ends inside a block",
                        ByteRange::new(len as u64, 0),
                    );
                    break;
                }
                at = end;
            }
            Some(0x3B) => {
                field(
                    &mut tree,
                    root,
                    "trailer",
                    at,
                    1,
                    Value::Text("end of the GIF".into()),
                );
                at += 1;
                ended = true;
                break;
            }
            Some(b) => {
                tree.error(
                    root,
                    format!("unknown block 0x{b:02X}"),
                    ByteRange::new(at as u64, 1),
                );
                break;
            }
            None => break,
        }
    }
    if ended && at < len {
        tree.warning(
            root,
            "data after the end of the image",
            ByteRange::new(at as u64, (len - at) as u64),
        );
    } else if !ended && !tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        tree.error(
            root,
            "no trailer: the file was cut short",
            ByteRange::new(len as u64, 0),
        );
    }
    if let Some(node) = comment_node {
        facts.caption = facts.caption.take().or(Some(crate::exif::Fact {
            text: comments.join(" · ").chars().take(MAX_SHOWN).collect(),
            node,
        }));
    }
    let mut summary = String::from("GIF");
    if let (Some(w), Some(h)) = (width, height) {
        summary.push_str(&format!(" · {w}×{h}"));
    }
    if frames > 1 {
        summary.push_str(&format!(" · {frames} frames"));
    }
    tree.set_value(root, Some(Value::Text(summary)));
    GifDocument {
        tree,
        facts,
        width: width.map(u32::from),
        height: height.map(u32::from),
        frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1×1 GIF with a comment and one frame.
    pub(crate) fn gif(comment: &[u8]) -> Vec<u8> {
        let mut g = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xFF\xFF\xFF".to_vec();
        g.extend_from_slice(&[0x21, 0xFE, comment.len() as u8]);
        g.extend_from_slice(comment);
        g.push(0);
        g.extend_from_slice(b"\x2C\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3B");
        g
    }

    #[test]
    fn a_comment_is_read_and_the_frame_found() {
        let doc = parse_gif(&gif(b"Made by Olena on her laptop"));
        assert_eq!(doc.frames, 1);
        assert_eq!((doc.width, doc.height), (Some(1), Some(1)));
        assert_eq!(
            doc.facts.caption.map(|f| f.text).as_deref(),
            Some("Made by Olena on her laptop")
        );
        assert!(
            !doc.tree
                .nodes()
                .iter()
                .any(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        );
    }

    #[test]
    fn damaged_gifs_still_make_a_tree() {
        let mut cut = gif(b"x");
        cut.truncate(cut.len() - 4);
        assert!(
            parse_gif(&cut)
                .tree
                .nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Error)
        );
        for data in [
            &b"GIF89a"[..],
            b"GIF87a\x01\x00\x01\x00\xF7",
            b"GIF89a\x01\x00\x01\x00\x00\x00\x00\x21\xFF\x0B",
        ] {
            assert!(!parse_gif(data).tree.is_empty());
        }
    }

    #[test]
    fn the_clean_copy_drops_comments_and_keeps_the_frames() {
        let file = gif(b"Made by Olena on her laptop");
        let c = crate::clean::clean(&file).unwrap();
        let doc = parse_gif(&c.bytes);
        assert_eq!(doc.frames, 1);
        assert!(doc.facts.caption.is_none());
        assert!(
            !doc.tree
                .nodes()
                .iter()
                .any(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        );
        assert_eq!(c.bytes.len(), file.len() - 31);
    }
}
