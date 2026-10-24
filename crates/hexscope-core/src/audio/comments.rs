use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};

const MAX_FACTS: usize = 256;
const MAX_VALUE_BYTES: usize = 512;
const MAX_FIELDS: usize = 16_384;

#[derive(Clone, Copy)]
pub(super) struct CommentContext {
    pub source_offset: u64,
    pub source_span: Option<ByteRange>,
    pub parent: u32,
    pub framing_bit: bool,
    pub allow_trailing: bool,
}

/// Reads a Vorbis-comment-shaped structure. `source_span` is used for packets
/// whose bytes cross Ogg pages, where one contiguous source range is the
/// closest available evidence in the parse tree.
pub(super) fn parse_comment_block(
    bytes: &[u8],
    context: CommentContext,
    tree: &mut ParseTree,
    facts: &mut Vec<DocumentFact>,
) -> bool {
    let CommentContext {
        source_offset,
        source_span,
        parent,
        framing_bit,
        allow_trailing,
    } = context;
    if bytes.len() > 16 * 1024 * 1024 {
        tree.add(
            Some(parent),
            "comment metadata exceeds the 16 MiB inspection limit",
            source_span.unwrap_or(ByteRange::new(source_offset, bytes.len() as u64)),
            NodeKind::Field,
            None,
        );
        return false;
    }
    let mut cursor = 0usize;
    let Some(vendor_len) = read_u32(bytes, &mut cursor) else {
        tree.error(
            parent,
            "truncated comment vendor length",
            range(source_offset, source_span, bytes.len()),
        );
        return false;
    };
    let Some(vendor) = take(bytes, &mut cursor, vendor_len as usize) else {
        tree.error(
            parent,
            "comment vendor length exceeds the structure",
            range(source_offset, source_span, bytes.len()),
        );
        return false;
    };
    let vendor_text = match std::str::from_utf8(vendor) {
        Ok(s) => s,
        Err(_) => {
            tree.error(
                parent,
                "invalid UTF-8 in comment vendor",
                range(source_offset, source_span, vendor.len()),
            );
            return false;
        }
    };
    if !vendor_text.is_empty() {
        if facts.len() == MAX_FACTS {
            limit_node(tree, parent, source_offset, source_span, bytes.len());
            return false;
        }
        let (text, truncated) = truncate(vendor_text);
        let node = tree.add(
            Some(parent),
            format!("encoder: {text}"),
            range(source_offset, source_span, vendor.len()),
            NodeKind::Field,
            Some(Value::Text(text.clone())),
        );
        facts.push(DocumentFact {
            kind: "encoder",
            text,
            node,
        });
        if truncated {
            tree.add(
                Some(parent),
                "vendor value truncated at the 512-byte display limit",
                range(source_offset, source_span, bytes.len()),
                NodeKind::Field,
                None,
            );
            return false;
        }
    }

    let Some(field_count) = read_u32(bytes, &mut cursor) else {
        tree.error(
            parent,
            "truncated comment field count",
            range(source_offset, source_span, bytes.len()),
        );
        return false;
    };
    let remaining = bytes.len().saturating_sub(cursor);
    if field_count as usize > remaining / 4 || field_count as usize > MAX_FIELDS {
        tree.error(
            parent,
            "invalid or excessive comment field count",
            range(source_offset, source_span, bytes.len()),
        );
        return false;
    }

    let mut complete = true;
    for _ in 0..field_count {
        let Some(field_len) = read_u32(bytes, &mut cursor) else {
            tree.error(
                parent,
                "truncated comment field length",
                range(source_offset, source_span, bytes.len()),
            );
            return false;
        };
        let field_start = cursor;
        let Some(field) = take(bytes, &mut cursor, field_len as usize) else {
            tree.error(
                parent,
                "comment field length exceeds the structure",
                range(source_offset, source_span, bytes.len()),
            );
            return false;
        };
        let field_text = match std::str::from_utf8(field) {
            Ok(s) => s,
            Err(_) => {
                tree.error(
                    parent,
                    "invalid UTF-8 in comment field",
                    range(source_offset, source_span, field_len as usize),
                );
                return false;
            }
        };
        let Some((name, value)) = field_text.split_once('=') else {
            tree.error(
                parent,
                "comment field has no name/value separator",
                range(source_offset, source_span, field_len as usize),
            );
            return false;
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte) && byte != b'=')
        {
            tree.error(
                parent,
                "invalid comment field name",
                range(source_offset, source_span, field_len as usize),
            );
            return false;
        }
        if facts.len() == MAX_FACTS {
            limit_node(tree, parent, source_offset, source_span, bytes.len());
            return false;
        }
        let (kind, value) = fact_value(name, value);
        let (text, truncated) = truncate(&value);
        let node = tree.add(
            Some(parent),
            format!("{kind}: {text}"),
            source_span.unwrap_or(ByteRange::new(
                source_offset.saturating_add(field_start as u64),
                field.len() as u64,
            )),
            NodeKind::Field,
            Some(Value::Text(text.clone())),
        );
        facts.push(DocumentFact { kind, text, node });
        if truncated {
            tree.add(
                Some(parent),
                format!("{name} value truncated at the 512-byte display limit"),
                range(source_offset, source_span, field.len()),
                NodeKind::Field,
                None,
            );
            complete = false;
        }
    }

    if framing_bit {
        let Some(&flag) = bytes.get(cursor) else {
            tree.error(
                parent,
                "missing Vorbis comment framing bit",
                range(source_offset, source_span, bytes.len()),
            );
            return false;
        };
        if flag & 1 == 0 {
            tree.error(
                parent,
                "invalid Vorbis comment framing bit",
                range(source_offset, source_span, 1),
            );
            return false;
        }
        cursor += 1;
    }
    if cursor < bytes.len() {
        let trailing = &bytes[cursor..];
        if !allow_trailing || trailing.iter().any(|&byte| byte != 0) {
            tree.add(
                Some(parent),
                "uninspected bytes after comment fields",
                source_span.unwrap_or(ByteRange::new(
                    source_offset.saturating_add(cursor as u64),
                    trailing.len() as u64,
                )),
                NodeKind::Field,
                None,
            );
            complete = false;
        }
    }
    complete
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    let value = u32::from_le_bytes(bytes.get(*cursor..end)?.try_into().ok()?);
    *cursor = end;
    Some(value)
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(len)?;
    let value = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(value)
}

fn range(offset: u64, span: Option<ByteRange>, len: usize) -> ByteRange {
    span.unwrap_or(ByteRange::new(offset, len as u64))
}

fn limit_node(tree: &mut ParseTree, parent: u32, offset: u64, span: Option<ByteRange>, len: usize) {
    tree.add(
        Some(parent),
        "metadata fact count exceeds the 256-item inspection limit",
        range(offset, span, len),
        NodeKind::Field,
        None,
    );
}

fn truncate(value: &str) -> (String, bool) {
    if value.len() <= MAX_VALUE_BYTES {
        return (value.to_owned(), false);
    }
    let mut end = MAX_VALUE_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

fn fact_value(name: &str, value: &str) -> (&'static str, String) {
    let kind = match name.to_ascii_uppercase().as_str() {
        "TITLE" => "title",
        "ARTIST" | "PERFORMER" => "artist",
        "ALBUM" => "album",
        "ALBUMARTIST" | "ALBUM ARTIST" => "album-artist",
        "DATE" | "YEAR" => "year",
        "TRACKNUMBER" | "TRACK" => "track",
        "DISCNUMBER" | "DISC" => "disc",
        "GENRE" => "genre",
        "COMMENT" | "DESCRIPTION" => "comment",
        "COMPOSER" => "composer",
        "ENCODER" => "encoder",
        "ORGANIZATION" => "organization",
        "LOCATION" => "location",
        _ => "audio-tag",
    };
    if kind == "audio-tag" {
        (kind, format!("{name}: {value}"))
    } else {
        (kind, value.to_owned())
    }
}
