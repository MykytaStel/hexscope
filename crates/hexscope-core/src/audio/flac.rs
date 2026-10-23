use super::AudioDocument;
use super::AudioFormat;
use super::comments::{CommentContext, parse_comment_block};
use crate::model::{ByteRange, NodeKind, ParseTree};

const MAX_BLOCKS: usize = 1024;
const MAX_BLOCK_SIZE: usize = 16 * 1024 * 1024;

pub(super) fn is_flac(data: &[u8]) -> bool {
    data.starts_with(b"fLaC")
}

pub(super) fn parse_flac(data: &[u8]) -> AudioDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "FLAC audio",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = Vec::new();
    let mut complete = true;
    let mut cursor = 4usize;
    let mut block_count = 0usize;
    let mut has_streaminfo = false;
    let mut has_comments = false;
    let mut saw_last = false;

    while cursor < data.len() {
        if block_count == MAX_BLOCKS {
            tree.add(
                Some(root),
                "FLAC metadata block count exceeds the inspection limit",
                ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
                NodeKind::Field,
                None,
            );
            complete = false;
            break;
        }
        if data.len() - cursor < 4 {
            tree.error(
                root,
                "truncated FLAC metadata block header",
                ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
            );
            complete = false;
            break;
        }
        block_count += 1;
        let header = data[cursor];
        let last = header & 0x80 != 0;
        let block_type = header & 0x7f;
        let len = ((data[cursor + 1] as usize) << 16)
            | ((data[cursor + 2] as usize) << 8)
            | data[cursor + 3] as usize;
        let block_start = cursor;
        let payload_start = cursor + 4;
        let Some(block_end) = payload_start.checked_add(len) else {
            tree.error(
                root,
                "invalid FLAC metadata block length",
                ByteRange::new(cursor as u64, 4),
            );
            complete = false;
            break;
        };
        if block_end > data.len() {
            tree.error(
                root,
                "truncated FLAC metadata block",
                ByteRange::new(block_start as u64, (data.len() - block_start) as u64),
            );
            complete = false;
            break;
        }
        let type_name = match block_type {
            0 => "STREAMINFO",
            1 => "padding",
            2 => "application",
            3 => "seek table",
            4 => "Vorbis comments",
            5 => "cue sheet",
            6 => "picture",
            127 => "forbidden",
            _ => "reserved",
        };
        let block_node = tree.add(
            Some(root),
            format!("FLAC {type_name} block"),
            ByteRange::new(block_start as u64, (block_end - block_start) as u64),
            NodeKind::Container,
            None,
        );
        if block_type == 127 {
            tree.error(
                block_node,
                "FLAC metadata block type 127 is forbidden",
                ByteRange::new(block_start as u64, 1),
            );
            complete = false;
            break;
        }
        if block_count == 1 {
            has_streaminfo = block_type == 0 && len == 34;
            if !has_streaminfo {
                tree.error(
                    block_node,
                    "first FLAC metadata block must be 34-byte STREAMINFO",
                    ByteRange::new(block_start as u64, (block_end - block_start) as u64),
                );
                complete = false;
                break;
            }
        } else if block_type == 0 {
            tree.error(
                block_node,
                "duplicate FLAC STREAMINFO block",
                ByteRange::new(block_start as u64, (block_end - block_start) as u64),
            );
            complete = false;
            break;
        }

        match block_type {
            4 => {
                if has_comments {
                    tree.error(
                        block_node,
                        "duplicate FLAC Vorbis comment block",
                        ByteRange::new(block_start as u64, (block_end - block_start) as u64),
                    );
                    complete = false;
                    break;
                }
                has_comments = true;
                if len > MAX_BLOCK_SIZE {
                    tree.add(
                        Some(block_node),
                        "FLAC comment block exceeds the 16 MiB inspection limit",
                        ByteRange::new(payload_start as u64, len as u64),
                        NodeKind::Field,
                        None,
                    );
                    complete = false;
                } else {
                    complete &= parse_comment_block(
                        &data[payload_start..block_end],
                        CommentContext {
                            source_offset: payload_start as u64,
                            source_span: None,
                            parent: block_node,
                            framing_bit: false,
                            allow_trailing: false,
                        },
                        &mut tree,
                        &mut facts,
                    );
                }
            }
            1 | 3 => {} // Padding and seek points do not contain user tag values.
            2 | 5 | 6 | 7..=126 => {
                // These blocks can contain application values, cue metadata,
                // artwork, or extensions; their payloads are skipped safely.
                tree.add(
                    Some(block_node),
                    "FLAC metadata block content not inspected",
                    ByteRange::new(payload_start as u64, len as u64),
                    NodeKind::Field,
                    None,
                );
                complete = false;
            }
            _ => {}
        }

        cursor = block_end;
        if last {
            saw_last = true;
            break;
        }
    }
    if !has_streaminfo {
        tree.error(
            root,
            "missing FLAC STREAMINFO block",
            ByteRange::new(4, data.len().saturating_sub(4).min(4) as u64),
        );
        complete = false;
    }
    if !saw_last {
        tree.error(
            root,
            "FLAC metadata has no last-block flag before end of input",
            ByteRange::new(cursor as u64, data.len().saturating_sub(cursor) as u64),
        );
        complete = false;
    } else if cursor < data.len() {
        tree.add(
            Some(root),
            "audio frames (not decoded)",
            ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
            NodeKind::Field,
            Some(crate::model::Value::Bytes((data.len() - cursor) as u64)),
        );
    }

    AudioDocument {
        format: AudioFormat::Flac,
        tree,
        facts,
        metadata_complete: complete,
    }
}
