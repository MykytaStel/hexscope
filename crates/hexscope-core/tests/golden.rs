use hexscope_core::model::{NodeKind, ParseTree};
use hexscope_core::png::{PngDocument, parse_png};
use std::fs;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pngsuite")
        .join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Renders the tree as indented text so snapshots stay readable in review.
fn render(tree: &ParseTree) -> String {
    fn walk(tree: &ParseTree, id: u32, depth: usize, out: &mut String) {
        let node = tree.get(id);
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!(
            "{} [{}..{}] {:?}",
            node.label,
            node.range.start,
            node.range.end(),
            node.kind
        ));
        if let Some(value) = &node.value {
            out.push_str(&format!(" = {value:?}"));
        }
        out.push('\n');
        for &child in &node.children {
            walk(tree, child, depth + 1, out);
        }
    }

    let mut out = String::new();
    if let Some(root) = tree.root() {
        walk(tree, root, 0, &mut out);
    }
    out
}

#[test]
fn basic_rgb_snapshot() {
    let doc = parse_png(&fixture("basn2c08.png"));
    insta::assert_snapshot!(render(&doc.tree));
}

#[test]
fn decodes_pixels_for_a_basic_image() {
    let doc = parse_png(&fixture("basn2c08.png"));
    let ihdr = doc.ihdr.expect("IHDR present");
    assert_eq!((ihdr.width, ihdr.height), (32, 32));
    assert_eq!(doc.pixels.expect("pixels decoded").len(), 32 * 32 * 3);
}

#[test]
fn corrupt_files_are_flagged_and_still_produce_a_tree() {
    // Every PngSuite file starting with `x` is intentionally broken. We do not
    // depend on any single filename — only on the guarantee that damage is
    // reported rather than hidden, and that a tree comes back regardless.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut corrupt_seen = 0;
    let mut flagged = 0;

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with('x') || !name.ends_with(".png") {
            continue;
        }
        corrupt_seen += 1;

        let doc = parse_png(&fs::read(&path).unwrap());
        assert!(!doc.tree.is_empty(), "{name} produced an empty tree");

        let problems = doc
            .tree
            .nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .count();
        if problems > 0 {
            flagged += 1;
        }
    }

    assert!(
        corrupt_seen >= 10,
        "only found {corrupt_seen} corrupt fixtures"
    );
    assert!(
        flagged * 2 >= corrupt_seen,
        "only {flagged} of {corrupt_seen} corrupt files were flagged"
    );
}

#[test]
fn every_pngsuite_file_produces_a_tree_without_panicking() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let doc: PngDocument = parse_png(&bytes);
        assert!(
            !doc.tree.is_empty(),
            "{} produced an empty tree",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 100, "only checked {checked} files");
}

#[test]
fn arbitrary_bytes_still_produce_a_tree() {
    for input in [
        vec![],
        vec![0u8; 8],
        b"not a png at all".to_vec(),
        vec![0x89, b'P', b'N', b'G'],
    ] {
        let doc = parse_png(&input);
        assert!(!doc.tree.is_empty(), "empty tree for {input:?}");
    }
}
