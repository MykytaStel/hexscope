use super::{AudioDocument, AudioFormat};
use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};

const MAX_TAG_SIZE: usize = 16 * 1024 * 1024;
const MAX_FACTS: usize = 256;
const MAX_VALUE_BYTES: usize = 512;
const MAX_FRAMES: usize = 65_536;

pub fn is_mp3(data: &[u8]) -> bool {
    data.starts_with(b"ID3") || data.get(..4).is_some_and(valid_mpeg_frame_header)
}

fn valid_mpeg_frame_header(header: &[u8]) -> bool {
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

    if data.starts_with(b"ID3") {
        complete &= parse_id3v2(data, root, &mut tree, &mut facts);
        if let Some(end) = id3v2_end(data) {
            audio_start = end;
        } else {
            can_mark_audio = false;
        }
    }
    let id3v1_start = if data.len() >= 128 && data[data.len() - 128..].starts_with(b"TAG") {
        data.len() - 128
    } else {
        data.len()
    };
    if can_mark_audio && audio_start < id3v1_start {
        tree.add(
            Some(root),
            "audio frames (not decoded)",
            ByteRange::new(audio_start as u64, (id3v1_start - audio_start) as u64),
            NodeKind::Field,
            Some(Value::Bytes((id3v1_start - audio_start) as u64)),
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

fn id3v2_end(data: &[u8]) -> Option<usize> {
    if data.len() < 10 || &data[..3] != b"ID3" || !(2..=4).contains(&data[3]) {
        return None;
    }
    let size = synchsafe(&data[6..10])?;
    if size > MAX_TAG_SIZE {
        return None;
    }
    let end = 10usize.checked_add(size)?;
    (end <= data.len()).then_some(end)
}

fn parse_id3v2(
    data: &[u8],
    root: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    if data.len() < 10 {
        tree.error(
            root,
            "truncated ID3 header",
            ByteRange::new(0, data.len() as u64),
        );
        return false;
    }
    let version = data[3];
    if !(2..=4).contains(&version) {
        tree.warning(root, "unsupported ID3 version", ByteRange::new(0, 10));
        return false;
    }
    if data[4] == 0xff {
        tree.warning(root, "reserved ID3 revision", ByteRange::new(3, 2));
        return false;
    }
    let Some(size) = synchsafe(data.get(6..10).unwrap_or_default()) else {
        tree.error(root, "invalid ID3 synchsafe size", ByteRange::new(6, 4));
        return false;
    };
    if size > MAX_TAG_SIZE {
        tree.add(
            Some(root),
            "ID3 tag exceeds the 16 MiB inspection limit",
            ByteRange::new(0, 10),
            NodeKind::Field,
            None,
        );
        return false;
    }
    let Some(tag_end) = 10usize.checked_add(size) else {
        tree.error(root, "invalid ID3 tag length", ByteRange::new(6, 4));
        return false;
    };
    let available_end = tag_end.min(data.len());
    let tag_node = tree.add(
        Some(root),
        format!("ID3v2.{version} tag"),
        ByteRange::new(0, available_end as u64),
        NodeKind::Container,
        None,
    );
    if tag_end > data.len() {
        tree.error(
            tag_node,
            "truncated ID3 tag body",
            ByteRange::new(10, data.len().saturating_sub(10) as u64),
        );
        return false;
    }

    let flags = data[5];
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
            ByteRange::new(5, 1),
        );
        return false;
    }
    // The legacy compression flag and tag-level unsynchronisation alter the
    // frame stream. Skip the whole tag rather than risk false frame offsets.
    let unsupported_transform =
        (version == 2 && flags & 0x40 != 0) || (version >= 3 && flags & 0x80 != 0);
    if unsupported_transform {
        tree.add(
            Some(tag_node),
            "ID3 tag uses an unsupported compression or unsynchronisation transform",
            ByteRange::new(5, 1),
            NodeKind::Field,
            None,
        );
        return false;
    }

    let mut frames_start = 10usize;
    let mut frames_end = tag_end;
    if version == 4 && flags & 0x10 != 0 {
        if frames_end < frames_start + 10
            || data.get(frames_end - 10..frames_end - 7) != Some(b"3DI")
        {
            tree.error(
                tag_node,
                "invalid ID3v2.4 footer",
                ByteRange::new(
                    frames_end.saturating_sub(10) as u64,
                    frames_end.min(10) as u64,
                ),
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
        version,
        frames_start,
        frames_end,
        tag_node,
        tree,
        facts,
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
    version: u8,
    mut cursor: usize,
    end: usize,
    tag_node: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    let mut count = 0usize;
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
        if count == MAX_FRAMES {
            tree.add(
                Some(tag_node),
                "ID3 frame count exceeds the inspection limit",
                ByteRange::new(cursor as u64, (end - cursor) as u64),
                NodeKind::Field,
                None,
            );
            return false;
        }
        count += 1;
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
