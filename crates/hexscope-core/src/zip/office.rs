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
    facts.extend(photo_facts(data, entries));
    facts
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
fn photo_says(f: &PhotoFacts) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(l) = &f.location {
        let ns = if l.latitude < 0.0 { 'S' } else { 'N' };
        let ew = if l.longitude < 0.0 { 'W' } else { 'E' };
        parts.push(format!(
            "taken at {:.5}° {ns}, {:.5}° {ew}",
            l.latitude.abs(),
            l.longitude.abs()
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

/// Every `<name …>` tag, as its text from `<` to `>`.
fn tags<'a>(xml: &'a str, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let mut from = 0;
    std::iter::from_fn(move || {
        let (start, end) = find_tag(xml, from, name)?;
        from = end;
        Some(&xml[start..end])
    })
}

/// The value of `name="…"` or `name='…'` in a tag, decoded.
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
}
