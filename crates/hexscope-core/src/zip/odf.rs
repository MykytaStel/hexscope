//! What an OpenDocument file — LibreOffice's or OpenOffice's .odt, .ods,
//! .odp — says about who made it: its `meta.xml` (who created it, who saved
//! it last and when, how long it was edited, with which program on which
//! system, who printed it), the printer its `settings.xml` names, the
//! comments and tracked changes in its `content.xml`, with the text a
//! tracked deletion still holds, and the picture of its first page it keeps
//! as a thumbnail.
//!
//! Read with the same tag reader as Office files: no XML parser, no DTDs,
//! no entities beyond the predefined and numeric ones.

use super::office::{
    DocumentFact, MAX_PART, attribute, by, cap, date_time, decode, duration, element_text,
    find_tag, plural, texts,
};
use super::{ZipEntry, extract};
use crate::model::NodeId;

/// What the `mimetype` entry of every OpenDocument file starts with.
const MIME: &[u8] = b"application/vnd.oasis.opendocument.";
/// Property and settings files larger than this are not read. Real ones are a few KB.
const MAX_PROPS: u64 = 1024 * 1024;
/// The picture of the first page every save writes.
pub(crate) const THUMBNAIL: &str = "Thumbnails/thumbnail.png";
/// Pictures this small say nothing: the blank one a clean copy keeps.
const TINY: u32 = 16;

/// Whether the archive is an OpenDocument file, by what its `mimetype` entry says.
pub(crate) fn is_odf(data: &[u8], entries: &[ZipEntry]) -> bool {
    entries
        .iter()
        .find(|e| e.name == "mimetype" && e.uncompressed <= 256)
        .and_then(|e| extract(data, e, 256).ok())
        .is_some_and(|b| b.starts_with(MIME))
}

/// The photos an OpenDocument file holds, under `Pictures/`.
pub(crate) fn is_picture(name: &str) -> bool {
    name.starts_with("Pictures/")
}

pub(super) fn odf_facts(data: &[u8], entries: &[ZipEntry]) -> Vec<DocumentFact> {
    let read = |name: &str, max: u64| {
        let e = entries
            .iter()
            .find(|e| e.name == name && e.uncompressed <= max)?;
        let bytes = extract(data, e, max).ok()?;
        Some((String::from_utf8_lossy(&bytes).into_owned(), e.node))
    };
    let mut facts = Vec::new();
    let mut push = |kind, text: Option<String>, node: NodeId| {
        if let Some(text) = text {
            facts.push(DocumentFact { kind, text, node });
        }
    };

    if let Some((xml, node)) = read("meta.xml", MAX_PROPS) {
        push("title", element_text(&xml, "dc:title"), node);
        push("author", element_text(&xml, "meta:initial-creator"), node);
        push("editor", element_text(&xml, "dc:creator"), node);
        push(
            "created",
            element_text(&xml, "meta:creation-date").map(date_time),
            node,
        );
        push(
            "modified",
            element_text(&xml, "dc:date").map(date_time),
            node,
        );
        push("revisions", element_text(&xml, "meta:editing-cycles"), node);
        let minutes = element_text(&xml, "meta:editing-duration").and_then(|d| minutes(&d));
        push("editing", minutes.filter(|&m| m > 0).map(duration), node);
        push(
            "application",
            element_text(&xml, "meta:generator").map(generator),
            node,
        );
        push("subject", element_text(&xml, "dc:subject"), node);
        let words = texts(&xml, "meta:keyword");
        push(
            "keywords",
            (!words.is_empty()).then(|| cap(words.join(", "))),
            node,
        );
        push("description", element_text(&xml, "dc:description"), node);
        push("template", template(&xml), node);
        let printed = match (
            element_text(&xml, "meta:printed-by"),
            element_text(&xml, "meta:print-date"),
        ) {
            (Some(who), Some(when)) => Some(format!("by {who}, {}", date_time(when))),
            (Some(who), None) => Some(format!("by {who}")),
            (None, Some(when)) => Some(date_time(when)),
            (None, None) => None,
        };
        push("printed", printed, node);
        push("custom", user_defined(&xml), node);
    }
    if let Some((xml, node)) = read("settings.xml", MAX_PROPS) {
        push("printer", config_item(&xml, "PrinterName"), node);
    }
    for part in ["content.xml", "styles.xml"] {
        let Some((xml, node)) = read(part, MAX_PART) else {
            continue;
        };
        push("comments", comments(&xml), node);
        push("tracked", tracked(&xml), node);
        push("deleted", deleted(&xml), node);
    }
    if let Some(e) = entries
        .iter()
        .find(|e| e.name == THUMBNAIL && e.uncompressed <= MAX_PART)
        && let Ok(png) = extract(data, e, MAX_PART)
        && let Some((w, h)) = png_size(&png)
        && w > TINY
        && h > TINY
    {
        push(
            "thumbnail",
            Some(format!("a picture of the first page, {w}×{h}")),
            e.node,
        );
    }
    facts
}

/// A white picture one pixel square: the thumbnail a clean copy keeps, so
/// the file's list of its parts still matches what is there.
pub(super) const BLANK_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0x33, 0x12, 0x95, 0x14, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// What a clean copy writes in place of one part of an OpenDocument file,
/// and what that took out; `None` for a part it keeps as it is.
///
/// - `meta.xml` and `settings.xml` become empty, of the same ODF version.
/// - The first page's thumbnail becomes a blank pixel.
/// - In `content.xml` and `styles.xml`, tracked changes are accepted — what
///   was deleted goes, with its text — and comments are deleted, with their
///   authors.
pub(crate) fn revise(data: &[u8], e: &ZipEntry) -> Option<(Vec<u8>, String)> {
    let max = if e.name == "content.xml" || e.name == "styles.xml" {
        MAX_PART
    } else {
        MAX_PROPS
    };
    if !matches!(
        e.name.as_str(),
        "meta.xml" | "settings.xml" | "content.xml" | "styles.xml" | THUMBNAIL
    ) || e.uncompressed > max
    {
        return None;
    }
    let bytes = extract(data, e, max).ok()?;
    if e.name == THUMBNAIL {
        let (w, h) = png_size(&bytes)?;
        return (w > TINY && h > TINY).then(|| {
            (
                BLANK_PNG.to_vec(),
                "a picture of the first page".to_string(),
            )
        });
    }
    let xml = std::str::from_utf8(&bytes).ok()?;
    let version = find_tag(xml, 0, "office:document-meta")
        .or_else(|| find_tag(xml, 0, "office:document-settings"))
        .and_then(|(start, end)| attribute(&xml[start..end], "office:version"))
        .unwrap_or_else(|| "1.2".into());
    match e.name.as_str() {
        "meta.xml" => {
            let inner = regions(xml, "office:meta").first().map(|m| m.trim().to_string()).unwrap_or_default();
            (!inner.is_empty()).then(|| {
                let empty = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" office:version="{version}"><office:meta/></office:document-meta>"#
                );
                (
                    empty.into_bytes(),
                    "who made it and when, who saved it last, how long it was edited, the program and system, who printed it".to_string(),
                )
            })
        }
        "settings.xml" => find_tag(xml, 0, "config:config-item").map(|_| {
            let empty = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><office:document-settings xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" office:version="{version}"><office:settings/></office:document-settings>"#
            );
            (empty.into_bytes(), "the printer's name and settings, and where the view was left".to_string())
        }),
        _ => accept(xml).map(|(out, said)| (out.into_bytes(), said)),
    }
}

/// `xml` with its tracked changes accepted and its comments deleted, and
/// what that did; `None` when it has neither.
fn accept(xml: &str) -> Option<(String, String)> {
    let changes = regions(xml, "text:changed-region").len();
    let (out, _) = strip(xml, "text:tracked-changes");
    let mut marks = 0;
    let mut out = out;
    for mark in ["text:change", "text:change-start", "text:change-end"] {
        let (next, n) = strip(&out, mark);
        out = next;
        marks += n;
    }
    let mut notes = 0;
    for note in [
        "office:annotation",
        "office:annotation-end",
        "officeooo:annotation",
    ] {
        let (next, n) = strip(&out, note);
        out = next;
        if note != "office:annotation-end" {
            notes += n;
        }
    }
    let mut said = Vec::new();
    if changes > 0 || marks > 0 {
        let n = changes.max(marks.min(1));
        said.push(if n == 1 {
            "1 tracked change, accepted".to_string()
        } else {
            format!("{n} tracked changes, accepted")
        });
    }
    if notes > 0 {
        said.push(format!(
            "{} and who wrote them",
            plural(notes, "comment", "comments")
        ));
    }
    (!said.is_empty() && out.len() < xml.len()).then(|| (out, said.join("; ")))
}

/// `xml` without any `<name …/>` or `<name …>…</name>` element (which do
/// not nest), and how many it took out.
fn strip(xml: &str, name: &str) -> (String, usize) {
    let close = format!("</{name}>");
    let mut out = String::with_capacity(xml.len());
    let (mut from, mut kept, mut n) = (0, 0, 0);
    while let Some((start, body)) = find_tag(xml, from, name) {
        let end = if xml[..body - 1].ends_with('/') {
            body
        } else {
            match xml[body..].find(&close) {
                Some(len) => body + len + close.len(),
                None => break,
            }
        };
        out.push_str(&xml[kept..start]);
        kept = end;
        from = end;
        n += 1;
    }
    out.push_str(&xml[kept..]);
    (out, n)
}

/// `PT1H23M45S` or `P2DT3H` in whole minutes.
fn minutes(iso: &str) -> Option<u64> {
    let rest = iso.strip_prefix('P')?;
    let (mut total, mut number, mut in_time) = (0u64, String::new(), false);
    for c in rest.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' | '.' => number.push(c),
            'D' | 'H' | 'M' | 'S' => {
                let n = number.split('.').next()?.parse::<u64>().ok()?;
                number.clear();
                total = total.saturating_add(match (c, in_time) {
                    ('D', _) => n * 24 * 60,
                    ('H', true) => n * 60,
                    ('M', true) => n,
                    ('S', true) => n / 60,
                    // Months in a duration: not something an editing time holds.
                    _ => return None,
                });
            }
            _ => return None,
        }
    }
    Some(total)
}

/// `LibreOffice/7.6.4.1$Linux_X86_64 LibreOffice_project/…` as
/// `LibreOffice 7.6.4.1 on Linux (x86-64)`: the program, and the system it ran on.
fn generator(raw: String) -> String {
    let Some((program, rest)) = raw.split_once('$') else {
        return raw;
    };
    let system = rest.split_whitespace().next().unwrap_or(rest);
    let (os, cpu) = system.split_once('_').unwrap_or((system, ""));
    let os = match os {
        "MacOSX" => "macOS",
        "Windows" => "Windows",
        "Linux" => "Linux",
        other => other,
    };
    let cpu = match cpu {
        "X86_64" => " (x86-64)",
        "AARCH64" => " (ARM)",
        "X86" => " (x86)",
        _ => "",
    };
    cap(format!("{} on {os}{cpu}", program.replace('/', " ")))
}

/// The template it was made from: its name, and where it was on the author's computer.
fn template(xml: &str) -> Option<String> {
    let (start, end) = find_tag(xml, 0, "meta:template")?;
    let tag = &xml[start..end];
    match (attribute(tag, "xlink:title"), attribute(tag, "xlink:href")) {
        (Some(name), Some(path)) => Some(format!("“{name}”, from {path}")),
        (Some(name), None) => Some(format!("“{name}”")),
        (None, Some(path)) => Some(path),
        (None, None) => None,
    }
}

/// Properties its author added, by name and value: `Project: Atlas · Client: Northwind`.
fn user_defined(xml: &str) -> Option<String> {
    let mut shown = Vec::new();
    let mut count = 0;
    let mut from = 0;
    while let Some((start, body)) = find_tag(xml, from, "meta:user-defined") {
        from = body;
        let Some(name) = attribute(&xml[start..body], "meta:name") else {
            continue;
        };
        count += 1;
        let value = if xml[..body - 1].ends_with('/') {
            String::new()
        } else {
            xml[body..]
                .find("</meta:user-defined>")
                .map(|end| decode(xml[body..body + end].trim()))
                .unwrap_or_default()
        };
        if shown.len() < 5 {
            shown.push(if value.is_empty() {
                name
            } else {
                format!("{name}: {value}")
            });
        }
    }
    let mut list = shown.join(" · ");
    if count > shown.len() {
        list.push_str(&format!(" +{}", count - shown.len()));
    }
    (!list.is_empty()).then(|| cap(list))
}

/// The value of `<config:config-item config:name="…">value</config:config-item>`.
fn config_item(xml: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some((start, body)) = find_tag(xml, from, "config:config-item") {
        from = body;
        if attribute(&xml[start..body], "config:name").as_deref() != Some(name) {
            continue;
        }
        let end = xml[body..].find("</config:config-item>")?;
        let value = decode(xml[body..body + end].trim());
        return (!value.is_empty()).then(|| cap(value));
    }
    None
}

/// The text between each `<name …>` and its `</name>`, which do not nest.
fn regions<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
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
        out.push(&xml[body..body + len]);
        from = body + len;
    }
    out
}

/// `3 comments by Olena and Petro`, from Writer's and Calc's
/// `<office:annotation>` and Impress's `<officeooo:annotation>`.
fn comments(xml: &str) -> Option<String> {
    let notes: Vec<&str> = regions(xml, "office:annotation")
        .into_iter()
        .chain(regions(xml, "officeooo:annotation"))
        .collect();
    if notes.is_empty() {
        return None;
    }
    let authors: Vec<String> = notes
        .iter()
        .filter_map(|n| element_text(n, "dc:creator"))
        .collect();
    Some(by(plural(notes.len(), "comment", "comments"), &authors))
}

/// `2 insertions and 1 deletion by Olena, not accepted yet`.
fn tracked(xml: &str) -> Option<String> {
    let changes = regions(xml, "text:changed-region");
    if changes.is_empty() {
        return None;
    }
    let authors: Vec<String> = changes
        .iter()
        .filter_map(|c| element_text(c, "dc:creator"))
        .collect();
    let count = |kind: &str| {
        changes
            .iter()
            .filter(|c| find_tag(c, 0, kind).is_some())
            .count()
    };
    let (ins, del) = (count("text:insertion"), count("text:deletion"));
    let what = match (ins, del) {
        (0, 0) => plural(changes.len(), "change", "changes"),
        (i, 0) => plural(i, "insertion", "insertions"),
        (0, d) => plural(d, "deletion", "deletions"),
        (i, d) => format!(
            "{} and {}",
            plural(i, "insertion", "insertions"),
            plural(d, "deletion", "deletions")
        ),
    };
    Some(format!("{}, not accepted yet", by(what, &authors)))
}

/// What tracked deletions still hold: the text each `<text:deletion>` keeps,
/// deletions set apart by an ellipsis.
fn deleted(xml: &str) -> Option<String> {
    let pieces: Vec<String> = regions(xml, "text:deletion")
        .into_iter()
        .map(|d| {
            // Who deleted it and when is not the text.
            let body = match d.find("</office:change-info>") {
                Some(i) => &d[i + "</office:change-info>".len()..],
                None => d,
            };
            words(body)
        })
        .filter(|t| !t.is_empty())
        .collect();
    let text = pieces.join(" … ");
    (!text.is_empty()).then(|| cap(format!("“{text}”")))
}

/// The words of a piece of XML, its tags taken out.
fn words(xml: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut chunk = String::new();
    for c in xml.chars() {
        match c {
            '<' => {
                out.push_str(&decode(&chunk));
                chunk.clear();
                out.push(' ');
                inside = true;
            }
            '>' => inside = false,
            _ if !inside => chunk.push(c),
            _ => {}
        }
    }
    out.push_str(&decode(&chunk));
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A PNG's width and height, from its header.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") || png.get(12..16)? != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_editing_time_reads_in_minutes() {
        assert_eq!(minutes("PT1H23M45S"), Some(83));
        assert_eq!(minutes("P1DT2H"), Some(26 * 60));
        assert_eq!(minutes("PT0S"), Some(0));
        assert_eq!(minutes("PT12M3.5S"), Some(12));
        assert_eq!(minutes("P1M"), None);
        assert_eq!(minutes("1H"), None);
    }

    #[test]
    fn the_generator_says_the_program_and_the_system() {
        assert_eq!(
            generator("LibreOffice/7.6.4.1$Linux_X86_64 LibreOffice_project/e19e193f88cd6c0525a17fb7a176ed8e6a3e2aa1".into()),
            "LibreOffice 7.6.4.1 on Linux (x86-64)"
        );
        assert_eq!(
            generator("LibreOffice/24.2.0.3$MacOSX_AARCH64 LibreOffice_project/x".into()),
            "LibreOffice 24.2.0.3 on macOS (ARM)"
        );
        assert_eq!(generator("OpenOffice.org/3.4".into()), "OpenOffice.org/3.4");
    }

    #[test]
    fn deleted_text_leaves_out_who_deleted_it() {
        let xml = r#"<text:tracked-changes><text:changed-region text:id="ct1"><text:deletion><office:change-info><dc:creator>Olena</dc:creator><dc:date>2024-03-01T10:00:00</dc:date></office:change-info><text:p>The real price is <text:span>40&#160;000</text:span> UAH.</text:p></text:deletion></text:changed-region></text:tracked-changes>"#;
        assert_eq!(
            deleted(xml).as_deref(),
            Some("“The real price is 40 000 UAH.”")
        );
        assert_eq!(
            tracked(xml).as_deref(),
            Some("1 deletion by Olena, not accepted yet")
        );
    }

    #[test]
    fn accepting_takes_the_deleted_text_and_the_comments_and_keeps_the_rest() {
        let xml = r#"<office:text><text:tracked-changes><text:changed-region text:id="ct1"><text:deletion><office:change-info><dc:creator>Olena</dc:creator></office:change-info><text:p>40 000 UAH</text:p></text:deletion></text:changed-region><text:changed-region text:id="ct2"><text:insertion><office:change-info><dc:creator>Petro</dc:creator></office:change-info></text:insertion></text:changed-region></text:tracked-changes><text:p>Price: <text:change text:change-id="ct1"/><text:change-start text:change-id="ct2"/>50 000 UAH<text:change-end text:change-id="ct2"/><office:annotation office:name="a"><dc:creator>Olena</dc:creator><text:p>Ask the director</text:p></office:annotation>.<office:annotation-end office:name="a"/></text:p></office:text>"#;
        let (out, said) = accept(xml).unwrap();
        assert_eq!(
            out,
            "<office:text><text:p>Price: 50 000 UAH.</text:p></office:text>"
        );
        assert_eq!(
            said,
            "2 tracked changes, accepted; 1 comment and who wrote them"
        );
        assert!(accept(&out).is_none());
    }

    #[test]
    fn the_blank_thumbnail_is_a_picture_too_small_to_say_anything() {
        assert_eq!(png_size(BLANK_PNG), Some((1, 1)));
        assert!(
            crate::png::parse_png(BLANK_PNG)
                .tree
                .nodes()
                .iter()
                .all(|n| n.kind != crate::model::NodeKind::Error)
        );
    }

    #[test]
    fn comments_name_their_authors_in_every_kind_of_document() {
        let xml = r#"<office:annotation office:name="a"><dc:creator>Olena</dc:creator><text:p>Ask</text:p></office:annotation><office:annotation-end office:name="a"/><officeooo:annotation><dc:creator>Petro</dc:creator></officeooo:annotation>"#;
        assert_eq!(
            comments(xml).as_deref(),
            Some("2 comments by Olena and Petro")
        );
    }

    #[test]
    fn the_printer_and_custom_properties_are_read_by_name() {
        let settings = r#"<config:config-item config:name="PrinterPaperFromSetup" config:type="boolean">false</config:config-item><config:config-item config:name="PrinterName" config:type="string">HP LaserJet (2nd floor)</config:config-item>"#;
        assert_eq!(
            config_item(settings, "PrinterName").as_deref(),
            Some("HP LaserJet (2nd floor)")
        );
        let meta = r#"<meta:user-defined meta:name="Project" meta:value-type="string">Atlas</meta:user-defined><meta:user-defined meta:name="Draft" meta:value-type="boolean"/>"#;
        assert_eq!(
            user_defined(meta).as_deref(),
            Some("Project: Atlas · Draft")
        );
    }
}
