//! A file's findings in one place, for tools other than the page — the
//! command line, a CI check: what the file is, what is wrong with it and how
//! much that matters, and what it gives away about the people behind it.
//! The same answers the page's verdict is built from.

use crate::docs::{Concern, describe};
use crate::document::{Document, parse};
use crate::exif::PhotoFacts;
use crate::fixed::fixed;
use crate::model::NodeKind;

/// One problem: what it is, how much it matters, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub label: String,
    pub concern: Concern,
    pub offset: u64,
    pub len: u64,
}

/// What a file is, what is wrong with it and what it gives away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// `png`, `jpeg`, `heif`, `video`, `pdf`, `zip`, `wasm` or `unknown`.
    pub format: &'static str,
    pub problems: Vec<Problem>,
    /// What it gives away: a kind such as `location` or `author`, and its
    /// text.
    pub facts: Vec<(&'static str, String)>,
}

impl Summary {
    /// The problems of one concern.
    pub fn count(&self, concern: Concern) -> usize {
        self.problems
            .iter()
            .filter(|p| p.concern == concern)
            .count()
    }
}

fn photo(facts: &PhotoFacts, out: &mut Vec<(&'static str, String)>) {
    if let Some(l) = &facts.location {
        out.push((
            "location",
            format!("{}, {}", fixed(l.latitude, 5), fixed(l.longitude, 5)),
        ));
    }
    let listed = [
        ("camera", &facts.camera),
        ("lens", &facts.lens),
        ("serial", &facts.serial),
        ("owner", &facts.owner),
        ("place", &facts.place),
        ("taken", &facts.taken),
        ("caption", &facts.caption),
        ("software", &facts.software),
        ("original", &facts.original),
        ("history", &facts.history),
        ("thumbnail", &facts.thumbnail),
        ("shutter", &facts.shutter),
        ("uptime", &facts.uptime),
        ("linked", &facts.linked),
    ];
    for (kind, fact) in listed {
        if let Some(f) = fact {
            out.push((kind, f.text.clone()));
        }
    }
}

pub fn summarize(data: &[u8]) -> Summary {
    let doc = parse(data);
    let tree = doc.tree();
    let problems = tree
        .nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        .map(|n| Problem {
            label: n.label.clone(),
            concern: if n.kind == NodeKind::Error {
                Concern::Damage
            } else {
                describe(tree, n.id, doc.format())
                    .and_then(|d| d.concern)
                    .unwrap_or(Concern::Oddity)
            },
            offset: n.range.start,
            len: n.range.len,
        })
        .collect();
    let mut facts = Vec::new();
    let format = match &doc {
        Document::Png(d) => {
            photo(&d.facts, &mut facts);
            "png"
        }
        Document::Jpeg(d) => {
            photo(&d.facts, &mut facts);
            "jpeg"
        }
        Document::Heif(d) => {
            photo(&d.facts, &mut facts);
            "heif"
        }
        Document::Video(d) => {
            photo(&d.facts, &mut facts);
            "video"
        }
        Document::Pdf(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            "pdf"
        }
        Document::Zip(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            "zip"
        }
        Document::Wasm(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            "wasm"
        }
        Document::Unknown(_) => "unknown",
    };
    Summary {
        format,
        problems,
        facts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn a_photo_says_where_and_with_what() {
        let s = summarize(&fixture("photo.jpg"));
        assert_eq!(s.format, "jpeg");
        assert!(
            s.facts.iter().any(|(k, _)| *k == "location"),
            "{:?}",
            s.facts
        );
        assert_eq!(s.count(Concern::Damage), 0);
    }

    #[test]
    fn a_blacked_out_pdf_hides_something() {
        let s = summarize(&fixture("redacted.pdf"));
        assert_eq!(s.format, "pdf");
        assert!(s.count(Concern::Hidden) >= 2, "{:?}", s.problems);
        assert!(s.facts.iter().any(|(k, _)| *k == "covered"));
    }

    #[test]
    fn a_broken_png_is_damage() {
        let mut png = fixture("pngsuite/basn2c08.png");
        png[29] ^= 0xFF;
        let s = summarize(&png);
        assert_eq!(s.format, "png");
        assert_eq!(s.count(Concern::Damage), 1);
        assert_eq!(summarize(b"hello").format, "unknown");
    }
}
