use super::AudioDocument;
use super::AudioFormat;
use super::comments::{CommentContext, parse_comment_block};
use crate::model::{ByteRange, NodeKind, ParseTree};

const MAX_PACKET_SIZE: usize = 16 * 1024 * 1024;
const MAX_PAGES: usize = 4096;

#[derive(Clone, Copy)]
enum Codec {
    Vorbis,
    Opus,
}

pub(super) fn is_ogg(data: &[u8]) -> bool {
    data.starts_with(b"OggS")
}

pub(super) fn parse_ogg(data: &[u8]) -> AudioDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "Ogg media stream",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = Vec::new();
    let mut cursor = 0usize;
    let mut page_count = 0usize;
    let mut serial = None;
    let mut previous_sequence = None;
    let mut pending = false;
    let mut packet = Vec::new();
    let mut packet_start = None;
    let mut codec = None;
    let mut packet_count = 0usize;

    while cursor < data.len() {
        if page_count == MAX_PAGES {
            tree.add(
                Some(root),
                "Ogg header pages exceed the inspection limit",
                ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
                NodeKind::Field,
                None,
            );
            return audio(tree, facts, false);
        }
        page_count += 1;
        let page_start = cursor;
        if data.len() - cursor < 27 {
            tree.error(
                root,
                "truncated Ogg page header",
                ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
            );
            return audio(tree, facts, false);
        }
        if &data[cursor..cursor + 4] != b"OggS" {
            tree.error(
                root,
                "invalid Ogg page capture pattern",
                ByteRange::new(cursor as u64, 4),
            );
            return audio(tree, facts, false);
        }
        let version = data[cursor + 4];
        let flags = data[cursor + 5];
        if version != 0 || flags & !0x07 != 0 {
            tree.error(
                root,
                "unsupported Ogg page version or flags",
                ByteRange::new((cursor + 4) as u64, 2),
            );
            return audio(tree, facts, false);
        }
        let stream_serial = u32::from_le_bytes(data[cursor + 14..cursor + 18].try_into().unwrap());
        let sequence = u32::from_le_bytes(data[cursor + 18..cursor + 22].try_into().unwrap());
        if let Some(expected_serial) = serial {
            if expected_serial != stream_serial {
                tree.add(
                    Some(root),
                    "multiplexed Ogg streams are not inspected",
                    ByteRange::new((cursor + 14) as u64, 4),
                    NodeKind::Field,
                    None,
                );
                return audio(tree, facts, false);
            }
            if previous_sequence.is_some_and(|prev: u32| sequence != prev.wrapping_add(1)) {
                tree.error(
                    root,
                    "Ogg page sequence is not consecutive",
                    ByteRange::new((cursor + 18) as u64, 4),
                );
                return audio(tree, facts, false);
            }
        } else if flags & 0x02 == 0 {
            tree.error(
                root,
                "first Ogg page is not marked as beginning of stream",
                ByteRange::new((cursor + 5) as u64, 1),
            );
            return audio(tree, facts, false);
        }
        serial = Some(stream_serial);
        previous_sequence = Some(sequence);
        if (flags & 0x01 != 0) != pending {
            tree.error(
                root,
                "Ogg continued-packet flag does not match previous page",
                ByteRange::new((cursor + 5) as u64, 1),
            );
            return audio(tree, facts, false);
        }

        let segment_count = data[cursor + 26] as usize;
        let lacing_start = cursor + 27;
        let body_start = match lacing_start.checked_add(segment_count) {
            Some(end) if end <= data.len() => end,
            _ => {
                tree.error(
                    root,
                    "truncated Ogg segment table",
                    ByteRange::new(
                        lacing_start as u64,
                        data.len().saturating_sub(lacing_start) as u64,
                    ),
                );
                return audio(tree, facts, false);
            }
        };
        let laces = &data[lacing_start..body_start];
        let body_len = match laces
            .iter()
            .try_fold(0usize, |sum, lace| sum.checked_add(*lace as usize))
        {
            Some(len) => len,
            None => {
                tree.error(
                    root,
                    "invalid Ogg page body length",
                    ByteRange::new(page_start as u64, (body_start - page_start) as u64),
                );
                return audio(tree, facts, false);
            }
        };
        let Some(page_end) = body_start.checked_add(body_len) else {
            tree.error(
                root,
                "invalid Ogg page body length",
                ByteRange::new(page_start as u64, (body_start - page_start) as u64),
            );
            return audio(tree, facts, false);
        };
        if page_end > data.len() {
            tree.error(
                root,
                "truncated Ogg page body",
                ByteRange::new(page_start as u64, (data.len() - page_start) as u64),
            );
            return audio(tree, facts, false);
        }
        let page_bytes = &data[page_start..page_end];
        let expected_crc = u32::from_le_bytes(data[cursor + 22..cursor + 26].try_into().unwrap());
        if crate::ogg_crc::checksum(page_bytes) != Some(expected_crc) {
            tree.error(
                root,
                "invalid Ogg page checksum",
                ByteRange::new((cursor + 22) as u64, 4),
            );
            return audio(tree, facts, false);
        }
        let page_node = tree.add(
            Some(root),
            format!("Ogg page {sequence}"),
            ByteRange::new(page_start as u64, (page_end - page_start) as u64),
            NodeKind::Container,
            None,
        );

        let mut body_cursor = body_start;
        for &lace in laces {
            if packet_start.is_none() {
                packet_start = Some(body_cursor);
            }
            let new_len = match packet.len().checked_add(lace as usize) {
                Some(len) => len,
                None => {
                    tree.error(
                        page_node,
                        "invalid Ogg packet length",
                        ByteRange::new(body_cursor as u64, lace as u64),
                    );
                    return audio(tree, facts, false);
                }
            };
            if new_len > MAX_PACKET_SIZE {
                tree.add(
                    Some(page_node),
                    "Ogg metadata packet exceeds the 16 MiB inspection limit",
                    ByteRange::new(packet_start.unwrap_or(body_cursor) as u64, new_len as u64),
                    NodeKind::Field,
                    None,
                );
                return audio(tree, facts, false);
            }
            packet.extend_from_slice(&data[body_cursor..body_cursor + lace as usize]);
            body_cursor += lace as usize;
            if lace < 255 {
                let start = packet_start.take().unwrap_or(body_cursor);
                let span = ByteRange::new(start as u64, body_cursor.saturating_sub(start) as u64);
                let packet_label = if packet_count == 0 {
                    "Ogg identification packet"
                } else {
                    match codec.unwrap() {
                        Codec::Vorbis => "Vorbis comment packet",
                        Codec::Opus => "Opus tags packet",
                    }
                };
                let packet_node =
                    tree.add(Some(root), packet_label, span, NodeKind::Container, None);
                if packet_count == 0 {
                    codec = identify_codec(&packet);
                    if codec.is_none() {
                        tree.add(
                            Some(packet_node),
                            "unsupported Ogg codec; metadata tags are not inspected",
                            span,
                            NodeKind::Field,
                            None,
                        );
                        return audio(tree, facts, false);
                    }
                    packet_count = 1;
                } else {
                    let (signature, framing, allow_trailing, format) = match codec.unwrap() {
                        Codec::Vorbis => (
                            b"\x03vorbis".as_slice(),
                            true,
                            false,
                            AudioFormat::OggVorbis,
                        ),
                        Codec::Opus => (b"OpusTags".as_slice(), false, true, AudioFormat::OggOpus),
                    };
                    if !packet.starts_with(signature) {
                        tree.error(packet_node, "invalid Ogg comment packet signature", span);
                        return audio(tree, facts, false);
                    }
                    let comments_offset = signature.len();
                    let comment_bytes = &packet[comments_offset..];
                    let complete = parse_comment_block(
                        comment_bytes,
                        CommentContext {
                            source_offset: start.saturating_add(comments_offset) as u64,
                            source_span: Some(span),
                            parent: packet_node,
                            framing_bit: framing,
                            allow_trailing,
                        },
                        &mut tree,
                        &mut facts,
                    );
                    if body_cursor < data.len() {
                        tree.add(
                            Some(root),
                            "remaining Ogg packets (not decoded)",
                            ByteRange::new(body_cursor as u64, (data.len() - body_cursor) as u64),
                            NodeKind::Field,
                            Some(crate::model::Value::Bytes(
                                (data.len() - body_cursor) as u64,
                            )),
                        );
                    }
                    return audio_with_format(tree, facts, complete, format);
                }
                packet.clear();
            }
        }
        if let Some(&last_lace) = laces.last() {
            pending = last_lace == 255;
        }
        cursor = page_end;
    }

    let label = if packet_count == 0 {
        "Ogg stream has no complete identification packet"
    } else if pending {
        "truncated Ogg packet continues past end of input"
    } else {
        "Ogg stream has no complete comment packet"
    };
    tree.error(
        root,
        label,
        ByteRange::new(
            packet_start.unwrap_or(cursor) as u64,
            data.len().saturating_sub(packet_start.unwrap_or(cursor)) as u64,
        ),
    );
    audio(tree, facts, false)
}

fn identify_codec(packet: &[u8]) -> Option<Codec> {
    if packet.starts_with(b"\x01vorbis") {
        let valid = packet.len() >= 30
            && u32::from_le_bytes(packet[7..11].try_into().ok()?) == 0
            && packet[11] != 0
            && u32::from_le_bytes(packet[12..16].try_into().ok()?) != 0
            && packet[29] & 1 != 0;
        return valid.then_some(Codec::Vorbis);
    }
    if packet.starts_with(b"OpusHead") {
        let valid = packet.len() >= 19 && packet[9] != 0;
        return valid.then_some(Codec::Opus);
    }
    None
}

fn audio(
    tree: ParseTree,
    facts: Vec<crate::facts::DocumentFact>,
    metadata_complete: bool,
) -> AudioDocument {
    audio_with_format(tree, facts, metadata_complete, AudioFormat::Ogg)
}

fn audio_with_format(
    tree: ParseTree,
    facts: Vec<crate::facts::DocumentFact>,
    metadata_complete: bool,
    format: AudioFormat,
) -> AudioDocument {
    AudioDocument {
        format,
        tree,
        facts,
        metadata_complete,
    }
}
