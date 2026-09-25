//! Recognising a file's format and parsing it with the right module.

use crate::heif::{self, HeifDocument, parse_heif};
use crate::jpeg::{self, JpegDocument, parse_jpeg};
use crate::model::{ByteRange, NodeKind, ParseTree};
use crate::pdf::{self, PdfDocument, parse_pdf};
use crate::png::{PngDocument, parse_png};
use crate::zip::{self, ZipDocument, parse_zip};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Heif,
    Pdf,
    Zip,
    Unknown,
}

/// A parsed file of any supported format. Every variant carries a tree;
/// what else it carries depends on what the format has to show.
#[derive(Debug)]
pub enum Document {
    Png(PngDocument),
    Jpeg(JpegDocument),
    Heif(HeifDocument),
    Pdf(PdfDocument),
    Zip(ZipDocument),
    Unknown(ParseTree),
}

impl Document {
    pub fn tree(&self) -> &ParseTree {
        match self {
            Document::Png(d) => &d.tree,
            Document::Jpeg(d) => &d.tree,
            Document::Heif(d) => &d.tree,
            Document::Pdf(d) => &d.tree,
            Document::Zip(d) => &d.tree,
            Document::Unknown(t) => t,
        }
    }

    pub fn format(&self) -> Format {
        match self {
            Document::Png(_) => Format::Png,
            Document::Jpeg(_) => Format::Jpeg,
            Document::Heif(_) => Format::Heif,
            Document::Pdf(_) => Format::Pdf,
            Document::Zip(_) => Format::Zip,
            Document::Unknown(_) => Format::Unknown,
        }
    }
}

/// Parses any file. Never fails: an unrecognised format still yields a tree
/// that says so, and names the format when its signature is familiar.
pub fn parse(data: &[u8]) -> Document {
    // "PNG" in bytes 1-3 is enough: a signature damaged elsewhere — by a
    // text-mode transfer, say — is a broken PNG, not an unknown file.
    if data.get(1..4) == Some(b"PNG") {
        return Document::Png(parse_png(data));
    }
    if data.starts_with(&jpeg::MAGIC) {
        return Document::Jpeg(parse_jpeg(data));
    }
    if heif::is_heif(data) {
        return Document::Heif(parse_heif(data));
    }
    if pdf::is_pdf(data) {
        return Document::Pdf(parse_pdf(data));
    }
    // After the images, whose files may carry a ZIP on their end: an
    // archive is what the file is only when nothing earlier claimed it.
    if zip::is_zip(data) {
        return Document::Zip(parse_zip(data));
    }
    Document::Unknown(unknown(data))
}

/// Signatures of formats people are likely to drop in, so the answer can be
/// "that is a gzip" rather than "unrecognised".
pub(crate) fn identify(data: &[u8]) -> Option<(&'static str, u64)> {
    const SIGNATURES: [(&[u8], &str); 10] = [
        (b"GIF87a", "a GIF image"),
        (b"GIF89a", "a GIF image"),
        (b"\0asm", "a WebAssembly module"),
        (b"\x7FELF", "an ELF executable"),
        (b"\xCF\xFA\xED\xFE", "a Mach-O executable"),
        (b"MZ", "a Windows executable"),
        (b"\x1F\x8B", "a gzip archive"),
        (b"7z\xBC\xAF\x27\x1C", "a 7-Zip archive"),
        (b"Rar!", "a RAR archive"),
        (b"OggS", "an Ogg media file"),
    ];
    if let Some((sig, name)) = SIGNATURES.iter().find(|(sig, _)| data.starts_with(sig)) {
        return Some((name, sig.len() as u64));
    }
    if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        return Some(("a WebP image", 12));
    }
    // HEIF brands were read as HEIF before this; what is left is video.
    if data.get(4..8) == Some(b"ftyp") {
        return Some(("an MP4 or QuickTime video", 12));
    }
    None
}

fn unknown(data: &[u8]) -> ParseTree {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "File",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let (label, len) = match (data.is_empty(), identify(data)) {
        (true, _) => ("empty file".to_string(), 0),
        (false, Some((name, len))) => (format!("this looks like {name} — not supported yet"), len),
        (false, None) => (
            "format not recognised: not a PNG, a JPEG or a ZIP".to_string(),
            data.len().min(8) as u64,
        ),
    };
    tree.error(root, label, ByteRange::new(0, len));
    tree
}

/// What the parts of a file hexscope does not read are.
pub(crate) fn docs(label: &str) -> Option<crate::docs::Doc> {
    use crate::docs::{Concern, Doc, Table};
    const TABLE: Table = &[
        (
            "File",
            Doc::new("A file in a format hexscope does not take apart; its bytes are still shown."),
        ),
        (
            "empty file",
            Doc::new("The file has no bytes at all.").concern(Concern::Damage),
        ),
        (
            "this looks like *",
            Doc::new(
                "The file starts the way this other format does, one hexscope does not read yet.",
            ),
        ),
        (
            "format not recognised*",
            Doc::new(
                "The first bytes match no format hexscope knows, so what the file is cannot be told from them.",
            ),
        ),
    ];
    // Unknown files are described whatever the node's kind: their only
    // nodes are the root and the message saying what the file is not.
    TABLE
        .iter()
        .find(|(p, _)| crate::docs::glob(p, label))
        .map(|(_, d)| *d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_by_signature() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(parse(&png).format(), Format::Png);
        assert_eq!(parse(&[0xFF, 0xD8, 0xFF, 0xE0]).format(), Format::Jpeg);
        assert_eq!(parse(b"hello").format(), Format::Unknown);
        assert_eq!(parse(b"PK\x03\x04 and the rest").format(), Format::Zip);
        assert_eq!(parse(b"PK\x05\x06").format(), Format::Zip);
        assert_eq!(parse(b"%PDF-1.7 ...").format(), Format::Pdf);
        assert_eq!(parse(b"\0\0\0\x10ftypheic\0\0\0\0").format(), Format::Heif);
    }

    #[test]
    fn a_png_with_a_damaged_signature_is_still_a_png() {
        // What an 8-bit-stripping transfer does to the first byte.
        let damaged = [0x09, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(parse(&damaged).format(), Format::Png);
    }

    #[test]
    fn familiar_formats_are_named() {
        let label = |data: &[u8]| {
            let doc = parse(data);
            let tree = doc.tree();
            tree.get(tree.get(0).children[0]).label.clone()
        };
        assert!(label(b"\x1F\x8B\x08....").contains("gzip"));
        assert!(label(b"\0\0\0\x18ftypisom....").contains("MP4"));
        assert!(label(b"").contains("empty"));
        assert!(label(b"just some text").contains("not recognised"));
    }

    #[test]
    fn every_input_returns_a_tree() {
        for data in [
            &b""[..],
            b"\xFF",
            b"\xFF\xD8",
            b"\xFF\xD8\xFF",
            b"xPNG",
            b"RIFF",
        ] {
            assert!(!parse(data).tree().is_empty());
        }
    }
}
