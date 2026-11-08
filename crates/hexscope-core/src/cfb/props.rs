//! Property sets (MS-OLEPS): the SummaryInformation and
//! DocumentSummaryInformation streams, where Office 97–2003 keeps who wrote
//! a document, who saved it last, for which company, from which template,
//! and when.

use crate::zip::office::duration;

/// Longest property stream read.
pub(crate) const MAX: u64 = 1 << 20;

/// `{F29F85E0-4FF9-1068-AB91-08002B27B3D9}`, as stored.
const SUMMARY: [u8; 16] = [
    0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3, 0xD9,
];
/// `{D5CDD502-2E9C-101B-9397-08002B2CF9AE}`, as stored.
const DOCUMENT: [u8; 16] = [
    0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

const VT_I2: u32 = 2;
const VT_LPSTR: u32 = 0x1E;
const VT_LPWSTR: u32 = 0x1F;
const VT_FILETIME: u32 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Set {
    Summary,
    Document,
    /// The second set in DocumentSummaryInformation: properties anyone added.
    Custom,
}

#[derive(Debug)]
enum Val {
    Text(String),
    Time(u64),
    Other,
}

struct Prop {
    set: Set,
    id: u32,
    val: Val,
    /// Where its text or time is in the stream: what a clean copy blanks.
    span: Option<(usize, usize)>,
}

fn le(b: &[u8], at: usize, n: usize) -> Option<u64> {
    Some(
        b.get(at..at.checked_add(n)?)?
            .iter()
            .rev()
            .fold(0, |v, &x| (v << 8) | u64::from(x)),
    )
}

/// Text in a code page: UTF-16 for 1200, UTF-8 for 65001, otherwise read
/// byte for byte as Latin-1, which Windows-1252 matches in letters.
fn decode(b: &[u8], codepage: u64) -> String {
    let s = match codepage {
        1200 => super::utf16(b),
        65001 => String::from_utf8_lossy(b).into_owned(),
        _ => b.iter().map(|&c| char::from(c)).collect(),
    };
    s.trim_end_matches('\0').trim().to_string()
}

fn read(stream: &[u8]) -> Vec<Prop> {
    let mut out = Vec::new();
    if le(stream, 0, 2) != Some(0xFFFE) {
        return out;
    }
    let sets = le(stream, 24, 4).unwrap_or(0).min(2) as usize;
    for k in 0..sets {
        let (Some(fmtid), Some(base)) = (
            stream.get(28 + 20 * k..44 + 20 * k),
            le(stream, 44 + 20 * k, 4),
        ) else {
            break;
        };
        let set = match fmtid {
            f if f == SUMMARY => Set::Summary,
            f if f == DOCUMENT && k == 0 => Set::Document,
            _ if k == 1 => Set::Custom,
            _ => continue,
        };
        let base = base as usize;
        let count = le(stream, base + 4, 4).unwrap_or(0).min(1000) as usize;
        let mut entries: Vec<(u32, usize)> = Vec::new();
        for j in 0..count {
            let (Some(id), Some(off)) = (
                le(stream, base + 8 + 8 * j, 4),
                le(stream, base + 12 + 8 * j, 4),
            ) else {
                break;
            };
            entries.push((id as u32, base.saturating_add(off as usize)));
        }
        let vt = |at: usize| le(stream, at, 4).map(|t| t as u32 & 0xFFFF);
        let codepage = entries
            .iter()
            .find(|&&(id, at)| id == 1 && vt(at) == Some(VT_I2))
            .and_then(|&(_, at)| le(stream, at + 4, 2))
            .unwrap_or(1252);
        for (id, at) in entries {
            let v = at + 4;
            let (val, span) = match vt(at) {
                Some(t @ (VT_LPSTR | VT_LPWSTR)) => {
                    let (unit, cp) = if t == VT_LPWSTR {
                        (2, 1200)
                    } else {
                        (1, codepage)
                    };
                    let n = le(stream, v, 4).unwrap_or(0) as usize;
                    let end = (v + 4)
                        .saturating_add(n.saturating_mul(unit))
                        .min(stream.len());
                    let raw = stream.get(v + 4..end).unwrap_or(&[]);
                    (Val::Text(decode(raw, cp)), Some((v + 4, end)))
                }
                Some(VT_FILETIME) if v + 8 <= stream.len() => {
                    (Val::Time(le(stream, v, 8).unwrap_or(0)), Some((v, v + 8)))
                }
                _ => (Val::Other, None),
            };
            out.push(Prop { set, id, val, span });
        }
    }
    out
}

/// A FILETIME as an Office document's dates are given: `2025-11-02 09:14 UTC`.
fn filetime(t: u64) -> Option<String> {
    let secs = (t / 10_000_000) as i64 - 11_644_473_600;
    // Before 1980 is an empty or damaged field, not a date a document was made.
    if secs < 315_532_800 {
        return None;
    }
    Some(crate::clock::minute_utc(secs))
}

/// What the properties give away, as the kinds an Office document's facts use.
pub(crate) fn facts(stream: &[u8]) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for p in read(stream) {
        let kind = match (p.set, p.id) {
            (Set::Summary, 2) => "title",
            (Set::Summary, 3) => "subject",
            (Set::Summary, 4) => "author",
            (Set::Summary, 5) => "keywords",
            (Set::Summary, 6) => "description",
            (Set::Summary, 7) => "template",
            (Set::Summary, 8) => "editor",
            (Set::Summary, 9) => "revisions",
            (Set::Summary, 10) => "editing",
            (Set::Summary, 12) => "created",
            (Set::Summary, 13) => "modified",
            (Set::Summary, 18) => "application",
            (Set::Document, 2) => "category",
            (Set::Document, 14) => "manager",
            (Set::Document, 15) => "company",
            _ => continue,
        };
        let text = match p.val {
            // Editing time is a length of time, in the same units.
            Val::Time(t) if kind == "editing" => match t / 600_000_000 {
                0 => continue,
                minutes => duration(minutes),
            },
            Val::Time(t) => match filetime(t) {
                Some(d) => d,
                None => continue,
            },
            Val::Text(t) if !t.is_empty() => t,
            _ => continue,
        };
        out.push((kind, text));
    }
    out
}

/// Where a clean copy blanks the stream: every text and time its sets hold.
/// Their lengths stay, so nothing else in the file moves.
pub(crate) fn blanks(stream: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for p in read(stream) {
        let set = match &p.val {
            Val::Text(t) => !t.is_empty(),
            Val::Time(t) => *t != 0,
            Val::Other => false,
        };
        if let (true, Some(span)) = (set, p.span) {
            out.push(span);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{DOCUMENT, SUMMARY, VT_FILETIME, VT_I2, VT_LPSTR, blanks, facts};

    fn u32le(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn u64le(out: &mut Vec<u8>, value: u64) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn string_property(id: u32, value: &str) -> (u32, Vec<u8>) {
        let bytes = value.as_bytes();
        let mut out = VT_LPSTR.to_le_bytes().to_vec();
        u32le(&mut out, bytes.len() as u32 + 1);
        out.extend_from_slice(bytes);
        out.push(0);
        (id, out)
    }

    fn time_property(id: u32, value: u64) -> (u32, Vec<u8>) {
        let mut out = VT_FILETIME.to_le_bytes().to_vec();
        u64le(&mut out, value);
        (id, out)
    }

    fn set(properties: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![0; 8 + properties.len() * 8];
        u32le_at(&mut out, 4, properties.len() as u32);
        for (index, (id, value)) in properties.iter().enumerate() {
            let entry = 8 + index * 8;
            u32le_at(&mut out, entry, *id);
            let offset = out.len() as u32;
            u32le_at(&mut out, entry + 4, offset);
            out.extend_from_slice(value);
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
        }
        let len = out.len() as u32;
        u32le_at(&mut out, 0, len);
        out
    }

    fn u32le_at(out: &mut [u8], at: usize, value: u32) {
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn stream(fmtid: &[u8; 16], properties: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let property_set = set(properties);
        let base = 48u32;
        let mut out = vec![0xFE, 0xFF, 0, 0, 0, 0, 0, 0];
        out.extend_from_slice(&[0; 16]);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(fmtid);
        out.extend_from_slice(&base.to_le_bytes());
        out.extend_from_slice(&property_set);
        out
    }

    #[test]
    fn every_office97_property_kind_has_a_blankable_read_back_span() {
        let filetime = 132_537_600_000_000_000;
        let mut codepage = VT_I2.to_le_bytes().to_vec();
        codepage.extend_from_slice(&1252u16.to_le_bytes());
        codepage.extend_from_slice(&[0; 2]);
        let mut summary = stream(
            &SUMMARY,
            &[
                (1, codepage),
                string_property(2, "Title"),
                string_property(3, "Subject"),
                string_property(4, "Author"),
                string_property(5, "Keywords"),
                string_property(6, "Description"),
                string_property(7, "Template"),
                string_property(8, "Editor"),
                string_property(9, "3"),
                time_property(10, 3_600_000_000),
                time_property(12, filetime),
                time_property(13, filetime + 60_000_000),
                string_property(18, "Office"),
            ],
        );
        let mut document = stream(
            &DOCUMENT,
            &[
                string_property(2, "Category"),
                string_property(14, "Manager"),
                string_property(15, "Company"),
            ],
        );
        let mut before = facts(&summary);
        before.extend(facts(&document));
        let kinds: Vec<_> = before.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(
            kinds,
            [
                "title",
                "subject",
                "author",
                "keywords",
                "description",
                "template",
                "editor",
                "revisions",
                "editing",
                "created",
                "modified",
                "application",
                "category",
                "manager",
                "company",
            ]
        );

        let summary_spans = blanks(&summary);
        let document_spans = blanks(&document);
        assert_eq!(summary_spans.len() + document_spans.len(), kinds.len());
        for (start, end) in summary_spans {
            summary[start..end].fill(0);
        }
        for (start, end) in document_spans {
            document[start..end].fill(0);
        }
        assert!(facts(&summary).is_empty());
        assert!(facts(&document).is_empty());
    }
}
