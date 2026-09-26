//! A PDF rewritten without what it says about the people behind it.
//!
//! Blanking the author in place is not enough: a PDF edited after it was
//! first saved still holds the earlier version, the old author and the old
//! text with it. So the copy is written anew, as one revision: only the
//! objects the document still uses, reached from its catalog, each copied
//! byte for byte, and a fresh cross-reference table. What nothing reaches
//! is left behind: the document information, objects an update replaced,
//! objects an update deleted. XMP streams are kept, but empty.

use super::facts::{MAX_DECODED_TOTAL, is_photo, unpack};
use super::lexer::{Lexer, Obj};
use super::{ObjRec, parse_with};
use crate::clean::{CleanError, Cleaned, Removed};
use crate::model::NodeKind;

/// An XMP packet with nothing in it.
const EMPTY_XMP: &[u8] = b"<?xpacket begin=\"\xEF\xBB\xBF\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/><?xpacket end=\"w\"?>";

/// Where the current version of an object comes from.
enum Source<'a> {
    Top(&'a ObjRec),
    /// Unpacked from an object stream: its value's bytes, and the value.
    Packed(Vec<u8>, Obj),
}

/// The current version of every object, sorted by number: of the versions
/// of one number, the last met wins, as an update comes after what it
/// replaces — except that an object in the file's body never gives way to
/// one before it. A sorted list, not a map: maps cost the WebAssembly build
/// several kilobytes.
struct Current<'a>(Vec<(u32, u64, Source<'a>)>);

impl<'a> Current<'a> {
    fn new(versions: Vec<(u32, u64, Source<'a>)>) -> Self {
        let mut order: Vec<(u64, usize)> = versions
            .iter()
            .enumerate()
            .map(|(i, v)| ((u64::from(v.0) << 32) | (i as u64 & 0xFFFF_FFFF), i))
            .collect();
        order.sort_unstable();
        let mut slots: Vec<Option<(u32, u64, Source<'a>)>> =
            versions.into_iter().map(Some).collect();
        let mut out: Vec<(u32, u64, Source<'a>)> = Vec::new();
        for (_, i) in order {
            let Some(v) = slots[i].take() else { continue };
            match out.last_mut() {
                Some(last) if last.0 == v.0 => {
                    let wins = match v.2 {
                        Source::Packed(..) => true,
                        Source::Top(_) => last.1 <= v.1,
                    };
                    if wins {
                        *last = v;
                    }
                }
                _ => out.push(v),
            }
        }
        Current(out)
    }

    fn index(&self, n: u32) -> Option<usize> {
        self.0.binary_search_by_key(&n, |v| v.0).ok()
    }

    fn get(&self, n: u32) -> Option<&Source<'a>> {
        self.index(n).map(|i| &self.0[i].2)
    }
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

    // Every version of every object, in the order they are met.
    let mut versions: Vec<(u32, u64, Source)> = Vec::new();
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
                versions.push((n, rec.start, Source::Packed(text, value.obj)));
            }
        } else if rec.value.get("Type").and_then(Obj::name) == Some("ObjStm") {
            // Objects we cannot unpack could be the ones the document uses.
            return Err(CleanError::Unreadable);
        }
        versions.push((rec.num, rec.start, Source::Top(rec)));
    }
    let current = Current::new(versions);
    // Pages with text under black boxes, marked for redaction, or hidden:
    // their content written again without it.
    let super::redact::Rewrites {
        streams: rewritten,
        removed: taken_out,
        applied,
    } = super::redact::rewrites(data, &ctx);

    // Everything the catalog reaches, and nothing else.
    let mut reached = vec![false; current.0.len()];
    let mut queue = vec![root];
    while let Some(n) = queue.pop() {
        let Some(i) = current.index(n) else { continue };
        if std::mem::replace(&mut reached[i], true) {
            continue;
        }
        match &current.0[i].2 {
            Source::Top(rec) => refs(&rec.value, &mut queue),
            Source::Packed(_, obj) => refs(obj, &mut queue),
        }
    }

    let version = doc.version.as_deref().unwrap_or("1.7");
    let mut out = format!("%PDF-{version}\n%").into_bytes();
    out.extend_from_slice(&[0xE2, 0xE3, 0xCF, 0xD3, b'\n']);
    let mut offsets: Vec<(u32, u16, usize)> = Vec::new();
    let mut xmp_bytes = 0u64;
    let mut photo_bytes = 0u64;
    for (n, _, source) in current
        .0
        .iter()
        .zip(&reached)
        .filter(|(_, r)| **r)
        .map(|(c, _)| c)
    {
        let n = *n;
        let at = out.len();
        // A redaction applied: its mark is retired, a hidden empty annotation
        // in its place so the page's list of annotations still holds.
        if applied.contains(&n) {
            out.extend_from_slice(format!("{n} 0 obj\n<< /Type /Annot /Subtype /Link /Rect [0 0 0 0] /Border [0 0 0] /F 2 >>\nendobj\n").as_bytes());
            offsets.push((n, 0, at));
            continue;
        }
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
                    // Written anew, and so uncompressed: a dictionary of its own.
                    Some(_) if let Some((_, content)) = rewritten.iter().find(|(k, _)| *k == n) => {
                        out.extend_from_slice(
                            format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
                        );
                        out.extend_from_slice(content);
                        out.extend_from_slice(b"\nendstream");
                    }
                    Some((range, _)) => {
                        out.extend_from_slice(slice(data, rec.value_range));
                        out.extend_from_slice(b"\nstream\n");
                        if is_photo(rec) {
                            let mut jpeg = slice(data, range).to_vec();
                            photo_bytes += blank_photo(&mut jpeg);
                            out.extend_from_slice(&jpeg);
                        } else {
                            out.extend_from_slice(slice(data, range));
                        }
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
            !matches!(current.get(rec.num), Some(Source::Top(r)) if std::ptr::eq(*r, rec));
        if Some(rec.num) == info {
            // Counted below, wherever its current version is.
        } else if matches!(ty, Some("XRef" | "ObjStm")) {
            // Structure the rewrite replaces.
        } else if superseded {
            earlier += length(rec);
        } else if !current.index(rec.num).is_some_and(|i| reached[i]) {
            unused += length(rec);
        }
    }
    match info.and_then(|n| current.get(n)) {
        Some(Source::Top(rec)) => info_bytes = length(rec),
        Some(Source::Packed(text, _)) => info_bytes = text.len() as u64,
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
    // First: it is what someone checking a redaction wants to read.
    if taken_out > 0 {
        removed.insert(
            0,
            Removed {
                what: format!(
                    "Text under black boxes, marked for redaction, or hidden: {taken_out} {}",
                    if taken_out == 1 {
                        "character"
                    } else {
                        "characters"
                    }
                ),
                bytes: taken_out,
            },
        );
    }
    if photo_bytes > 0 {
        removed.push(Removed {
            what: "EXIF and XMP of the photos in it: camera, location, dates".into(),
            bytes: photo_bytes,
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

/// Zeroes, in place, what a camera writes into a JPEG: EXIF and XMP (APP1)
/// and Photoshop's IPTC (APP13). Every length stays, so the stream's
/// `/Length` does too, and a reader skips a segment it does not recognise.
/// Returns the bytes zeroed.
pub(crate) fn blank_photo(jpeg: &mut [u8]) -> u64 {
    if !jpeg.starts_with(&[0xFF, 0xD8]) {
        return 0;
    }
    let mut at = 2;
    let mut blanked = 0;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF {
        let marker = jpeg[at + 1];
        match marker {
            // Fill bytes before a marker.
            0xFF => {
                at += 1;
                continue;
            }
            // The picture starts, or ends: nothing after is metadata.
            0xDA | 0xD9 => break,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => {
                at += 2;
                continue;
            }
            _ => {}
        }
        let len = u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]) as usize;
        let end = at + 2 + len;
        if len < 2 || end > jpeg.len() {
            break;
        }
        if marker == 0xE1 || marker == 0xED {
            jpeg[at + 4..end].fill(0);
            blanked += (len - 2) as u64;
        }
        at = end;
    }
    blanked
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
