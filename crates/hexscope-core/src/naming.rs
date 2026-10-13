//! A file whose name says one format while its bytes are another: a HEIC
//! photo saved as `.jpg`, a Word document named `.pdf`. The bytes are fine;
//! apps that go by the name refuse the file or call it damaged, which looks
//! like damage and is not. What the file is decides the name that fits it,
//! down to the kind of document a ZIP holds.

use crate::document::Document;

/// A name that says another format than the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misnamed {
    /// The extension the name has, lower case, without its dot.
    pub ext: String,
    /// What the file is, in words: `a Word document`.
    pub what: &'static str,
    /// The extension that fits it, with its dot: `.docx`.
    pub fits: &'static str,
}

/// The extensions each format is named with.
const EXTENSIONS: &[(&str, &[&str])] = &[
    ("jpeg", &["jpg", "jpeg", "jpe", "jfif"]),
    ("png", &["png"]),
    ("heif", &["heic", "heif", "hif", "avif"]),
    ("webp", &["webp"]),
    ("gif", &["gif"]),
    ("tiff", &["tif", "tiff"]),
    ("video", &["mp4", "m4v", "mov", "qt", "3gp", "3g2", "m4a"]),
    ("pdf", &["pdf"]),
    (
        "zip",
        &[
            "zip", "docx", "docm", "xlsx", "xlsm", "pptx", "pptm", "odt", "ods", "odp", "epub",
            "apk", "aab", "jar", "war", "xpi", "ipa", "whl", "nupkg", "kmz", "3mf", "usdz",
        ],
    ),
    ("wasm", &["wasm"]),
    ("eml", &["eml", "mbox"]),
    ("msg", &["msg"]),
    (
        "office97",
        &["doc", "dot", "xls", "xlt", "xla", "ppt", "pot", "pps"],
    ),
    ("rtf", &["rtf"]),
];

/// Documents a ZIP can be, each with the extensions that are its own.
const ZIP_KINDS: &[(&str, &str, &[&str])] = &[
    ("a Word document", ".docx", &["docx", "docm"]),
    ("an Excel workbook", ".xlsx", &["xlsx", "xlsm"]),
    ("a PowerPoint presentation", ".pptx", &["pptx", "pptm"]),
    ("a LibreOffice text document", ".odt", &["odt"]),
    ("a LibreOffice spreadsheet", ".ods", &["ods"]),
    ("a LibreOffice presentation", ".odp", &["odp"]),
    ("an EPUB book", ".epub", &["epub"]),
    ("an Android app", ".apk", &["apk", "aab"]),
];

/// Whether `name` says another format than `doc` is, and if so what it is
/// and the name that fits; `None` when the name fits, has no extension, or
/// the file is not one hexscope knows.
pub fn misnamed(name: &str, doc: &Document, data: &[u8]) -> Option<Misnamed> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let (stem, ext) = base.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    let ext = ext.to_ascii_lowercase();
    let claimed = EXTENSIONS
        .iter()
        .find(|(_, exts)| exts.contains(&ext.as_str()))?
        .0;
    let (format, what, fits) = what_it_is(doc, data)?;
    if claimed != format {
        return Some(Misnamed { ext, what, fits });
    }
    // A ZIP named as another kind of document: a Word file called `.xlsx`.
    if format == "zip" {
        let named_as_document = ZIP_KINDS
            .iter()
            .any(|(_, _, exts)| exts.contains(&ext.as_str()));
        let own = ZIP_KINDS
            .iter()
            .find(|(w, _, _)| *w == what)
            .map(|(_, _, exts)| *exts);
        if named_as_document
            && let Some(own) = own
            && !own.contains(&ext.as_str())
        {
            return Some(Misnamed { ext, what, fits });
        }
    }
    None
}

/// The file's format as `EXTENSIONS` names it, what it is in words, and the extension that fits.
fn what_it_is(doc: &Document, data: &[u8]) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match doc {
        Document::Jpeg(_) => ("jpeg", "a JPEG", ".jpg"),
        Document::Png(_) => ("png", "a PNG", ".png"),
        Document::Heif(_) => {
            if matches!(data.get(8..12), Some(b"avif" | b"avis")) {
                ("heif", "an AVIF image", ".avif")
            } else {
                ("heif", "a HEIC image", ".heic")
            }
        }
        Document::Webp(_) => ("webp", "a WebP image", ".webp"),
        Document::Gif(_) => ("gif", "a GIF", ".gif"),
        Document::Tiff(_) => ("tiff", "a TIFF image", ".tif"),
        Document::Video(_) => ("video", "an MP4 or QuickTime movie", ".mp4"),
        Document::Pdf(_) => ("pdf", "a PDF", ".pdf"),
        Document::Wasm(_) => ("wasm", "a WebAssembly module", ".wasm"),
        Document::Eml(_) => ("eml", "an email", ".eml"),
        Document::Rtf(_) => ("rtf", "a Rich Text document", ".rtf"),
        Document::Cfb(d) => match d.kind {
            crate::cfb::Kind::Message => ("msg", "an Outlook message", ".msg"),
            crate::cfb::Kind::Word => ("office97", "a Word 97–2003 document", ".doc"),
            crate::cfb::Kind::Excel => ("office97", "an Excel 97–2003 workbook", ".xls"),
            crate::cfb::Kind::PowerPoint => {
                ("office97", "a PowerPoint 97–2003 presentation", ".ppt")
            }
            crate::cfb::Kind::Other => return None,
        },
        Document::Zip(d) => {
            let has = |n: &str| d.entries.iter().any(|e| e.name == n);
            let mimetype = d
                .entries
                .iter()
                .find(|e| e.name == "mimetype" && e.method == 0)
                .and_then(|e| data.get(e.data.start as usize..e.data.end() as usize))
                .unwrap_or_default();
            let kind = if has("word/document.xml") {
                0
            } else if has("xl/workbook.xml") {
                1
            } else if has("ppt/presentation.xml") {
                2
            } else if mimetype.ends_with(b"opendocument.text") {
                3
            } else if mimetype.ends_with(b"opendocument.spreadsheet") {
                4
            } else if mimetype.ends_with(b"opendocument.presentation") {
                5
            } else if mimetype == b"application/epub+zip" {
                6
            } else if has("AndroidManifest.xml") {
                7
            } else {
                return Some(("zip", "a ZIP archive", ".zip"));
            };
            let (what, fits, _) = ZIP_KINDS[kind];
            ("zip", what, fits)
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::parse;

    fn named(name: &str, path: &str) -> Option<Misnamed> {
        let data = std::fs::read(format!(
            "{}/tests/fixtures/{path}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        misnamed(name, &parse(&data), &data)
    }

    #[test]
    fn a_name_that_fits_says_nothing() {
        assert_eq!(named("photo.jpg", "photo.jpg"), None);
        assert_eq!(named("report.DOCX", "report.docx"), None);
        assert_eq!(named("lease.odt", "lease.odt"), None);
        assert_eq!(named("no-extension", "photo.jpg"), None);
        assert_eq!(named(".hidden", "photo.jpg"), None);
        // An extension hexscope does not know is not a wrong one.
        assert_eq!(named("photo.bin", "photo.jpg"), None);
    }

    #[test]
    fn a_name_that_says_another_format_is_named_with_the_one_that_fits() {
        let pdf = named("contract.pdf", "report.docx").unwrap();
        assert_eq!(
            (pdf.ext.as_str(), pdf.what, pdf.fits),
            ("pdf", "a Word document", ".docx")
        );
        let jpg = named("IMG_0001.jpg", "photo.heic").unwrap();
        assert_eq!((jpg.what, jpg.fits), ("a HEIC image", ".heic"));
        let ods = named("budget.xlsx", "budget.ods").unwrap();
        assert_eq!((ods.what, ods.fits), ("a LibreOffice spreadsheet", ".ods"));
        let rtf = named("lease.doc", "lease.rtf").unwrap();
        assert_eq!((rtf.what, rtf.fits), ("a Rich Text document", ".rtf"));
    }
}
