use super::AudioDocument;
use super::AudioFormat;
use super::comments::{CommentContext, parse_comment_block};
use crate::clean::{CleanError, Cleaned, Removed};
use crate::model::{ByteRange, NodeKind, ParseTree};
use std::collections::HashMap;

const MAX_PACKET_SIZE: usize = 16 * 1024 * 1024;
const MAX_PAGES: usize = 4096;
const MAX_LOGICAL_STREAMS: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
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
    packet_size: usize,
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
            packet_size: 0,
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
        let granule_position =
            u64::from_le_bytes(data[cursor + 6..cursor + 14].try_into().unwrap());
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
        let mut comment_completed_this_page = false;
        let mut setup_completed_this_page = false;
        for &lace in laces {
            let current = stream.as_mut().expect("a stream is active");
            if current.packet_count == 1
                && current.packet_start.is_none()
                && current.codec.is_some()
                && sequence != 1
            {
                tree.error(
                    page_node,
                    "Ogg comment packet does not begin on the second page",
                    ByteRange::new(body_cursor as u64, lace as u64),
                );
                return audio(tree, facts, false);
            }
            let first_media_packet = if current.codec == Some(Codec::Vorbis) {
                3
            } else {
                2
            };
            if current.packet_count >= first_media_packet {
                media_start.get_or_insert(body_cursor);
            }
            if current.packet_start.is_none() {
                current.packet_start = Some(body_cursor);
                current.packet_size = 0;
            }
            current.packet_size = match current.packet_size.checked_add(lace as usize) {
                Some(size) => size,
                None => {
                    tree.error(
                        page_node,
                        "invalid Ogg packet length",
                        ByteRange::new(body_cursor as u64, lace as u64),
                    );
                    return audio(tree, facts, false);
                }
            };
            let is_metadata = current.packet_count < 2;
            let is_vorbis_setup = current.packet_count == 2 && current.codec == Some(Codec::Vorbis);
            if is_metadata || is_vorbis_setup {
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
                if is_metadata && new_len > MAX_PACKET_SIZE {
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
                let take = if is_vorbis_setup {
                    (7usize.saturating_sub(current.packet.len())).min(lace as usize)
                } else {
                    lace as usize
                };
                current
                    .packet
                    .extend_from_slice(&data[body_cursor..body_cursor + take]);
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
                            comment_completed_this_page = true;
                        }
                    }
                    2 if current.codec == Some(Codec::Vorbis) => {
                        let setup_node = tree.add(
                            Some(page_node),
                            "Vorbis setup packet (not decoded)",
                            span,
                            NodeKind::Container,
                            None,
                        );
                        if current.packet_size < 8 || !current.packet.starts_with(b"\x05vorbis") {
                            tree.error(setup_node, "invalid Vorbis setup packet signature", span);
                            return audio(tree, facts, false);
                        }
                        setup_completed_this_page = true;
                    }
                    _ => {}
                }
                current.packet_count = current.packet_count.saturating_add(1);
                current.packet.clear();
                current.packet_size = 0;
            }
        }
        let final_header_page = comment_completed_this_page
            && stream
                .as_ref()
                .is_some_and(|s| s.codec == Some(Codec::Opus))
            || setup_completed_this_page;
        if (starts_stream || final_header_page) && granule_position != 0 {
            tree.error(
                page_node,
                "Ogg header page has an invalid granule position",
                ByteRange::new((page_start + 6) as u64, 8),
            );
            return audio(tree, facts, false);
        }
        if let Some(start) = media_start.filter(|_| final_header_page) {
            tree.error(
                page_node,
                "Ogg audio begins on the final header page",
                ByteRange::new(start as u64, (page_end - start) as u64),
            );
            return audio(tree, facts, false);
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
        if starts_stream {
            let current = stream.as_ref().expect("a stream is active");
            if current.packet_count != 1 || current.pending {
                tree.error(
                    page_node,
                    "Ogg identification packet is not complete and alone on its start page",
                    ByteRange::new(page_start as u64, (page_end - page_start) as u64),
                );
                return audio(tree, facts, false);
            }
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

#[derive(Clone, Copy)]
enum CommentCodec {
    Vorbis,
    Opus,
}

struct CommentPacket {
    codec: CommentCodec,
}

struct RewriteStream {
    serial: u32,
    packet: u8,
    replacing_comment: bool,
    comment_bytes: usize,
    comment_codec: Option<CommentCodec>,
    output_pending: bool,
}

/// Replaces each fully inspected Vorbis/Opus comment packet with an empty one.
/// The audio packets remain byte-for-byte intact; pages and checksums are rebuilt.
/// The result is parsed again before it is returned.
pub(crate) fn clean_ogg(data: &[u8], doc: &AudioDocument) -> Result<Cleaned, CleanError> {
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

    let mut comments = HashMap::new();
    let mut vorbis_comment_packets = 0usize;
    let mut vorbis_setup_packets = 0usize;
    for node in doc.tree.nodes() {
        let codec = match node.label.as_str() {
            "Vorbis comment packet" => {
                vorbis_comment_packets += 1;
                CommentCodec::Vorbis
            }
            "Opus tags packet" => CommentCodec::Opus,
            "Vorbis setup packet (not decoded)" => {
                vorbis_setup_packets += 1;
                continue;
            }
            _ => continue,
        };
        let start = usize::try_from(node.range.start).map_err(|_| CleanError::Damaged)?;
        let end = usize::try_from(node.range.end()).map_err(|_| CleanError::Damaged)?;
        if end > data.len()
            || start >= end
            || comments.insert(start, CommentPacket { codec }).is_some()
        {
            return Err(CleanError::Damaged);
        }
    }
    if comments.is_empty() {
        return Err(CleanError::Unsupported);
    }
    if vorbis_comment_packets != vorbis_setup_packets {
        return Err(CleanError::Unsupported);
    }

    let mut removed_vorbis = 0u64;
    let mut removed_opus = 0u64;
    let mut output = Vec::with_capacity(data.len());
    let mut cursor = 0usize;
    let mut stream: Option<RewriteStream> = None;
    let mut next_sequence = HashMap::<u32, u32>::new();

    while cursor < data.len() {
        let page_start = cursor;
        if data.len().saturating_sub(cursor) < 27 || data.get(cursor..cursor + 4) != Some(b"OggS") {
            return Err(CleanError::Damaged);
        }
        let flags = data[cursor + 5];
        let input_sequence = u32::from_le_bytes(
            data[cursor + 18..cursor + 22]
                .try_into()
                .map_err(|_| CleanError::Damaged)?,
        );
        let serial = u32::from_le_bytes(
            data[cursor + 14..cursor + 18]
                .try_into()
                .map_err(|_| CleanError::Damaged)?,
        );
        let starts_stream = flags & 0x02 != 0;
        if starts_stream {
            if stream.is_some() {
                return Err(CleanError::Damaged);
            }
            stream = Some(RewriteStream {
                serial,
                packet: 0,
                replacing_comment: false,
                comment_bytes: 0,
                comment_codec: None,
                output_pending: false,
            });
        }
        let current = stream.as_mut().ok_or(CleanError::Damaged)?;
        if current.serial != serial {
            return Err(CleanError::Unsupported);
        }
        let continued_from_previous = current.output_pending;

        let segment_count = data[cursor + 26] as usize;
        let lacing_start = cursor.checked_add(27).ok_or(CleanError::Damaged)?;
        let body_start = lacing_start
            .checked_add(segment_count)
            .filter(|&end| end <= data.len())
            .ok_or(CleanError::Damaged)?;
        let laces = data
            .get(lacing_start..body_start)
            .ok_or(CleanError::Damaged)?;
        let body_len = laces
            .iter()
            .try_fold(0usize, |sum, &lace| sum.checked_add(lace as usize))
            .ok_or(CleanError::Damaged)?;
        let page_end = body_start
            .checked_add(body_len)
            .ok_or(CleanError::Damaged)?;
        if page_end > data.len() {
            return Err(CleanError::Damaged);
        }

        let mut body_cursor = body_start;
        let mut out_laces = Vec::with_capacity(segment_count);
        let mut out_body = Vec::with_capacity(body_len);
        let mut comment_touched = false;
        let mut inserted_comment = false;
        let mut setup_completed_this_page = false;
        for &lace in laces {
            let segment_end = body_cursor
                .checked_add(lace as usize)
                .ok_or(CleanError::Damaged)?;
            let segment = data
                .get(body_cursor..segment_end)
                .ok_or(CleanError::Damaged)?;
            if current.packet == 1 {
                comment_touched = true;
                if !current.replacing_comment {
                    if input_sequence != 1 {
                        return Err(CleanError::Unsupported);
                    }
                    let packet = comments.get(&body_cursor).ok_or(CleanError::Damaged)?;
                    let empty = match packet.codec {
                        CommentCodec::Vorbis => b"\x03vorbis\0\0\0\0\0\0\0\0\x01".as_slice(),
                        CommentCodec::Opus => b"OpusTags\0\0\0\0\0\0\0\0".as_slice(),
                    };
                    out_laces.push(empty.len() as u8);
                    out_body.extend_from_slice(empty);
                    current.replacing_comment = true;
                    current.comment_bytes = 0;
                    current.comment_codec = Some(packet.codec);
                    inserted_comment = true;
                }
                current.comment_bytes = current
                    .comment_bytes
                    .checked_add(lace as usize)
                    .ok_or(CleanError::Damaged)?;
            } else {
                let is_audio = match current.comment_codec {
                    Some(CommentCodec::Vorbis) => current.packet >= 3,
                    Some(CommentCodec::Opus) => current.packet >= 2,
                    None => false,
                };
                if (comment_touched && is_audio) || (setup_completed_this_page && is_audio) {
                    return Err(CleanError::Unsupported);
                }
                out_laces.push(lace);
                out_body.extend_from_slice(segment);
            }
            body_cursor = segment_end;
            if lace < 255 {
                if current.packet == 1 {
                    if !current.replacing_comment {
                        return Err(CleanError::Damaged);
                    }
                    let removed = current.comment_bytes.saturating_sub(16) as u64;
                    match current.comment_codec.ok_or(CleanError::Damaged)? {
                        CommentCodec::Vorbis => removed_vorbis += removed,
                        CommentCodec::Opus => removed_opus += removed,
                    }
                    current.packet = 2;
                    current.replacing_comment = false;
                    current.comment_bytes = 0;
                } else {
                    if current
                        .comment_codec
                        .is_some_and(|codec| matches!(codec, CommentCodec::Vorbis))
                        && current.packet == 2
                    {
                        setup_completed_this_page = true;
                    }
                    current.packet = current.packet.saturating_add(1);
                }
            }
        }

        // Both supported codecs require the identification packet alone on
        // the BOS page; comments begin on the following page.
        if starts_stream && (current.packet != 1 || comment_touched) {
            return Err(CleanError::Unsupported);
        }

        let end_of_stream = flags & 0x04 != 0;
        if end_of_stream {
            match current.comment_codec.ok_or(CleanError::Unsupported)? {
                CommentCodec::Vorbis if current.packet < 3 => {
                    return Err(CleanError::Unsupported);
                }
                CommentCodec::Opus if current.packet < 2 => {
                    return Err(CleanError::Unsupported);
                }
                _ => {}
            }
        }
        let drop_page =
            out_laces.is_empty() && segment_count > 0 && !starts_stream && !end_of_stream;
        let continues_after_page = out_laces
            .last()
            .map_or(continued_from_previous, |&lace| lace == 255);
        if !drop_page {
            let mut page = Vec::with_capacity(27 + out_laces.len() + out_body.len());
            page.extend_from_slice(
                data.get(page_start..page_start + 27)
                    .ok_or(CleanError::Damaged)?,
            );
            page[5] = (flags & !0x01) | if continued_from_previous { 0x01 } else { 0 };
            page[26] = u8::try_from(out_laces.len()).map_err(|_| CleanError::Damaged)?;
            if inserted_comment {
                if u64::from_le_bytes(page[6..14].try_into().map_err(|_| CleanError::Damaged)?) != 0
                    && u64::from_le_bytes(page[6..14].try_into().map_err(|_| CleanError::Damaged)?)
                        != u64::MAX
                {
                    return Err(CleanError::Unsupported);
                }
                page[6..14].fill(0);
            }
            page.extend_from_slice(&out_laces);
            page.extend_from_slice(&out_body);
            let sequence = if starts_stream {
                0
            } else {
                *next_sequence.get(&serial).ok_or(CleanError::Damaged)?
            };
            page[18..22].copy_from_slice(&sequence.to_le_bytes());
            next_sequence.insert(serial, sequence.wrapping_add(1));
            page[22..26].fill(0);
            crate::ogg_crc::write_checksum(&mut page);
            output.extend_from_slice(&page);
            current.output_pending = continues_after_page;
        } else {
            // The only packet fragments removed from the output are the
            // replaced comment packet; they cannot leave an output packet open.
            current.output_pending = false;
        }

        cursor = page_end;
        if end_of_stream {
            stream = None;
        }
    }

    if stream.is_some() {
        return Err(CleanError::Damaged);
    }

    let read_back = parse_ogg(&output);
    let expected_vorbis_comments = comments
        .values()
        .filter(|packet| matches!(packet.codec, CommentCodec::Vorbis))
        .count();
    let expected_opus_comments = comments
        .values()
        .filter(|packet| matches!(packet.codec, CommentCodec::Opus))
        .count();
    let (read_back_vorbis_comments, read_back_opus_comments, read_back_vorbis_setups) = read_back
        .tree
        .nodes()
        .iter()
        .fold((0, 0, 0), |(vorbis, opus, setups), node| {
            match node.label.as_str() {
                "Vorbis comment packet" => (vorbis + 1, opus, setups),
                "Opus tags packet" => (vorbis, opus + 1, setups),
                "Vorbis setup packet (not decoded)" => (vorbis, opus, setups + 1),
                _ => (vorbis, opus, setups),
            }
        });
    if read_back.format != doc.format
        || !read_back.metadata_complete
        || !read_back.facts.is_empty()
        || read_back_vorbis_comments != expected_vorbis_comments
        || read_back_opus_comments != expected_opus_comments
        || read_back_vorbis_setups != expected_vorbis_comments
        || read_back
            .tree
            .nodes()
            .iter()
            .any(|node| node.kind == NodeKind::Error)
    {
        return Err(CleanError::Damaged);
    }

    let removed = [
        ("Ogg Vorbis comments", removed_vorbis),
        ("Ogg Opus tags", removed_opus),
    ]
    .into_iter()
    .filter(|(_, bytes)| *bytes > 0)
    .map(|(what, bytes)| Removed {
        what: what.into(),
        bytes,
    })
    .collect::<Vec<_>>();
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: output,
        removed,
        orientation_kept: None,
    })
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
