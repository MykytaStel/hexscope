//! Audio writers: format checks and payload preservation stay together.
use super::{CleanError, Cleaned, Removed};
use crate::model::NodeKind;

/// Removes only the bounded ID3v2, APE and ID3v1 tags at an MP3's edges.
/// MPEG frames are copied byte for byte and never decoded or re-encoded.
pub(super) fn clean_mp3(data: &[u8]) -> Result<Cleaned, CleanError> {
    let ranges = crate::audio::removable_tag_ranges(data).ok_or(CleanError::Damaged)?;
    if ranges.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    let mut bytes = Vec::with_capacity(data.len());
    let mut removed = Vec::with_capacity(ranges.len());
    let mut at = 0u64;
    for (range, what) in ranges {
        let end = range.end();
        if range.start < at || end > data.len() as u64 {
            return Err(CleanError::Damaged);
        }
        bytes.extend_from_slice(
            data.get(at as usize..range.start as usize)
                .ok_or(CleanError::Damaged)?,
        );
        removed.push(Removed {
            what: what.into(),
            bytes: range.len,
        });
        at = end;
    }
    bytes.extend_from_slice(data.get(at as usize..).ok_or(CleanError::Damaged)?);
    Ok(Cleaned {
        bytes,
        removed,
        orientation_kept: None,
    })
}

/// Removes FLAC Vorbis comments and complete picture blocks while leaving
/// every retained metadata block and compressed audio frame byte for byte intact.
pub(super) fn clean_flac(
    data: &[u8],
    doc: &crate::audio::AudioDocument,
) -> Result<Cleaned, CleanError> {
    use crate::audio::AudioFormat;

    if doc.format != AudioFormat::Flac || !data.starts_with(b"fLaC") {
        return Err(CleanError::Damaged);
    }
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
    let root = doc.tree.root().ok_or(CleanError::Damaged)?;
    let children = &doc.tree.get(root).children;
    let blocks = children
        .iter()
        .map(|&id| doc.tree.get(id))
        .filter(|node| node.label.starts_with("FLAC ") && node.label.ends_with(" block"))
        .collect::<Vec<_>>();
    let comments = blocks
        .iter()
        .filter(|node| node.label == "FLAC Vorbis comments block")
        .count();
    let pictures = blocks
        .iter()
        .filter(|node| node.label == "FLAC picture block")
        .count();
    if comments == 0 && pictures == 0 {
        return Err(CleanError::NothingToRemove);
    }
    if comments > 1 {
        return Err(CleanError::Damaged);
    }

    let frame = children
        .iter()
        .map(|&id| doc.tree.get(id))
        .find(|node| node.label == "audio frames (not decoded)")
        .ok_or(CleanError::Damaged)?;
    if frame.range.len == 0 || frame.range.end() != data.len() as u64 {
        return Err(CleanError::Damaged);
    }

    let mut cursor = 4u64;
    for block in &blocks {
        if block.range.start != cursor
            || block.range.len < 4
            || block.range.end() > frame.range.start
        {
            return Err(CleanError::Damaged);
        }
        cursor = block.range.end();
    }
    if cursor != frame.range.start {
        return Err(CleanError::Damaged);
    }

    let retained = blocks
        .iter()
        .copied()
        .filter(|node| {
            node.label != "FLAC Vorbis comments block" && node.label != "FLAC picture block"
        })
        .collect::<Vec<_>>();
    if retained.is_empty() || retained[0].label != "FLAC STREAMINFO block" {
        return Err(CleanError::Damaged);
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(b"fLaC");
    for (index, block) in retained.iter().enumerate() {
        let start = usize::try_from(block.range.start).map_err(|_| CleanError::Damaged)?;
        let end = usize::try_from(block.range.end()).map_err(|_| CleanError::Damaged)?;
        let bytes = data.get(start..end).ok_or(CleanError::Damaged)?;
        let mut copied = bytes.to_vec();
        copied[0] = (copied[0] & 0x7f) | if index + 1 == retained.len() { 0x80 } else { 0 };
        out.extend_from_slice(&copied);
    }
    out.extend_from_slice(
        data.get(frame.range.start as usize..)
            .ok_or(CleanError::Damaged)?,
    );

    let mut removed = Vec::new();
    if comments == 1 {
        removed.push(Removed {
            what: "FLAC Vorbis comments".into(),
            bytes: blocks
                .iter()
                .find(|node| node.label == "FLAC Vorbis comments block")
                .ok_or(CleanError::Damaged)?
                .range
                .len,
        });
    }
    if pictures > 0 {
        removed.push(Removed {
            what: "FLAC pictures".into(),
            bytes: blocks
                .iter()
                .filter(|node| node.label == "FLAC picture block")
                .map(|node| node.range.len)
                .sum(),
        });
    }

    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

/// Removes complete WAVE INFO and Broadcast Wave chunks while preserving
/// every retained RIFF chunk, including the format and audio data, byte for byte.
pub(super) fn clean_wav(
    data: &[u8],
    doc: &crate::audio::AudioDocument,
) -> Result<Cleaned, CleanError> {
    use crate::audio::AudioFormat;

    if doc.format != AudioFormat::Wav
        || data.get(..4) != Some(b"RIFF")
        || data.get(8..12) != Some(b"WAVE")
        || data.len() < 12
        || !doc.metadata_complete
        || doc
            .tree
            .nodes()
            .iter()
            .any(|node| matches!(node.kind, NodeKind::Error | NodeKind::Warning))
    {
        return Err(CleanError::Damaged);
    }

    let declared_size =
        u32::from_le_bytes(data[4..8].try_into().map_err(|_| CleanError::Damaged)?) as usize;
    let form_end = 8usize
        .checked_add(declared_size)
        .filter(|&end| end == data.len())
        .ok_or(CleanError::Damaged)?;
    let root = doc.tree.root().ok_or(CleanError::Damaged)?;
    let chunks = doc
        .tree
        .get(root)
        .children
        .iter()
        .map(|&id| doc.tree.get(id))
        .filter(|node| {
            node.kind == NodeKind::Container
                && node.label.starts_with("WAVE ")
                && node.label.ends_with(" chunk")
        })
        .collect::<Vec<_>>();

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..12]);
    let mut cursor = 12u64;
    let mut info_bytes = 0u64;
    let mut broadcast_bytes = 0u64;
    let mut has_format = false;
    let mut has_audio = false;

    for chunk in chunks {
        let end = chunk.range.end();
        if chunk.range.start != cursor || end > form_end as u64 {
            return Err(CleanError::Damaged);
        }
        let start = usize::try_from(chunk.range.start).map_err(|_| CleanError::Damaged)?;
        let end = usize::try_from(end).map_err(|_| CleanError::Damaged)?;
        let bytes = data.get(start..end).ok_or(CleanError::Damaged)?;
        match chunk.label.as_str() {
            "WAVE LIST/INFO chunk" => info_bytes += chunk.range.len,
            "WAVE bext chunk" => broadcast_bytes += chunk.range.len,
            "WAVE fmt chunk" => {
                has_format = true;
                out.extend_from_slice(bytes);
            }
            "WAVE data chunk" => {
                has_audio = true;
                out.extend_from_slice(bytes);
            }
            _ => out.extend_from_slice(bytes),
        }
        cursor = end as u64;
    }
    if cursor != form_end as u64 || !has_format || !has_audio {
        return Err(CleanError::Damaged);
    }
    if info_bytes == 0 && broadcast_bytes == 0 {
        return Err(CleanError::NothingToRemove);
    }

    let new_size = u32::try_from(out.len().checked_sub(8).ok_or(CleanError::Damaged)?)
        .map_err(|_| CleanError::Damaged)?;
    out[4..8].copy_from_slice(&new_size.to_le_bytes());

    let after = crate::audio::parse(&out).ok_or(CleanError::Damaged)?;
    if after.format != AudioFormat::Wav
        || !after.metadata_complete
        || after.tree.nodes().iter().any(|node| {
            matches!(
                node.label.as_str(),
                "WAVE LIST/INFO chunk" | "WAVE bext chunk"
            )
        })
    {
        return Err(CleanError::Damaged);
    }

    let mut removed = Vec::with_capacity(2);
    if info_bytes > 0 {
        removed.push(Removed {
            what: "WAVE INFO metadata".into(),
            bytes: info_bytes,
        });
    }
    if broadcast_bytes > 0 {
        removed.push(Removed {
            what: "Broadcast Wave metadata".into(),
            bytes: broadcast_bytes,
        });
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}
