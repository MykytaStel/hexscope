//! A Word document with its tracked changes accepted and its comments
//! deleted, as Word's own Accept All Changes and Delete All Comments would
//! leave it: what was deleted goes, with its text; what was inserted stays,
//! no longer marked; moves settle where they went; formatting changes keep
//! their result and lose their record; comments lose their marks in the text,
//! and the parts that hold them and their authors are emptied.
//!
//! Done on the XML as text, element by element, so the rest of the document
//! is copied as it was (ISO/IEC 29500-1 §17.13.5 for revisions, §17.13.4
//! for comments).

/// Elements removed with everything inside them: deleted text, text moved
/// away, and the records of what formatting was before a change.
const DROP: [&str; 7] = [
    "w:del",
    "w:moveFrom",
    "w:rPrChange",
    "w:pPrChange",
    "w:sectPrChange",
    "w:tblPrChange",
    "w:trPrChange",
];
/// Elements whose marks go and whose content stays: inserted text, text
/// moved here.
const UNWRAP: [&str; 2] = ["w:ins", "w:moveTo"];
/// Empty elements removed: comment anchors and move ranges.
const MARKS: [&str; 7] = [
    "w:commentRangeStart",
    "w:commentRangeEnd",
    "w:commentReference",
    "w:moveFromRangeStart",
    "w:moveFromRangeEnd",
    "w:moveToRangeStart",
    "w:moveToRangeEnd",
];

/// Where the next `<name` tag starts, as a whole name, and whether it
/// closes itself; `(start, end of tag, self-closing)`.
fn next_tag(xml: &str, from: usize, name: &str) -> Option<(usize, usize, bool)> {
    let open = format!("<{name}");
    let mut at = from;
    loop {
        let i = at + xml.get(at..)?.find(&open)?;
        let after = i + open.len();
        match xml[after..].chars().next()? {
            '>' | ' ' | '\t' | '\r' | '\n' | '/' => {
                let end = after + xml[after..].find('>')? + 1;
                return Some((i, end, xml[..end - 1].ends_with('/')));
            }
            _ => at = after,
        }
    }
}

/// Removes every `<name …>…</name>` and `<name …/>`, contents and all.
/// Returns how many went. An element whose close is missing is left alone.
fn drop_all(xml: &str, name: &str, count: &mut usize) -> String {
    let close = format!("</{name}>");
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    while let Some((start, end, empty)) = next_tag(xml, at, name) {
        let stop = if empty {
            end
        } else {
            // The same element does not nest inside itself in a document.
            match xml[end..].find(&close) {
                Some(i) => end + i + close.len(),
                None => break,
            }
        };
        out.push_str(&xml[at..start]);
        at = stop;
        *count += 1;
    }
    out.push_str(&xml[at..]);
    out
}

/// Removes the `<name …>` and `</name>` tags, keeping what is between.
fn unwrap_all(xml: &str, name: &str, count: &mut usize) -> String {
    let close = format!("</{name}>");
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    while let Some((start, end, _)) = next_tag(xml, at, name) {
        out.push_str(&xml[at..start]);
        at = end;
        *count += 1;
    }
    out.push_str(&xml[at..]);
    out.replace(&close, "")
}

/// A document part with its revisions accepted and its comment marks
/// taken out, and how many of each there were.
pub(crate) fn accept(xml: &str) -> (String, usize) {
    let mut n = 0;
    let mut out = xml.to_string();
    for name in DROP {
        out = drop_all(&out, name, &mut n);
    }
    for name in UNWRAP {
        out = unwrap_all(&out, name, &mut n);
    }
    for name in MARKS {
        out = drop_all(&out, name, &mut n);
    }
    (out, n)
}

/// Why a run of text is not seen on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unseen {
    /// Formatted as hidden (§17.3.2.41): Word shows it only when asked to.
    Hidden,
    /// White, where nothing in the document is shaded dark behind it.
    White,
    /// Under a point high: a speck, or nothing, on the page.
    Tiny,
}

/// The value of `w:val` in a tag, when it has one.
fn val(tag: &str) -> Option<&str> {
    let at = tag.find("w:val=\"")? + 7;
    tag.get(at..)?.split('"').next()
}

/// Whether anything in the document is shaded or coloured behind its text,
/// so white letters could be meant to show: a page colour, a shaded cell or
/// paragraph, a highlight.
fn shaded(xml: &str) -> bool {
    xml.contains("<w:background")
        || xml.contains("<w:highlight")
        || xml.match_indices("<w:shd").any(|(i, _)| {
            let tag = &xml[i..xml[i..].find('>').map_or(xml.len(), |e| i + e)];
            let fill = tag
                .find("w:fill=\"")
                .and_then(|a| tag[a + 8..].split('"').next());
            !matches!(fill, None | Some("auto" | "FFFFFF" | "ffffff"))
        })
}

/// Every run of text no one reading the page sees — hidden, white where
/// nothing is shaded, or too small — as where it lies in `xml`, why, and its
/// text as written. A run with no text is not counted.
pub(crate) fn hidden_runs(xml: &str) -> Vec<(usize, usize, Unseen, String)> {
    let white_counts = !shaded(xml);
    let mut out = Vec::new();
    let mut at = 0;
    while let Some((start, end, empty)) = next_tag(xml, at, "w:r") {
        at = end;
        if empty {
            continue;
        }
        let Some(close) = xml[end..].find("</w:r>").map(|i| end + i + 6) else {
            break;
        };
        at = close;
        let run = &xml[start..close];
        let props = run
            .find("<w:rPr>")
            .and_then(|p| run[p..].find("</w:rPr>").map(|q| &run[p..p + q]))
            .unwrap_or("");
        let tag_of = |name: &str| next_tag(props, 0, name).map(|(s, e, _)| &props[s..e]);
        let on = |name: &str| {
            tag_of(name).is_some_and(|t| !matches!(val(t), Some("0" | "false" | "off")))
        };
        let why = if on("w:vanish") || on("w:specVanish") {
            Some(Unseen::Hidden)
        } else if tag_of("w:sz")
            .and_then(val)
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|half| half <= 2)
        {
            Some(Unseen::Tiny)
        } else if white_counts
            && tag_of("w:color")
                .and_then(val)
                .is_some_and(|c| c.eq_ignore_ascii_case("FFFFFF"))
        {
            Some(Unseen::White)
        } else {
            None
        };
        let Some(why) = why else { continue };
        let mut text = String::new();
        let mut t = 0;
        while let Some((_, te, e)) = next_tag(run, t, "w:t") {
            t = te;
            if e {
                continue;
            }
            if let Some(stop) = run[te..].find("</w:t>") {
                text.push_str(&run[te..te + stop]);
                t = te + stop;
            }
        }
        if !text.trim().is_empty() {
            out.push((start, close, why, text));
        }
    }
    out
}

/// The part with its unseen runs taken out, and how many went.
pub(crate) fn drop_hidden(xml: &str) -> (String, usize) {
    let runs = hidden_runs(xml);
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    for (start, end, ..) in &runs {
        out.push_str(&xml[at..*start]);
        at = *end;
    }
    out.push_str(&xml[at..]);
    (out, runs.len())
}

/// A part with its root element kept and everything in it gone: an empty
/// list of comments, or of people, that the document's relationships can
/// still point at.
pub(crate) fn emptied(xml: &str) -> Option<String> {
    // Past the declaration and any comment, the root's start tag.
    let mut at = 0;
    loop {
        let i = at + xml[at..].find('<')?;
        match xml[i + 1..].chars().next()? {
            '?' | '!' => at = i + xml[i..].find('>')? + 1,
            _ => {
                let end = i + xml[i..].find('>')? + 1;
                let tag = &xml[i..end];
                let name: String = tag[1..]
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
                    .collect();
                let open = tag.trim_end_matches('>').trim_end_matches('/');
                return Some(format!("{}{open}></{name}>", &xml[..i]));
            }
        }
    }
}

/// Parts of a Word document whose revisions and comment marks are taken
/// out: the body, headers, footers and notes.
pub(crate) fn is_story(name: &str) -> bool {
    name == "word/document.xml"
        || [
            "word/header",
            "word/footer",
            "word/footnotes",
            "word/endnotes",
        ]
        .iter()
        .any(|p| name.starts_with(p) && name.ends_with(".xml"))
}

/// Parts that hold comments and their authors: emptied.
pub(crate) fn is_comments(name: &str) -> bool {
    matches!(
        name,
        "word/comments.xml"
            | "word/commentsExtended.xml"
            | "word/commentsIds.xml"
            | "word/commentsExtensible.xml"
            | "word/people.xml"
    )
}

/// The words of Excel's and PowerPoint's comments and speaker notes, and
/// the names of who wrote them: text elements emptied, name attributes
/// blanked, every element kept, so the file opens as it did — with empty
/// comments and notes. Returns the part and how many were emptied; `None`
/// for any other part.
pub(crate) fn blank_notes(name: &str, xml: &str) -> Option<(String, usize)> {
    let xml_part = name.ends_with(".xml");
    let (elements, attrs): (&[&str], &[&str]) = if name.starts_with("ppt/notesSlides/") && xml_part
    {
        (&["a:t"], &[])
    } else if (name.starts_with("xl/comments") || name.starts_with("xl/threadedComments/"))
        && xml_part
    {
        (&["t", "author", "text"], &[])
    } else if name.starts_with("ppt/comments/") && xml_part {
        (&["p:text", "a:t"], &[])
    } else if matches!(
        name,
        "xl/persons/person.xml" | "ppt/commentAuthors.xml" | "ppt/authors.xml"
    ) {
        (&[], &["name", "initials", "displayName", "userId"])
    } else {
        return None;
    };
    let mut n = 0;
    let mut out = xml.to_string();
    for tag in elements {
        out = empty_all(&out, tag, &mut n);
    }
    for attr in attrs {
        out = blank_attr(&out, attr, &mut n);
    }
    Some((out, n))
}

/// Empties every `<name …>…</name>`, keeping its tags.
fn empty_all(xml: &str, name: &str, count: &mut usize) -> String {
    let close = format!("</{name}>");
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    while let Some((_, end, empty)) = next_tag(xml, at, name) {
        out.push_str(&xml[at..end]);
        at = end;
        if empty {
            continue;
        }
        let Some(i) = xml[end..].find(&close) else {
            break;
        };
        if i > 0 {
            *count += 1;
        }
        at = end + i;
    }
    out.push_str(&xml[at..]);
    out
}

/// Blanks every ` name="…"` attribute's value.
fn blank_attr(xml: &str, name: &str, count: &mut usize) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut at = 0;
    loop {
        let found = [format!(" {name}=\""), format!(" {name}='")]
            .into_iter()
            .filter_map(|p| xml.get(at..)?.find(&p).map(|i| (at + i + p.len(), p)))
            .min_by_key(|(i, _)| *i);
        let Some((start, p)) = found else { break };
        let q = if p.ends_with('"') { '"' } else { '\'' };
        let Some(len) = xml[start..].find(q) else {
            break;
        };
        out.push_str(&xml[at..start]);
        if len > 0 {
            *count += 1;
        }
        at = start + len;
    }
    out.push_str(&xml[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_are_accepted_and_comment_marks_go() {
        let xml = r#"<w:body><w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Sales grew</w:t></w:r><w:del w:id="1" w:author="P"><w:r><w:delText>, except the east</w:delText></w:r></w:del><w:ins w:id="2" w:author="O"><w:r><w:t> everywhere</w:t></w:r></w:ins><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r><w:r><w:rPr><w:b/><w:rPrChange w:id="3"><w:rPr/></w:rPrChange></w:rPr><w:t>.</w:t></w:r></w:p><w:p><w:pPr><w:rPr><w:ins w:id="4"/></w:rPr></w:pPr><w:r><w:instrText>PAGE</w:instrText></w:r></w:p></w:body>"#;
        let (out, n) = accept(xml);
        assert_eq!(
            out,
            r#"<w:body><w:p><w:r><w:t>Sales grew</w:t></w:r><w:r><w:t> everywhere</w:t></w:r><w:r></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>.</w:t></w:r></w:p><w:p><w:pPr><w:rPr></w:rPr></w:pPr><w:r><w:instrText>PAGE</w:instrText></w:r></w:p></w:body>"#
        );
        assert_eq!(n, 7);
        let (same, none) = accept("<w:body><w:p><w:r><w:t>plain</w:t></w:r></w:p></w:body>");
        assert_eq!(
            (same.as_str(), none),
            ("<w:body><w:p><w:r><w:t>plain</w:t></w:r></w:p></w:body>", 0)
        );
    }

    #[test]
    fn a_part_is_emptied_to_its_root() {
        let xml = r#"<?xml version="1.0"?><!-- x --><w:comments xmlns:w="w"><w:comment w:id="0" w:author="O"/></w:comments>"#;
        assert_eq!(
            emptied(xml).as_deref(),
            Some(r#"<?xml version="1.0"?><!-- x --><w:comments xmlns:w="w"></w:comments>"#)
        );
        assert_eq!(
            emptied(r#"<w15:people xmlns:w15="x"/>"#).as_deref(),
            Some(r#"<w15:people xmlns:w15="x"></w15:people>"#)
        );
        assert_eq!(emptied("no xml"), None);
    }

    #[test]
    fn broken_markup_is_left_as_it_is() {
        let (out, _) = accept("<w:del w:id=\"1\"><w:r>never closed");
        assert_eq!(out, "<w:del w:id=\"1\"><w:r>never closed");
        for s in [
            "<w:del",
            "<w:ins>",
            "</w:ins><w:ins",
            "<w:commentReference",
            "<?",
            "<",
        ] {
            let _ = accept(s);
            let _ = emptied(s);
        }
    }

    #[test]
    fn comments_and_notes_are_emptied_in_place() {
        let notes = r#"<p:notes><p:sp><p:cNvPr id="175" name="Shape 175"/><a:p><a:r><a:t>Don't mention the layoffs</a:t></a:r></a:p></p:sp></p:notes>"#;
        let (out, n) = blank_notes("ppt/notesSlides/notesSlide1.xml", notes).unwrap();
        assert_eq!(
            out,
            r#"<p:notes><p:sp><p:cNvPr id="175" name="Shape 175"/><a:p><a:r><a:t></a:t></a:r></a:p></p:sp></p:notes>"#
        );
        assert_eq!(n, 1);
        let xl = r#"<comments><authors><author>Olena Koval</author></authors><commentList><comment ref="B2" authorId="0"><text><r><t xml:space="preserve">Too high</t></r></text></comment></commentList></comments>"#;
        let (out, _) = blank_notes("xl/comments1.xml", xl).unwrap();
        assert!(!out.contains("Olena") && !out.contains("Too high"), "{out}");
        assert!(out.contains(r#"<comment ref="B2" authorId="0">"#), "{out}");
        let people = r#"<p:cmAuthorLst><p:cmAuthor id="0" name="Olena Koval" initials='OK' lastIdx="1"/></p:cmAuthorLst>"#;
        let (out, n) = blank_notes("ppt/commentAuthors.xml", people).unwrap();
        assert_eq!(
            out,
            r#"<p:cmAuthorLst><p:cmAuthor id="0" name="" initials='' lastIdx="1"/></p:cmAuthorLst>"#
        );
        assert_eq!(n, 2);
        assert!(blank_notes("word/document.xml", "<w:t>x</w:t>").is_none());
        for s in ["<a:t", "<a:t>", " name=\"", "<t>x</t", " name='x"] {
            let _ = blank_notes("xl/comments1.xml", s);
            let _ = blank_notes("ppt/authors.xml", s);
        }
    }

    #[test]
    fn text_no_one_sees_is_found_and_taken_out() {
        let doc = r#"<w:body><w:p><w:r><w:t>Seen.</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:t>secret note</w:t></w:r><w:r><w:rPr><w:vanish w:val="0"/></w:rPr><w:t>shown</w:t></w:r><w:r><w:rPr><w:color w:val="FFFFFF"/></w:rPr><w:t>hire me</w:t></w:r><w:r><w:rPr><w:sz w:val="2"/></w:rPr><w:t>tiny</w:t></w:r><w:r><w:rPr><w:sz w:val="24"/></w:rPr><w:t>normal</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:t> </w:t></w:r></w:p></w:body>"#;
        let runs = hidden_runs(doc);
        let found: Vec<(Unseen, &str)> = runs.iter().map(|r| (r.2, r.3.as_str())).collect();
        assert_eq!(
            found,
            [
                (Unseen::Hidden, "secret note"),
                (Unseen::White, "hire me"),
                (Unseen::Tiny, "tiny")
            ]
        );
        let (out, n) = drop_hidden(doc);
        assert_eq!(n, 3);
        assert!(out.contains("Seen.") && out.contains("shown") && out.contains("normal"));
        assert!(!out.contains("secret") && !out.contains("hire me") && !out.contains("tiny"));
        // White text on a dark cell is a design, not a secret.
        let shaded = doc.replace(
            "<w:body>",
            r#"<w:body><w:tc><w:tcPr><w:shd w:fill="1F3864"/></w:tcPr></w:tc>"#,
        );
        assert!(hidden_runs(&shaded).iter().all(|r| r.2 != Unseen::White));
        let light = doc.replace(
            "<w:body>",
            r#"<w:body><w:shd w:val="clear" w:fill="auto"/>"#,
        );
        assert!(hidden_runs(&light).iter().any(|r| r.2 == Unseen::White));
    }
}
