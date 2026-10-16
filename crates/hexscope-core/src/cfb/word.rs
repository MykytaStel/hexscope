//! What a Word 97–2003 document keeps in its own streams beside the property
//! streams: the dates, revision number and editing time of its document
//! properties (the DOP), who saved it and where (the SttbSavedBy), and who
//! wrote its comments, with their initials — [MS-DOC] 2.5.1 (the FIB), 2.7.2
//! (DopBase), 2.9.276 (SttbSavedBy), 2.9.15 (ATRDPre10). A copy whose
//! property streams are blank still carries all of these unless they are
//! blanked too: ExifTool reads them, and so does anyone with a hex viewer.

use super::{Compound, le, utf16};
use crate::zip::office::duration;

/// Where the FIB's array of offsets and lengths names each part, in bytes
/// from the array's start: fcDop, fcPlcfandRef, fcGrpXstAtnOwners and
/// fcSttbSavedBy.
const DOP: u64 = 0xF8;
const ANNOTATIONS: u64 = 0x20;
const OWNERS: u64 = 0x120;
const SAVED_BY: u64 = 0x238;
/// A table stream larger than this is not read for these parts.
const MAX: u64 = 16 << 20;
/// Bytes `start..end` of a stream.
type Span = (usize, usize);
/// What a clean copy blanks, and where.
type Blank = (&'static str, Vec<Span>);
/// An annotation's record before Word 2000's: its author's initials, 20 bytes, then 10 more.
const ATRD: usize = 30;

pub(crate) struct Word {
    /// The table stream's entry, and its bytes.
    pub(crate) table: usize,
    bytes: Vec<u8>,
    /// The WordDocument stream's entry, and its first bytes, where the FIB is.
    pub(crate) word: usize,
    fib: Vec<u8>,
    /// Where the FIB's array of offsets and lengths starts, and how many pairs it holds.
    blob: u64,
    pairs: u64,
}

impl Word {
    pub(crate) fn open(c: &Compound) -> Option<Word> {
        let word = c.find(0, "WordDocument")?;
        let fib = c.read(&c.entries[word], 2048);
        if le(&fib, 0, 2)? != 0xA5EC {
            return None;
        }
        // Which table stream the document uses: 1Table when the flag says so.
        let table = c.find(
            0,
            if le(&fib, 0x0A, 2)? & 0x0200 != 0 {
                "1Table"
            } else {
                "0Table"
            },
        )?;
        let mut pos = 32;
        pos += 2 + 2 * le(&fib, pos, 2)?;
        pos += 2 + 4 * le(&fib, pos, 2)?;
        let pairs = le(&fib, pos, 2)?;
        let e = &c.entries[table];
        Some(Word {
            table,
            bytes: c.read(e, e.size.min(MAX)),
            word,
            fib,
            blob: pos + 2,
            pairs,
        })
    }

    /// The bytes `start..end` of the table stream the FIB gives a part, when they are there.
    fn part(&self, at: u64) -> Option<(usize, usize)> {
        if at + 8 > self.pairs * 8 {
            return None;
        }
        let start = usize::try_from(le(&self.fib, self.blob + at, 4)?).ok()?;
        let len = usize::try_from(le(&self.fib, self.blob + at + 4, 4)?).ok()?;
        let end = start.checked_add(len)?;
        (len > 0 && end <= self.bytes.len()).then_some((start, end))
    }

    /// The strings of the list of who saved it and where: name, path, name, path…
    fn saved_by(&self) -> Vec<(usize, usize)> {
        self.part(SAVED_BY)
            .map(|(a, b)| {
                strings(&self.bytes[a..b])
                    .into_iter()
                    .map(|(s, e)| (a + s, a + e))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The comments' authors' names, in order, as the comments refer to them.
    fn owners(&self) -> Vec<(usize, usize)> {
        let Some((a, b)) = self.part(OWNERS) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut pos = a;
        while let Some(n) = le(&self.bytes, pos as u64, 2) {
            let end = pos + 2 + 2 * n as usize;
            if end > b {
                break;
            }
            out.push((pos + 2, end));
            pos = end;
        }
        out
    }

    /// Each comment's record: where its author's initials are.
    fn initials(&self) -> Vec<(usize, usize)> {
        let Some((a, b)) = self.part(ANNOTATIONS) else {
            return Vec::new();
        };
        // n + 1 positions of four bytes, then n records.
        let len = b - a;
        if len < 4 || (len - 4) % (4 + ATRD) != 0 {
            return Vec::new();
        }
        let n = (len - 4) / (4 + ATRD);
        (0..n)
            .filter_map(|i| {
                let at = a + (n + 1) * 4 + i * ATRD;
                let cch = (le(&self.bytes, at as u64, 2)? as usize).min(9);
                Some((at + 2, at + 2 + 2 * cch))
            })
            .collect()
    }

    fn text(&self, (a, b): (usize, usize)) -> String {
        utf16(&self.bytes[a..b])
            .trim_end_matches('\0')
            .trim()
            .to_string()
    }

    /// What these parts give away, as the kinds an Office document's facts use.
    pub(crate) fn facts(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let t = &self.bytes;
        if let Some((at, _)) = self.part(DOP).filter(|&(a, b)| b - a >= 0x26) {
            let at = at as u64;
            for (kind, off) in [("created", 0x14), ("modified", 0x18), ("printed", 0x1C)] {
                if let Some(d) = le(t, at + off, 4).and_then(|v| dttm(v as u32)) {
                    out.push((kind, d));
                }
            }
            // The first save is revision 1: only more says something.
            if let Some(n) = le(t, at + 0x20, 2).filter(|&n| n > 1) {
                out.push(("revisions", n.to_string()));
            }
            if let Some(m) = le(t, at + 0x22, 4).filter(|&m| m > 0) {
                out.push(("editing", duration(m)));
            }
        }
        let saved: Vec<String> = self.saved_by().into_iter().map(|s| self.text(s)).collect();
        let (mut names, mut paths) = (Vec::new(), Vec::new());
        for (i, s) in saved.into_iter().enumerate() {
            let list = if i % 2 == 0 { &mut names } else { &mut paths };
            if !s.is_empty() && !list.contains(&s) {
                list.push(s);
            }
        }
        if let Some(last) = names.last() {
            out.push(("editor", last.clone()));
        }
        if !paths.is_empty() {
            out.push(("folder", paths.join("; ")));
        }
        let owners: Vec<String> = self
            .owners()
            .into_iter()
            .map(|s| self.text(s))
            .filter(|s| !s.is_empty())
            .collect();
        let count = self.initials().len();
        if count > 0 && !owners.is_empty() {
            let word = if count == 1 { "comment" } else { "comments" };
            out.push(("comments", format!("{count} {word} by {}", joined(&owners))));
        }
        out
    }

    /// Where a clean copy blanks these parts, in each stream's own bytes
    /// (start, end), and what each is: the WordDocument stream's are the
    /// FIB's pair naming the list of who saved it, so no reader looks for it.
    pub(crate) fn blanks(&self) -> (Vec<Blank>, Vec<Span>) {
        let mut table = Vec::new();
        if let Some((at, _)) = self.part(DOP).filter(|&(a, b)| b - a >= 0x26) {
            let span = (at + 0x14, at + 0x26);
            if self.bytes[span.0..span.1].iter().any(|&b| b != 0) {
                table.push((
                    "the document's dates, revision number and editing time",
                    vec![span],
                ));
            }
        }
        let mut fib = Vec::new();
        if let Some(span) = self.part(SAVED_BY) {
            table.push(("who saved it, and where", vec![span]));
            let at = (self.blob + SAVED_BY) as usize;
            fib.push((at, at + 8));
        }
        let names: Vec<(usize, usize)> = self
            .owners()
            .into_iter()
            .chain(self.initials())
            .filter(|&(a, b)| self.bytes[a..b].iter().any(|&x| x != 0))
            .collect();
        if !names.is_empty() {
            table.push(("who wrote the comments", names));
        }
        (table, fib)
    }
}

/// An extended string table's strings: 0xFFFF, a count, the size of extra
/// data after each, then each string's length in characters and its UTF-16.
fn strings(b: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if le(b, 0, 2) != Some(0xFFFF) {
        return out;
    }
    let (Some(n), Some(extra)) = (le(b, 2, 2), le(b, 4, 2)) else {
        return out;
    };
    let mut pos = 6;
    for _ in 0..n {
        let Some(cch) = le(b, pos as u64, 2) else {
            break;
        };
        let end = pos + 2 + 2 * cch as usize;
        if end > b.len() {
            break;
        }
        out.push((pos + 2, end));
        pos = end + extra as usize;
    }
    out
}

/// A date and time as Word packs it in 32 bits, local time: `2002-12-09 15:26`.
fn dttm(v: u32) -> Option<String> {
    let (min, hour, day, month, year) = (
        v & 0x3F,
        (v >> 6) & 0x1F,
        (v >> 11) & 0x1F,
        (v >> 16) & 0x0F,
        (v >> 20) & 0x1FF,
    );
    if v == 0 || !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 {
        return None;
    }
    Some(format!(
        "{:04}-{month:02}-{day:02} {hour:02}:{min:02}",
        1900 + year
    ))
}

/// "A", "A and B", "A, B and C".
fn joined(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_date_reads_as_word_wrote_it() {
        // 2002-12-09 15:26: year 102, month 12, day 9, hour 15, minute 26.
        let v = (102 << 20) | (12 << 16) | (9 << 11) | (15 << 6) | 26;
        assert_eq!(dttm(v).as_deref(), Some("2002-12-09 15:26"));
        assert_eq!(dttm(0), None);
        assert_eq!(dttm(13 << 16 | 1 << 11), None);
    }

    #[test]
    fn a_string_table_gives_each_string() {
        let mut b = vec![0xFF, 0xFF, 2, 0, 0, 0];
        for s in ["Olena", "C:\\Docs\\plan.doc"] {
            b.extend_from_slice(&(s.len() as u16).to_le_bytes());
            b.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
        }
        let got: Vec<String> = strings(&b)
            .into_iter()
            .map(|(a, e)| utf16(&b[a..e]))
            .collect();
        assert_eq!(got, ["Olena", "C:\\Docs\\plan.doc"]);
        assert!(strings(&b[..9]).len() < 2);
    }

    /// A table stream holding who saved it and where, and two comments by one author.
    fn sample() -> Word {
        let utf = |s: &str| {
            s.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<u8>>()
        };
        let mut bytes = vec![0u8; 0x40];
        // The DOP: created 2024-03-05 10:20, revision 7, 95 minutes of editing.
        let dop = bytes.len();
        bytes.extend([0u8; 0x40]);
        let created: u32 = (124 << 20) | (3 << 16) | (5 << 11) | (10 << 6) | 20;
        bytes[dop + 0x14..dop + 0x18].copy_from_slice(&created.to_le_bytes());
        bytes[dop + 0x20..dop + 0x22].copy_from_slice(&7u16.to_le_bytes());
        bytes[dop + 0x22..dop + 0x26].copy_from_slice(&95u32.to_le_bytes());
        let saved = bytes.len();
        bytes.extend([0xFF, 0xFF, 2, 0, 0, 0]);
        for s in ["Olena Koval", "C:\\Users\\olena\\plan.doc"] {
            bytes.extend((s.encode_utf16().count() as u16).to_le_bytes());
            bytes.extend(utf(s));
        }
        let owners = bytes.len();
        bytes.extend(5u16.to_le_bytes());
        bytes.extend(utf("Petro"));
        // Two comments: three positions, then two records with initials "PI".
        let notes = bytes.len();
        bytes.extend([0u8; 12]);
        for _ in 0..2 {
            let mut record = [0u8; ATRD];
            record[..2].copy_from_slice(&2u16.to_le_bytes());
            record[2..6].copy_from_slice(&utf("PI"));
            bytes.extend(record);
        }
        let mut fib = vec![0u8; 0x300];
        let mut pair = |at: u64, start: usize, end: usize| {
            fib[at as usize..at as usize + 4].copy_from_slice(&(start as u32).to_le_bytes());
            fib[at as usize + 4..at as usize + 8]
                .copy_from_slice(&((end - start) as u32).to_le_bytes());
        };
        pair(DOP, dop, dop + 0x40);
        pair(SAVED_BY, saved, owners);
        pair(OWNERS, owners, notes);
        pair(ANNOTATIONS, notes, bytes.len());
        Word {
            table: 0,
            bytes,
            word: 0,
            fib,
            blob: 0,
            pairs: 0x300 / 8,
        }
    }

    #[test]
    fn its_own_record_says_when_who_and_where() {
        let w = sample();
        assert_eq!(
            w.facts(),
            vec![
                ("created", "2024-03-05 10:20".to_string()),
                ("revisions", "7".to_string()),
                ("editing", duration(95)),
                ("editor", "Olena Koval".to_string()),
                ("folder", "C:\\Users\\olena\\plan.doc".to_string()),
                ("comments", "2 comments by Petro".to_string()),
            ]
        );
    }

    #[test]
    fn a_blanked_record_says_nothing() {
        let mut w = sample();
        let (table, fib) = w.blanks();
        let what: Vec<&str> = table.iter().map(|(what, _)| *what).collect();
        assert_eq!(
            what,
            [
                "the document's dates, revision number and editing time",
                "who saved it, and where",
                "who wrote the comments"
            ]
        );
        for (a, b) in table.iter().flat_map(|(_, spans)| spans) {
            w.bytes[*a..*b].fill(0);
        }
        for (a, b) in fib {
            w.fib[a..b].fill(0);
        }
        assert_eq!(w.facts(), Vec::new());
        assert_eq!(w.blanks(), (Vec::new(), Vec::new()));
    }

    #[test]
    fn names_are_joined_as_a_list() {
        let n = |v: &[&str]| joined(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(n(&["A"]), "A");
        assert_eq!(n(&["A", "B"]), "A and B");
        assert_eq!(n(&["A", "B", "C"]), "A, B and C");
    }
}
