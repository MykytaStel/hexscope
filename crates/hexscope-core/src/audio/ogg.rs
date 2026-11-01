use super::AudioDocument;
use super::AudioFormat;
use super::comments::{CommentContext, parse_comment_block};
use crate::model::{ByteRange, NodeKind, ParseTree};

const MAX_PACKET_SIZE: usize = 16 * 1024 * 1024;
const MAX_PAGES: usize = 4096;
const MAX_LOGICAL_STREAMS: usize = 64;

#[derive(Clone, Copy)]
enum Codec {
    Vorbis,
    Opus,
}

struct LogicalStream {
    serial: u32,
    previous_sequence: Option<u32>,
    pending: bool,
    packet: Vec<u8>,
    packet_start: Option<usize>,
    packet_count: u8,
    codec: Option<Codec>,
    comments_read: bool,
    eos: bool,
}

impl LogicalStream {
    fn new(serial: u32) -> Self {
        Self {
            serial,
            previous_sequence: None,
            pending: false,
            packet: Vec::new(),
            packet_start: None,
            packet_count: 0,
            codec: None,
            comments_read: false,
            eos: false,
        }
    }
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
    let mut stream: Option<LogicalStream> = None;
    let mut stream_count = 0usize;
    let mut formats = Vec::new();
    let mut metadata_complete = true;

    while cursor < data.len() {
        if page_count == MAX_PAGES {
            tree.add(
                Some(root),
                "Ogg page count exceeds the inspection limit",
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
        let starts_stream = flags & 0x02 != 0;
        let starts_new_stream = match stream.as_ref() {
            None => true,
            Some(current) if current.eos => true,
            Some(current) if current.serial != stream_serial => {
                tree.add(
                    Some(root),
                    "multiplexed Ogg streams are not inspected",
                    ByteRange::new((cursor + 14) as u64, 4),
                    NodeKind::Field,
                    None,
                );
                return audio(tree, facts, false);
            }
            Some(_) => false,
        };
        if starts_new_stream {
            if stream.as_ref().is_some_and(|current| !current.eos) || !starts_stream {
                tree.error(
                    root,
                    "Ogg logical stream does not begin with a beginning-of-stream page",
                    ByteRange::new((cursor + 5) as u64, 1),
                );
                return audio(tree, facts, false);
            }
            if stream_count == MAX_LOGICAL_STREAMS {
                tree.add(
                    Some(root),
                    "Ogg logical stream count exceeds the inspection limit",
                    ByteRange::new(cursor as u64, (data.len() - cursor) as u64),
                    NodeKind::Field,
                    None,
                );
                return audio(tree, facts, false);
            }
            stream_count += 1;
            stream = Some(LogicalStream::new(stream_serial));
        } else if starts_stream {
            tree.error(
                root,
                "Ogg page repeats the beginning-of-stream flag",
                ByteRange::new((cursor + 5) as u64, 1),
            );
            return audio(tree, facts, false);
        }
        let current = stream.as_mut().expect("a stream is initialized above");
        if sequence
            != current
                .previous_sequence
                .map_or(0, |prev| prev.wrapping_add(1))
        {
            tree.error(
                root,
                "Ogg page sequence is not consecutive",
                ByteRange::new((cursor + 18) as u64, 4),
            );
            return audio(tree, facts, false);
        }
        current.previous_sequence = Some(sequence);
        if (flags & 0x01 != 0) != current.pending {
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
        let mut media_start = None;
        for &lace in laces {
            let current = stream.as_mut().expect("a stream is active");
            if current.packet_count >= 2 {
                media_start.get_or_insert(body_cursor);
            }
            if current.packet_start.is_none() {
                current.packet_start = Some(body_cursor);
            }
            if current.packet_count < 2 {
                let new_len = match current.packet.len().checked_add(lace as usize) {
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
                        ByteRange::new(
                            current.packet_start.unwrap_or(body_cursor) as u64,
                            new_len as u64,
                        ),
                        NodeKind::Field,
                        None,
                    );
                    return audio(tree, facts, false);
                }
                current
                    .packet
                    .extend_from_slice(&data[body_cursor..body_cursor + lace as usize]);
            }
            body_cursor += lace as usize;
            if lace < 255 {
                let current = stream.as_mut().expect("a stream is active");
                let start = current.packet_start.take().unwrap_or(body_cursor);
                let span = ByteRange::new(start as u64, body_cursor.saturating_sub(start) as u64);
                match current.packet_count {
                    0 => {
                        let packet_node = tree.add(
                            Some(page_node),
                            "Ogg identification packet",
                            span,
                            NodeKind::Container,
                            None,
                        );
                        current.codec = identify_codec(&current.packet);
                        let stream_format = match current.codec {
                            Some(Codec::Vorbis) => AudioFormat::OggVorbis,
                            Some(Codec::Opus) => AudioFormat::OggOpus,
                            None => AudioFormat::Ogg,
                        };
                        formats.push(stream_format);
                        if current.codec.is_none() {
                            tree.add(
                                Some(packet_node),
                                "unsupported Ogg codec; metadata tags are not inspected",
                                span,
                                NodeKind::Field,
                                None,
                            );
                            metadata_complete = false;
                        }
                    }
                    1 => {
                        if let Some(codec) = current.codec {
                            let (packet_label, signature, framing, allow_trailing) = match codec {
                                Codec::Vorbis => (
                                    "Vorbis comment packet",
                                    b"\x03vorbis".as_slice(),
                                    true,
                                    false,
                                ),
                                Codec::Opus => {
                                    ("Opus tags packet", b"OpusTags".as_slice(), false, true)
                                }
                            };
                            let packet_node = tree.add(
                                Some(page_node),
                                packet_label,
                                span,
                                NodeKind::Container,
                                None,
                            );
                            if !current.packet.starts_with(signature) {
                                tree.error(
                                    packet_node,
                                    "invalid Ogg comment packet signature",
                                    span,
                                );
                                return audio(tree, facts, false);
                            }
                            let comments_offset = signature.len();
                            let complete = parse_comment_block(
                                &current.packet[comments_offset..],
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
                            metadata_complete &= complete;
                            current.comments_read = true;
                        }
                    }
                    _ => {}
                }
                current.packet_count = current.packet_count.saturating_add(1);
                current.packet.clear();
            }
        }
        if let Some(media_start) = media_start
            && media_start < page_end
        {
            tree.add(
                Some(page_node),
                "remaining Ogg packets (not decoded)",
                ByteRange::new(media_start as u64, (page_end - media_start) as u64),
                NodeKind::Field,
                Some(crate::model::Value::Bytes((page_end - media_start) as u64)),
            );
        }
        if let Some(&last_lace) = laces.last() {
            stream.as_mut().expect("a stream is active").pending = last_lace == 255;
        }
        if flags & 0x04 != 0 {
            let current = stream.as_mut().expect("a stream is active");
            if current.pending {
                tree.error(
                    page_node,
                    "Ogg stream ends with an incomplete packet",
                    ByteRange::new(page_start as u64, (page_end - page_start) as u64),
                );
                return audio(tree, facts, false);
            }
            if current.codec.is_some() && !current.comments_read {
                tree.error(
                    page_node,
                    "Ogg stream has no complete comment packet",
                    ByteRange::new(page_start as u64, (page_end - page_start) as u64),
                );
                return audio(tree, facts, false);
            }
            current.eos = true;
        }
        cursor = page_end;
    }

    let Some(last_stream) = stream else {
        tree.error(
            root,
            "Ogg stream has no pages",
            ByteRange::new(0, data.len() as u64),
        );
        return audio(tree, facts, false);
    };
    if last_stream.pending {
        tree.error(
            root,
            "truncated Ogg packet continues past end of input",
            ByteRange::new(
                last_stream.packet_start.unwrap_or(cursor) as u64,
                data.len()
                    .saturating_sub(last_stream.packet_start.unwrap_or(cursor))
                    as u64,
            ),
        );
        return audio(tree, facts, false);
    }
    if !last_stream.eos {
        tree.add(
            Some(root),
            "Ogg stream has no end-of-stream page",
            ByteRange::new(cursor as u64, 0),
            NodeKind::Field,
            None,
        );
        metadata_complete = false;
    }
    if formats.is_empty() {
        metadata_complete = false;
    }
    let format = formats
        .first()
        .copied()
        .filter(|first| formats.iter().all(|format| format == first))
        .unwrap_or(AudioFormat::Ogg);
    audio_with_format(tree, facts, metadata_complete, format)
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
