//! ICC profile headers and tag tables, shared by the image containers that
//! carry profiles. This reads structure only; it does not apply transforms.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use std::collections::HashSet;

const HEADER_SIZE: usize = 128;
const TAG_COUNT_SIZE: usize = 4;
const TAG_ENTRY_SIZE: usize = 12;
pub(crate) const MAX_PROFILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TAGS: usize = 4096;

#[derive(Debug)]
struct Tag {
    signature: String,
    entry_at: u64,
    offset: u64,
    size: u64,
}

#[derive(Debug)]
struct Issue {
    label: String,
    offset: u64,
    len: u64,
    error: bool,
}

#[derive(Debug)]
struct Profile {
    size: u32,
    version: String,
    class: String,
    data_space: String,
    pcs: String,
    rendering_intent: u32,
    tag_count: u32,
    tags: Vec<Tag>,
    issues: Vec<Issue>,
}

/// Inspects an ICC profile and adds a readable tree beneath `parent`.
/// `map` translates profile-relative byte positions to the carrier's bytes.
/// For compressed profiles a caller can map every position to the compressed
/// source span, since inflated offsets have no one-to-one file position.
pub(crate) fn add_profile(
    tree: &mut ParseTree,
    parent: NodeId,
    bytes: &[u8],
    source: ByteRange,
    mut map: impl FnMut(u64, u64) -> ByteRange,
) {
    let profile_node = tree.add(
        Some(parent),
        "ICC profile",
        source,
        NodeKind::Container,
        None,
    );

    if bytes.len() > MAX_PROFILE_BYTES {
        tree.add(
            Some(profile_node),
            "profile inspection limit",
            source,
            NodeKind::Field,
            Some(Value::Text("over 16 MiB".into())),
        );
        return;
    }

    let profile = match parse(bytes) {
        Ok(profile) => profile,
        Err((label, offset, len)) => {
            tree.error(profile_node, label, map(offset, len));
            return;
        }
    };

    let summary = format!(
        "ICC v{} · {} · {} → {} · {} tags",
        profile.version, profile.class, profile.data_space, profile.pcs, profile.tag_count
    );
    tree.set_value(profile_node, Some(Value::Text(summary)));

    add_field(
        tree,
        profile_node,
        "profile size",
        0,
        4,
        Value::U64(profile.size as u64),
        &mut map,
    );
    add_field(
        tree,
        profile_node,
        "version",
        8,
        4,
        Value::Text(profile.version.clone()),
        &mut map,
    );
    add_field(
        tree,
        profile_node,
        "device class",
        12,
        4,
        Value::Text(profile.class.clone()),
        &mut map,
    );
    add_field(
        tree,
        profile_node,
        "data colour space",
        16,
        4,
        Value::Text(profile.data_space.clone()),
        &mut map,
    );
    add_field(
        tree,
        profile_node,
        "PCS",
        20,
        4,
        Value::Text(profile.pcs.clone()),
        &mut map,
    );
    add_field(
        tree,
        profile_node,
        "signature",
        36,
        4,
        Value::Text("acsp".into()),
        &mut map,
    );
    let intent = match profile.rendering_intent {
        0 => "perceptual",
        1 => "media-relative colorimetric",
        2 => "saturation",
        3 => "ICC-absolute colorimetric",
        _ => "undefined",
    };
    tree.add(
        Some(profile_node),
        "rendering intent",
        map(64, 4),
        if profile.rendering_intent <= 3 {
            NodeKind::Field
        } else {
            NodeKind::Warning
        },
        Some(Value::Enum {
            raw: profile.rendering_intent as u64,
            name: intent,
        }),
    );

    let table_len = (profile.tag_count as u64)
        .saturating_mul(TAG_ENTRY_SIZE as u64)
        .min(bytes.len().saturating_sub(132) as u64)
        .min((profile.size as usize).saturating_sub(132) as u64);
    let table_node = tree.add(
        Some(profile_node),
        "tag table",
        map(128, TAG_COUNT_SIZE as u64 + table_len),
        NodeKind::Container,
        Some(Value::U64(profile.tag_count as u64)),
    );
    add_field(
        tree,
        table_node,
        "tag count",
        128,
        4,
        Value::U64(profile.tag_count as u64),
        &mut map,
    );
    for tag in &profile.tags {
        let entry = tree.add(
            Some(table_node),
            "tag",
            map(tag.entry_at, TAG_ENTRY_SIZE as u64),
            NodeKind::Container,
            Some(Value::Text(tag.signature.clone())),
        );
        tree.add(
            Some(entry),
            "tag signature",
            map(tag.entry_at, 4),
            NodeKind::Field,
            Some(Value::Text(tag.signature.clone())),
        );
        tree.add(
            Some(entry),
            "tag offset",
            map(tag.entry_at + 4, 4),
            NodeKind::Field,
            Some(Value::U64(tag.offset)),
        );
        tree.add(
            Some(entry),
            "tag length",
            map(tag.entry_at + 8, 4),
            NodeKind::Field,
            Some(Value::U64(tag.size)),
        );
        // Tag data is addressed from the profile root, not from its table row.
        tree.add(
            Some(profile_node),
            "tag data",
            map(tag.offset, tag.size),
            NodeKind::Field,
            Some(Value::Text(format!("{} · {} B", tag.signature, tag.size))),
        );
    }
    for issue in profile.issues {
        if issue.error {
            tree.error(profile_node, issue.label, map(issue.offset, issue.len));
        } else {
            tree.warning(profile_node, issue.label, map(issue.offset, issue.len));
        }
    }
    if (profile.size as usize) < bytes.len() {
        let extra = profile.size as u64;
        tree.warning(
            profile_node,
            "bytes after the declared ICC profile size",
            map(extra, bytes.len() as u64 - extra),
        );
    }
}

fn add_field(
    tree: &mut ParseTree,
    parent: NodeId,
    label: &str,
    offset: u64,
    len: u64,
    value: Value,
    map: &mut impl FnMut(u64, u64) -> ByteRange,
) {
    tree.add(
        Some(parent),
        label,
        map(offset, len),
        NodeKind::Field,
        Some(value),
    );
}

fn parse(bytes: &[u8]) -> Result<Profile, (String, u64, u64)> {
    if bytes.len() < HEADER_SIZE + TAG_COUNT_SIZE {
        return Err((
            "ICC profile header or tag count is truncated".into(),
            bytes.len() as u64,
            0,
        ));
    }
    let size = be32(bytes, 0).expect("four-byte header field");
    if size < (HEADER_SIZE + TAG_COUNT_SIZE) as u32 {
        return Err((
            "ICC profile size is smaller than its header and tag count".into(),
            0,
            4,
        ));
    }
    if size as usize > bytes.len() {
        return Err((
            "ICC profile runs past its carrier data".into(),
            0,
            bytes.len() as u64,
        ));
    }
    if bytes.get(36..40) != Some(b"acsp") {
        return Err(("ICC profile signature is not acsp".into(), 36, 4));
    }

    let tag_count = be32(bytes, 128).expect("tag count follows complete header");
    let table_bytes = (tag_count as u64).checked_mul(TAG_ENTRY_SIZE as u64);
    let table_end = table_bytes.and_then(|n| 132u64.checked_add(n));
    let mut issues = Vec::new();
    if table_end.is_none_or(|end| end > size as u64) {
        issues.push(Issue {
            label: "ICC tag table runs past the declared profile size".into(),
            offset: 128,
            len: 4,
            error: true,
        });
    }
    let safe_table_end = table_end.unwrap_or(u64::MAX).min(size as u64);
    let available = safe_table_end.saturating_sub(132) / TAG_ENTRY_SIZE as u64;
    let inspect_count = (tag_count as u64).min(available).min(MAX_TAGS as u64) as usize;
    if tag_count as usize > MAX_TAGS {
        issues.push(Issue {
            label: format!("ICC tag count exceeds the {MAX_TAGS}-tag inspection limit"),
            offset: 128,
            len: 4,
            error: false,
        });
    }

    let mut tags = Vec::with_capacity(inspect_count);
    let mut signatures = HashSet::with_capacity(inspect_count);
    for index in 0..inspect_count {
        let entry_at = 132 + (index * TAG_ENTRY_SIZE) as u64;
        let at = entry_at as usize;
        let raw_sig = bytes.get(at..at + 4).unwrap_or_default();
        let signature = signature(raw_sig);
        let offset = be32(bytes, at + 4).unwrap_or(0) as u64;
        let len = be32(bytes, at + 8).unwrap_or(0) as u64;
        if !signatures.insert(signature.clone()) {
            issues.push(Issue {
                label: format!("duplicate ICC tag signature {signature}"),
                offset: entry_at,
                len: TAG_ENTRY_SIZE as u64,
                error: false,
            });
        }
        let end = offset.checked_add(len);
        if offset < safe_table_end || end.is_none_or(|end| end > size as u64) {
            issues.push(Issue {
                label: format!("ICC tag {signature} points outside the tagged data area"),
                offset: entry_at + 4,
                len: 8,
                error: true,
            });
        } else {
            tags.push(Tag {
                signature,
                entry_at,
                offset,
                size: len,
            });
        }
    }

    let version_bytes = bytes.get(8..12).unwrap_or_default();
    let version = if version_bytes.len() == 4 {
        format!(
            "{}.{}.{}",
            version_bytes[0],
            version_bytes[1] >> 4,
            version_bytes[1] & 0x0F
        )
    } else {
        "unknown".into()
    };
    Ok(Profile {
        size,
        version,
        class: signature(bytes.get(12..16).unwrap_or_default()),
        data_space: signature(bytes.get(16..20).unwrap_or_default()),
        pcs: signature(bytes.get(20..24).unwrap_or_default()),
        rendering_intent: be32(bytes, 64).unwrap_or(u32::MAX),
        tag_count,
        tags,
        issues,
    })
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn signature(bytes: &[u8]) -> String {
    if bytes.len() == 4 && bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        String::from_utf8_lossy(bytes).trim_end().to_string()
    } else {
        format!(
            "0x{}",
            bytes.iter().map(|b| format!("{b:02X}")).collect::<String>()
        )
    }
}

const ICC: &str = "https://www.color.org/specifications/ICC.1-2022-05.pdf";
const fn icc(text: &'static str) -> Doc {
    Doc::new(text).cite("ICC.1:2022 §7", ICC)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const TABLE: Table = &[
    (
        "ICC profile",
        icc(
            "A colour profile: its header describes the colour spaces and intent, and its tag table points to the profile data.",
        ),
    ),
    ("profile size", icc("The profile's declared byte length.")),
    (
        "version",
        icc("The ICC specification version this profile follows."),
    ),
    (
        "device class",
        icc("What kind of device or transform this profile describes."),
    ),
    (
        "data colour space",
        icc("The colour space of the image data this profile describes."),
    ),
    (
        "PCS",
        icc("The profile connection space used to match colours between devices."),
    ),
    (
        "signature",
        icc("The fixed acsp signature identifies this as an ICC profile."),
    ),
    (
        "rendering intent",
        icc("How colours outside the destination device's range should be mapped."),
    ),
    (
        "tag table",
        icc("The profile's table of tags and the offsets and lengths of their data."),
    ),
    (
        "tag count",
        icc("How many tag entries the profile declares."),
    ),
    (
        "tag",
        icc("One ICC tag entry: its signature, offset and byte length."),
    ),
    (
        "tag signature",
        icc("The four-character name of an ICC tag."),
    ),
    (
        "tag offset",
        icc("Where this tag's data begins, relative to the profile."),
    ),
    (
        "tag length",
        icc("How many bytes this tag's data occupies."),
    ),
    (
        "tag data",
        icc("The bytes for this ICC tag, at the offset recorded in the table."),
    ),
    (
        "profile inspection limit",
        icc("The profile is larger than this inspector's 16 MiB limit."),
    ),
    (
        "ICC profile header or tag count is truncated",
        damage("The profile does not contain its complete 128-byte header and 4-byte tag count."),
    ),
    (
        "ICC profile size is smaller than its header and tag count",
        damage("The declared profile is too short to contain its header and tag count."),
    ),
    (
        "ICC profile runs past its carrier data",
        damage("The declared profile extends beyond the profile bytes stored by the image."),
    ),
    (
        "ICC profile signature is not acsp",
        damage("The expected ICC profile signature is missing."),
    ),
    (
        "ICC tag table runs past the declared profile size",
        damage("The profile's tag table is longer than the declared profile."),
    ),
    (
        "ICC tag * points outside the tagged data area",
        damage("A tag points inside the table or beyond the profile's declared size."),
    ),
    (
        "duplicate ICC tag signature *",
        damage("ICC tag signatures must be unique in a profile."),
    ),
    (
        "ICC tag count exceeds the *-tag inspection limit",
        icc("The profile declares more tags than this inspector reads."),
    ),
    (
        "bytes after the declared ICC profile size",
        damage("The carrier has extra bytes after the profile's declared end."),
    ),
];

pub(crate) mod docs {
    use super::{TABLE, lookup};
    use crate::docs::{Doc, Table};

    pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
        lookup(TABLE, label, problem)
    }

    pub(crate) const EVERY: &[Table] = &[TABLE];
}
