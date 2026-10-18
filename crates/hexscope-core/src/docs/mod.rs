//! What each part of a file is, in one plain sentence, and where its format
//! defines it.
//!
//! Every format keeps its explanations next to the parser that names the
//! parts, as a table of label patterns. [`describe`] picks the table by
//! format and the entry by label, with the parent's label where a label
//! alone is not enough (a ZIP entry is named after the file it holds).
//!
//! The sentences and their citations are behind the `explain` feature.
//! Without it a [`Doc`] keeps only its key and concern, and the browser's
//! builds, which
//! leave it out, are smaller by every sentence: the page fetches them, by
//! key, from a file [`every`] writes (examples/explanations.rs), when it
//! first shows one.

use crate::document::Format;
use crate::model::{NodeId, NodeKind, ParseTree};

#[cfg(test)]
pub(crate) mod tests;

/// Where a format defines something: a citation to show, and a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spec {
    /// e.g. "PNG §11.2.2" or "RFC 1951 §3.2.5".
    pub cite: &'static str,
    pub url: &'static str,
}

/// What kind of trouble a problem is, for someone who only wants to know
/// whether to worry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Concern {
    /// Bytes are corrupt or missing: the file will not open properly.
    Damage = 1,
    /// Something is hidden, disguised or contradictory: data outside the
    /// format, two records that disagree, bytes read twice.
    Hidden = 2,
    /// A rule is broken in a way that is usually harmless.
    Oddity = 3,
}

/// One part of a file, explained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Doc {
    /// One sentence in plain words. Only with the `explain` feature.
    #[cfg(feature = "explain")]
    pub text: &'static str,
    /// The explanation's name: FNV-1a of its sentence, then of its
    /// citation and link, the same with or without the feature.
    pub key: u32,
    /// Where the format defines it. Only with the `explain` feature: the
    /// page fetches it with the sentence.
    #[cfg(feature = "explain")]
    pub spec: Option<Spec>,
    /// Set on problems only.
    pub concern: Option<Concern>,
}

/// FNV-1a, 32 bits, going on from `hash`: worked out while compiling, so a
/// build without the sentences never holds them.
const fn fnv(mut hash: u32, text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash
}

impl Doc {
    pub(crate) const fn new(text: &'static str) -> Self {
        Self {
            #[cfg(feature = "explain")]
            text,
            key: fnv(0x811c_9dc5, text),
            #[cfg(feature = "explain")]
            spec: None,
            concern: None,
        }
    }

    /// The same sentence can cite two formats, or two pages of one: the
    /// citation and link are part of the key.
    #[cfg(feature = "explain")]
    pub(crate) const fn cite(self, cite: &'static str, url: &'static str) -> Self {
        Self {
            key: fnv(fnv(self.key, cite), url),
            spec: Some(Spec { cite, url }),
            ..self
        }
    }

    #[cfg(not(feature = "explain"))]
    pub(crate) const fn cite(self, cite: &'static str, url: &'static str) -> Self {
        Self {
            key: fnv(fnv(self.key, cite), url),
            ..self
        }
    }

    pub(crate) const fn concern(self, concern: Concern) -> Self {
        Self {
            concern: Some(concern),
            ..self
        }
    }
}

/// A format's explanations: label patterns, tried in order. `*` matches any
/// run of characters, including none.
pub(crate) type Table = &'static [(&'static str, Doc)];

/// The first entry of `table` whose pattern matches `label`, among problems
/// or among everything else: a problem is never explained as a field that
/// happens to share its first word, nor a field as a problem.
pub(crate) fn lookup(table: Table, label: &str, problem: bool) -> Option<Doc> {
    table
        .iter()
        .find(|(pattern, doc)| doc.concern.is_some() == problem && glob(pattern, label))
        .map(|(_, doc)| *doc)
}

/// Whether `label` matches `pattern`, where `*` stands for any run of
/// characters. Iterative, with one saved backtrack point: linear in practice
/// and never recursive, whatever the label holds.
pub(crate) fn glob(pattern: &str, label: &str) -> bool {
    let (p, l) = (pattern.as_bytes(), label.as_bytes());
    let (mut i, mut j) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while j < l.len() {
        if i < p.len() && p[i] == b'*' {
            star = Some((i, j));
            i += 1;
        } else if i < p.len() && p[i] == l[j] {
            i += 1;
            j += 1;
        } else if let Some((si, sj)) = star {
            i = si + 1;
            j = sj + 1;
            star = Some((si, sj + 1));
        } else {
            return false;
        }
    }
    p[i..].iter().all(|&c| c == b'*')
}

/// What node `id` is. Problems always get an answer, a generic one if their
/// format has nothing specific; other nodes get one when their format knows
/// them.
pub fn describe(tree: &ParseTree, id: NodeId, format: Format) -> Option<Doc> {
    specific(tree, id, format).or_else(|| generic(tree.try_get(id)?.kind))
}

/// The format's own explanation, without the generic fallback.
pub(crate) fn specific(tree: &ParseTree, id: NodeId, format: Format) -> Option<Doc> {
    let node = tree.try_get(id)?;
    let parent = node
        .parent
        .and_then(|p| tree.try_get(p))
        .map(|p| p.label.as_str());
    let label = node.label.as_str();
    let problem = matches!(node.kind, NodeKind::Warning | NodeKind::Error);
    match format {
        Format::Png => crate::png::docs::describe(label, parent, problem).or_else(|| {
            // Inside eXIf, the EXIF tables explain the TIFF block.
            let in_exif = std::iter::successors(node.parent, |&p| tree.try_get(p)?.parent)
                .any(|p| tree.try_get(p).is_some_and(|n| n.label == "eXIf"));
            // A thumbnail inside it is a JPEG of its own.
            in_exif
                .then(|| {
                    crate::exif::docs::describe(label, problem)
                        .or_else(|| crate::jpeg::docs::describe(label, problem))
                })
                .flatten()
        }),
        Format::Jpeg => crate::jpeg::docs::describe(label, problem),
        Format::Heif => crate::heif::docs::describe(tree, node, problem),
        Format::Webp => crate::webp::docs::describe(label, problem),
        Format::Gif => crate::gif::docs::describe(label, problem),
        // A TIFF file is the block EXIF is made of: the same tables explain it.
        Format::Tiff if label == "TIFF" => Some(TIFF),
        Format::Tiff => crate::exif::docs::describe(label, problem),
        Format::Pdf if cfg!(feature = "pdf") => crate::pdf::docs::describe(tree, node, problem),
        Format::Video => crate::video::docs::describe(tree, node, problem),
        Format::Zip if cfg!(feature = "office") => crate::zip::docs::describe(tree, node, problem),
        Format::Wasm if cfg!(feature = "module") => {
            crate::wasm::docs::describe(tree, node, problem)
        }
        Format::Eml if cfg!(feature = "mail") => crate::eml::docs::describe(tree, node, problem),
        Format::Cfb if cfg!(feature = "office") => crate::cfb::docs::describe(tree, node, problem),
        Format::Rtf if cfg!(feature = "office") => crate::rtf::docs::describe(label, problem),
        Format::Svg if cfg!(feature = "svg") => crate::svg::docs::describe(label, problem),
        Format::Unknown => crate::document::docs(label),
        _ => None,
    }
}

const TIFF: Doc = Doc::new(
    "A TIFF image, as scanners and cameras save them: a header, then directories of tags — the picture's size and colours, where its strips of pixels lie, and the same camera, place and time tags a JPEG's EXIF carries.",
)
.cite(
    "TIFF 6.0",
    "https://www.itu.int/itudoc/itu-t/com16/tiff-fx/docs/tiff6.pdf",
);
const UNREADABLE: Doc =
    Doc::new("These bytes could not be read as what the format says they should be.")
        .concern(Concern::Damage);
const RULE_BROKEN: Doc = Doc::new("These bytes were read, but they break a rule of the format.")
    .concern(Concern::Oddity);

fn generic(kind: NodeKind) -> Option<Doc> {
    match kind {
        NodeKind::Error => Some(UNREADABLE),
        NodeKind::Warning => Some(RULE_BROKEN),
        _ => None,
    }
}

/// Every explanation there is, with the family of the browser's build that
/// can give it: `media` (pictures, movies, drawings, unknown files), `pdf`
/// or `docs` (Office, email, WebAssembly). The page's copy of the sentences
/// is written from this.
pub fn every() -> Vec<(&'static str, Doc)> {
    let families: [(&str, &[Table]); 16] = [
        (
            "media",
            &[&[("", TIFF), ("", UNREADABLE), ("", RULE_BROKEN)]],
        ),
        ("media", crate::document::EVERY),
        ("media", crate::exif::docs::EVERY),
        ("media", crate::jpeg::docs::EVERY),
        ("media", crate::png::docs::EVERY),
        ("media", crate::heif::docs::EVERY),
        ("media", crate::webp::docs::EVERY),
        ("media", crate::gif::docs::EVERY),
        ("media", crate::video::docs::EVERY),
        ("media", crate::svg::docs::EVERY),
        ("pdf", crate::pdf::docs::EVERY),
        ("docs", crate::zip::docs::EVERY),
        ("docs", crate::cfb::docs::EVERY),
        ("docs", crate::rtf::docs::EVERY),
        ("docs", crate::eml::docs::EVERY),
        ("docs", crate::wasm::docs::EVERY),
    ];
    families
        .iter()
        .flat_map(|(family, tables)| {
            tables
                .iter()
                .flat_map(|t| t.iter())
                .map(move |(_, doc)| (*family, *doc))
        })
        .collect()
}
