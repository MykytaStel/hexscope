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
    /// Stable explanation code shared with the browser's `Doc.key`.
    pub code: u32,
    /// Structural detail from the parser, retained for machine consumers.
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
    /// What it gives away as stable code and file value pairs. Display words
    /// for those codes live in `finding_kinds`.
    pub facts: Vec<(&'static str, String)>,
    /// False when part of the file was not read: damage, bytes nothing in
    /// the file accounts for, or metadata the reader could not finish. A
    /// hidden finding the parser read whole — an attachment, text under a
    /// black box — leaves it complete: what it hides is a fact of its own.
    pub complete: bool,
}

impl Summary {
    /// Format-aware read status. Prefer this over `complete` for admission decisions:
    /// an unknown byte tree may account for every byte without inspecting its format.
    pub fn inspection_status(&self) -> crate::coverage::CoverageStatus {
        crate::coverage::CoverageStatus::from_format_and_completeness(self.format, self.complete)
    }

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
        ("screenshot", &facts.screenshot),
        ("ai", &facts.ai),
        ("credentials", &facts.credentials),
        ("prompt", &facts.prompt),
    ];
    for (kind, fact) in listed {
        if let Some(f) = fact {
            out.push((kind, f.text.clone()));
        }
    }
}

/// A problem that leaves bytes unread: damage, or data nothing in the file
/// accounts for — before or after it, in sectors no stream claims, in a ZIP
/// past its end or in entries nothing points at.
fn unread(p: &Problem) -> bool {
    let l = p.label.as_str();
    p.concern == Concern::Damage
        || (p.concern == Concern::Hidden
            && (["data after", "data before", "unclaimed"]
                .iter()
                .any(|s| l.starts_with(s))
                || l.ends_with("after the end of the archive")
                || l.ends_with("nothing points at")))
}

// The ZIP reader deliberately leaves these conditions visible without
// classifying them as damage. They still prevent a complete inspection.
fn zip_directory_complete(tree: &crate::model::ParseTree) -> bool {
    !tree.nodes().iter().any(|n| {
        let label = n.label.as_str();
        (n.kind == NodeKind::Field && label.starts_with("spanned over several disks:"))
            || (n.kind == NodeKind::Warning
                && ((label.starts_with("stopped after ") && label.ends_with(" entries"))
                    || (label.starts_with("counts ")
                        && label.contains(" entries; the central directory holds "))))
    })
}

pub fn summarize(data: &[u8]) -> Summary {
    summary_of(&parse(data))
}

/// The summary of a document already read, such as a movie given without
/// its media.
pub fn summary_of(doc: &Document) -> Summary {
    let tree = doc.tree();
    let problems: Vec<Problem> = tree
        .nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
        .map(|n| {
            // Every problem has a description, with a generic fallback. Keep
            // its key so every output can use the same wording as the page.
            let description =
                describe(tree, n.id, doc.format()).expect("every problem has a description");
            Problem {
                code: description.key,
                label: n.label.clone(),
                concern: if n.kind == NodeKind::Error {
                    Concern::Damage
                } else {
                    description.concern.unwrap_or(Concern::Oddity)
                },
                offset: n.range.start,
                len: n.range.len,
            }
        })
        .collect();
    let mut facts = Vec::new();
    let (format, metadata_complete) = match doc {
        Document::Png(d) => {
            photo(&d.facts, &mut facts);
            ("png", true)
        }
        Document::Jpeg(d) => {
            photo(&d.facts, &mut facts);
            ("jpeg", d.metadata_complete)
        }
        Document::Heif(d) => {
            photo(&d.facts, &mut facts);
            ("heif", true)
        }
        Document::Webp(d) => {
            photo(&d.facts, &mut facts);
            ("webp", true)
        }
        Document::Gif(d) => {
            photo(&d.facts, &mut facts);
            ("gif", true)
        }
        Document::Tiff(d) => {
            photo(&d.facts, &mut facts);
            ("tiff", true)
        }
        Document::Video(d) => {
            photo(&d.facts, &mut facts);
            ("video", true)
        }
        Document::Pdf(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("pdf", d.metadata_complete)
        }
        Document::Zip(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("zip", zip_directory_complete(&d.tree))
        }
        Document::Wasm(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("wasm", true)
        }
        Document::Cfb(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            (d.kind.format(), true)
        }
        Document::Eml(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("eml", true)
        }
        Document::Rtf(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("rtf", true)
        }
        Document::Svg(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("svg", true)
        }
        #[cfg(feature = "audio")]
        Document::Audio(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            let format = match d.format {
                crate::audio::AudioFormat::Mp3 => "mp3",
                crate::audio::AudioFormat::Flac => "flac",
                crate::audio::AudioFormat::Ogg
                | crate::audio::AudioFormat::OggVorbis
                | crate::audio::AudioFormat::OggOpus => "ogg",
                crate::audio::AudioFormat::Wav => "wav",
                crate::audio::AudioFormat::Rifx => "rifx",
                crate::audio::AudioFormat::Rf64 => "rf64",
                crate::audio::AudioFormat::Bw64 => "bw64",
            };
            (format, d.metadata_complete)
        }
        Document::Unknown(d) => {
            facts.extend(d.facts.iter().map(|f| (f.kind, f.text.clone())));
            ("unknown", true)
        }
    };
    let complete = metadata_complete && !problems.iter().any(unread);
    Summary {
        format,
        problems,
        facts,
        complete,
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
    fn zip_directory_limits_are_not_complete_inspections() {
        use crate::model::{ByteRange, ParseTree};

        for (kind, label) in [
            (
                NodeKind::Field,
                "spanned over several disks: only the last one's records are here",
            ),
            (NodeKind::Warning, "stopped after 100000 entries"),
            (
                NodeKind::Warning,
                "counts 3 entries; the central directory holds 2",
            ),
        ] {
            let mut tree = ParseTree::new();
            tree.add(None, label, ByteRange::new(0, 0), kind, None);
            assert!(!zip_directory_complete(&tree), "{label}");
        }
        let mut tree = ParseTree::new();
        tree.add(
            None,
            "counts 3 entries; the central directory holds 2",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );
        assert!(zip_directory_complete(&tree));
    }

    #[cfg(feature = "office")]
    #[test]
    fn zip_directory_status_reaches_the_shared_summary() {
        use crate::coverage::CoverageStatus;
        use crate::zip::testing::{Archive, Entry, build};

        let archive = Archive {
            entries: vec![Entry::new("example.txt", b"sample", 0)],
            ..Default::default()
        };
        let built = build(&archive);
        assert_eq!(
            summarize(&built.bytes).inspection_status(),
            CoverageStatus::Complete
        );

        let mut split = built.bytes.clone();
        let eocd = built.eocd as usize;
        split[eocd + 4..eocd + 6].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(
            summarize(&split).inspection_status(),
            CoverageStatus::Partial
        );

        let mut wrong_count = built.bytes;
        wrong_count[eocd + 8..eocd + 10].copy_from_slice(&2u16.to_le_bytes());
        wrong_count[eocd + 10..eocd + 12].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            summarize(&wrong_count).inspection_status(),
            CoverageStatus::Partial
        );
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
        let problem = &s.problems[0];
        let parsed = parse(&png);
        let node = parsed
            .tree()
            .nodes()
            .iter()
            .find(|n| matches!(n.kind, NodeKind::Error | NodeKind::Warning))
            .unwrap();
        let explanation = describe(parsed.tree(), node.id, parsed.format()).unwrap();
        assert_eq!(problem.code, explanation.key);
        #[cfg(feature = "explain")]
        assert_eq!(crate::docs::wording(problem.code), Some(explanation.text));
        assert_eq!(summarize(b"hello").format, "unknown");
    }
}
