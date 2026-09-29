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
    let [y, m, d, hh, mm, _] = crate::clock::civil(secs);
    Some(format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02} UTC"))
}

/// What the properties give away, as the kinds an Office document's facts use.
pub(crate) fn facts(stream: &[u8]) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for p in read(stream) {
        let kind = match (p.set, p.id) {
            (Set::Summary, 2) => "title",
            (Set::Summary, 4) => "author",
            (Set::Summary, 7) => "template",
            (Set::Summary, 8) => "editor",
            (Set::Summary, 9) => "revisions",
            (Set::Summary, 10) => "editing",
            (Set::Summary, 12) => "created",
            (Set::Summary, 13) => "modified",
            (Set::Summary, 18) => "application",
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
