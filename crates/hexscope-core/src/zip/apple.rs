//! What a Mac adds to an archive it makes: for each file with extended
//! attributes, a hidden `__MACOSX/…/._name` entry in AppleDouble format
//! (RFC 1740), whose Finder-information entry carries the attributes in an
//! `ATTR` block, as macOS's `copyfile` writes them. Some say where the file
//! was downloaded from (`kMDItemWhereFroms`, a binary property list of
//! addresses), with which app and when (`com.apple.quarantine`), and the
//! tags someone gave it in the Finder.

use super::office::DocumentFact;
use super::{ZipEntry, extract};

/// AppleDouble entries read, and how large each may be.
const MAX_FILES: usize = 200;
const MAX_SIZE: u64 = 1 << 20;
/// Files named in one fact, and characters in it.
const MAX_NAMED: usize = 5;
const MAX_TEXT: usize = 600;

/// A Mac's note about a file, not a file: `__MACOSX/…` or `._name`.
pub(crate) fn is_apple_double(name: &str) -> bool {
    name.starts_with("__MACOSX/") || name.rsplit('/').next().is_some_and(|n| n.starts_with("._"))
}

/// Where the archive's files were downloaded from, and their Finder tags.
pub(crate) fn mac_facts(data: &[u8], entries: &[ZipEntry]) -> Vec<DocumentFact> {
    let (mut downloaded, mut tagged) = (Vec::new(), Vec::new());
    let (mut first_download, mut first_tag) = (None, None);
    for e in entries
        .iter()
        .filter(|e| is_apple_double(&e.name) && !e.is_dir() && e.uncompressed <= MAX_SIZE)
        .take(MAX_FILES)
    {
        let Ok(bytes) = extract(data, e, MAX_SIZE) else {
            continue;
        };
        let file = described(&e.name);
        let mut froms = Vec::new();
        let mut how = None;
        for (name, value) in attributes(&bytes) {
            match name {
                "com.apple.metadata:kMDItemWhereFroms" => froms = plist_strings(value),
                "com.apple.quarantine" => how = quarantine(value),
                "com.apple.metadata:_kMDItemUserTags" => {
                    let tags: Vec<String> = plist_strings(value)
                        .into_iter()
                        // "Work\n6": a tag, and the number of its colour.
                        .map(|t| t.split('\n').next().unwrap_or_default().to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                    if !tags.is_empty() {
                        first_tag.get_or_insert(e.node);
                        tagged.push([file.as_str(), ": ", &tags.join(", ")].concat());
                    }
                }
                _ => {}
            }
        }
        if froms.is_empty() && how.is_none() {
            continue;
        }
        first_download.get_or_insert(e.node);
        // In no one language: the app, the time and the addresses say it.
        let mut said = file;
        said.push(':');
        if let Some(how) = how {
            said.push(' ');
            said.push_str(&how);
        }
        if let Some((from, rest)) = froms.split_first() {
            said.push_str(if said.ends_with(':') { " " } else { ", " });
            said.push_str(from);
            if !rest.is_empty() {
                said.push_str(" (");
                said.push_str(&rest.join(", "));
                said.push(')');
            }
        }
        downloaded.push(said);
    }
    let mut facts = Vec::new();
    for (kind, list, node) in [
        ("downloaded", downloaded, first_download),
        ("tags", tagged, first_tag),
    ] {
        if let Some(node) = node {
            facts.push(DocumentFact {
                kind,
                text: listed(list),
                node,
            });
        }
    }
    facts
}

/// `__MACOSX/Report/._plan.txt` is about `Report/plan.txt`.
fn described(name: &str) -> String {
    let path = name.strip_prefix("__MACOSX/").unwrap_or(name);
    match path.rsplit_once('/') {
        Some((dir, file)) => [dir, "/", file.strip_prefix("._").unwrap_or(file)].concat(),
        None => path.strip_prefix("._").unwrap_or(path).to_string(),
    }
}

fn listed(mut items: Vec<String>) -> String {
    let more = items.len().saturating_sub(MAX_NAMED);
    items.truncate(MAX_NAMED);
    let mut text = items.join(" · ");
    if more > 0 {
        text.push_str(" +");
        text.push_str(&more.to_string());
    }
    if text.len() > MAX_TEXT {
        let mut end = MAX_TEXT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push('…');
    }
    text
}

/// A big-endian number of `n` bytes at `at`. Every offset a file gives is
/// added and multiplied with checks: a broken file stops, it never wraps.
fn be(b: &[u8], at: usize, n: usize) -> Option<usize> {
    let bytes = b.get(at..at.checked_add(n)?)?;
    bytes.iter().try_fold(0usize, |v, &x| {
        v.checked_mul(256).map(|v| v | usize::from(x))
    })
}

/// The extended attributes in an AppleDouble file: each name and value.
fn attributes(b: &[u8]) -> Vec<(&str, &[u8])> {
    let mut out = Vec::new();
    if be(b, 0, 4) != Some(0x0005_1607) {
        return out;
    }
    let count = be(b, 24, 2).unwrap_or(0);
    // The Finder-information entry (id 9): 32 bytes, 2 of padding, then ATTR.
    let Some(finder) = (0..count).find_map(|i| {
        let at = 26 + i * 12;
        (be(b, at, 4)? == 9).then(|| be(b, at + 4, 4)).flatten()
    }) else {
        return out;
    };
    let head = finder + 34;
    if b.get(head..head + 4) != Some(b"ATTR") {
        return out;
    }
    // Magic, a debug tag, sizes and offsets, 12 reserved bytes, flags: the count.
    let attrs = be(b, head + 34, 2).unwrap_or(0);
    let mut at = head + 36;
    for _ in 0..attrs.min(256) {
        let (Some(offset), Some(len), Some(name_len)) =
            (be(b, at, 4), be(b, at + 4, 4), be(b, at + 10, 1))
        else {
            break;
        };
        let name = b
            .get(at + 11..at + 11 + name_len)
            .map(|n| n.strip_suffix(&[0]).unwrap_or(n))
            .and_then(|n| std::str::from_utf8(n).ok());
        if let (Some(name), Some(value)) = (
            name,
            offset.checked_add(len).and_then(|end| b.get(offset..end)),
        ) {
            out.push((name, value));
        }
        at += (11 + name_len + 3) & !3;
    }
    out
}

/// `0083;65e2f3a0;Telegram;9F1C…`: flags, the time in hexadecimal seconds,
/// the app that downloaded it, an id. Said as "Telegram, 2024-03-02 09:38 UTC".
fn quarantine(value: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(value).ok()?.trim_end_matches('\0');
    let mut parts = text.split(';');
    let _flags = parts.next()?;
    // Hexadecimal by hand: the standard parser is a sizeable part of the build.
    let time = parts
        .next()
        .filter(|t| !t.is_empty() && t.len() <= 12)
        .and_then(|t| {
            t.bytes().try_fold(0i64, |v, b| {
                Some(v * 16 + i64::from((b as char).to_digit(16)?))
            })
        });
    let app = parts.next().filter(|a| !a.is_empty());
    let mut out = String::new();
    if let Some(app) = app {
        out.push_str(app);
    }
    if let Some(secs) = time.filter(|&s| s > 0) {
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&crate::clock::minute_utc(secs));
    }
    (!out.is_empty()).then_some(out)
}

/// The strings in a binary property list whose top object is an array of
/// them, or is one: all `kMDItemWhereFroms` and Finder tags need.
fn plist_strings(p: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if !p.starts_with(b"bplist00") || p.len() < 40 {
        return out;
    }
    let t = p.len() - 32;
    let (Some(offset_size), Some(ref_size), Some(count), Some(top), Some(table)) = (
        be(p, t + 6, 1),
        be(p, t + 7, 1),
        be(p, t + 8, 8),
        be(p, t + 16, 8),
        be(p, t + 24, 8),
    ) else {
        return out;
    };
    let offset = |i: usize| {
        let at = table.checked_add(i.checked_mul(offset_size)?)?;
        (i < count).then(|| be(p, at, offset_size)).flatten()
    };
    let Some(at) = offset(top) else {
        return out;
    };
    match object(p, at) {
        Some((0xA, len, body)) => {
            for i in 0..len.min(64) {
                if let Some(s) = i
                    .checked_mul(ref_size)
                    .and_then(|r| body.checked_add(r))
                    .and_then(|at| be(p, at, ref_size))
                    .and_then(offset)
                    .and_then(|o| string(p, o))
                {
                    out.push(s);
                }
            }
        }
        _ => out.extend(string(p, at)),
    }
    out
}

/// An object's type, its length, and where its body starts.
fn object(p: &[u8], at: usize) -> Option<(u8, usize, usize)> {
    let marker = *p.get(at)?;
    let (kind, info) = (marker >> 4, usize::from(marker & 0xF));
    if info != 0xF {
        return Some((kind, info, at + 1));
    }
    // A longer length follows as an integer object: 0x1n, then 2^n bytes.
    let int = *p.get(at + 1)?;
    if int >> 4 != 1 || int & 0xF > 3 {
        return None;
    }
    let n = 1 << (int & 0xF);
    Some((kind, be(p, at + 2, n)?, at + 2 + n))
}

fn string(p: &[u8], at: usize) -> Option<String> {
    let (kind, len, body) = object(p, at)?;
    let text = match kind {
        0x5 => String::from_utf8_lossy(p.get(body..body.checked_add(len)?)?).into_owned(),
        0x6 => {
            let units: Vec<u16> = p
                .get(body..body.checked_add(len.checked_mul(2)?)?)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_be_bytes(c))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => return None,
    };
    Some(text.chars().take(300).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::parse_zip;

    fn fixture() -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/mac-report.zip",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn a_zip_made_on_a_mac_says_where_its_files_came_from() {
        // Made by `ditto -c -k --sequesterRsrc`, as the Finder's Compress
        // does, after giving plan.txt the attributes a download leaves.
        let doc = parse_zip(&fixture());
        let facts: Vec<_> = doc
            .facts
            .iter()
            .map(|f| (f.kind, f.text.as_str()))
            .collect();
        assert_eq!(
            facts,
            [
                (
                    "downloaded",
                    "Report/plan.txt: Telegram, 2024-03-02 09:38 UTC, https://mail.example.com/attachment/plan.txt?id=4411 (https://mail.example.com/inbox)"
                ),
                ("tags", "Report/plan.txt: Work, Client: Acme"),
            ]
        );
        assert!(doc.tree.get(doc.facts[0].node).label.contains("._plan.txt"));
    }

    #[test]
    fn the_clean_copy_leaves_the_macs_notes_out_and_keeps_the_files() {
        let c = crate::clean::clean(&fixture()).unwrap();
        let doc = parse_zip(&c.bytes);
        let names: Vec<_> = doc.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Report/", "Report/notes.txt", "Report/plan.txt"]);
        assert!(doc.facts.is_empty());
        assert!(c.removed.iter().any(|r| r.what.starts_with("__MACOSX")));
        let plan = doc
            .entries
            .iter()
            .find(|e| e.name == "Report/plan.txt")
            .unwrap();
        assert_eq!(
            extract(&c.bytes, plan, 100).unwrap(),
            b"Quarterly numbers\n"
        );
    }

    #[test]
    fn notes_about_files_are_told_from_files() {
        assert!(is_apple_double("__MACOSX/Report/._plan.txt"));
        assert!(is_apple_double("Report/._plan.txt"));
        assert!(!is_apple_double("Report/plan.txt"));
        assert_eq!(described("__MACOSX/Report/._plan.txt"), "Report/plan.txt");
        assert_eq!(described("__MACOSX/._top.pdf"), "top.pdf");
    }

    #[test]
    fn broken_notes_say_nothing_and_do_not_panic() {
        let real = fixture();
        let doc = parse_zip(&real);
        let e = doc
            .entries
            .iter()
            .find(|e| e.name.ends_with("._plan.txt"))
            .unwrap();
        let bytes = extract(&real, e, MAX_SIZE).unwrap();
        for cut in 0..bytes.len() {
            let _ = attributes(&bytes[..cut]);
        }
        for (_, value) in attributes(&bytes) {
            for cut in 0..value.len() {
                let _ = plist_strings(&value[..cut]);
                let _ = quarantine(&value[..cut]);
            }
        }
        assert_eq!(plist_strings(b"bplist00"), Vec::<String>::new());
        assert_eq!(quarantine(b"0083;zz;;"), None);
    }
}
