use super::{AudioDocument, AudioFormat};
use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};

const MAX_TAG_SIZE: usize = 16 * 1024 * 1024;
const MAX_ID3_TAGS: usize = 16;
const MAX_FACTS: usize = 256;
const MAX_VALUE_BYTES: usize = 512;
const MAX_FRAMES: usize = 65_536;

struct FrameContext {
    version: u8,
    tag_node: u32,
}

pub fn is_mp3(data: &[u8]) -> bool {
    data.starts_with(b"ID3") || data.get(..4).is_some_and(valid_mpeg_frame_header)
}

pub(crate) fn valid_mpeg_frame_header(header: &[u8]) -> bool {
    if header.len() < 4 {
        return false;
    }
    let bits = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let sync = bits >> 21 == 0x7ff;
    let version = (bits >> 19) & 0b11;
    let layer = (bits >> 17) & 0b11;
    let bitrate = (bits >> 12) & 0b1111;
    let sample_rate = (bits >> 10) & 0b11;
    // Layer bits `01` identify Layer III; Layer I and II are MPEG audio too,
    // but are outside this MP3 tag parser's format contract.
    sync && version != 0b01 && layer == 0b01 && !matches!(bitrate, 0 | 15) && sample_rate != 0b11
}

/// Byte ranges containing leading ID3v2 and trailing APE/ID3v1 tags that can
/// be removed without decoding or rewriting any audio frame. `None` means a
/// recognized tag has an unsafe or inconsistent boundary.
pub(crate) fn removable_tag_ranges(data: &[u8]) -> Option<Vec<(ByteRange, &'static str)>> {
    let mut ranges = Vec::new();
    let mut audio_start = 0usize;
    if data.starts_with(b"ID3") {
        let mut count = 0;
        while data.get(audio_start..audio_start.checked_add(3)?) == Some(b"ID3") {
            if count == MAX_ID3_TAGS {
                return None;
            }
            audio_start = id3v2_end(data, audio_start)?;
            count += 1;
        }
        ranges.push((ByteRange::new(0, audio_start as u64), "ID3v2 metadata tags"));
    }

    let id3v1_start = if data.len() >= 128 && data[data.len() - 128..].starts_with(b"TAG") {
        data.len() - 128
    } else {
        data.len()
    };
    let id3v1_plus_start = id3v1_start
        .checked_sub(227)
        .filter(|&start| data.get(start..start + 4) == Some(b"TAG+"));
    let trailer_start = id3v1_plus_start.unwrap_or(id3v1_start);
    let mut audio_end = trailer_start;
    if let Some(footer_start) = trailer_start.checked_sub(32)
        && data.get(footer_start..footer_start + 8) == Some(b"APETAGEX")
    {
        let footer = data.get(footer_start..trailer_start)?;
        let version = u32::from_le_bytes(footer.get(8..12)?.try_into().ok()?);
        let size = u32::from_le_bytes(footer.get(12..16)?.try_into().ok()?) as usize;
        let count = u32::from_le_bytes(footer.get(16..20)?.try_into().ok()?);
        let flags = u32::from_le_bytes(footer.get(20..24)?.try_into().ok()?);
        if !matches!(version, 1000 | 2000)
            || !(32..=MAX_TAG_SIZE).contains(&size)
            || flags & 0x7fff_ffff != 0
            || footer.get(24..32)?.iter().any(|&byte| byte != 0)
        {
            return None;
        }
        let item_start = trailer_start.checked_sub(size)?;
        if item_start < audio_start {
            return None;
        }
        let has_header = flags & 0x8000_0000 != 0;
        let tag_start = if has_header {
            let header_start = item_start.checked_sub(32)?;
            let header = data.get(header_start..item_start)?;
            if header_start < audio_start
                || header.get(..8)? != b"APETAGEX"
                || u32::from_le_bytes(header.get(8..12)?.try_into().ok()?) != version
                || u32::from_le_bytes(header.get(12..16)?.try_into().ok()?) as usize != size
                || u32::from_le_bytes(header.get(16..20)?.try_into().ok()?) != count
                || u32::from_le_bytes(header.get(20..24)?.try_into().ok()?) != (flags | 0x2000_0000)
                || header.get(24..32)?.iter().any(|&byte| byte != 0)
            {
                return None;
            }
            header_start
        } else {
            item_start
        };
        validate_ape_items(data, item_start, footer_start, count as usize)?;
        ranges.push((
            ByteRange::new(tag_start as u64, (trailer_start - tag_start) as u64),
            "APE metadata tags",
        ));
        audio_end = tag_start;
    }
    if let Some(start) = id3v1_plus_start {
        ranges.push((ByteRange::new(start as u64, 227), "ID3v1 Plus extension"));
    }
    if id3v1_start < data.len() {
        ranges.push((
            ByteRange::new(id3v1_start as u64, 128),
            "ID3v1 metadata tag",
        ));
    }

    // A leading ID3 signature alone is not enough to call arbitrary bytes an
    // audio file. Require a recognizable MPEG Layer III frame before making a
    // copy, while leaving the sound stream itself untouched.
    if !ranges.is_empty() {
        let start = audio_start;
        let header = data.get(start..start.checked_add(4)?)?;
        if start >= audio_end || !valid_mpeg_frame_header(header) {
            return None;
        }
    }
    Some(ranges)
}

/// Checks the APE item table and its declared count before treating its
/// footer as a tag boundary. Values remain opaque and are never rewritten.
fn validate_ape_items(data: &[u8], mut cursor: usize, end: usize, count: usize) -> Option<()> {
    if count > MAX_FRAMES {
        return None;
    }
    for _ in 0..count {
        let header_end = cursor.checked_add(8).filter(|&next| next <= end)?;
        let length = u32::from_le_bytes(data.get(cursor..cursor + 4)?.try_into().ok()?) as usize;
        let flags = u32::from_le_bytes(data.get(cursor + 4..header_end)?.try_into().ok()?);
        let key_start = header_end;
        let key_len = data
            .get(key_start..end)?
            .iter()
            .position(|&byte| byte == 0)?;
        if !(2..=255).contains(&key_len) {
            return None;
        }
        let key_end = key_start.checked_add(key_len)?;
        if !data
            .get(key_start..key_end)?
            .iter()
            .all(|byte| (0x20..=0x7e).contains(byte))
            || flags & !7 != 0
        {
            return None;
        }
        let value_start = key_end.checked_add(1)?;
        cursor = value_start
            .checked_add(length)
            .filter(|&next| next <= end)?;
    }
    (cursor == end).then_some(())
}

pub fn parse_mp3(data: &[u8]) -> AudioDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "MP3 audio",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = Vec::new();
    let mut complete = true;
    let mut audio_start = 0usize;
    let mut can_mark_audio = true;

    let mut id3_tag_count = 0;
    let mut id3_frame_count = 0;
    while data.get(audio_start..audio_start.saturating_add(3)) == Some(b"ID3") {
        if id3_tag_count == MAX_ID3_TAGS {
            tree.add(
                Some(root),
                format!("leading ID3v2 tag count exceeds the {MAX_ID3_TAGS}-tag inspection limit"),
                ByteRange::new(audio_start as u64, 3),
                NodeKind::Field,
                None,
            );
            complete = false;
            can_mark_audio = false;
            break;
        }
        complete &= parse_id3v2(
            data,
            audio_start,
            root,
            &mut tree,
            &mut facts,
            &mut id3_frame_count,
        );
        let Some(end) = id3v2_end(data, audio_start) else {
            can_mark_audio = false;
            break;
        };
        audio_start = end;
        id3_tag_count += 1;
    }
    let id3v1_start = if data.len() >= 128 && data[data.len() - 128..].starts_with(b"TAG") {
        data.len() - 128
    } else {
        data.len()
    };
    let id3v1_plus_start = id3v1_start
        .checked_sub(227)
        .filter(|&start| data.get(start..start + 4) == Some(b"TAG+"));
    let trailer_start = id3v1_plus_start.unwrap_or(id3v1_start);
    if let Some(start) = id3v1_plus_start {
        tree.warning(
            root,
            "ID3v1 Plus extension is not inspected",
            ByteRange::new(start as u64, 227),
        );
        complete = false;
    }
    // APE tags can precede ID3v1 or end the file. Only bounded text items
    // are inspected, so never call the metadata scan complete.
    let mut audio_end = trailer_start;
    if let Some(footer_start) = trailer_start.checked_sub(32)
        && data.get(footer_start..footer_start + 8) == Some(b"APETAGEX")
    {
        let footer = &data[footer_start..footer_start + 32];
        let size = u32::from_le_bytes(footer[12..16].try_into().unwrap()) as usize;
        let tag_start = trailer_start
            .checked_sub(size)
            .filter(|&start| size >= 32 && start >= audio_start);
        if let Some(start) = tag_start {
            audio_end = start;
            read_ape_text(data, start, footer_start, root, &mut tree, &mut facts);
        }
        tree.warning(
            root,
            "APE audio metadata is partially inspected",
            ByteRange::new(
                tag_start.unwrap_or(footer_start) as u64,
                (trailer_start - tag_start.unwrap_or(footer_start)) as u64,
            ),
        );
        complete = false;
    }
    if can_mark_audio
        && audio_start < audio_end
        && data
            .get(audio_start..audio_start.saturating_add(4))
            .is_some_and(valid_mpeg_frame_header)
    {
        tree.add(
            Some(root),
            "audio frames (not decoded)",
            ByteRange::new(audio_start as u64, (audio_end - audio_start) as u64),
            NodeKind::Field,
            Some(Value::Bytes((audio_end - audio_start) as u64)),
        );
    }
    if id3v1_start < data.len() {
        parse_id3v1(data, root, &mut tree, &mut facts, &mut complete);
    }

    AudioDocument {
        format: AudioFormat::Mp3,
        tree,
        facts,
        metadata_complete: complete,
    }
}

// Read bounded text items while keeping the overall APE check incomplete:
// binary items, external references and other tag locations are not inspected.
fn read_ape_text(
    data: &[u8],
    start: usize,
    footer_start: usize,
    root: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) {
    let footer = &data[footer_start..footer_start + 32];
    let version = u32::from_le_bytes(footer[8..12].try_into().unwrap());
    let count = u32::from_le_bytes(footer[16..20].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(footer[20..24].try_into().unwrap());
    if !matches!(version, 1000 | 2000)
        || footer_start - start > MAX_TAG_SIZE
        || count > MAX_FACTS
        || flags & 0x7fff_ffff != 0
        || footer[24..32].iter().any(|&b| b != 0)
    {
        return;
    }
    let mut cursor = start;
    for _ in 0..count {
        let Some(header_end) = cursor.checked_add(8).filter(|&end| end <= footer_start) else {
            break;
        };
        let length = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        let item_flags = u32::from_le_bytes(data[cursor + 4..header_end].try_into().unwrap());
        let key_start = header_end;
        let Some(key_len) = data[key_start..footer_start].iter().position(|&b| b == 0) else {
            break;
        };
        let key_end = key_start + key_len;
        if !(2..=255).contains(&key_len)
            || !data[key_start..key_end]
                .iter()
                .all(|b| (0x20..=0x7e).contains(b))
        {
            break;
        }
        let value_start = key_end + 1;
        let Some(value_end) = value_start
            .checked_add(length)
            .filter(|&end| end <= footer_start)
        else {
            break;
        };
        cursor = value_end;
        if item_flags & !1 != 0 {
            continue;
        }
        let Ok(key) = std::str::from_utf8(&data[key_start..key_end]) else {
            continue;
        };
        let Ok(value) = std::str::from_utf8(&data[value_start..value_end]) else {
            continue;
        };
        for part in value.split('\0').filter(|part| !part.is_empty()) {
            if facts.len() >= MAX_FACTS {
                return;
            }
            let (kind, value) = super::comments::fact_value(key, part);
            let (text, _) = super::comments::truncate(&value);
            let node = tree.add(
                Some(root),
                format!("{kind}: {text}"),
                ByteRange::new(value_start as u64, length as u64),
                NodeKind::Field,
                Some(Value::Text(text.clone())),
            );
            facts.push(DocumentFact { kind, text, node });
        }
    }
}

fn id3v2_end(data: &[u8], start: usize) -> Option<usize> {
    let header_end = start.checked_add(10)?;
    if data.get(start..start.checked_add(3)?) != Some(b"ID3")
        || !data
            .get(start + 3)
            .is_some_and(|version| (2..=4).contains(version))
    {
        return None;
    }
    let size = synchsafe(data.get(start.checked_add(6)?..header_end)?)?;
    if size > MAX_TAG_SIZE {
        return None;
    }
    let end = header_end.checked_add(size)?;
    (end <= data.len()).then_some(end)
}

fn parse_id3v2(
    data: &[u8],
    start: usize,
    root: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
    frame_count: &mut usize,
) -> bool {
    let Some(header_end) = start.checked_add(10) else {
        tree.error(
            root,
            "invalid ID3 tag length",
            ByteRange::new(start as u64, 0),
        );
        return false;
    };
    if data.len() < header_end {
        tree.error(
            root,
            "truncated ID3 header",
            ByteRange::new(start as u64, data.len().saturating_sub(start) as u64),
        );
        return false;
    }
    let version = data[start + 3];
    if !(2..=4).contains(&version) {
        tree.warning(
            root,
            "unsupported ID3 version",
            ByteRange::new(start as u64, 10),
        );
        return false;
    }
    if data[start + 4] == 0xff {
        tree.warning(
            root,
            "reserved ID3 revision",
            ByteRange::new((start + 3) as u64, 2),
        );
        return false;
    }
    let Some(size) = synchsafe(data.get(start + 6..header_end).unwrap_or_default()) else {
        tree.error(
            root,
            "invalid ID3 synchsafe size",
            ByteRange::new((start + 6) as u64, 4),
        );
        return false;
    };
    if size > MAX_TAG_SIZE {
        tree.add(
            Some(root),
            "ID3 tag exceeds the 16 MiB inspection limit",
            ByteRange::new(start as u64, 10),
            NodeKind::Field,
            None,
        );
        return false;
    }
    let Some(tag_end) = header_end.checked_add(size) else {
        tree.error(
            root,
            "invalid ID3 tag length",
            ByteRange::new((start + 6) as u64, 4),
        );
        return false;
    };
    let available_end = tag_end.min(data.len());
    let tag_node = tree.add(
        Some(root),
        format!("ID3v2.{version} tag"),
        ByteRange::new(start as u64, available_end.saturating_sub(start) as u64),
        NodeKind::Container,
        None,
    );
    if tag_end > data.len() {
        tree.error(
            tag_node,
            "truncated ID3 tag body",
            ByteRange::new(
                header_end as u64,
                data.len().saturating_sub(header_end) as u64,
            ),
        );
        return false;
    }

    let flags = data[start + 5];
    let legal_flags = match version {
        2 => 0xc0,
        3 => 0xe0,
        4 => 0xf0,
        _ => unreachable!(),
    };
    if flags & !legal_flags != 0 {
        tree.error(
            tag_node,
            "reserved ID3 header flags are set",
            ByteRange::new((start + 5) as u64, 1),
        );
        return false;
    }
    // The legacy compression flag and tag-level unsynchronisation alter the
    // frame stream. Skip the whole tag rather than risk false frame offsets.
    let unsupported_transform =
        (version == 2 && flags & 0xc0 != 0) || (version >= 3 && flags & 0x80 != 0);
    if unsupported_transform {
        tree.add(
            Some(tag_node),
            "ID3 tag uses an unsupported compression or unsynchronisation transform",
            ByteRange::new((start + 5) as u64, 1),
            NodeKind::Field,
            None,
        );
        return false;
    }

    let mut frames_start = header_end;
    let mut frames_end = tag_end;
    if version == 4 && flags & 0x10 != 0 {
        if frames_end < frames_start + 10
            || data.get(frames_end - 10..frames_end - 7) != Some(b"3DI")
        {
            tree.error(
                tag_node,
                "invalid ID3v2.4 footer",
                ByteRange::new(frames_end.saturating_sub(10) as u64, 10),
            );
            return false;
        }
        frames_end -= 10;
    }
    if flags & 0x40 != 0 {
        match skip_extended_header(data, version, frames_start, frames_end) {
            Some(next) => frames_start = next,
            None => {
                tree.error(
                    tag_node,
                    "invalid or truncated ID3 extended header",
                    ByteRange::new(
                        frames_start as u64,
                        frames_end.saturating_sub(frames_start) as u64,
                    ),
                );
                return false;
            }
        }
    }

    parse_frames(
        data,
        frames_start,
        frames_end,
        FrameContext { version, tag_node },
        tree,
        facts,
        frame_count,
    )
}

fn skip_extended_header(data: &[u8], version: u8, start: usize, end: usize) -> Option<usize> {
    let size_bytes = data.get(start..start.checked_add(4)?)?;
    let size = if version == 4 {
        synchsafe(size_bytes)?
    } else {
        u32::from_be_bytes(size_bytes.try_into().ok()?) as usize
    };
    let total = if version == 3 {
        size.checked_add(4)?
    } else {
        size
    };
    let minimum = if version == 3 { 10 } else { 6 };
    if total < minimum {
        return None;
    }
    let next = start.checked_add(total)?;
    (next <= end).then_some(next)
}

fn parse_frames(
    data: &[u8],
    mut cursor: usize,
    end: usize,
    context: FrameContext,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
    frame_count: &mut usize,
) -> bool {
    let FrameContext { version, tag_node } = context;
    let mut complete = true;
    while cursor < end {
        if data[cursor] == 0 {
            if data[cursor..end].iter().any(|&byte| byte != 0) {
                tree.error(
                    tag_node,
                    "nonzero bytes after ID3 padding",
                    ByteRange::new(cursor as u64, (end - cursor) as u64),
                );
                return false;
            }
            return complete;
        }
        if *frame_count == MAX_FRAMES {
            tree.add(
                Some(tag_node),
                "ID3 frame count exceeds the inspection limit",
                ByteRange::new(cursor as u64, (end - cursor) as u64),
                NodeKind::Field,
                None,
            );
            return false;
        }
        *frame_count += 1;
        let frame_header_len = if version == 2 { 6 } else { 10 };
        if end - cursor < frame_header_len {
            tree.error(
                tag_node,
                "truncated ID3 frame header",
                ByteRange::new(cursor as u64, (end - cursor) as u64),
            );
            return false;
        }
        let frame_start = cursor;
        let id_len = if version == 2 { 3 } else { 4 };
        let id_bytes = &data[cursor..cursor + id_len];
        if !id_bytes
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            tree.error(
                tag_node,
                "invalid ID3 frame identifier",
                ByteRange::new(cursor as u64, frame_header_len as u64),
            );
            return false;
        }
        let id = std::str::from_utf8(id_bytes).unwrap_or("");
        let size_at = cursor + id_len;
        let frame_size = match version {
            2 => {
                ((data[size_at] as usize) << 16)
                    | ((data[size_at + 1] as usize) << 8)
                    | data[size_at + 2] as usize
            }
            3 => u32::from_be_bytes(data[size_at..size_at + 4].try_into().unwrap()) as usize,
            4 => match synchsafe(&data[size_at..size_at + 4]) {
                Some(size) => size,
                None => {
                    tree.error(
                        tag_node,
                        "invalid ID3 frame synchsafe size",
                        ByteRange::new(size_at as u64, 4),
                    );
                    return false;
                }
            },
            _ => unreachable!(),
        };
        let flags = if version == 2 {
            0
        } else {
            u16::from_be_bytes([data[cursor + 8], data[cursor + 9]])
        };
        let payload_start = cursor + frame_header_len;
        let Some(payload_end) = payload_start.checked_add(frame_size) else {
            tree.error(
                tag_node,
                "invalid ID3 frame length",
                ByteRange::new(cursor as u64, frame_header_len as u64),
            );
            return false;
        };
        if payload_end > end {
            tree.error(
                tag_node,
                format!("truncated ID3 {id} frame"),
                ByteRange::new(cursor as u64, (end - cursor) as u64),
            );
            return false;
        }
        let frame_node = tree.add(
            Some(tag_node),
            format!("ID3 frame {id}"),
            ByteRange::new(cursor as u64, (payload_end - cursor) as u64),
            NodeKind::Container,
            None,
        );
        cursor = payload_end;

        if has_reserved_frame_flags(version, flags) {
            tree.add(
                Some(frame_node),
                "reserved ID3 frame flags are set",
                ByteRange::new((payload_start - 2) as u64, 2),
                NodeKind::Field,
                None,
            );
            complete = false;
            continue;
        }
        if has_unsupported_frame_flags(version, flags) {
            tree.add(
                Some(frame_node),
                "frame transform not inspected",
                ByteRange::new((payload_start - 2) as u64, 2),
                NodeKind::Field,
                None,
            );
            complete = false;
            continue;
        }
        if id.starts_with('T') && id != "TXXX" && id != "TXX" {
            let Some((&encoding, raw)) = data
                .get(payload_start..payload_end)
                .and_then(|s| s.split_first())
            else {
                tree.error(
                    frame_node,
                    "ID3 text frame has no encoding byte",
                    ByteRange::new(payload_start as u64, frame_size as u64),
                );
                complete = false;
                continue;
            };
            let values = match decode_values(raw, encoding, MAX_FACTS.saturating_sub(facts.len())) {
                Ok(decoded) => decoded,
                Err(()) => {
                    tree.error(
                        frame_node,
                        "invalid ID3 text encoding",
                        ByteRange::new(payload_start as u64, frame_size as u64),
                    );
                    complete = false;
                    continue;
                }
            };
            for value in values.values {
                if value.is_empty() {
                    continue;
                }
                if facts.len() == MAX_FACTS {
                    tree.add(
                        Some(frame_node),
                        "metadata fact count exceeds the 256-item inspection limit",
                        ByteRange::new(payload_start as u64, frame_size as u64),
                        NodeKind::Field,
                        None,
                    );
                    complete = false;
                    continue;
                }
                let (kind, text) = text_fact(id, value);
                let (text, display_truncated) = truncate_display(&text);
                let node = tree.add(
                    Some(frame_node),
                    format!("{kind}: {text}"),
                    ByteRange::new(payload_start as u64, frame_size as u64),
                    NodeKind::Field,
                    Some(Value::Text(text.clone())),
                );
                facts.push(DocumentFact { kind, text, node });
                if display_truncated {
                    tree.add(
                        Some(frame_node),
                        "text value truncated at the 512-byte display limit",
                        ByteRange::new(payload_start as u64, frame_size as u64),
                        NodeKind::Field,
                        None,
                    );
                    complete = false;
                }
            }
            if values.truncated {
                tree.add(
                    Some(frame_node),
                    "text value truncated at the 512-byte display limit",
                    ByteRange::new(payload_start as u64, frame_size as u64),
                    NodeKind::Field,
                    None,
                );
                complete = false;
            }
        } else if id == "COMM" || id == "COM" {
            complete &= parse_comment(data, payload_start, payload_end, frame_node, tree, facts);
        } else {
            tree.add(
                Some(frame_node),
                format!("ID3 frame {id} not inspected"),
                ByteRange::new(frame_start as u64, (payload_end - frame_start) as u64),
                NodeKind::Field,
                None,
            );
            complete = false;
        }
    }
    complete
}

fn parse_comment(
    data: &[u8],
    start: usize,
    end: usize,
    frame_node: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    if end - start < 4 {
        tree.error(
            frame_node,
            "truncated ID3 comment frame",
            ByteRange::new(start as u64, (end - start) as u64),
        );
        return false;
    }
    let encoding = data[start];
    let description_start = start + 4; // encoding byte and three-byte language
    let Some((description_end, comment_start)) =
        split_terminated(data, description_start, end, encoding)
    else {
        tree.error(
            frame_node,
            "unterminated ID3 comment description",
            ByteRange::new(description_start as u64, (end - description_start) as u64),
        );
        return false;
    };
    let description = match decode_values(&data[description_start..description_end], encoding, 1) {
        Ok(v) => v,
        Err(()) => {
            tree.error(
                frame_node,
                "invalid ID3 comment description encoding",
                ByteRange::new(
                    description_start as u64,
                    (description_end - description_start) as u64,
                ),
            );
            return false;
        }
    };
    let decoded = match decode_values(
        &data[comment_start..end],
        encoding,
        MAX_FACTS.saturating_sub(facts.len()),
    ) {
        Ok(v) => v,
        Err(()) => {
            tree.error(
                frame_node,
                "invalid ID3 comment encoding",
                ByteRange::new(comment_start as u64, (end - comment_start) as u64),
            );
            return false;
        }
    };
    let mut complete = !description.truncated;
    let description = description.values.into_iter().next().unwrap_or_default();
    for value in decoded.values {
        if value.is_empty() {
            continue;
        }
        if facts.len() == MAX_FACTS {
            tree.add(
                Some(frame_node),
                "metadata fact count exceeds the 256-item inspection limit",
                ByteRange::new(comment_start as u64, (end - comment_start) as u64),
                NodeKind::Field,
                None,
            );
            return false;
        }
        let text = if description.is_empty() {
            value
        } else {
            format!("{description}: {value}")
        };
        let (text, display_truncated) = truncate_display(&text);
        let node = tree.add(
            Some(frame_node),
            format!("comment: {text}"),
            ByteRange::new(comment_start as u64, (end - comment_start) as u64),
            NodeKind::Field,
            Some(Value::Text(text.clone())),
        );
        facts.push(DocumentFact {
            kind: "comment",
            text,
            node,
        });
        if display_truncated {
            tree.add(
                Some(frame_node),
                "comment truncated at the 512-byte display limit",
                ByteRange::new(comment_start as u64, (end - comment_start) as u64),
                NodeKind::Field,
                None,
            );
            complete = false;
        }
    }
    if decoded.truncated {
        tree.add(
            Some(frame_node),
            "comment truncated at the 512-byte display limit",
            ByteRange::new(comment_start as u64, (end - comment_start) as u64),
            NodeKind::Field,
            None,
        );
        return false;
    }
    complete && !decoded.truncated
}

fn has_unsupported_frame_flags(version: u8, flags: u16) -> bool {
    match version {
        // v2.3: compression, encryption, or grouping identity.
        3 => flags & 0x00e0 != 0,
        // v2.4: grouping, compression, encryption, unsynchronisation, or DLI.
        4 => flags & 0x004f != 0,
        _ => false,
    }
}

fn has_reserved_frame_flags(version: u8, flags: u16) -> bool {
    match version {
        3 => flags & !0xe0e0 != 0,
        4 => flags & !0x704f != 0,
        _ => false,
    }
}

struct DecodedValues {
    values: Vec<String>,
    truncated: bool,
}

fn decode_values(input: &[u8], encoding: u8, max_values: usize) -> Result<DecodedValues, ()> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut truncated = false;
    let mut push_char = |ch: char| {
        if ch == '\0' {
            if !current.is_empty() && values.len() < max_values {
                values.push(std::mem::take(&mut current));
            } else if !current.is_empty() {
                truncated = true;
            }
            return;
        }
        if current.len() + ch.len_utf8() <= MAX_VALUE_BYTES {
            current.push(ch);
        } else {
            truncated = true;
        }
    };
    match encoding {
        0 => {
            for &byte in input {
                push_char(byte as char);
            }
        }
        3 => {
            for ch in std::str::from_utf8(input).map_err(|_| ())?.chars() {
                push_char(ch);
            }
        }
        1 | 2 => {
            if !input.len().is_multiple_of(2) {
                return Err(());
            }
            let (bytes, little_endian) = if encoding == 1 {
                match input.get(..2) {
                    Some([0xff, 0xfe]) => (&input[2..], true),
                    Some([0xfe, 0xff]) => (&input[2..], false),
                    _ => return Err(()),
                }
            } else {
                (input, false)
            };
            let (pairs, _) = bytes.as_chunks::<2>();
            let units = pairs.iter().map(|pair| {
                if little_endian {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            });
            for decoded in char::decode_utf16(units) {
                push_char(decoded.map_err(|_| ())?);
            }
        }
        _ => return Err(()),
    }
    if !current.is_empty() {
        if values.len() < max_values {
            values.push(current);
        } else {
            truncated = true;
        }
    }
    Ok(DecodedValues { values, truncated })
}

fn split_terminated(data: &[u8], start: usize, end: usize, encoding: u8) -> Option<(usize, usize)> {
    if encoding == 0 || encoding == 3 {
        let rel = data.get(start..end)?.iter().position(|&b| b == 0)?;
        return Some((start + rel, start + rel + 1));
    }
    if encoding != 1 && encoding != 2 {
        return None;
    }
    let mut cursor = start;
    while cursor.checked_add(1)? < end {
        if data[cursor] == 0 && data[cursor + 1] == 0 {
            return Some((cursor, cursor + 2));
        }
        cursor += 2;
    }
    None
}

fn text_fact(id: &str, value: String) -> (&'static str, String) {
    let kind = match id {
        "TT2" | "TIT2" => "title",
        "TP1" | "TPE1" => "artist",
        "TAL" | "TALB" => "album",
        "TYE" | "TYER" | "TDRC" => "year",
        "TRK" | "TRCK" => "track",
        "TCO" | "TCON" => "genre",
        "TP2" | "TPE2" => "album-artist",
        "TCM" | "TCOM" => "composer",
        "TPA" | "TPOS" => "disc",
        "TSSE" => "encoder",
        _ => "audio-tag",
    };
    if kind == "audio-tag" {
        (kind, format!("{id}: {value}"))
    } else {
        (kind, value)
    }
}

fn truncate_display(value: &str) -> (String, bool) {
    if value.len() <= MAX_VALUE_BYTES {
        return (value.to_owned(), false);
    }
    let mut end = MAX_VALUE_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

fn synchsafe(bytes: &[u8]) -> Option<usize> {
    let [a, b, c, d] = <[u8; 4]>::try_from(bytes).ok()?;
    if [a, b, c, d].iter().any(|b| b & 0x80 != 0) {
        return None;
    }
    Some(((a as usize) << 21) | ((b as usize) << 14) | ((c as usize) << 7) | d as usize)
}

fn parse_id3v1(
    data: &[u8],
    root: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
    complete: &mut bool,
) {
    let start = data.len() - 128;
    let tag_node = tree.add(
        Some(root),
        "ID3v1 tag",
        ByteRange::new(start as u64, 128),
        NodeKind::Container,
        None,
    );
    let mut facts = Id3v1Facts {
        data,
        tag_node,
        tree,
        facts,
        complete,
    };
    let fields = [
        ("title", start + 3, 30),
        ("artist", start + 33, 30),
        ("album", start + 63, 30),
        ("year", start + 93, 4),
    ];
    for (kind, offset, len) in fields {
        facts.add(kind, offset, len);
    }
    let comment_len = if data[start + 125] == 0 && data[start + 126] != 0 {
        28
    } else {
        30
    };
    facts.add("comment", start + 97, comment_len);
    if comment_len == 28 {
        facts.add("track", start + 126, 1);
    }
    facts.add("genre", start + 127, 1);
}

struct Id3v1Facts<'a, 'tree> {
    data: &'a [u8],
    tag_node: u32,
    tree: &'tree mut ParseTree,
    facts: &'tree mut Vec<DocumentFact>,
    complete: &'tree mut bool,
}

impl Id3v1Facts<'_, '_> {
    fn add(&mut self, kind: &'static str, offset: usize, len: usize) {
        if self.facts.len() >= MAX_FACTS {
            *self.complete = false;
            return;
        }
        let raw = &self.data[offset..offset + len];
        let text = if kind == "track" || kind == "genre" {
            if raw[0] == 0xff {
                return;
            }
            if kind == "genre" {
                // ID3v1 genre numbers are an index; retain the source value
                // rather than guessing from a catalogue that changes.
                raw[0].to_string()
            } else {
                raw[0].to_string()
            }
        } else {
            let bytes = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())];
            let text: String = bytes.iter().map(|&b| b as char).collect();
            text.trim_end_matches(' ').to_string()
        };
        if text.is_empty() {
            return;
        }
        let node = self.tree.add(
            Some(self.tag_node),
            format!("{kind}: {text}"),
            ByteRange::new(offset as u64, len as u64),
            NodeKind::Field,
            Some(Value::Text(text.clone())),
        );
        self.facts.push(DocumentFact { kind, text, node });
    }
}
