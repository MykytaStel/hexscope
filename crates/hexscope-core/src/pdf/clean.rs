//! A PDF rewritten without what it says about the people behind it.
//!
//! Blanking the author in place is not enough: a PDF edited after it was
//! first saved still holds the earlier version, the old author and the old
//! text with it. So the copy is written anew, as one revision: only the
//! objects the document still uses, reached from its catalog, each copied
//! byte for byte, and a fresh cross-reference table. What nothing reaches
//! is left behind: the document information, objects an update replaced,
//! objects an update deleted. XMP streams are kept, but empty.

use super::facts::{MAX_DECODED_TOTAL, unpack};
use super::lexer::{Lexer, Obj};
use super::{ObjRec, parse_with};
use crate::clean::{CleanError, Cleaned, Removed};
use crate::model::NodeKind;
use std::collections::{BTreeMap, BTreeSet};

/// An XMP packet with nothing in it.
const EMPTY_XMP: &[u8] = b"<?xpacket begin=\"\xEF\xBB\xBF\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/><?xpacket end=\"w\"?>";

/// Where the current version of an object comes from.
enum Source<'a> {
    Top(&'a ObjRec),
    /// Unpacked from an object stream: its value's bytes, and the value.
    Packed(Vec<u8>, Obj),
}

pub(crate) fn clean_pdf(data: &[u8]) -> Result<Cleaned, CleanError> {
    let (doc, ctx) = parse_with(data);
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    if doc.encrypted {
        return Err(CleanError::Locked);
    }
    let trailer = |key: &str| ctx.trailers.iter().rev().find_map(|t| t.get(key));
    let Some(&Obj::Ref(root, root_gen)) = trailer("Root") else {
        return Err(CleanError::Damaged);
    };

    // The current version of every object: later in the file wins, as an
    // update comes after what it replaces.
    let mut current: BTreeMap<u32, (u64, Source)> = BTreeMap::new();
    let mut budget = MAX_DECODED_TOTAL;
    for rec in &ctx.objects {
        if let Some((bytes, packed)) = unpack(data, rec, ctx.crypt.as_ref(), &mut budget) {
            for (n, start, end) in packed {
                let Some(value) = Lexer::new(&bytes[..end], start).value() else {
                    return Err(CleanError::Damaged);
                };
                // Only the value: whatever follows it before the next
                // object is not part of it.
                let r = value.range;
                let text = bytes[r.start as usize..r.end() as usize].to_vec();
                current.insert(n, (rec.start, Source::Packed(text, value.obj)));
            }
        } else if rec.value.get("Type").and_then(Obj::name) == Some("ObjStm") {
            // Objects we cannot unpack could be the ones the document uses.
            return Err(CleanError::Unreadable);
        }
        if current.get(&rec.num).is_none_or(|&(at, _)| at <= rec.start) {
            current.insert(rec.num, (rec.start, Source::Top(rec)));
        }
    }

    // Everything the catalog reaches, and nothing else.
    let mut reached = BTreeSet::new();
    let mut queue = vec![root];
    while let Some(n) = queue.pop() {
        if !reached.insert(n) {
            continue;
        }
        match current.get(&n) {
            Some((_, Source::Top(rec))) => refs(&rec.value, &mut queue),
            Some((_, Source::Packed(_, obj))) => refs(obj, &mut queue),
            None => {}
        }
    }

    let version = doc.version.as_deref().unwrap_or("1.7");
    let mut out = format!("%PDF-{version}\n%").into_bytes();
    out.extend_from_slice(&[0xE2, 0xE3, 0xCF, 0xD3, b'\n']);
    let mut offsets: Vec<(u32, u16, usize)> = Vec::new();
    let mut xmp_bytes = 0u64;
    for &n in &reached {
        let Some((_, source)) = current.get(&n) else {
            continue;
        };
        let at = out.len();
        match source {
            Source::Top(rec) => {
                let gen_ = rec.gen_;
                out.extend_from_slice(format!("{n} {gen_} obj\n").as_bytes());
                let is_xmp = rec.value.get("Type").and_then(Obj::name) == Some("Metadata");
                match rec.stream {
                    Some((range, _)) if is_xmp => {
                        xmp_bytes += range.len;
                        out.extend_from_slice(
                            format!(
                                "<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n",
                                EMPTY_XMP.len()
                            )
                            .as_bytes(),
                        );
                        out.extend_from_slice(EMPTY_XMP);
                        out.extend_from_slice(b"\nendstream");
                    }
                    Some((range, _)) => {
                        out.extend_from_slice(slice(data, rec.value_range));
                        out.extend_from_slice(b"\nstream\n");
                        out.extend_from_slice(slice(data, range));
                        out.extend_from_slice(b"\nendstream");
                    }
                    None => out.extend_from_slice(slice(data, rec.value_range)),
                }
                out.extend_from_slice(b"\nendobj\n");
                offsets.push((n, gen_, at));
            }
            Source::Packed(text, _) => {
                out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
                out.extend_from_slice(text);
                out.extend_from_slice(b"\nendobj\n");
                offsets.push((n, 0, at));
            }
        }
    }

    // One subsection per run of consecutive numbers, and the free head.
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n0 1\n0000000000 65535 f\r\n");
    let mut i = 0;
    while i < offsets.len() {
        let mut j = i + 1;
        while j < offsets.len() && offsets[j].0 == offsets[j - 1].0 + 1 {
            j += 1;
        }
        out.extend_from_slice(format!("{} {}\n", offsets[i].0, j - i).as_bytes());
        for &(_, gen_, at) in &offsets[i..j] {
            out.extend_from_slice(format!("{at:010} {gen_:05} n\r\n").as_bytes());
        }
        i = j;
    }
    let size = offsets.last().map_or(1, |&(n, ..)| n + 1);
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {size} /Root {root} {root_gen} R >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );

    // What went, in words.
    let mut removed = Vec::new();
    let length = |rec: &ObjRec| doc.tree.get(rec.node).range.len;
    let info = match trailer("Info") {
        Some(&Obj::Ref(n, _)) => Some(n),
        _ => None,
    };
    let (mut info_bytes, mut earlier, mut unused) = (0u64, 0u64, 0u64);
    for rec in &ctx.objects {
        let ty = rec.value.get("Type").and_then(Obj::name);
        let superseded =
            !matches!(current.get(&rec.num), Some((_, Source::Top(r))) if std::ptr::eq(*r, rec));
        if Some(rec.num) == info {
            // Counted below, wherever its current version is.
        } else if matches!(ty, Some("XRef" | "ObjStm")) {
            // Structure the rewrite replaces.
        } else if superseded {
            earlier += length(rec);
        } else if !reached.contains(&rec.num) {
            unused += length(rec);
        }
    }
    match info.and_then(|n| current.get(&n)) {
        Some((_, Source::Top(rec))) => info_bytes = length(rec),
        Some((_, Source::Packed(text, _))) => info_bytes = text.len() as u64,
        None => {}
    }
    if info_bytes > 0 {
        removed.push(Removed {
            what: "Document information: author, title, programs, dates".into(),
            bytes: info_bytes,
        });
    }
    if xmp_bytes > 0 {
        removed.push(Removed {
            what: "XMP: editing history and author".into(),
            bytes: xmp_bytes,
        });
    }
    if doc.edits > 0 || earlier > 0 {
        removed.push(Removed {
            what: "Earlier versions of the document, kept by later edits".into(),
            bytes: earlier,
        });
    }
    if unused > 0 {
        removed.push(Removed {
            what: "Objects the document no longer uses".into(),
            bytes: unused,
        });
    }
    if let Some(Obj::Array(ids)) = trailer("ID") {
        let bytes = ids
            .iter()
            .map(|i| match &i.obj {
                Obj::Str(s) => s.len() as u64,
                _ => 0,
            })
            .sum();
        removed.push(Removed {
            what: "The file identifier that links copies of the document".into(),
            bytes,
        });
    }
    if removed.is_empty() {
        return Err(CleanError::NothingToRemove);
    }
    Ok(Cleaned {
        bytes: out,
        removed,
        orientation_kept: None,
    })
}

fn slice(data: &[u8], range: crate::model::ByteRange) -> &[u8] {
    data.get(range.start as usize..range.end() as usize)
        .unwrap_or_default()
}

/// Every object number `obj` refers to.
fn refs(obj: &Obj, out: &mut Vec<u32>) {
    match obj {
        Obj::Ref(n, _) => out.push(*n),
        Obj::Array(items) => items.iter().for_each(|i| refs(&i.obj, out)),
        Obj::Dict(entries) => entries.iter().for_each(|e| refs(&e.value.obj, out)),
        _ => {}
    }
}
