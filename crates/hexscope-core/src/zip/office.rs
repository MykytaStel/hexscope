//! What an Office document says about who made it: the properties Word,
//! Excel and PowerPoint keep in `docProps/core.xml` and `docProps/app.xml`;
//! Word's comments and tracked changes, with the text a tracked deletion
//! still holds; and what the photos placed in it say.
//!
//! Only a handful of known elements are read, with a tag reader rather than
//! an XML parser: no DTDs, no external entities, no entity expansion beyond
//! the predefined and numeric ones. Nothing in these files can make it do
//! more work than a scan of their bytes.

use super::{ZipEntry, extract};
use crate::exif::PhotoFacts;
use crate::fixed::fixed;
use crate::model::NodeId;

/// Property files larger than this are not read. Real ones are a few KB.
const MAX_PROPS: u64 = 1024 * 1024;
/// A document's body, its comments and each photo are read up to this size.
pub(crate) const MAX_PART: u64 = 64 * 1024 * 1024;
/// Photos looked into, at most.
pub(crate) const MAX_PHOTOS: usize = 64;
/// Longest property kept, in bytes.
const MAX_TEXT: usize = 512;
/// Authors named in a line, at most; the rest are counted.
const MAX_NAMES: usize = 4;

/// One thing a document reveals, and the entry that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFact {
    /// title, author, editor, created, modified, revisions, editing,
    /// company, application, template, comments, tracked, deleted, or
    /// photoplace and photo for a photo in it with and without a location.
    pub kind: &'static str,
    pub text: String,
    pub node: NodeId,
}

pub(super) fn document_facts(data: &[u8], entries: &[ZipEntry]) -> Vec<DocumentFact> {
    let read = |name: &str| {
        let e = entries
            .iter()
            .find(|e| e.name == name && e.uncompressed <= MAX_PROPS)?;
        let bytes = extract(data, e, MAX_PROPS).ok()?;
        Some((String::from_utf8_lossy(&bytes).into_owned(), e.node))
    };
    let mut facts = Vec::new();
    let mut push = |kind, text: Option<String>, node| {
        if let Some(text) = text {
            facts.push(DocumentFact { kind, text, node });
        }
    };

    if let Some((xml, node)) = read("docProps/core.xml") {
        push("title", element_text(&xml, "dc:title"), node);
        push("author", element_text(&xml, "dc:creator"), node);
        push("editor", element_text(&xml, "cp:lastModifiedBy"), node);
        push(
            "created",
            element_text(&xml, "dcterms:created").map(date),
            node,
        );
        push(
            "modified",
            element_text(&xml, "dcterms:modified").map(date),
            node,
        );
        push("revisions", element_text(&xml, "cp:revision"), node);
    }
    if let Some((xml, node)) = read("docProps/app.xml") {
        push("company", element_text(&xml, "Company"), node);
        let app =
            element_text(&xml, "Application").map(|a| match element_text(&xml, "AppVersion") {
                Some(v) => format!("{a} {v}"),
                None => a,
            });
        push("application", app, node);
        let minutes = element_text(&xml, "TotalTime").and_then(|t| t.parse::<u64>().ok());
        push("editing", minutes.filter(|&m| m > 0).map(duration), node);
        push("template", element_text(&xml, "Template"), node);
    }

    let read_part = |name: &str| {
        let e = entries
            .iter()
            .find(|e| e.name == name && e.uncompressed <= MAX_PART)?;
        let bytes = extract(data, e, MAX_PART).ok()?;
        Some((String::from_utf8_lossy(&bytes).into_owned(), e.node))
    };
    if let Some((xml, node)) = read_part("word/comments.xml") {
        let mut authors = Vec::new();
        let mut n = 0;
        for tag in tags(&xml, "w:comment") {
            n += 1;
            if let Some(a) = attribute(tag, "w:author") {
                authors.push(a);
            }
        }
        if n > 0 {
            let what = plural(n, "comment", "comments");
            push("comments", Some(by(what, &authors)), node);
        }
    }
    if let Some((xml, node)) = read_part("word/document.xml") {
        let mut authors = Vec::new();
        let (mut ins, mut del) = (0, 0);
        for (name, count) in [("w:ins", &mut ins), ("w:del", &mut del)] {
            for tag in tags(&xml, name) {
                *count += 1;
                if let Some(a) = attribute(tag, "w:author") {
                    authors.push(a);
                }
            }
        }
        let what = match (ins, del) {
            (0, 0) => None,
            (i, 0) => Some(plural(i, "insertion", "insertions")),
            (0, d) => Some(plural(d, "deletion", "deletions")),
            (i, d) => Some(format!(
                "{} and {}",
                plural(i, "insertion", "insertions"),
                plural(d, "deletion", "deletions")
            )),
        };
        let tracked = what.map(|w| format!("{}, not accepted yet", by(w, &authors)));
        push("tracked", tracked, node);
        push("deleted", deleted_text(&xml), node);
    }
    sheets_and_slides(data, entries, &mut facts);
    facts.extend(photo_facts(data, entries));
    facts
}

/// Every element's text: `<a:t>one</a:t>…<a:t>two</a:t>`.
#[inline(never)]
fn texts(xml: &str, name: &str) -> Vec<String> {
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some((_, body)) = find_tag(xml, from, name) {
        from = body;
        if xml[..body - 1].ends_with('/') {
            continue;
        }
        let Some(len) = xml[body..].find(&close) else {
            break;
        };
        out.push(decode(&xml[body..body + len]));
        from = body + len;
    }
    out
}

/// A sheet's or a slide's number, from its part's name: `slide7.xml` is 7.
fn number(name: &str) -> String {
    let stem = name
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .trim_end_matches(".xml");
    stem.trim_start_matches(|c: char| !c.is_ascii_digit())
        .to_string()
}

/// What an Excel workbook or a PowerPoint deck keeps out of sight: hidden
/// sheets, rows, columns and slides; comments and their authors; links to
/// files on the author's computer; the speaker's notes.
fn sheets_and_slides(data: &[u8], entries: &[ZipEntry], facts: &mut Vec<DocumentFact>) {
    let read = |e: &ZipEntry| {
        (e.uncompressed <= MAX_PART)
            .then(|| extract(data, e, MAX_PART).ok())
            .flatten()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    let parts = |prefix: &'static str| {
        entries.iter().filter(move |e| {
            e.name.starts_with(prefix) && e.name.ends_with(".xml") && !e.name.contains("/_rels/")
        })
    };
    let mut push = |kind: &'static str, text: String, node: NodeId| {
        facts.push(DocumentFact {
            kind,
            text: cap(text),
            node,
        })
    };
    let quote = |v: &[String]| {
        let q: Vec<String> = v.iter().take(MAX_NAMES).map(|s| format!("“{s}”")).collect();
        let more = v.len().saturating_sub(MAX_NAMES);
        if more > 0 {
            format!("{} and {more} more", q.join(", "))
        } else {
            q.join(", ")
        }
    };

    // Sheets hidden from their tabs: "hidden", or "veryHidden", which only
    // a macro or an XML editor brings back.
    if let Some(e) = entries.iter().find(|e| e.name == "xl/workbook.xml")
        && let Some(xml) = read(e)
    {
        let hidden: Vec<String> = tags(&xml, "sheet")
            .into_iter()
            .filter(|t| attribute(t, "state").is_some_and(|s| s != "visible"))
            .filter_map(|t| attribute(t, "name"))
            .collect();
        if !hidden.is_empty() {
            push("hiddensheets", quote(&hidden), e.node);
        }
    }
    let (mut rows, mut cols, mut node) = (0usize, 0usize, None);
    for e in parts("xl/worksheets/") {
        let Some(xml) = read(e) else { continue };
        let hidden = |t: &&&str| attribute(t, "hidden").is_some_and(|h| h == "1" || h == "true");
        let r = tags(&xml, "row").iter().filter(hidden).count();
        let c: usize = tags(&xml, "col")
            .iter()
            .filter(hidden)
            .map(|t| {
                let n = |k| attribute(t, k).and_then(|v| v.parse::<usize>().ok());
                n("max")
                    .zip(n("min"))
                    .map_or(1, |(max, min)| max.saturating_sub(min) + 1)
            })
            .sum();
        if r + c > 0 {
            node.get_or_insert(e.node);
        }
        rows += r;
        cols += c;
    }
    if let Some(node) = node {
        let what = match (rows, cols) {
            (r, 0) => plural(r, "hidden row", "hidden rows"),
            (0, c) => plural(c, "hidden column", "hidden columns"),
            (r, c) => format!(
                "{} and {}",
                plural(r, "hidden row", "hidden rows"),
                plural(c, "hidden column", "hidden columns")
            ),
        };
        push("hiddencells", what, node);
    }

    // Comments, and who wrote them: Excel's notes and threads, PowerPoint's
    // comments old and new.
    let mut authors: Vec<String> = Vec::new();
    for (file, tag, attr) in [
        ("xl/persons/person.xml", "person", "displayName"),
        ("ppt/commentAuthors.xml", "p:cmAuthor", "name"),
        ("ppt/authors.xml", "p188:author", "name"),
    ] {
        if let Some(xml) = entries.iter().find(|e| e.name == file).and_then(&read) {
            authors.extend(
                tags(&xml, tag)
                    .into_iter()
                    .filter_map(|t| attribute(t, attr)),
            );
        }
    }
    let (mut comments, mut node) = (0, None);
    for (prefix, tag) in [
        ("xl/comments", "comment"),
        ("xl/threadedComments/", "threadedComment"),
        ("ppt/comments/", "p:cm"),
        ("ppt/comments/", "p188:cm"),
    ] {
        for e in parts(prefix) {
            let Some(xml) = read(e) else { continue };
            let n = tags(&xml, tag).len();
            if n > 0 {
                node.get_or_insert(e.node);
                comments += n;
                if prefix == "xl/comments" {
                    authors.extend(texts(&xml, "author"));
                }
            }
        }
    }
    if let Some(node) = node {
        push(
            "comments",
            by(plural(comments, "comment", "comments"), &authors),
            node,
        );
    }

    // Links to other files, which name where they are on the author's disk.
    let mut links: Vec<String> = Vec::new();
    let mut node = None;
    for e in entries
        .iter()
        .filter(|e| e.name.starts_with("xl/externalLinks/_rels/"))
    {
        let Some(xml) = read(e) else { continue };
        for t in tags(&xml, "Relationship") {
            if attribute(t, "TargetMode").as_deref() == Some("External")
                && let Some(target) = attribute(t, "Target")
            {
                node.get_or_insert(e.node);
                links.push(target);
            }
        }
    }
    if let Some(node) = node {
        push("links", quote(&links), node);
    }

    // PowerPoint: the speaker's notes, and slides left out of the show.
    let (mut notes, mut first, mut node) = (0, String::new(), None);
    for e in parts("ppt/notesSlides/") {
        let Some(xml) = read(e) else { continue };
        // A notes page also holds its slide's number: that is not a note.
        let words: Vec<String> = texts(&xml, "a:t")
            .into_iter()
            .filter(|t| !t.trim().chars().all(|c| c.is_ascii_digit()))
            .collect();
        let said = words
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !said.is_empty() {
            notes += 1;
            node.get_or_insert(e.node);
            if first.is_empty() {
                first = said;
            }
        }
    }
    if let Some(node) = node {
        push(
            "notes",
            format!("on {}: “{first}”", plural(notes, "slide", "slides")),
            node,
        );
    }
    let mut hidden: Vec<(usize, NodeId)> = Vec::new();
    for e in parts("ppt/slides/") {
        let Some(xml) = read(e) else { continue };
        if let Some(t) = tags(&xml, "p:sld").first()
            && attribute(t, "show").is_some_and(|v| v == "0" || v == "false")
        {
            hidden.push((number(&e.name).parse().unwrap_or(0), e.node));
        }
    }
    if let Some(&(_, node)) = hidden.first() {
        // In order, by hand: a few numbers need no library sort, which
        // would cost the build kilobytes.
        let mut n: Vec<usize> = Vec::new();
        for &(v, _) in &hidden {
            let at = n.iter().position(|&x| x > v).unwrap_or(n.len());
            n.insert(at, v);
        }
        let list: Vec<String> = n.iter().map(|v| v.to_string()).collect();
        push(
            "hiddenslides",
            format!(
                "{} {}",
                if n.len() == 1 { "slide" } else { "slides" },
                list.join(", ")
            ),
            node,
        );
    }
}

/// Entries that hold the pictures placed in a Word, Excel or PowerPoint file.
pub(crate) fn is_media(name: &str) -> bool {
    ["word/media/", "xl/media/", "ppt/media/"]
        .iter()
        .any(|dir| name.starts_with(dir))
}

/// What the photos placed in the document say: each JPEG or PNG under
/// `media/` is read as a photo of its own.
fn photo_facts(data: &[u8], entries: &[ZipEntry]) -> Vec<DocumentFact> {
    let mut out = Vec::new();
    let photos = entries
        .iter()
        .filter(|e| is_media(&e.name) && e.uncompressed <= MAX_PART)
        .take(MAX_PHOTOS);
    for e in photos {
        let Ok(bytes) = extract(data, e, MAX_PART) else {
            continue;
        };
        let facts = if bytes.starts_with(&[0xFF, 0xD8]) {
            crate::jpeg::parse_jpeg(&bytes).facts
        } else if bytes.starts_with(b"\x89PNG") {
            crate::png::parse_png(&bytes).facts
        } else {
            continue;
        };
        let Some(said) = photo_says(&facts) else {
            continue;
        };
        let file = e.name.rsplit('/').next().unwrap_or(&e.name);
        out.push(DocumentFact {
            kind: if facts.location.is_some() {
                "photoplace"
            } else {
                "photo"
            },
            text: cap(format!("{file}: {said}")),
            node: e.node,
        });
    }
    out
}

/// A photo's revealing facts in one line: where, with what, by whom, when.
pub(crate) fn photo_says(f: &PhotoFacts) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(l) = &f.location {
        let ns = if l.latitude < 0.0 { 'S' } else { 'N' };
        let ew = if l.longitude < 0.0 { 'W' } else { 'E' };
        parts.push(format!(
            "taken at {}° {ns}, {}° {ew}",
            fixed(l.latitude.abs(), 5),
            fixed(l.longitude.abs(), 5)
        ));
    }
    let listed = [
        ("", &f.camera),
        ("serial ", &f.serial),
        ("owner ", &f.owner),
        ("", &f.taken),
    ];
    for (prefix, fact) in listed {
        if let Some(fact) = fact {
            parts.push(format!("{prefix}{}", fact.text));
        }
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// What tracked deletions still hold: the text of every `<w:delText>`, one
/// deletion's pieces run together and deletions set apart by an ellipsis.
fn deleted_text(xml: &str) -> Option<String> {
    let mut out = String::new();
    let mut last = 0;
    let mut from = 0;
    while let Some((start, body)) = find_tag(xml, from, "w:delText") {
        from = body;
        if xml[..body - 1].ends_with('/') {
            continue;
        }
        let Some(len) = xml[body..].find("</w:delText>") else {
            break;
        };
        if !out.is_empty() && xml[last..start].contains("</w:del>") {
            out.push_str(" … ");
        }
        out.push_str(&decode(&xml[body..body + len]));
        // Past the close, so a tag inside the text is not read twice.
        last = body + len;
        from = last;
        if out.len() > MAX_TEXT {
            break;
        }
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then(|| cap(format!("“{out}”")))
}

/// `3 comments by Olena and Petro`: distinct authors in order of appearance.
fn by(what: String, authors: &[String]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for a in authors {
        if !a.is_empty() && !names.contains(&a.as_str()) {
            names.push(a);
        }
    }
    let more = names.len().saturating_sub(MAX_NAMES);
    names.truncate(MAX_NAMES);
    let mut list = names.join(", ");
    if more > 0 {
        list = format!("{list} and {}", plural(more, "other", "others"));
    } else if let Some(i) = list.rfind(", ") {
        list.replace_range(i..i + 2, " and ");
    }
    if list.is_empty() {
        what
    } else {
        cap(format!("{what} by {list}"))
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Where the next `<name …>` tag starts, and where its body starts, after
/// its `>`. Lookalikes with a longer name, such as `<w:delText>` for
/// `<w:del>`, are passed over.
fn find_tag(xml: &str, mut from: usize, name: &str) -> Option<(usize, usize)> {
    let open = format!("<{name}");
    loop {
        let at = from + xml[from..].find(&open)?;
        let after = at + open.len();
        match xml[after..].chars().next()? {
            '>' | ' ' | '\t' | '\r' | '\n' | '/' => {
                let end = after + xml[after..].find('>')?;
                return Some((at, end + 1));
            }
            _ => from = after,
        }
    }
}

/// Every `<name …>` tag, as its text from `<` to `>`. A list rather than
/// an iterator, and never inlined: one copy of the loop serves every
/// caller, which keeps the WebAssembly build small.
#[inline(never)]
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some((start, end)) = find_tag(xml, from, name) {
        from = end;
        out.push(&xml[start..end]);
    }
    out
}

/// The value of `name="…"` or `name='…'` in a tag, decoded.
#[inline(never)]
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    loop {
        let at = from + tag[from..].find(name)?;
        from = at + name.len();
        // ` w:author=`, not `w:authorX=` or `xw:author=`.
        let before = tag[..at].chars().next_back();
        if !matches!(before, Some(' ' | '\t' | '\r' | '\n')) {
            continue;
        }
        let rest = tag[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value = &rest[1..];
        let end = value.find(quote)?;
        return Some(cap(decode(value[..end].trim())));
    }
}

/// The text of the first `<name …>text</name>` element, decoded and trimmed.
/// `None` for a missing, empty or self-closing element.
pub(crate) fn element_text(xml: &str, name: &str) -> Option<String> {
    // `<dc:creator>` or `<dc:creator attr…>`, not `<dc:creatorX>`.
    let (_, body) = find_tag(xml, 0, name)?;
    if xml[..body - 1].ends_with('/') {
        return None;
    }
    let close = body + xml[body..].find(&format!("</{name}>"))?;
    let text = decode(xml[body..close].trim());
    if text.is_empty() {
        return None;
    }
    Some(cap(text))
}

/// The five predefined entities and numeric references; anything else is
/// left as written.
fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&i| i <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .map(|h| u32::from_str_radix(h, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn cap(mut s: String) -> String {
    if s.len() > MAX_TEXT {
        let mut end = MAX_TEXT;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

/// `2025-11-02T11:14:00Z` as `2025-11-02 11:14 UTC`; anything else as written.
fn date(raw: String) -> String {
    let b = raw.as_bytes();
    let shaped = b.len() >= 17
        && b.get(4) == Some(&b'-')
        && b.get(10) == Some(&b'T')
        && b.get(13) == Some(&b':')
        && raw.ends_with('Z');
    // `get`, not indexing: the checks above are on bytes, and a multibyte
    // character straddling position 16 must not become a panic.
    match (raw.get(..10), raw.get(11..16)) {
        (Some(day), Some(time)) if shaped => format!("{day} {time} UTC"),
        _ => raw,
    }
}

fn duration(minutes: u64) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::parse_zip;
    use crate::zip::testing::{Archive, Entry, build};

    #[test]
    fn reads_an_element_with_attributes_and_entities() {
        let xml = r#"<a><dcterms:created xsi:type="dcterms:W3CDTF">2025-11-02T11:14:00Z</dcterms:created><dc:creator> Olena &amp; Co &#x41;&#66; </dc:creator></a>"#;
        assert_eq!(
            element_text(xml, "dcterms:created").as_deref(),
            Some("2025-11-02T11:14:00Z")
        );
        assert_eq!(
            element_text(xml, "dc:creator").as_deref(),
            Some("Olena & Co AB")
        );
    }

    #[test]
    fn missing_empty_and_prefixed_lookalikes_give_nothing() {
        let xml = "<Company/><Template></Template><Companyx>no</Companyx>";
        assert_eq!(element_text(xml, "Company"), None);
        assert_eq!(element_text(xml, "Template"), None);
        assert_eq!(element_text(xml, "Application"), None);
        assert_eq!(element_text("<Title>unclosed", "Title"), None);
    }

    #[test]
    fn unknown_entities_stay_as_written_and_text_is_capped() {
        assert_eq!(decode("a &nbsp; b &#xZZ; &"), "a &nbsp; b &#xZZ; &");
        let long = format!("<T>{}</T>", "é".repeat(400));
        let got = element_text(&long, "T").unwrap();
        assert!(got.len() <= MAX_TEXT + '…'.len_utf8() && got.ends_with('…'));
    }

    #[test]
    fn dates_read_as_utc_and_odd_ones_stay_as_written() {
        assert_eq!(date("2025-11-02T11:14:00Z".into()), "2025-11-02 11:14 UTC");
        assert_eq!(date("2 Nov 2025".into()), "2 Nov 2025");
        let odd = "2025-11-02T11:1\u{e9}0000Z".to_string();
        assert_eq!(date(odd.clone()), odd);
    }

    #[test]
    fn office_properties_become_facts() {
        let core = r#"<?xml version="1.0"?><cp:coreProperties><dc:title>Plan</dc:title><dc:creator>Olena Koval</dc:creator><cp:lastModifiedBy>o.koval</cp:lastModifiedBy><cp:revision>14</cp:revision><dcterms:created xsi:type="x">2025-11-02T09:14:00Z</dcterms:created></cp:coreProperties>"#;
        let app = "<Properties><TotalTime>192</TotalTime><Application>Microsoft Office Word</Application><AppVersion>16.0000</AppVersion><Company>Acme</Company></Properties>";
        let b = build(&Archive {
            entries: vec![
                Entry::new("docProps/core.xml", core.as_bytes(), 8),
                Entry::new("docProps/app.xml", app.as_bytes(), 8),
            ],
            ..Default::default()
        });
        let doc = parse_zip(&b.bytes);
        let facts: Vec<_> = doc
            .facts
            .iter()
            .map(|f| (f.kind, f.text.as_str()))
            .collect();
        assert_eq!(
            facts,
            [
                ("title", "Plan"),
                ("author", "Olena Koval"),
                ("editor", "o.koval"),
                ("created", "2025-11-02 09:14 UTC"),
                ("revisions", "14"),
                ("company", "Acme"),
                ("application", "Microsoft Office Word 16.0000"),
                ("editing", "3 h 12 min"),
            ]
        );
        assert_eq!(doc.facts[0].node, doc.entries[0].node);
        assert_eq!(doc.facts[5].node, doc.entries[1].node);
    }

    const COMMENTS: &str = r#"<w:comments xmlns:w="x"><w:comment w:id="0" w:author="Olena Koval" w:initials="OK"><w:p><w:r><w:t>too low</w:t></w:r></w:p></w:comment><w:comment w:id="1" w:author='Petro &amp; Co'/><w:comment w:id="2" w:author="Olena Koval"/></w:comments>"#;
    const BODY: &str = r#"<w:document><w:body><w:p><w:r><w:instrText>PAGE</w:instrText></w:r><w:ins w:id="3" w:author="Petro &amp; Co" w:date="2025-11-02T09:00:00Z"><w:r><w:t>new</w:t></w:r></w:ins><w:del w:id="4" w:author="Olena Koval"><w:r><w:delText xml:space="preserve">we can pay </w:delText></w:r><w:r><w:delText>40k max</w:delText></w:r></w:del><w:r><w:t>kept</w:t></w:r><w:del w:id="5" w:author="Olena Koval"><w:r><w:delText>&lt;draft&gt;</w:delText><w:delText/></w:r></w:del></w:p></w:body></w:document>"#;

    fn photo() -> Vec<u8> {
        use crate::exif::ByteOrder;
        use crate::exif::testing::{Spec, V, build};
        let mut s = Spec::new(ByteOrder::Little);
        s.ifd0 = vec![(0x0110, V::Ascii("Canon EOS R5"))];
        s.exif = vec![(0xA431, V::Ascii("HX-000042"))];
        s.gps = vec![
            (0x01, V::Ascii("S")),
            (0x02, V::Rational(vec![(33, 1), (51, 1), (0, 1)])),
            (0x03, V::Ascii("E")),
            (0x04, V::Rational(vec![(151, 1), (12, 1), (0, 1)])),
        ];
        crate::jpeg::testing::jpeg_with_exif(Some(&build(s)))
    }

    fn facts_of(entries: Vec<Entry>) -> Vec<(&'static str, String)> {
        let b = build(&Archive {
            entries,
            ..Default::default()
        });
        parse_zip(&b.bytes)
            .facts
            .into_iter()
            .map(|f| (f.kind, f.text))
            .collect()
    }

    #[test]
    fn word_comments_and_tracked_changes_say_who_and_what_was_deleted() {
        let facts = facts_of(vec![
            Entry::new("word/document.xml", BODY.as_bytes(), 8),
            Entry::new("word/comments.xml", COMMENTS.as_bytes(), 0),
        ]);
        assert_eq!(
            facts,
            [
                (
                    "comments",
                    "3 comments by Olena Koval and Petro & Co".to_string()
                ),
                (
                    "tracked",
                    "1 insertion and 2 deletions by Petro & Co and Olena Koval, not accepted yet"
                        .to_string()
                ),
                ("deleted", "“we can pay 40k max … <draft>”".to_string()),
            ]
        );
    }

    #[test]
    fn a_clean_document_says_nothing_of_comments_or_changes() {
        let body = "<w:document><w:body><w:p><w:r><w:instrText>x</w:instrText><w:t>plain</w:t></w:r></w:p></w:body></w:document>";
        let facts = facts_of(vec![
            Entry::new("word/document.xml", body.as_bytes(), 8),
            Entry::new("word/comments.xml", b"<w:comments/>", 8),
        ]);
        assert_eq!(facts, []);
    }

    #[test]
    fn authors_past_the_limit_are_counted() {
        let authors: Vec<String> = (1..=6).map(|i| format!("A{i}")).collect();
        assert_eq!(
            by("6 comments".into(), &authors),
            "6 comments by A1, A2, A3, A4 and 2 others"
        );
        assert_eq!(by("1 comment".into(), &[]), "1 comment");
    }

    #[test]
    fn attributes_are_matched_whole() {
        let tag = r#"<w:x xw:author="no" w:authorId="no" w:author = 'yes'>"#;
        assert_eq!(attribute(tag, "w:author").as_deref(), Some("yes"));
        assert_eq!(attribute("<w:x w:author=unquoted>", "w:author"), None);
        assert_eq!(attribute("<w:x w:author=\"open>", "w:author"), None);
    }

    #[test]
    fn a_photo_in_a_document_says_where_it_was_taken() {
        let jpeg = photo();
        let facts = facts_of(vec![
            Entry::new("word/media/image1.jpeg", &jpeg, 8),
            Entry::new("word/media/image2.png", b"\x89PNG\r\n\x1a\n", 0),
            Entry::new("media/elsewhere.jpeg", &jpeg, 0),
        ]);
        assert_eq!(
            facts,
            [(
                "photoplace",
                "image1.jpeg: taken at 33.85000° S, 151.20000° E · Canon EOS R5 · serial HX-000042"
                    .to_string()
            )]
        );
    }

    #[test]
    fn scrambled_parts_never_panic() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let pieces = [
            "<w:del",
            "<w:ins",
            "<w:delText",
            "</w:delText>",
            "</w:del>",
            "<w:comment",
            " w:author=",
            "\"",
            "'",
            ">",
            "/>",
            "é",
            "&amp;",
            "&#x1F600;",
            " ",
        ];
        for _ in 0..2000 {
            let xml: String = (0..next() % 24)
                .map(|_| pieces[(next() % pieces.len() as u64) as usize])
                .collect();
            let _ = facts_of(vec![
                Entry::new("word/document.xml", xml.as_bytes(), 0),
                Entry::new("word/comments.xml", xml.as_bytes(), 0),
            ]);
        }
    }

    #[test]
    fn a_workbook_says_what_it_hides_and_who_commented() {
        let facts = facts_of(vec![
            Entry::new(
                "xl/workbook.xml",
                br#"<workbook><sheets><sheet name="Summary" sheetId="1"/><sheet name="Salaries" state="hidden" sheetId="2"/><sheet name="Old" state="veryHidden" sheetId="3"/></sheets></workbook>"#,
                8,
            ),
            Entry::new(
                "xl/worksheets/sheet1.xml",
                br#"<worksheet><cols><col min="3" max="5" hidden="1"/></cols><sheetData><row r="1"/><row r="2" hidden="1"/><row r="3" hidden="true"/></sheetData></worksheet>"#,
                8,
            ),
            Entry::new(
                "xl/comments1.xml",
                br#"<comments><authors><author>Olena Koval</author></authors><commentList><comment ref="A1" authorId="0"/><comment ref="B2" authorId="0"/></commentList></comments>"#,
                0,
            ),
            Entry::new(
                "xl/externalLinks/_rels/externalLink1.xml.rels",
                br#"<Relationships><Relationship Id="rId1" Target="file:///C:/Users/olena/Documents/budget.xlsx" TargetMode="External"/></Relationships>"#,
                0,
            ),
        ]);
        assert_eq!(
            facts,
            [
                ("hiddensheets", "“Salaries”, “Old”".to_string()),
                (
                    "hiddencells",
                    "2 hidden rows and 3 hidden columns".to_string()
                ),
                ("comments", "2 comments by Olena Koval".to_string()),
                (
                    "links",
                    "“file:///C:/Users/olena/Documents/budget.xlsx”".to_string()
                ),
            ]
        );
    }

    #[test]
    fn a_deck_says_its_notes_and_hidden_slides() {
        let facts = facts_of(vec![
            Entry::new("ppt/slides/slide1.xml", br#"<p:sld><p:cSld/></p:sld>"#, 8),
            Entry::new("ppt/slides/slide10.xml", br#"<p:sld show="0"><p:cSld/></p:sld>"#, 8),
            Entry::new("ppt/slides/slide2.xml", br#"<p:sld show="0"/>"#, 8),
            Entry::new(
                "ppt/notesSlides/notesSlide1.xml",
                br#"<p:notes><a:t>Do not mention</a:t><a:t> the delay.</a:t><a:fld type="slidenum"><a:t>1</a:t></a:fld></p:notes>"#,
                8,
            ),
            Entry::new("ppt/notesSlides/notesSlide2.xml", br#"<p:notes><a:t>2</a:t></p:notes>"#, 8),
            Entry::new("ppt/commentAuthors.xml", br#"<p:cmAuthorLst><p:cmAuthor id="0" name="Petro"/></p:cmAuthorLst>"#, 8),
            Entry::new("ppt/comments/comment1.xml", br#"<p:cmLst><p:cm authorId="0"/></p:cmLst>"#, 8),
        ]);
        assert_eq!(
            facts,
            [
                ("comments", "1 comment by Petro".to_string()),
                (
                    "notes",
                    "on 1 slide: “Do not mention the delay.”".to_string()
                ),
                ("hiddenslides", "slides 2, 10".to_string()),
            ]
        );
    }
}
