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
    // Every intentionally-corrupt fixture is flagged today. Asserting the exact
    // count rather than a fraction is what makes this a real regression guard:
    // surfacing damage instead of hiding it is the crate's whole job.
    assert_eq!(
        flagged, corrupt_seen,
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

#[test]
fn a_truncated_chunk_points_at_where_the_damage_starts() {
    let mut bytes = fixture("basn2c08.png");
    // Cut inside IDAT, which the committed snapshot shows begins at offset 49.
    bytes.truncate(60);

    let doc = parse_png(&bytes);
    let error = doc
        .tree
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Error)
        .expect("a truncated file must produce an Error node");

    assert_eq!(error.label, "truncated chunk");
    assert_eq!(
        error.range.start, 49,
        "the error must point at the damaged chunk, not at end of file"
    );
    assert_eq!(error.range.end(), bytes.len() as u64);
}

/// The counterpart to the corrupt-file test, and the guard that was missing:
/// nothing stopped the parser from flagging files that are perfectly fine.
/// It shipped a bug that reported 44 of these 162 files as truncated, because
/// every existing test only asserted "a tree came back" or "damage was found".
#[test]
fn valid_files_are_never_reported_as_damaged() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut checked = 0;

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('x') || !name.ends_with(".png") {
            continue;
        }
        checked += 1;

        let doc = parse_png(&fs::read(&path).unwrap());
        let errors: Vec<&str> = doc
            .tree
            .nodes()
            .iter()
            .filter(|n| n.kind == NodeKind::Error)
            .map(|n| n.label.as_str())
            .collect();
        assert!(errors.is_empty(), "{name} falsely reported: {errors:?}");
    }

    assert!(checked > 100, "only checked {checked} valid files");
}

/// Pixels must actually come out. An Error-free tree with `pixels: None`
/// would pass the test above while the image silently fails to decode.
#[test]
fn every_non_interlaced_valid_file_decodes_to_pixels() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut checked = 0;

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('x') || !name.ends_with(".png") {
            continue;
        }

        let doc = parse_png(&fs::read(&path).unwrap());
        let Some(ihdr) = doc.ihdr else { continue };
        if ihdr.interlace != 0 {
            continue; // seven-pass layout is out of scope for v1
        }
        checked += 1;

        let pixels = doc
            .pixels
            .unwrap_or_else(|| panic!("{name} produced no pixels"));
        let expected = ihdr.stride().unwrap() * ihdr.height as usize;
        assert_eq!(pixels.len(), expected, "{name} decoded to the wrong size");
    }

    assert!(checked > 80, "only checked {checked} non-interlaced files");
}

#[test]
fn idat_ranges_reassemble_the_zlib_stream() {
    // The animation maps a bit in the zlib stream back to a byte in the file
    // through these ranges, so they must be exactly the stream, in order.
    let bytes = fixture("basn2c08.png");
    let doc = parse_png(&bytes);
    assert!(!doc.idat.is_empty());

    let mut stream = Vec::new();
    for r in &doc.idat {
        stream.extend_from_slice(&bytes[r.start as usize..r.end() as usize]);
    }
    let out = hexscope_core::inflate::zlib_decompress(
        &stream,
        u64::MAX,
        &mut hexscope_core::inflate::NoTrace,
    )
    .expect("reassembled stream decompresses");
    assert_eq!(Some(out), doc.inflated);
}

#[test]
fn a_corrupt_idat_is_reported_where_decoding_breaks() {
    let mut bytes = fixture("basn2c08.png");
    let idat = parse_png(&bytes).idat[0];
    for b in &mut bytes[idat.start as usize + 10..idat.start as usize + 20] {
        *b ^= 0x5A;
    }

    let doc = parse_png(&bytes);
    let error = doc
        .tree
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Error && n.label.starts_with("IDAT decompression failed"))
        .expect("corrupt compressed data is reported");

    // Nested inside the IDAT chunk that holds the damage, never overlapping
    // a sibling: the hex view's byte-to-node index relies on that.
    let parent = doc.tree.get(error.parent.expect("has a parent"));
    assert_eq!(parent.label, "IDAT");
    assert!(error.range.start >= idat.start && error.range.end() <= idat.end());
    // It ends where the chunk's data ends: everything from the break on is
    // unreadable.
    assert_eq!(error.range.end(), idat.end());
    // And it starts no later than the corrupted bytes themselves.
    assert!(
        error.range.start <= idat.start + 20,
        "error starts at {}",
        error.range.start
    );
}

#[test]
fn a_bad_adler_checksum_points_at_the_trailer() {
    let mut bytes = fixture("basn2c08.png");
    let idat = parse_png(&bytes).idat[0];
    // The last byte of the only IDAT payload is the last Adler-32 byte.
    bytes[idat.end() as usize - 1] ^= 0xFF;

    let doc = parse_png(&bytes);
    let error = doc
        .tree
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Error && n.label.contains("ChecksumMismatch"))
        .expect("checksum mismatch is reported");
    assert_eq!(error.range.start, idat.end() - 4);
    assert_eq!(error.range.len, 4);
}

/// The sample photo, checked against values Apple's ImageIO reads from the
/// same file — an independent EXIF reader, not our own parser agreeing with
/// our own generator.
#[test]
fn the_sample_photo_reveals_what_an_independent_reader_sees() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photo.jpg");
    let doc = hexscope_core::parse(&fs::read(path).unwrap());
    let hexscope_core::Document::Jpeg(jpeg) = doc else {
        panic!("the sample must parse as a JPEG");
    };

    assert_eq!((jpeg.width, jpeg.height), (Some(640), Some(480)));
    let f = &jpeg.facts;
    assert_eq!(f.camera.as_ref().unwrap().text, "hexscope Sample Camera X1");
    assert_eq!(f.serial.as_ref().unwrap().text, "HX-000042");
    assert_eq!(f.taken.as_ref().unwrap().text, "2026-06-14 18:32:07 +02:00");
    assert_eq!(f.owner.as_ref().unwrap().text, "Sample Owner");
    let loc = f.location.expect("GPS");
    assert!((loc.latitude - 48.8584).abs() < 1e-4 && (loc.longitude - 2.2945).abs() < 1e-4);
    assert_eq!(loc.altitude, Some(35.0));
    assert!(f.thumbnail.is_some(), "the embedded thumbnail is found");

    // The thumbnail is parsed as a JPEG of its own, placed on its real bytes.
    let thumb = jpeg.tree.get(f.thumbnail.as_ref().unwrap().node);
    let inside: Vec<&str> = thumb
        .children
        .iter()
        .map(|&c| jpeg.tree.get(c).label.as_str())
        .collect();
    assert_eq!(inside.first(), Some(&"SOI"), "{inside:?}");
    assert_eq!(inside.last(), Some(&"EOI"), "{inside:?}");
    for &c in &thumb.children {
        let r = jpeg.tree.get(c).range;
        assert!(r.start >= thumb.range.start && r.end() <= thumb.range.end());
    }

    let errors: Vec<&str> = jpeg
        .tree
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Error)
        .map(|n| n.label.as_str())
        .collect();
    assert!(
        errors.is_empty(),
        "a valid photo reported damage: {errors:?}"
    );
}

#[test]
fn one_corrupt_chunk_length_costs_one_chunk_not_the_file() {
    let mut bytes = fixture("basn2c08.png");
    // gAMA starts at 33 (see the snapshot); make its length absurd.
    bytes[33] = 0x7F;

    let doc = parse_png(&bytes);
    let labels: Vec<&str> = doc
        .tree
        .get(0)
        .children
        .iter()
        .map(|&c| doc.tree.get(c).label.as_str())
        .collect();
    assert!(labels.contains(&"IDAT"), "IDAT survives: {labels:?}");
    assert!(labels.contains(&"IEND"), "IEND survives: {labels:?}");
    assert!(doc.pixels.is_some(), "the image still decodes");

    let error = doc
        .tree
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Error)
        .expect("the damage is reported");
    // Exactly the damaged gAMA chunk: 33 up to IDAT at 49.
    assert_eq!((error.range.start, error.range.end()), (33, 49));
}

/// Children of every IDAT chunk, as (label, start, end), in file order.
fn idat_children(doc: &PngDocument) -> Vec<Vec<(String, u64, u64)>> {
    let tree = &doc.tree;
    tree.get(0)
        .children
        .iter()
        .map(|&c| tree.get(c))
        .filter(|n| n.label == "IDAT")
        .map(|chunk| {
            let mut kids: Vec<(String, u64, u64)> = chunk
                .children
                .iter()
                .map(|&k| tree.get(k))
                .filter(|k| k.kind == NodeKind::Field)
                .map(|k| (k.label.clone(), k.range.start, k.range.end()))
                .collect();
            kids.sort_by_key(|k| k.1);
            kids
        })
        .collect()
}

#[test]
fn idat_children_name_the_stream_and_never_overlap() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('x') || !name.ends_with(".png") {
            continue;
        }
        let doc = parse_png(&fs::read(&path).unwrap());
        let chunks = idat_children(&doc);
        assert!(!chunks.is_empty(), "{name}: no IDAT");
        assert_eq!(chunks[0][0].0, "zlib header", "{name}");
        // Starts with: in oi9n2c16.png every IDAT is one byte long, so the
        // trailer is spread over four chunks.
        assert!(
            chunks
                .last()
                .unwrap()
                .last()
                .unwrap()
                .0
                .starts_with("Adler-32"),
            "{name}"
        );
        for kids in &chunks {
            for pair in kids.windows(2) {
                assert!(
                    pair[0].2 <= pair[1].1,
                    "{name}: {:?} overlaps {:?}",
                    pair[0],
                    pair[1]
                );
            }
        }
    }
}

#[test]
fn blocks_split_across_chunks_cover_the_stream_exactly() {
    use flate2::{Compression, write::ZlibEncoder};
    use hexscope_core::crc32::crc32;
    use std::io::Write;

    // Mixed content so the encoder emits several blocks, then 97-byte IDAT
    // chunks so nearly every block crosses a chunk boundary.
    let (w, h) = (200u32, 150u32);
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            let noise = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) >> 13;
            raw.extend_from_slice(&[
                (x ^ y) as u8,
                noise as u8,
                if x < 100 { 0 } else { y as u8 },
            ]);
        }
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&raw).unwrap();
    let z = enc.finish().unwrap();
    let chunk = |kind: &[u8; 4], data: &[u8]| {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
        out
    };
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend(chunk(b"IHDR", &ihdr));
    for part in z.chunks(97) {
        png.extend(chunk(b"IDAT", part));
    }
    png.extend(chunk(b"IEND", &[]));

    let doc = parse_png(&png);
    assert!(doc.pixels.is_some());
    let chunks = idat_children(&doc);
    let covered: u64 = chunks.iter().flatten().map(|(_, s, e)| e - s).sum();
    assert_eq!(
        covered,
        z.len() as u64,
        "every stream byte belongs to exactly one child"
    );
    assert!(
        chunks
            .iter()
            .flatten()
            .any(|(l, _, _)| l.ends_with("(continued)")),
        "blocks cross chunk boundaries here"
    );
    let blocks: std::collections::BTreeSet<&str> = chunks
        .iter()
        .flatten()
        .filter(|(l, _, _)| l.starts_with("block "))
        .map(|(l, _, _)| l.split(" · ").next().unwrap())
        .collect();
    assert!(blocks.len() > 1, "several blocks: {blocks:?}");
}
