//! Every part a parser can name must be explained: the coverage tests walk
//! every fixture, and the damaged files the parser tests build, and fail on
//! any node without its own explanation.

use super::*;
use crate::document::parse;
use crate::model::NodeKind;
use std::path::Path;

pub(crate) fn fixtures() -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    // Keep a snapshot of the site's public examples inside this crate so the
    // published package's own tests do not depend on the monorepo layout.
    for dir in [
        "tests/fixtures/pngsuite",
        "tests/fixtures",
        "tests/fixtures/site-samples",
    ] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let name = path.display().to_string();
                files.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    files
}

/// Damage the fixtures do not have: cut, flipped and padded variants of a
/// few files, so problem nodes of every kind appear.
pub(crate) fn damaged() -> Vec<(String, Vec<u8>)> {
    use crate::zip::testing::{Archive, Descriptor, Entry, build};
    let mut out = Vec::new();
    for (name, bytes) in fixtures() {
        let kinds = [
            "basn2c08.png",
            "photo.jpg",
            "report.docx",
            "photo.heic",
            "report.pdf",
            "compact.pdf",
            "iphone.mov",
            "android.mp4",
            "hello.wasm",
        ];
        if !kinds.iter().any(|k| name.ends_with(k)) {
            continue;
        }
        for cut in [9, 20, 40, bytes.len() / 3, bytes.len() / 2, bytes.len() - 5] {
            out.push((format!("{name} cut at {cut}"), bytes[..cut].to_vec()));
        }
        let mut flipped = bytes.clone();
        for i in (60..flipped.len()).step_by(97) {
            flipped[i] ^= 0x5A;
        }
        out.push((format!("{name} flipped"), flipped));
        let mut padded = bytes.clone();
        padded.extend_from_slice(b"trailing bytes");
        out.push((format!("{name} padded"), padded));
    }

    let two = Archive {
        entries: vec![
            Entry::new("a.txt", b"hello hello hello", 0),
            Entry::new("b.bin", &b"hexscope ".repeat(40), 8),
        ],
        ..Default::default()
    };
    let b = build(&two);
    let mut named = b.bytes.clone();
    named[b.local[0] as usize + 30] = b'A';
    out.push(("zip: names differ".into(), named));
    let mut pointed = b.bytes.clone();
    pointed[b.central[1] as usize + 42..b.central[1] as usize + 46]
        .copy_from_slice(&0u32.to_le_bytes());
    out.push(("zip: overlap".into(), pointed));
    let mut nowhere = b.bytes.clone();
    nowhere[b.central[1] as usize + 42..b.central[1] as usize + 46]
        .copy_from_slice(&0xFFFF_0000u32.to_le_bytes());
    out.push(("zip: nowhere".into(), nowhere));
    let mut counted = b.bytes.clone();
    counted[b.eocd as usize + 10] = 3;
    out.push(("zip: count".into(), counted));
    let prefixed = build(&Archive {
        prefix: b"MZ junk before the archive".to_vec(),
        ..two.clone()
    });
    out.push(("zip: prefix".into(), prefixed.bytes));
    let mut deferred = two.clone();
    deferred.entries[1].descriptor = Descriptor::NoSignature;
    let d = build(&deferred);
    out.push((
        "zip: no eocd".into(),
        d.bytes[..d.central[0] as usize].to_vec(),
    ));
    out.push(("empty".into(), Vec::new()));
    out.push(("pdf".into(), b"%PDF-1.7 not really".to_vec()));
    out.push(("text".into(), b"just some text".to_vec()));
    out
}

#[test]
fn every_node_of_every_file_is_explained() {
    let mut missing = std::collections::BTreeSet::new();
    let mut nodes = 0;
    for (name, bytes) in fixtures().into_iter().chain(damaged()) {
        let doc = parse(&bytes);
        let tree = doc.tree();
        for n in tree.nodes() {
            nodes += 1;
            if specific(tree, n.id, doc.format()).is_none() {
                missing.insert(format!(
                    "{:?} {:?} {:?} (in {name})",
                    doc.format(),
                    n.kind,
                    n.label
                ));
            }
        }
    }
    let shown: Vec<_> = missing.iter().take(40).collect();
    assert!(
        missing.is_empty(),
        "{} of {nodes} nodes unexplained:\n{shown:#?}",
        missing.len()
    );
}

#[test]
fn problems_say_how_worried_to_be() {
    for (name, bytes) in fixtures().into_iter().chain(damaged()) {
        let doc = parse(&bytes);
        let tree = doc.tree();
        for n in tree.nodes() {
            if matches!(n.kind, NodeKind::Warning | NodeKind::Error)
                && doc.format() != crate::document::Format::Unknown
            {
                let d = describe(tree, n.id, doc.format()).unwrap();
                assert!(
                    d.concern.is_some(),
                    "{:?} in {name} has no concern",
                    n.label
                );
            }
        }
    }
}

#[test]
fn every_exif_tag_is_explained() {
    for ifd in [
        crate::exif::Ifd::Zero,
        crate::exif::Ifd::Exif,
        crate::exif::Ifd::Gps,
    ] {
        for tag in 0..=u16::MAX {
            if let Some(name) = crate::exif::tag_name_for_tests(ifd, tag) {
                assert!(crate::exif::docs::describe(name, false).is_some(), "{name}");
            }
        }
    }
}

#[test]
fn explanations_are_one_plain_sentence_and_links_are_https() {
    // About two lines of the inspector: the longest, TIFF's, is 215 bytes.
    for (family, d) in every() {
        assert!(
            d.text.len() <= 220,
            "{family}: {} bytes: {}",
            d.text.len(),
            d.text
        );
        assert!(d.text.ends_with('.'), "{family}: no full stop: {}", d.text);
        if let Some(s) = d.spec {
            assert!(s.url.starts_with("https://"), "{family}: {}", s.url);
            assert!(!s.cite.is_empty());
        }
    }
}

/// The page finds a sentence and its citation by key in what `every`
/// lists: each part of every file must be there, and no two different
/// explanations may share a key.
#[test]
fn every_explanation_given_is_listed_and_keys_name_one_sentence() {
    let named = |d: &Doc| (d.key, d.text, d.spec.map(|s| (s.cite, s.url)));
    let mut keys: Vec<_> = every().iter().map(|(_, d)| named(d)).collect();
    keys.sort_unstable();
    keys.dedup();
    for pair in keys.windows(2) {
        assert_ne!(
            pair[0].0, pair[1].0,
            "{:?} and {:?} share a key",
            pair[0], pair[1]
        );
    }
    for (name, bytes) in fixtures().into_iter().chain(damaged()) {
        let doc = parse(&bytes);
        let tree = doc.tree();
        for n in tree.nodes() {
            if let Some(d) = describe(tree, n.id, doc.format()) {
                assert!(
                    keys.binary_search(&named(&d)).is_ok(),
                    "{:?} in {name}: {:?} is not listed by every()",
                    n.label,
                    d.text
                );
            }
        }
    }
}

#[cfg(feature = "audio")]
#[test]
fn every_audio_node_is_explained_and_listed_for_the_audio_build() {
    fn ogg_page(sequence: u32, flags: u8, packet: &[u8]) -> Vec<u8> {
        let mut page = b"OggS".to_vec();
        page.extend_from_slice(&[0, flags]);
        page.extend_from_slice(&[0; 8]); // granule position
        page.extend_from_slice(&7u32.to_le_bytes());
        page.extend_from_slice(&sequence.to_le_bytes());
        page.extend_from_slice(&[0; 4]); // CRC is outside metadata inspection
        page.push(1);
        page.push(packet.len() as u8);
        page.extend_from_slice(packet);
        page
    }

    let mut mp3 = b"ID3\x04\0\0\0\0\0\x0d".to_vec();
    mp3.extend_from_slice(b"TIT2\0\0\0\x03\0\0\x03ok");

    let mut flac = b"fLaC\0\0\0\x22".to_vec();
    flac.extend_from_slice(&[0; 34]); // STREAMINFO
    flac.extend_from_slice(&[0x84, 0, 0, 8]); // last block: Vorbis comments
    flac.extend_from_slice(&[0; 8]); // empty vendor and zero comments

    let mut identification = vec![1];
    identification.extend_from_slice(b"vorbis");
    identification.extend_from_slice(&0u32.to_le_bytes());
    identification.push(2);
    identification.extend_from_slice(&44_100u32.to_le_bytes());
    identification.extend_from_slice(&[0; 12]);
    identification.extend_from_slice(&[0x86, 1]);
    let mut comments = b"\x03vorbis".to_vec();
    comments.extend_from_slice(&[0; 8]); // empty vendor and zero comments
    comments.push(1); // Vorbis framing bit
    let mut ogg = ogg_page(0, 2, &identification);
    ogg.extend(ogg_page(1, 0, &comments));

    let samples = [("MP3", mp3), ("FLAC", flac), ("Ogg Vorbis", ogg)];
    let listed: std::collections::BTreeSet<_> = every()
        .into_iter()
        .filter(|(family, _)| *family == "audio")
        .map(|(_, doc)| doc.key)
        .collect();
    assert!(
        !listed.is_empty(),
        "the audio family is absent from every()"
    );

    for (name, bytes) in samples {
        let parsed = parse(&bytes);
        let tree = parsed.tree();
        let format = parsed.format();
        assert!(matches!(format, Format::Mp3 | Format::Flac | Format::Ogg));
        for node in tree.nodes() {
            let doc = specific(tree, node.id, format)
                .unwrap_or_else(|| panic!("{name}: {:?} has no explanation", node.label));
            assert!(
                listed.contains(&doc.key),
                "{name}: {:?} is missing from the audio explanation catalogue",
                node.label
            );
        }
    }
}

#[test]
fn glob_matches_runs_of_anything() {
    assert!(glob("block * · stored*", "block 3 · stored (continued)"));
    assert!(glob("CRC mismatch*", "CRC mismatch: stored 0, computed 1"));
    assert!(glob(
        "* segment runs past the end of the file",
        "DQT segment runs past the end of the file"
    ));
    assert!(glob("a*b*c", "abc"));
    assert!(glob("a*b*c", "a--b--c"));
    assert!(!glob("a*b*c", "a--c--b"));
    assert!(glob("exact", "exact"));
    assert!(!glob("exact", "exactly"));
    assert!(glob("*", ""));
}

#[test]
fn unknown_problems_fall_back_but_unknown_fields_do_not() {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "PNG",
        crate::model::ByteRange::new(0, 0),
        NodeKind::Container,
        None,
    );
    let odd = tree.warning(
        root,
        "a problem no table knows",
        crate::model::ByteRange::new(0, 0),
    );
    let field = tree.add(
        Some(root),
        "IHDR",
        crate::model::ByteRange::new(0, 0),
        NodeKind::Container,
        None,
    );
    let child = tree.add(
        Some(field),
        "notAField",
        crate::model::ByteRange::new(0, 0),
        NodeKind::Field,
        None,
    );
    assert_eq!(
        describe(&tree, odd, Format::Png).unwrap().concern,
        Some(Concern::Oddity)
    );
    assert!(specific(&tree, odd, Format::Png).is_none());
    assert!(describe(&tree, child, Format::Png).is_none());
}
