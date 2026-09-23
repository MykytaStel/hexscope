//! What an Office document says about who made it: the properties Word,
//! Excel and PowerPoint keep in `docProps/core.xml` and `docProps/app.xml`.
//!
//! Only a handful of known elements are read, with a tag reader rather than
//! an XML parser: no DTDs, no external entities, no entity expansion beyond
//! the predefined and numeric ones. Nothing in these files can make it do
//! more work than a scan of their bytes.

use super::{ZipEntry, extract};
use crate::model::NodeId;

/// Property files larger than this are not read. Real ones are a few KB.
const MAX_PROPS: u64 = 1024 * 1024;
/// Longest property kept, in bytes.
const MAX_TEXT: usize = 512;

/// One thing a document reveals, and the entry that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentFact {
    /// title, author, editor, created, modified, revisions, editing,
    /// company, application or template.
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
    facts
}

/// The text of the first `<name …>text</name>` element, decoded and trimmed.
/// `None` for a missing, empty or self-closing element.
fn element_text(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}");
    let mut from = 0;
    let start = loop {
        let at = from + xml[from..].find(&open)?;
        let after = at + open.len();
        // `<dc:creator>` or `<dc:creator attr…>`, not `<dc:creatorX>`.
        match xml[after..].chars().next()? {
            '>' | ' ' | '\t' | '\r' | '\n' | '/' => break after,
            _ => from = after,
        }
    };
    let tag_end = start + xml[start..].find('>')?;
    if xml[..tag_end].ends_with('/') {
        return None;
    }
    let body = tag_end + 1;
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
}
