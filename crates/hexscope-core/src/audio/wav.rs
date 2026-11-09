//! Bounded, read-only inspection of RIFF/WAVE headers and metadata chunks.

use super::{AudioDocument, AudioFormat};
use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};

const MAX_CHUNKS: usize = 4096;
const MAX_INFO_BYTES: usize = 1024 * 1024;
const MAX_VALUE_BYTES: usize = 512;
const MAX_FACTS: usize = 256;

pub(super) fn is_wave(data: &[u8]) -> bool {
    data.get(..4) == Some(b"RIFF") && data.get(8..12) == Some(b"WAVE")
}

pub(super) fn parse_wave(data: &[u8]) -> AudioDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "WAV audio",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut facts = Vec::new();
    let mut complete = true;
    tree.add(
        Some(root),
        "RIFF signature",
        ByteRange::new(0, 4),
        NodeKind::Field,
        Some(Value::Text("RIFF".into())),
    );

    let declared =
        u32::from_le_bytes(data[4..8].try_into().expect("WAVE signature has a size")) as u64;
    tree.add(
        Some(root),
        "RIFF form size",
        ByteRange::new(4, 4),
        NodeKind::Field,
        Some(Value::U64(declared)),
    );
    tree.add(
        Some(root),
        "WAVE form type",
        ByteRange::new(8, 4),
        NodeKind::Field,
        Some(Value::Text("WAVE".into())),
    );

    let declared_end = 8u64.saturating_add(declared);
    if declared < 4 {
        tree.error(
            root,
            "RIFF form size is smaller than its form type",
            ByteRange::new(4, 4),
        );
        complete = false;
    }
    if declared_end > data.len() as u64 {
        tree.error(
            root,
            "RIFF form extends beyond the file",
            ByteRange::new(4, data.len().saturating_sub(4).min(4) as u64),
        );
        complete = false;
    }
    let form_end = declared_end.min(data.len() as u64) as usize;
    let mut cursor = 12usize;
    let mut chunks = 0usize;
    let mut has_format = false;
    let mut has_data = false;
    let mut data_bytes = 0u64;
    let mut byte_rate = None;

    while cursor < form_end {
        if chunks == MAX_CHUNKS {
            tree.add(
                Some(root),
                "WAVE chunk count exceeds the inspection limit",
                ByteRange::new(cursor as u64, (form_end - cursor) as u64),
                NodeKind::Field,
                None,
            );
            complete = false;
            break;
        }
        chunks += 1;
        if form_end - cursor < 8 {
            tree.error(
                root,
                "truncated WAVE chunk header",
                ByteRange::new(cursor as u64, (form_end - cursor) as u64),
            );
            complete = false;
            break;
        }

        let id = &data[cursor..cursor + 4];
        let size = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let body_start = cursor + 8;
        let Some(body_end) = body_start.checked_add(size) else {
            tree.error(
                root,
                "invalid WAVE chunk size",
                ByteRange::new(cursor as u64, 8),
            );
            complete = false;
            break;
        };
        let padded_end = body_end.saturating_add(size & 1);
        let label = if id == b"LIST" && size >= 4 && &data[body_start..body_start + 4] == b"INFO" {
            "LIST/INFO".to_owned()
        } else {
            chunk_name(id)
        };
        let chunk = tree.add(
            Some(root),
            format!("WAVE {label} chunk"),
            ByteRange::new(cursor as u64, (padded_end.min(form_end) - cursor) as u64),
            NodeKind::Container,
            None,
        );
        if body_end > form_end {
            tree.error(
                chunk,
                "WAVE chunk extends beyond the RIFF form",
                ByteRange::new((cursor + 4) as u64, 4),
            );
            complete = false;
            break;
        }
        if padded_end > form_end {
            tree.error(
                chunk,
                "WAVE chunk is missing its required padding byte",
                ByteRange::new(body_end as u64, 0),
            );
            complete = false;
            break;
        }

        tree.add(
            Some(chunk),
            "chunk data size",
            ByteRange::new((cursor + 4) as u64, 4),
            NodeKind::Field,
            Some(Value::U64(size as u64)),
        );
        match id {
            b"fmt " => {
                if has_format {
                    tree.warning(
                        chunk,
                        "duplicate WAVE format chunk",
                        ByteRange::new(cursor as u64, (body_end - cursor) as u64),
                    );
                    complete = false;
                }
                has_format = true;
                let rate =
                    parse_format(data, body_start, body_end, chunk, &mut tree, &mut complete);
                byte_rate = rate;
            }
            b"data" => {
                has_data = true;
                data_bytes = data_bytes.saturating_add(size as u64);
                tree.add(
                    Some(chunk),
                    "WAVE audio data (not decoded)",
                    ByteRange::new(body_start as u64, size as u64),
                    NodeKind::Field,
                    Some(Value::Bytes(size as u64)),
                );
            }
            b"LIST" => {
                if size < 4 {
                    tree.error(
                        chunk,
                        "truncated WAVE LIST type",
                        ByteRange::new(body_start as u64, size as u64),
                    );
                    complete = false;
                } else if &data[body_start..body_start + 4] == b"INFO" {
                    complete &=
                        parse_info(data, body_start + 4, body_end, chunk, &mut tree, &mut facts);
                } else {
                    tree.add(
                        Some(chunk),
                        "WAVE LIST contents not inspected",
                        ByteRange::new((body_start + 4) as u64, (size - 4) as u64),
                        NodeKind::Field,
                        Some(Value::Bytes((size - 4) as u64)),
                    );
                    complete = false;
                }
            }
            b"bext" => {
                complete &= parse_bext(data, body_start, body_end, chunk, &mut tree, &mut facts)
            }
            _ => {
                tree.add(
                    Some(chunk),
                    "WAVE chunk data not inspected",
                    ByteRange::new(body_start as u64, size as u64),
                    NodeKind::Field,
                    Some(Value::Bytes(size as u64)),
                );
                complete = false;
            }
        }
        cursor = padded_end;
    }

    if !has_format {
        tree.warning(root, "WAVE form has no format chunk", ByteRange::new(12, 0));
        complete = false;
    }
    if !has_data {
        tree.warning(
            root,
            "WAVE form has no audio data chunk",
            ByteRange::new(12, 0),
        );
        complete = false;
    }
    if let Some(rate) = byte_rate.filter(|rate| *rate != 0) {
        let duration = data_bytes / u64::from(rate);
        tree.add(
            Some(root),
            "audio duration in whole seconds (estimated from chunk sizes)",
            ByteRange::new(0, data.len().min(12) as u64),
            NodeKind::Field,
            Some(Value::U64(duration)),
        );
    }
    if declared_end < data.len() as u64 {
        tree.warning(
            root,
            "bytes after the RIFF form",
            ByteRange::new(declared_end, data.len() as u64 - declared_end),
        );
        complete = false;
    }

    AudioDocument {
        format: AudioFormat::Wav,
        tree,
        facts,
        metadata_complete: complete,
    }
}

fn parse_format(
    data: &[u8],
    start: usize,
    end: usize,
    parent: u32,
    tree: &mut ParseTree,
    complete: &mut bool,
) -> Option<u32> {
    if end - start < 16 {
        tree.error(
            parent,
            "truncated WAVE format chunk",
            ByteRange::new(start as u64, (end - start) as u64),
        );
        *complete = false;
        return None;
    }
    let tag = u16le(data, start);
    let channels = u16le(data, start + 2);
    let sample_rate = u32le(data, start + 4);
    let rate = u32le(data, start + 8);
    let align = u16le(data, start + 12);
    let bits = u16le(data, start + 14);
    field(
        tree,
        parent,
        "audio encoding",
        start,
        2,
        Value::Enum {
            raw: tag as u64,
            name: encoding_name(tag),
        },
    );
    field(
        tree,
        parent,
        "audio channels",
        start + 2,
        2,
        Value::U64(channels as u64),
    );
    field(
        tree,
        parent,
        "sample rate in Hz",
        start + 4,
        4,
        Value::U64(sample_rate as u64),
    );
    field(
        tree,
        parent,
        "average bytes per second",
        start + 8,
        4,
        Value::U64(rate as u64),
    );
    field(
        tree,
        parent,
        "block alignment",
        start + 12,
        2,
        Value::U64(align as u64),
    );
    field(
        tree,
        parent,
        "bits per sample",
        start + 14,
        2,
        Value::U64(bits as u64),
    );
    if end - start == 17 {
        tree.error(
            parent,
            "truncated WAVE format extension size",
            ByteRange::new((start + 16) as u64, 1),
        );
        *complete = false;
    } else if end - start >= 18 {
        let extra = u16le(data, start + 16) as usize;
        field(
            tree,
            parent,
            "WAVE format extension size",
            start + 16,
            2,
            Value::U64(extra as u64),
        );
        if 18usize.saturating_add(extra) > end - start {
            tree.error(
                parent,
                "WAVE format extension exceeds its chunk",
                ByteRange::new((start + 16) as u64, 2),
            );
            *complete = false;
        } else if tag != 0xfffe && extra > 0 {
            tree.add(
                Some(parent),
                "WAVE format extension not inspected",
                ByteRange::new((start + 18) as u64, extra as u64),
                NodeKind::Field,
                Some(Value::Bytes(extra as u64)),
            );
            *complete = false;
        } else if tag == 0xfffe && extra > 22 {
            let tail = extra - 22;
            tree.add(
                Some(parent),
                "WAVE format extension not inspected",
                ByteRange::new((start + 40) as u64, tail as u64),
                NodeKind::Field,
                Some(Value::Bytes(tail as u64)),
            );
            *complete = false;
        }
        if 18usize.saturating_add(extra) < end - start {
            let tail_start = start + 18 + extra;
            let tail_len = end - tail_start;
            tree.add(
                Some(parent),
                "WAVE format trailing bytes not inspected",
                ByteRange::new(tail_start as u64, tail_len as u64),
                NodeKind::Field,
                Some(Value::Bytes(tail_len as u64)),
            );
            *complete = false;
        }
    }
    if channels == 0 || sample_rate == 0 || align == 0 {
        tree.warning(
            parent,
            "invalid WAVE format values",
            ByteRange::new(start as u64, 16),
        );
        *complete = false;
    }
    if tag == 0xfffe {
        if end - start < 40 {
            tree.error(
                parent,
                "truncated WAVE extensible format",
                ByteRange::new(start as u64, (end - start) as u64),
            );
            *complete = false;
        } else {
            let extra = u16le(data, start + 16) as usize;
            if extra < 22 || 18usize.saturating_add(extra) > end - start {
                tree.warning(
                    parent,
                    "invalid WAVE extensible format size",
                    ByteRange::new((start + 16) as u64, 2),
                );
                *complete = false;
            }
            field(
                tree,
                parent,
                "WAVE channel mask",
                start + 20,
                4,
                Value::U64(u32le(data, start + 20) as u64),
            );
            field(
                tree,
                parent,
                "WAVE valid bits per sample",
                start + 18,
                2,
                Value::U64(u16le(data, start + 18) as u64),
            );
            tree.add(
                Some(parent),
                "WAVE extensible subformat GUID",
                ByteRange::new((start + 24) as u64, 16),
                NodeKind::Field,
                Some(Value::Text(hex(&data[start + 24..start + 40]))),
            );
        }
    }
    matches!(tag, 1 | 3).then_some(rate)
}

fn parse_info(
    data: &[u8],
    mut cursor: usize,
    end: usize,
    parent: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    let mut complete = true;
    let mut count = 0usize;
    while cursor < end {
        if count == MAX_CHUNKS {
            tree.add(
                Some(parent),
                "WAVE INFO item count exceeds the inspection limit",
                ByteRange::new(cursor as u64, (end - cursor) as u64),
                NodeKind::Field,
                None,
            );
            return false;
        }
        count += 1;
        if end - cursor < 8 {
            tree.error(
                parent,
                "truncated WAVE INFO item header",
                ByteRange::new(cursor as u64, (end - cursor) as u64),
            );
            return false;
        }
        let id = &data[cursor..cursor + 4];
        let size = u32le(data, cursor + 4) as usize;
        let body = cursor + 8;
        let Some(item_end) = body.checked_add(size) else {
            return false;
        };
        let padded_end = item_end.saturating_add(size & 1);
        let name = info_name(id);
        let node = tree.add(
            Some(parent),
            format!("WAVE INFO {name}"),
            ByteRange::new(cursor as u64, padded_end.saturating_sub(cursor) as u64),
            NodeKind::Container,
            None,
        );
        if item_end > end || padded_end > end {
            tree.error(
                node,
                "WAVE INFO item extends beyond its LIST chunk",
                ByteRange::new((cursor + 4) as u64, 4),
            );
            return false;
        }
        if size > MAX_INFO_BYTES {
            tree.add(
                Some(node),
                "WAVE INFO value exceeds the inspection limit",
                ByteRange::new(body as u64, size as u64),
                NodeKind::Field,
                Some(Value::Bytes(size as u64)),
            );
            complete = false;
        } else {
            let value_bytes = &data[body..item_end];
            if let Some(nul) = value_bytes.iter().position(|&byte| byte == 0)
                && value_bytes[nul + 1..].iter().any(|&byte| byte != 0)
            {
                tree.warning(
                    node,
                    "nonzero bytes after WAVE INFO text",
                    ByteRange::new((body + nul) as u64, (size - nul) as u64),
                );
                complete = false;
            }
            let text_bytes = value_bytes
                .split(|&byte| byte == 0)
                .next()
                .unwrap_or_default();
            match std::str::from_utf8(text_bytes) {
                Ok(text) => {
                    let text = text.trim().to_owned();
                    let display = display_prefix(&text, MAX_VALUE_BYTES);
                    let value_node = tree.add(
                        Some(node),
                        "WAVE INFO value",
                        ByteRange::new(body as u64, size as u64),
                        NodeKind::Field,
                        Some(Value::Text(display.to_owned())),
                    );
                    let truncated = display.len() < text.len();
                    if truncated {
                        tree.warning(
                            value_node,
                            "WAVE INFO value truncated at the 512-byte display limit",
                            ByteRange::new(body as u64, size as u64),
                        );
                        complete = false;
                    }
                    if let Some(kind) = info_fact_kind(id)
                        && !display.is_empty()
                    {
                        if facts.len() < MAX_FACTS {
                            facts.push(DocumentFact {
                                kind,
                                text: display.to_owned(),
                                node: value_node,
                            });
                        } else {
                            tree.add(
                                Some(node),
                                "metadata fact count exceeds the 256-item inspection limit",
                                ByteRange::new(body as u64, size as u64),
                                NodeKind::Field,
                                None,
                            );
                            complete = false;
                        }
                    }
                }
                Err(_) => {
                    tree.warning(
                        node,
                        "WAVE INFO value is not UTF-8",
                        ByteRange::new(body as u64, size as u64),
                    );
                    complete = false;
                }
            }
        }
        cursor = padded_end;
    }
    complete
}

fn parse_bext(
    data: &[u8],
    start: usize,
    end: usize,
    parent: u32,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    // BWF v1's fixed fields end after the 180-byte reserved area (602 bytes).
    const FIELDS: [(&str, usize, usize, Option<&str>); 5] = [
        ("broadcast description", 0, 256, Some("description")),
        ("broadcast originator", 256, 32, Some("originator")),
        (
            "broadcast originator reference",
            288,
            32,
            Some("broadcast-reference"),
        ),
        ("broadcast origination date", 320, 10, Some("created")),
        ("broadcast origination time", 330, 8, None),
    ];
    let len = end - start;
    if len < 602 {
        tree.warning(
            parent,
            "truncated Broadcast WAVE extension",
            ByteRange::new(start as u64, len as u64),
        );
    }
    let mut complete = len >= 602;
    for (label, offset, width, kind) in FIELDS {
        if offset + width > len {
            continue;
        }
        let bytes = &data[start + offset..start + offset + width];
        let used = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let text = &bytes[..used];
        if used < bytes.len() && bytes[used + 1..].iter().any(|&byte| byte != 0) {
            tree.warning(
                parent,
                "nonzero bytes after Broadcast Wave fixed text",
                ByteRange::new((start + offset) as u64, width as u64),
            );
            complete = false;
        }
        if !text.is_ascii() {
            tree.warning(
                parent,
                "Broadcast Wave fixed text is not ASCII",
                ByteRange::new((start + offset) as u64, width as u64),
            );
            complete = false;
            continue;
        }
        let value = std::str::from_utf8(text)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let node = tree.add(
            Some(parent),
            label,
            ByteRange::new((start + offset) as u64, width as u64),
            NodeKind::Field,
            Some(Value::Text(value.clone())),
        );
        if !value.is_empty()
            && let Some(kind) = kind
        {
            if facts.len() < MAX_FACTS {
                facts.push(DocumentFact {
                    kind,
                    text: value,
                    node,
                });
            } else {
                tree.add(
                    Some(parent),
                    "metadata fact count exceeds the 256-item inspection limit",
                    ByteRange::new((start + offset) as u64, width as u64),
                    NodeKind::Field,
                    None,
                );
                complete = false;
            }
        }
    }
    if len >= 346 {
        field(
            tree,
            parent,
            "broadcast sample time reference low",
            start + 338,
            4,
            Value::U64(u32le(data, start + 338) as u64),
        );
        field(
            tree,
            parent,
            "broadcast sample time reference high",
            start + 342,
            4,
            Value::U64(u32le(data, start + 342) as u64),
        );
    }
    if len >= 602 {
        let version = u16le(data, start + 346);
        field(
            tree,
            parent,
            "broadcast extension version",
            start + 346,
            2,
            Value::U64(version as u64),
        );
        if version > 2 {
            tree.warning(
                parent,
                "unsupported Broadcast Wave extension version",
                ByteRange::new((start + 346) as u64, 2),
            );
            complete = false;
        }
        let umid = &data[start + 348..start + 412];
        let umid_value = hex(umid);
        let umid_node = tree.add(
            Some(parent),
            "broadcast UMID",
            ByteRange::new((start + 348) as u64, 64),
            NodeKind::Field,
            Some(Value::Text(umid_value.clone())),
        );
        if umid.iter().any(|&byte| byte != 0) {
            if facts.len() < MAX_FACTS {
                facts.push(DocumentFact {
                    kind: "broadcast-reference",
                    text: umid_value,
                    node: umid_node,
                });
            } else {
                tree.add(
                    Some(parent),
                    "metadata fact count exceeds the 256-item inspection limit",
                    ByteRange::new((start + 348) as u64, 64),
                    NodeKind::Field,
                    None,
                );
                complete = false;
            }
        }
        for i in 0..5 {
            let offset = start + 412 + i * 2;
            let value = i16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
            field(
                tree,
                parent,
                "broadcast loudness stored value",
                offset,
                2,
                Value::Text(value.to_string()),
            );
        }
        let reserved = &data[start + 422..start + 602];
        tree.add(
            Some(parent),
            "broadcast reserved bytes",
            ByteRange::new((start + 422) as u64, 180),
            NodeKind::Field,
            Some(Value::Bytes(180)),
        );
        if reserved.iter().any(|&byte| byte != 0) {
            tree.warning(
                parent,
                "nonzero Broadcast Wave reserved bytes",
                ByteRange::new((start + 422) as u64, 180),
            );
            complete = false;
        }
        if len > 602 {
            let history = &data[start + 602..end];
            if history.len() > MAX_INFO_BYTES {
                tree.add(
                    Some(parent),
                    "broadcast coding history exceeds the inspection limit",
                    ByteRange::new((start + 602) as u64, history.len() as u64),
                    NodeKind::Field,
                    Some(Value::Bytes(history.len() as u64)),
                );
                return false;
            }
            match std::str::from_utf8(history) {
                Ok(text) => {
                    let text = text.trim_matches(char::from(0)).trim();
                    let display = display_prefix(text, MAX_VALUE_BYTES);
                    let node = tree.add(
                        Some(parent),
                        "broadcast coding history",
                        ByteRange::new((start + 602) as u64, history.len() as u64),
                        NodeKind::Field,
                        Some(Value::Text(display.to_owned())),
                    );
                    if display.len() < text.len() {
                        tree.warning(
                            node,
                            "Broadcast Wave coding history truncated at the 512-byte display limit",
                            ByteRange::new((start + 602) as u64, history.len() as u64),
                        );
                        complete = false;
                    }
                    if !display.is_empty() && facts.len() < MAX_FACTS {
                        facts.push(DocumentFact {
                            kind: "history",
                            text: display.to_owned(),
                            node,
                        });
                    } else if !display.is_empty() {
                        tree.add(
                            Some(parent),
                            "metadata fact count exceeds the 256-item inspection limit",
                            ByteRange::new((start + 602) as u64, history.len() as u64),
                            NodeKind::Field,
                            None,
                        );
                        complete = false;
                    }
                }
                Err(_) => {
                    tree.warning(
                        parent,
                        "Broadcast Wave coding history is not UTF-8",
                        ByteRange::new((start + 602) as u64, history.len() as u64),
                    );
                    return false;
                }
            }
        }
        complete
    } else {
        complete
    }
}

fn field(tree: &mut ParseTree, parent: u32, label: &str, start: usize, len: usize, value: Value) {
    tree.add(
        Some(parent),
        label,
        ByteRange::new(start as u64, len as u64),
        NodeKind::Field,
        Some(value),
    );
}

fn u16le(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(data[at..at + 2].try_into().unwrap())
}
fn u32le(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn display_prefix(text: &str, max_bytes: usize) -> &str {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn chunk_name(id: &[u8]) -> String {
    String::from_utf8_lossy(id).trim().to_owned()
}

fn encoding_name(tag: u16) -> &'static str {
    match tag {
        1 => "integer PCM",
        3 => "IEEE floating point",
        0xfffe => "WAVE extensible",
        2 => "Microsoft ADPCM",
        6 => "A-law",
        7 => "mu-law",
        _ => "other or unknown encoding",
    }
}

fn info_name(id: &[u8]) -> String {
    match id {
        b"INAM" => "title".into(),
        b"IART" => "artist".into(),
        b"IPRD" => "product or album".into(),
        b"ICMT" => "comment".into(),
        b"ICOP" => "copyright".into(),
        b"ICRD" => "creation date".into(),
        b"ISFT" => "software".into(),
        b"IENG" => "engineer".into(),
        b"IKEY" => "keywords".into(),
        b"ISBJ" => "subject".into(),
        b"IARL" => "archival location".into(),
        b"ITRK" => "track number".into(),
        _ => format!("{}", String::from_utf8_lossy(id)),
    }
}

fn info_fact_kind(id: &[u8]) -> Option<&'static str> {
    match id {
        b"INAM" => Some("title"),
        b"IART" => Some("artist"),
        b"IPRD" => Some("album"),
        b"ICMT" => Some("description"),
        b"ICOP" => Some("copyright"),
        b"ICRD" => Some("created"),
        b"ISFT" => Some("software"),
        b"IENG" => Some("engineer"),
        b"IKEY" => Some("keywords"),
        b"ISBJ" => Some("subject"),
        b"IARL" => Some("place"),
        b"ITRK" => Some("track"),
        _ => Some("audio-tag"),
    }
}
