//! The file as a whole: what each byte belongs to, and how random its bytes
//! are. Both feed views that show the entire file at once.

use crate::docs::{Concern, describe};
use crate::document::Format;
use crate::model::{NodeId, NodeKind, ParseTree};

/// What a stretch of bytes is for, in words anyone reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Role {
    /// The picture, or an archive's files.
    Content = 0,
    /// Text, camera data, document properties.
    Metadata = 1,
    /// A preview image stored inside the file.
    Thumbnail = 2,
    /// Headers, tables, directories: what holds the rest together.
    Structure = 3,
    /// Bytes outside the format, or that nothing points at.
    Hidden = 4,
    /// Bytes that could not be read as what they claim to be.
    Damaged = 5,
}

/// A run of bytes with one role, and the node that gave it that role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slice {
    pub start: u64,
    pub len: u64,
    pub role: Role,
    /// None for bytes no node covers.
    pub node: Option<NodeId>,
}

/// Every byte of the file in exactly one slice, in file order, with
/// neighbours of the same role merged. The deepest node with a role decides
/// a byte's role, so a thumbnail inside a metadata segment is a thumbnail.
pub fn composition(tree: &ParseTree, format: Format, file_len: u64) -> Vec<Slice> {
    let nodes = tree.nodes();
    if file_len == 0 {
        return Vec::new();
    }
    if format == Format::Unknown {
        let root = tree.root();
        return vec![Slice {
            start: 0,
            len: file_len,
            role: Role::Structure,
            node: root,
        }];
    }

    // Parents are always added before their children, so one pass gives
    // every node's depth.
    let mut depth = vec![0u32; nodes.len()];
    for n in nodes {
        if let Some(p) = n.parent {
            depth[n.id as usize] = depth[p as usize] + 1;
        }
    }

    // Intervals as start and end events, clipped to the file: the position
    // doubled, plus one for a start, so that at one position ends sort
    // before starts. (One key type for every sort in the crate keeps the
    // build small: each kind of sort is kilobytes of code.)
    let mut events: Vec<(u64, usize)> = Vec::new();
    let mut roles: Vec<(Role, u32, NodeId)> = Vec::new();
    for n in nodes {
        let start = n.range.start.min(file_len);
        let end = n.range.end().min(file_len);
        if end <= start {
            continue;
        }
        if let Some(role) = role_of(tree, n.id, format, depth[n.id as usize]) {
            let i = roles.len();
            roles.push((role, depth[n.id as usize], n.id));
            events.push(((start << 1) | 1, i));
            events.push((end << 1, i));
        }
    }
    // At one position, ends before starts: a node ending where the next
    // begins does not overlap it.
    events.sort_unstable();

    // Sweep: between consecutive event positions, the deepest active node
    // (latest added among equals, as children follow parents) owns the bytes.
    // Kept sorted; nested parts make it a short list.
    let mut active: Vec<(u32, usize)> = Vec::new();
    let mut out: Vec<Slice> = Vec::new();
    let mut at = 0u64;
    let mut k = 0;
    while at < file_len {
        while k < events.len() && events[k].0 >> 1 <= at {
            let (key, i) = events[k];
            let is_start = key & 1 == 1;
            let key = (roles[i].1, i);
            match (active.binary_search(&key), is_start) {
                (Err(at), true) => active.insert(at, key),
                (Ok(at), false) => {
                    active.remove(at);
                }
                _ => {}
            }
            k += 1;
        }
        let next = events.get(k).map_or(file_len, |e| (e.0 >> 1).min(file_len));
        let (role, node) = match active.last() {
            Some(&(_, i)) => (roles[i].0, Some(roles[i].2)),
            None => (Role::Hidden, None),
        };
        push(&mut out, at, next - at, role, node);
        at = next;
    }
    out
}

/// Appends a slice, merging it into the previous one when the role is the
/// same. The merged slice keeps the node of its larger part.
fn push(out: &mut Vec<Slice>, start: u64, len: u64, role: Role, node: Option<NodeId>) {
    if len == 0 {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.role == role
        && last.start + last.len == start
    {
        if len > last.len {
            last.node = node;
        }
        last.len += len;
        return;
    }
    out.push(Slice {
        start,
        len,
        role,
        node,
    });
}

/// The role a node claims for its bytes, if it claims one. Problems claim by
/// their concern; parts by their format and label.
fn role_of(tree: &ParseTree, id: NodeId, format: Format, depth: u32) -> Option<Role> {
    let node = tree.try_get(id)?;
    match node.kind {
        NodeKind::Error => return Some(Role::Damaged),
        NodeKind::Warning => {
            return match describe(tree, id, format).and_then(|d| d.concern) {
                Some(Concern::Damage) => Some(Role::Damaged),
                Some(Concern::Hidden) => Some(Role::Hidden),
                _ => None,
            };
        }
        _ => {}
    }
    let label = node.label.as_str();
    match format {
        Format::Png if depth == 1 => Some(match label {
            "IDAT" => Role::Content,
            "tEXt" | "zTXt" | "iTXt" | "eXIf" | "tIME" => Role::Metadata,
            _ => Role::Structure,
        }),
        Format::Jpeg if label == "thumbnail" => Some(Role::Thumbnail),
        Format::Jpeg if depth == 1 => Some(match label {
            "scan data" => Role::Content,
            "COM" => Role::Metadata,
            l if l.starts_with("APP") && !l.ends_with("· JFIF") => Role::Metadata,
            _ => Role::Structure,
        }),
        Format::Heif
            if label.starts_with("item ")
                && label.contains(" · ")
                && !label.ends_with("location") =>
        {
            let what = match &node.value {
                Some(crate::model::Value::Text(t)) => t.as_str(),
                _ => "",
            };
            Some(match what {
                "EXIF metadata" | "XMP metadata" => Role::Metadata,
                "thumbnail" => Role::Thumbnail,
                _ if label.contains("· Exif") || label.contains("· mime") => Role::Metadata,
                _ => Role::Content,
            })
        }
        Format::Heif if depth == 1 => Some(Role::Structure),
        Format::Video if depth == 1 => Some(if label == "mdat" {
            Role::Content
        } else {
            Role::Structure
        }),
        // What describes the movie, wherever it sits in the tree.
        Format::Video
            if matches!(label, "udta" | "meta" | "loci")
                || label.starts_with('©')
                || (label == "uuid"
                    && matches!(&node.value, Some(crate::model::Value::Text(t)) if t == "XMP metadata")) =>
        {
            Some(Role::Metadata)
        }
        // A module's code and data run; custom sections describe it,
        // except those linkers need.
        Format::Wasm if depth == 1 => Some(match label {
            "section · code" | "section · data" => Role::Content,
            "custom section · dylink.0" | "custom section · linking" => Role::Structure,
            l if l.starts_with("custom section · reloc.") => Role::Structure,
            l if l.starts_with("custom section · ") => Role::Metadata,
            _ => Role::Structure,
        }),
        Format::Pdf if label.starts_with("object ") => {
            let what = match &node.value {
                Some(crate::model::Value::Text(t)) => t.as_str(),
                _ => "",
            };
            Some(match what {
                "document information" | "XMP metadata" => Role::Metadata,
                "cross-reference stream" | "object stream" | "linearization" => Role::Structure,
                "image" | "embedded file" => Role::Content,
                t if t.ends_with("stream") => Role::Content,
                _ => Role::Structure,
            })
        }
        Format::Pdf
            if matches!(
                label,
                "header"
                    | "binary marker"
                    | "cross-reference table"
                    | "trailer"
                    | "startxref"
                    | "%%EOF"
            ) =>
        {
            Some(Role::Structure)
        }
        Format::Zip if depth == 1 => Some(Role::Structure),
        Format::Zip if label == "data" && depth == 2 => {
            let entry = tree.try_get(node.parent?)?;
            let meta = entry.label.starts_with("docProps/") || entry.label.starts_with("META-INF/");
            Some(if meta { Role::Metadata } else { Role::Content })
        }
        _ => None,
    }
}

/// Windows shorter than this cannot reach 8 bits per byte, and a scale whose
/// top depends on file size would mislead.
const MIN_WINDOW: usize = 256;

/// Shannon entropy, in bits per byte from 0 to 8, of up to `bins` equal
/// windows covering `data`. Near 8: compressed or encrypted. Near 0: zeros or
/// padding. Text and structure fall between.
pub fn entropy(data: &[u8], bins: usize) -> Vec<f32> {
    if data.is_empty() || bins == 0 {
        return Vec::new();
    }
    // Rounded down, so every window is at least MIN_WINDOW long.
    let n = bins.min(data.len() / MIN_WINDOW).max(1);
    let window = data.len().div_ceil(n);
    data.chunks(window)
        .map(|w| {
            let mut counts = [0u32; 256];
            for &b in w {
                counts[b as usize] += 1;
            }
            let total = w.len() as f64;
            let h: f64 = counts
                .iter()
                .filter(|&&c| c > 0)
                .map(|&c| {
                    let p = c as f64 / total;
                    -p * p.log2()
                })
                .sum();
            h as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::tests::{damaged, fixtures};
    use crate::document::parse;

    fn slices_of(bytes: &[u8]) -> Vec<Slice> {
        let doc = parse(bytes);
        composition(doc.tree(), doc.format(), bytes.len() as u64)
    }

    fn largest(slices: &[Slice]) -> Role {
        let mut total = std::collections::HashMap::new();
        for s in slices {
            *total.entry(s.role).or_insert(0u64) += s.len;
        }
        *total.iter().max_by_key(|&(_, &n)| n).unwrap().0
    }

    fn sample(name: &str) -> Vec<u8> {
        fixtures()
            .into_iter()
            .find(|(path, _)| path.ends_with(name))
            .unwrap_or_else(|| panic!("no fixture {name}"))
            .1
    }

    #[test]
    fn every_byte_of_every_file_has_exactly_one_role() {
        for (name, bytes) in fixtures().into_iter().chain(damaged()) {
            let slices = slices_of(&bytes);
            let mut at = 0;
            for (i, s) in slices.iter().enumerate() {
                assert_eq!(s.start, at, "{name}: slice {i} leaves a gap or overlaps");
                assert!(s.len > 0, "{name}: empty slice {i}");
                if i > 0 {
                    assert_ne!(slices[i - 1].role, s.role, "{name}: slices {i} not merged");
                }
                at += s.len;
            }
            assert_eq!(at, bytes.len() as u64, "{name}: does not reach the end");
        }
    }

    #[test]
    fn roles_read_the_way_people_expect() {
        let png = slices_of(&sample("samples/sample.png"));
        assert_eq!(largest(&png), Role::Content, "a PNG is mostly picture");

        let photo = slices_of(&sample("samples/photo.jpg"));
        let roles: Vec<_> = photo.iter().map(|s| s.role).collect();
        assert!(roles.contains(&Role::Thumbnail) && roles.contains(&Role::Metadata));
        assert!(!roles.contains(&Role::Hidden) && !roles.contains(&Role::Damaged));

        let docx = slices_of(&sample("fixtures/report.docx"));
        assert!(
            docx.iter().any(|s| s.role == Role::Metadata),
            "docProps is metadata"
        );
        assert!(docx.iter().any(|s| s.role == Role::Content));

        let mut prefixed = b"MZ not a program at all.".to_vec();
        prefixed.extend(sample("fixtures/report.docx"));
        assert_eq!(slices_of(&prefixed)[0].role, Role::Hidden);

        let png = sample("samples/sample.png");
        let cut = slices_of(&png[..png.len() - 20]);
        assert!(cut.iter().any(|s| s.role == Role::Damaged), "{cut:?}");

        let unknown = slices_of(b"GIF89a just a header");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].role, Role::Structure);
    }

    #[test]
    fn entropy_runs_from_zeros_to_noise() {
        assert_eq!(entropy(&[0u8; 4096], 16), vec![0.0; 16]);

        let every: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        assert!(entropy(&every, 16).iter().all(|&h| (h - 8.0).abs() < 1e-6));

        // Pseudo-random bytes: as random as data gets, like compressed data.
        let mut x = 0x2545_F491u32;
        let noise: Vec<u8> = (0..65536)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        assert!(entropy(&noise, 8).iter().all(|&h| h > 7.5));

        // Windows stay at least 256 bytes: 1,000 bytes allow 3, not 1,024.
        assert_eq!(entropy(&[1u8; 1000], 1024).len(), 3);
        assert_eq!(entropy(&[1u8; 100], 1024).len(), 1);
        assert!(entropy(&[], 1024).is_empty());
    }
}
