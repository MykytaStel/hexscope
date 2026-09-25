//! What each part of a file is, in one plain sentence, and where its format
//! defines it.
//!
//! Every format keeps its explanations next to the parser that names the
//! parts, as a table of label patterns. [`describe`] picks the table by
//! format and the entry by label, with the parent's label where a label
//! alone is not enough (a ZIP entry is named after the file it holds).

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
    /// One sentence in plain words.
    pub text: &'static str,
    pub spec: Option<Spec>,
    /// Set on problems only.
    pub concern: Option<Concern>,
}

impl Doc {
    pub(crate) const fn new(text: &'static str) -> Self {
        Self {
            text,
            spec: None,
            concern: None,
        }
    }

    pub(crate) const fn cite(self, cite: &'static str, url: &'static str) -> Self {
        Self {
            spec: Some(Spec { cite, url }),
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
        Format::Pdf => crate::pdf::docs::describe(tree, node, problem),
        Format::Video => crate::video::docs::describe(tree, node, problem),
        Format::Zip => crate::zip::docs::describe(tree, node, problem),
        Format::Unknown => crate::document::docs(label),
    }
}

fn generic(kind: NodeKind) -> Option<Doc> {
    match kind {
        NodeKind::Error => Some(
            Doc::new("These bytes could not be read as what the format says they should be.")
                .concern(Concern::Damage),
        ),
        NodeKind::Warning => Some(
            Doc::new("These bytes were read, but they break a rule of the format.")
                .concern(Concern::Oddity),
        ),
        _ => None,
    }
}
