//! Every part a parser can name must be explained: the coverage tests walk
//! every fixture, and the damaged files the parser tests build, and fail on
//! any node without its own explanation.

use super::*;
use crate::document::parse;
use crate::model::NodeKind;
use std::path::Path;

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in [
        "tests/fixtures/pngsuite",
        "tests/fixtures",
        "../../apps/web/public/samples",
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
fn damaged() -> Vec<(String, Vec<u8>)> {
    use crate::zip::testing::{Archive, Descriptor, Entry, build};
    let mut out = Vec::new();
    for (name, bytes) in fixtures() {
        if !(name.ends_with("basn2c08.png")
            || name.ends_with("photo.jpg")
            || name.ends_with("report.docx"))
        {
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
    let tables: [(&str, Table); 5] = [
        ("png", crate::png::docs::ALL),
        ("exif tags", crate::exif::docs::TAGS),
        ("exif", crate::exif::docs::ALL),
        ("jpeg", crate::jpeg::docs::ALL),
        ("zip", crate::zip::docs::ALL),
    ];
    for (name, table) in tables {
        for (pattern, d) in table.iter().copied() {
            assert!(
                d.text.len() <= 200,
                "{name} {pattern}: {} chars",
                d.text.len()
            );
            assert!(d.text.ends_with('.'), "{name} {pattern}: no full stop");
            if let Some(s) = d.spec {
                assert!(s.url.starts_with("https://"), "{name} {pattern}");
                assert!(!s.cite.is_empty());
            }
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
