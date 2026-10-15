//! What Office leaves about the computer a document was saved on, beyond
//! its properties: the folder a workbook was last saved in (Excel's
//! `x15ac:absPath`, written on every save on Windows), the full path of the
//! template a Word document is attached to, the printer a printer-settings
//! part names, an organisation's SharePoint properties kept as custom XML,
//! and a picture of the first page. Each is a fact, and each is taken out
//! of a clean copy, with the numbers of the editing sessions that tie a
//! Word document to others written in the same sittings.

use super::office::{DocumentFact, MAX_PART, attribute, find_tag, tags};
use super::{ZipEntry, extract};

/// SharePoint's document properties, the root of a custom XML part that holds them.
const SHAREPOINT: &str = "http://schemas.microsoft.com/office/2006/metadata/properties";
/// The pictures of the first page Office may keep, by name.
const THUMBNAILS: [&str; 4] = [
    "docProps/thumbnail.jpeg",
    "docProps/thumbnail.jpg",
    "docProps/thumbnail.png",
    "docProps/thumbnail.wmf",
];
/// A white picture one pixel square, in the place of a JPEG thumbnail:
/// quantisation and Huffman tables, a frame and a scan, nothing else.
const BLANK_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x10, 0x0B, 0x0C, 0x0E, 0x0C, 0x0A, 0x10, 0x0E, 0x0D,
    0x0E, 0x12, 0x11, 0x10, 0x13, 0x18, 0x28, 0x1A, 0x18, 0x16, 0x16, 0x18, 0x31, 0x23, 0x25, 0x1D,
    0x28, 0x3A, 0x33, 0x3D, 0x3C, 0x39, 0x33, 0x38, 0x37, 0x40, 0x48, 0x5C, 0x4E, 0x40, 0x44, 0x57,
    0x45, 0x37, 0x38, 0x50, 0x6D, 0x51, 0x57, 0x5F, 0x62, 0x67, 0x68, 0x67, 0x3E, 0x4D, 0x71, 0x79,
    0x70, 0x64, 0x78, 0x5C, 0x65, 0x67, 0x63, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01,
    0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x40, 0x7F, 0xFF, 0xD9,
];

/// Whether a part is a printer's settings: Word's, Excel's or PowerPoint's.
fn is_printer(name: &str) -> bool {
    name.contains("/printerSettings/") && name.ends_with(".bin")
}

/// The printer a printer-settings part names: the device name a Windows
/// DEVMODE starts with, 32 UTF-16 code units.
fn printer_name(bin: &[u8]) -> Option<String> {
    let units: Vec<u16> = bin
        .get(..64)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0)
        .collect();
    let name = String::from_utf16(&units).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// What a custom XML part of SharePoint's document properties holds, and
/// where: the bytes between its root's tags; `None` for any other part, or
/// one already emptied.
fn sharepoint(name: &str, xml: &str) -> Option<(usize, usize)> {
    if !name.starts_with("customXml/item") || name.contains("Props") || !xml.contains(SHAREPOINT) {
        return None;
    }
    let (_, start) = find_tag(xml, 0, "p:properties")?;
    let end = xml.rfind("</p:properties>")?;
    (end > start && !xml[start..end].trim().is_empty()).then_some((start, end))
}

/// What a custom XML part says about people: the addresses and the names
/// SharePoint keeps for whoever a field names, at most a few.
fn people(xml: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in xml.split(['<', '>', ' ', '"', '\'', ';']) {
        if word.contains('@')
            && word.contains('.')
            && !word.starts_with("http")
            && !out.iter().any(|w| w == word)
        {
            out.push(word.to_string());
        }
    }
    for name in super::office::texts(xml, "pc:DisplayName")
        .into_iter()
        .chain(super::office::texts(xml, "DisplayName"))
    {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out.truncate(4);
    out
}

/// The facts the parts above hold, each at its entry.
pub(super) fn trace_facts(data: &[u8], entries: &[ZipEntry]) -> Vec<DocumentFact> {
    let mut facts = Vec::new();
    let mut printers: Vec<String> = Vec::new();
    let mut printer_node = None;
    for e in entries.iter().filter(|e| e.uncompressed <= MAX_PART) {
        let read = || extract(data, e, MAX_PART).ok();
        let mut push = |kind, text: String| {
            facts.push(DocumentFact {
                kind,
                text,
                node: e.node,
            })
        };
        if e.name == "xl/workbook.xml" {
            if let Some(xml) = read().map(|b| String::from_utf8_lossy(&b).into_owned())
                && let Some((s, t)) = find_tag(&xml, 0, "x15ac:absPath")
                && let Some(folder) = attribute(&xml[s..t], "url").filter(|f| !f.is_empty())
            {
                push("folder", folder);
            }
        } else if e.name == "word/_rels/settings.xml.rels" {
            let Some(xml) = read().map(|b| String::from_utf8_lossy(&b).into_owned()) else {
                continue;
            };
            for tag in tags(&xml, "Relationship") {
                if attribute(tag, "Type").is_some_and(|t| t.ends_with("/attachedTemplate"))
                    && let Some(path) = attribute(tag, "Target").filter(|p| p.contains(['/', '\\']))
                {
                    push("template", path);
                }
            }
        } else if is_printer(&e.name) {
            if let Some(name) = read().as_deref().and_then(printer_name)
                && !printers.contains(&name)
            {
                printers.push(name);
                printer_node.get_or_insert(e.node);
            }
        } else if e.name.starts_with("customXml/item") && e.name.ends_with(".xml") {
            let Some(xml) = read().map(|b| String::from_utf8_lossy(&b).into_owned()) else {
                continue;
            };
            let found = people(&xml);
            let what = if sharepoint(&e.name, &xml).is_some() {
                "SharePoint properties"
            } else {
                "custom XML"
            };
            if sharepoint(&e.name, &xml).is_some() || !found.is_empty() {
                push(
                    "custom",
                    if found.is_empty() {
                        what.to_string()
                    } else {
                        format!("{what}: {}", found.join(", "))
                    },
                );
            }
        } else if THUMBNAILS.contains(&e.name.as_str()) {
            // A clean copy's blank pixel says nothing.
            if read().is_some_and(|b| b != BLANK_JPEG && b != super::odf::BLANK_PNG) {
                push("thumbnail", "a picture of the first page".to_string());
            }
        }
    }
    if let Some(node) = printer_node {
        facts.push(DocumentFact {
            kind: "printer",
            text: printers.join(", "),
            node,
        });
    }
    facts
}

/// Word's editing-session numbers: the `w:rsid…` attributes on every
/// paragraph and run, and the list of them in the settings. Documents
/// written from one file, or in the same sittings, share them.
pub(crate) fn drop_sessions(xml: &str) -> (String, usize) {
    let mut out = String::with_capacity(xml.len());
    let mut n = 0;
    let mut rest = xml;
    while let Some(at) = rest.find(" w:rsid") {
        let tail = &rest[at + 1..];
        // ` w:rsidR="00A1"`: the name, `=`, a quoted value.
        let Some(eq) = tail.find('=') else { break };
        let quote = tail[eq + 1..].chars().next();
        let end = match quote {
            Some(q @ ('"' | '\'')) => tail[eq + 2..].find(q).map(|e| eq + 2 + e + 1),
            _ => None,
        };
        match end.filter(|_| {
            tail[..eq]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b':')
        }) {
            Some(end) => {
                out.push_str(&rest[..at]);
                rest = &tail[end..];
                n += 1;
            }
            None => {
                out.push_str(&rest[..at + 1]);
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    // The list in the settings, `<w:rsids>…</w:rsids>`, goes whole.
    if let (Some(s), Some(e)) = (out.find("<w:rsids>"), out.find("</w:rsids>"))
        && s < e
    {
        out.replace_range(s..e + "</w:rsids>".len(), "");
        n += 1;
    }
    (out, n)
}

/// A part with what it says about the computer taken out, and what that
/// was, in words; `None` when the part is none of these, or says nothing.
pub(crate) fn revise(data: &[u8], e: &ZipEntry) -> Option<(Vec<u8>, String)> {
    if e.uncompressed > MAX_PART {
        return None;
    }
    if THUMBNAILS[..2].contains(&e.name.as_str()) {
        return Some((BLANK_JPEG.to_vec(), "a picture of the first page".into()));
    }
    if e.name == THUMBNAILS[2] {
        return Some((
            super::odf::BLANK_PNG.to_vec(),
            "a picture of the first page".into(),
        ));
    }
    let bytes = extract(data, e, MAX_PART).ok()?;
    if is_printer(&e.name) {
        printer_name(&bytes)?;
        let mut out = bytes;
        out[..64].fill(0);
        return Some((out, "the printer's name".into()));
    }
    let xml = std::str::from_utf8(&bytes).ok()?;
    match e.name.as_str() {
        "xl/workbook.xml" => {
            let (s, mut t) = find_tag(xml, 0, "x15ac:absPath")?;
            // Self-closing as Excel writes it, or with a closing tag of its own.
            if xml[t..].starts_with("</x15ac:absPath>") {
                t += "</x15ac:absPath>".len();
            }
            Some((
                [&xml[..s], &xml[t..]].concat().into_bytes(),
                "the folder it was last saved in".into(),
            ))
        }
        "word/_rels/settings.xml.rels" => {
            let mut out = xml.to_string();
            for tag in tags(xml, "Relationship") {
                if let Some(path) = attribute(tag, "Target").filter(|p| p.contains(['/', '\\'])) {
                    let file = path.rsplit(['/', '\\']).next().unwrap_or_default();
                    out = out.replace(tag, &tag.replacen(&path, file, 1));
                }
            }
            (out != xml).then(|| (out.into_bytes(), "the folder of its template".into()))
        }
        "word/settings.xml" => {
            let (out, n) = drop_sessions(xml);
            (n > 0).then(|| (out.into_bytes(), "editing-session numbers".into()))
        }
        name => {
            // The root kept, with its namespaces; what it holds goes.
            let (start, end) = sharepoint(name, xml)?;
            Some((
                [&xml[..start], &xml[end..]].concat().into_bytes(),
                "SharePoint properties".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_devmode_names_its_printer() {
        let mut bin: Vec<u8> = "HP LaserJet Pro M404"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        bin.resize(220, 0);
        assert_eq!(printer_name(&bin).as_deref(), Some("HP LaserJet Pro M404"));
        assert_eq!(printer_name(&[0; 220]), None);
        assert_eq!(printer_name(&[0x41; 10]), None);
    }

    #[test]
    fn session_numbers_go_and_the_rest_stays() {
        let xml = r#"<w:p w:rsidR="00A1B2C3" w:rsidRDefault='00D4E5F6'><w:r w:rsidRPr="00112233"><w:t>rsid stays as text: w:rsid</w:t></w:r></w:p><w:rsids><w:rsidRoot w:val="00A1"/><w:rsid w:val="00B2"/></w:rsids><w:x/>"#;
        let (out, n) = drop_sessions(xml);
        assert_eq!(
            out,
            r#"<w:p><w:r><w:t>rsid stays as text: w:rsid</w:t></w:r></w:p><w:x/>"#
        );
        assert_eq!(n, 4);
        assert_eq!(
            drop_sessions("<w:p w:val=\"1\"/>"),
            ("<w:p w:val=\"1\"/>".to_string(), 0)
        );
    }

    #[test]
    fn the_blank_jpeg_is_a_picture() {
        let doc = crate::jpeg::parse_jpeg(BLANK_JPEG);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .all(|n| n.kind != crate::model::NodeKind::Error)
        );
        assert!(people("<p:properties xmlns:p=\"x\"><Owner>olena@northwind.example</Owner><pc:DisplayName>Olena Koval</pc:DisplayName></p:properties>")
            .iter()
            .eq(["olena@northwind.example", "Olena Koval"].iter()));
    }
}
