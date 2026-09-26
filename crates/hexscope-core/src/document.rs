//! Recognising a file's format and parsing it with the right module.

use crate::heif::{self, HeifDocument, parse_heif};
use crate::jpeg::{self, JpegDocument, parse_jpeg};
use crate::model::{ByteRange, NodeKind, ParseTree};
use crate::pdf::{self, PdfDocument, parse_pdf};
use crate::png::{PngDocument, parse_png};
use crate::video::{self, VideoDocument, parse_video};
use crate::wasm::{self, WasmDocument, parse_wasm};
use crate::zip::{self, ZipDocument, parse_zip};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Heif,
    Video,
    Pdf,
    Zip,
    Wasm,
    Unknown,
}

/// A parsed file of any supported format. Every variant carries a tree;
/// what else it carries depends on what the format has to show.
#[derive(Debug)]
pub enum Document {
    Png(PngDocument),
    Jpeg(JpegDocument),
    Heif(HeifDocument),
    Video(VideoDocument),
    Pdf(PdfDocument),
    Zip(ZipDocument),
    Wasm(WasmDocument),
    Unknown(ParseTree),
}

impl Document {
    pub fn tree(&self) -> &ParseTree {
        match self {
            Document::Png(d) => &d.tree,
            Document::Jpeg(d) => &d.tree,
            Document::Heif(d) => &d.tree,
            Document::Video(d) => &d.tree,
            Document::Pdf(d) => &d.tree,
            Document::Zip(d) => &d.tree,
            Document::Wasm(d) => &d.tree,
            Document::Unknown(t) => t,
        }
    }

    pub fn format(&self) -> Format {
        match self {
            Document::Png(_) => Format::Png,
            Document::Jpeg(_) => Format::Jpeg,
            Document::Heif(_) => Format::Heif,
            Document::Video(_) => Format::Video,
            Document::Pdf(_) => Format::Pdf,
            Document::Zip(_) => Format::Zip,
            Document::Wasm(_) => Format::Wasm,
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
    if video::is_video(data) {
        return Document::Video(parse_video(data));
    }
    if wasm::is_wasm(data) {
        return Document::Wasm(parse_wasm(data));
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
    const SIGNATURES: [(&[u8], &str); 9] = [
        (b"GIF87a", "a GIF image"),
        (b"GIF89a", "a GIF image"),
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
    None
}

/// Whether the start of a file reads as text: UTF-8, with no control
/// characters but tabs and line ends. A cut in the middle of a character
/// at the end of the sample does not count against it.
fn is_text(data: &[u8]) -> bool {
    let sample = &data[..data.len().min(4096)];
    let valid = match std::str::from_utf8(sample) {
        Ok(s) => s,
        Err(e) if e.valid_up_to() + 4 > sample.len() && e.error_len().is_none() => {
            std::str::from_utf8(&sample[..e.valid_up_to()]).unwrap_or_default()
        }
        Err(_) => return false,
    };
    !valid.is_empty()
        && valid
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
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
        (false, None) if is_text(data) => (
            "plain text, which reads in the column beside the bytes".to_string(),
            data.len() as u64,
        ),
        (false, None) => (
            "format not recognised by its first bytes".to_string(),
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
            "plain text*",
            Doc::new(
                "Text, not a format hexscope takes apart: every byte is shown, and the words read in the text column beside them.",
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
        assert_eq!(parse(b"\0\0\0\x10ftypisom\0\0\0\0").format(), Format::Video);
        assert_eq!(parse(b"\0asm\x01\0\0\0").format(), Format::Wasm);
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
        assert!(label(b"").contains("empty"));
        assert!(label(b"just some text\n").starts_with("plain text"));
        assert!(label("пам'ять, cut mid-char: \u{00e9}".as_bytes()).starts_with("plain text"));
        assert!(label(b"\x00\x01binary").contains("not recognised"));
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
