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
}
