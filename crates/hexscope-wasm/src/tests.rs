use super::*;
use crate::flat::display;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../hexscope-core/tests/fixtures/pngsuite")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn every_array_describes_every_node() {
    let parsed = parse(&fixture("basn2c08.png"));
    let n = parsed.starts.len();

    assert!(n > 5);
    assert_eq!(parsed.lens.len(), n);
    assert_eq!(parsed.parents.len(), n);
    assert_eq!(parsed.kinds.len(), n);
    assert_eq!(parsed.labels.split(SEPARATOR).count(), n);
    assert_eq!(parsed.values.split(SEPARATOR).count(), n);
    assert_eq!(parsed.parents[0], -1, "node 0 is the root");
}

#[test]
fn carries_file_level_facts() {
    let parsed = parse(&fixture("basn2c08.png"));
    assert_eq!(parsed.ihdr(), vec![32, 32, 8, 2, 0]);

    let trace = parsed.trace();
    assert_eq!(trace.len(), 4);
    assert!(trace[0] > 0.0, "some DEFLATE events were recorded");
    assert!(parsed.idat_bytes > 0.0);
}

#[test]
fn a_non_png_still_flattens_to_a_tree() {
    let parsed = parse(b"definitely not a png");
    assert!(parsed.starts.len() >= 2);
    // Text is what the file is, not something wrong with it; an empty
    // file is.
    assert_eq!(parsed.kinds[1], 1, "the text is a field");
    assert_eq!(parse(b"").kinds[1], 3, "an empty file is an error");
    assert!(parsed.ihdr().is_empty());
    assert!(parsed.trace().is_empty());
}

/// Every step of a file, fetched the slow way: one call from the start.
fn every_step(parsed: &Parsed) -> Vec<f64> {
    parsed.steps(0.0, u32::MAX / 8)
}

#[test]
fn steps_cover_the_whole_stream_and_rebuild_the_output() {
    let parsed = parse(&fixture("basn2c08.png"));
    let steps = every_step(&parsed);
    let n = steps.len() / STEP_STRIDE;
    assert_eq!(n as f64, parsed.trace()[0], "one entry per traced event");

    // Replaying literals and back-references must rebuild the inflated
    // stream byte for byte: that is what the player draws.
    assert_eq!(rebuild(&steps), parsed.inflated());
}

#[test]
fn a_window_anywhere_matches_the_full_sequence() {
    // Build a stream long enough to have many checkpoints, then check that
    // windows fetched by resuming agree with one straight pass.
    let parsed = parse(&fixture("basn2c08.png"));
    let all = every_step(&parsed);
    let n = all.len() / STEP_STRIDE;
    for from in [0, 1, n / 3, n / 2, n - 1] {
        let window = parsed.steps(from as f64, 5);
        let expected = &all[from * STEP_STRIDE..((from + 5).min(n)) * STEP_STRIDE];
        assert_eq!(window, expected, "window at step {from}");
    }
}

/// A zlib stream long enough to span many checkpoints.
fn big_zlib() -> Vec<u8> {
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    let (w, h) = (300u32, 200u32);
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.extend_from_slice(&[(x ^ y) as u8, (x * 3) as u8, (y * 7) as u8]);
        }
    }
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(&raw).unwrap();
    enc.finish().unwrap()
}

/// A 300x200 RGB PNG around `zlib`, split across 4 KB IDAT chunks.
fn png_around(zlib: &[u8]) -> Vec<u8> {
    use hexscope_core::crc32::crc32;

    let chunk = |kind: &[u8; 4], data: &[u8]| {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
        out
    };
    let mut ihdr = 300u32.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&200u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend(chunk(b"IHDR", &ihdr));
    for part in zlib.chunks(4096) {
        png.extend(chunk(b"IDAT", part));
    }
    png.extend(chunk(b"IEND", &[]));
    png
}

fn big_png() -> Vec<u8> {
    png_around(&big_zlib())
}

/// Replays literal and back-reference steps into the bytes they produce.
fn rebuild(steps: &[f64]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for s in steps.chunks(STEP_STRIDE) {
        match s[0] as u8 {
            1 => out.push(s[1] as u8),
            2 => {
                let start = out.len() - s[1] as usize;
                for i in 0..s[2] as usize {
                    let b = out[start + i];
                    out.push(b);
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn windows_resumed_from_checkpoints_match_one_straight_pass() {
    let parsed = parse(&big_png());
    assert!(
        parsed.checkpoints.len() > 10,
        "need several checkpoints to test resume"
    );
    assert!(parsed.segments().len() / 2 > 3, "need several IDAT chunks");

    let all = every_step(&parsed);
    let n = all.len() / STEP_STRIDE;
    for from in (0..n).step_by(n / 40).chain([511, 512, 513, n - 1]) {
        let window = parsed.steps(from as f64, 7);
        let expected = &all[from * STEP_STRIDE..((from + 7).min(n)) * STEP_STRIDE];
        assert_eq!(window, expected, "window at step {from}");
    }
}

#[test]
fn segments_map_stream_bytes_back_into_the_file() {
    let bytes = fixture("basn2c08.png");
    let parsed = parse(&bytes);
    let seg = parsed.segments();
    assert_eq!(seg.len() % 2, 0);
    let (start, len) = (seg[0] as usize, seg[1] as usize);
    assert_eq!(&bytes[start..start + len], &parsed.stream[..len]);
    // zlib header bytes: CMF 0x78 for a 32 KB window.
    assert_eq!(parsed.stream[0], 0x78);
}

#[test]
fn a_truncated_stream_plays_up_to_where_it_breaks() {
    // Cut the compressed data in half: decoding produces real output, then
    // runs out of bits partway through a block. A fake trailer keeps the
    // stream long enough to have a body at all.
    let z = big_zlib();
    let mut cut = z[..z.len() / 2].to_vec();
    cut.extend_from_slice(&[0, 0, 0, 0]);
    let parsed = parse(&png_around(&cut));

    let steps = every_step(&parsed);
    let last = &steps[steps.len() - STEP_STRIDE..];
    assert_eq!(last[0] as u8, 4, "the final step reports the failure");
    assert!(last[4] >= last[3]);

    let output = parsed.inflated();
    assert!(
        output.len() > 10_000,
        "real output before the break, got {}",
        output.len()
    );
    // The partial output is exactly what the steps before the break
    // produce: the player draws one from the other.
    assert_eq!(rebuild(&steps), output);

    // Seeking into the middle still resumes from checkpoints recorded
    // before the failure, and agrees with the straight pass.
    let n = steps.len() / STEP_STRIDE;
    let from = n / 2;
    assert_eq!(
        parsed.steps(from as f64, 5),
        &steps[from * STEP_STRIDE..(from + 5) * STEP_STRIDE]
    );
}

#[test]
fn a_corrupt_stream_still_rebuilds_its_output() {
    // Corruption early in the stream: whatever happens, the steps and the
    // output the player is handed must agree.
    let mut bytes = fixture("basn2c08.png");
    let start = parse(&bytes).segments()[0] as usize;
    for b in &mut bytes[start + 10..start + 20] {
        *b ^= 0x5A;
    }
    let parsed = parse(&bytes);
    let steps = every_step(&parsed);
    assert!(!steps.is_empty());
    assert_eq!(rebuild(&steps), parsed.inflated());
}

#[test]
fn a_huge_count_is_not_trusted_for_allocation() {
    // Found on CI: preallocating count * STRIDE for u32::MAX steps asked
    // for 25 GB and aborted on Linux, where memory is not lazily
    // committed. The request must cost only what it actually returns.
    let parsed = parse(&fixture("basn2c08.png"));
    let steps = parsed.steps(0.0, u32::MAX);
    assert_eq!(steps.len() / STEP_STRIDE, parsed.trace()[0] as usize);
    assert!(steps.capacity() < 1 << 20, "capacity {}", steps.capacity());
}

#[test]
fn a_photo_reports_its_format_facts_and_location() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../hexscope-core/tests/fixtures/photo.jpg");
    let parsed = parse(&std::fs::read(path).unwrap());
    assert_eq!(parsed.format(), "jpeg");
    assert_eq!(parsed.dimensions(), vec![640, 480]);
    assert!(
        parsed.trace().is_empty(),
        "JPEG has no DEFLATE stream to play"
    );

    let facts: Vec<String> = parsed
        .facts()
        .split(SEPARATOR)
        .map(str::to_string)
        .collect();
    assert_eq!(facts.len() % 3, 0);
    let find = |kind: &str| {
        facts
            .chunks(3)
            .find(|t| t[0] == kind)
            .map(|t| t[1].clone())
            .unwrap_or_else(|| panic!("no {kind}"))
    };
    assert_eq!(find("serial"), "HX-000042");
    assert_eq!(find("camera"), "hexscope Sample Camera X1");

    let loc = parsed.location();
    assert!((loc[0] - 48.8584).abs() < 1e-4 && (loc[1] - 2.2945).abs() < 1e-4);
    // The node index points at the GPS IFD in the flattened tree.
    let labels: Vec<&str> = parsed.labels.split(SEPARATOR).collect();
    assert_eq!(labels[loc[3] as usize], "GPS IFD");
}

#[test]
fn an_unknown_file_is_named_not_ignored() {
    let parsed = parse(b"\x1F\x8B\x08 not decoded");
    assert_eq!(parsed.format(), "unknown");
    assert!(parsed.labels.contains("gzip"));
    assert!(parsed.facts().is_empty() && parsed.location().is_empty());
}

#[test]
fn a_pixel_leads_to_the_step_that_wrote_it_and_back() {
    let parsed = parse(&fixture("basn2c08.png"));
    assert_eq!(parsed.preview_size(), vec![32, 32]);
    assert_eq!(parsed.preview_pixels().len(), 32 * 32 * 4);
    // Every output byte has a step, and that step's bits lead back to it.
    for pos in [0.0, 1.0, 97.0, 500.0, 3000.0, 3103.0] {
        let s = parsed.locate(0, pos);
        assert_eq!(s.len(), 7, "{pos}");
        let (kind, len, bit_start, out_start) = (s[1], s[3], s[4], s[6]);
        let produced = if kind == 1.0 { 1.0 } else { len };
        assert!(
            out_start <= pos && pos < out_start + produced,
            "{pos}: {s:?}"
        );
        assert_eq!(parsed.locate(1, bit_start), s, "{pos}");
    }
    assert!(parsed.locate(0, 1e9).is_empty());
    assert!(parsed.locate(1, f64::NAN).is_empty());
}

#[test]
fn a_photo_says_where_its_blocks_are() {
    let photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    let mut parsed = parse(&photo);
    let map = parsed.block_map();
    // Sequential, three components, one scan of all three: 40 × 30 MCUs of 16 × 16.
    assert_eq!(&map[..3], &[0.0, 3.0, 1.0]);
    assert_eq!(
        &map[3..13],
        &[7.0, 0.0, 63.0, 0.0, 0.0, 16.0, 16.0, 40.0, 30.0, 6.0]
    );
    assert_eq!(map[14], (40 * 30 + 1) as f64);
    assert_eq!(map.len(), 15 + 40 * 30 + 1);
    assert_eq!(parsed.blocks_note(), "");
    assert_eq!(parsed.block_map(), map, "found once, then kept");
    assert!(parse(b"\x89PNG\r\n\x1a\n").block_map().is_empty());
}

#[test]
fn an_interlaced_png_has_a_picture_and_every_pass_row_filter() {
    let png = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../hexscope-core/tests/fixtures/pngsuite/basi0g08.png"),
    )
    .unwrap();
    let parsed = parse(&png);
    assert_eq!(parsed.preview_size(), vec![32, 32]);
    // 32 × 32 in seven passes: 4, 4, 4, 8, 8, 16 and 16 rows.
    let filters = parsed.row_filters();
    assert_eq!(filters.len(), 60);
    assert!(filters.iter().all(|&f| f <= 4));
}

#[test]
fn a_webassembly_module_says_who_built_it() {
    let module = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/hello.wasm"),
    )
    .unwrap();
    let parsed = parse(&module);
    assert_eq!(parsed.format(), "wasm");
    let facts = parsed.facts();
    for kind in ["names", "language", "toolchain", "paths"] {
        assert!(facts.contains(kind), "{kind} in {facts}");
    }
    assert!(facts.contains("/Users/sample/projects/hello"));
}

#[test]
fn a_photo_says_which_way_up_it_is() {
    let mut photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    assert_eq!(parse(&photo).orientation(), 1);
    // Its Orientation entry, turned to "rotate 90° clockwise".
    let entry: &[u8] = &[0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1, 0, 1];
    let at = photo.windows(entry.len()).position(|w| w == entry).unwrap();
    photo[at + 9] = 6;
    assert_eq!(parse(&photo).orientation(), 6);
    assert_eq!(parse(b"\x89PNG\r\n\x1a\n").orientation(), 1);
}

#[test]
fn non_jpeg_orientation_is_propagated_and_validated() {
    use hexscope_core::crc32::crc32;
    for (orientation, expected) in [(6u8, 6u16), (99, 1)] {
        let tiff = vec![
            b'M',
            b'M',
            0,
            42,
            0,
            0,
            0,
            8,
            0,
            1,
            1,
            18,
            0,
            3,
            0,
            0,
            0,
            1,
            0,
            orientation,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let original = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/sample.png"),
        )
        .unwrap();
        let mut png = original[..33].to_vec();
        png.extend_from_slice(&(tiff.len() as u32).to_be_bytes());
        let mut chunk = b"eXIf".to_vec();
        chunk.extend_from_slice(&tiff);
        png.extend_from_slice(&chunk);
        png.extend_from_slice(&crc32(&chunk).to_be_bytes());
        png.extend_from_slice(&original[33..]);
        assert_eq!(
            parse(&png).orientation(),
            expected,
            "PNG orientation {orientation}"
        );

        let mut body = b"WEBPVP8X".to_vec();
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend_from_slice(&[8, 0, 0, 0, 99, 0, 0, 49, 0, 0]);
        body.extend_from_slice(b"EXIF");
        body.extend_from_slice(&(tiff.len() as u32).to_le_bytes());
        body.extend_from_slice(&tiff);
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&(body.len() as u32).to_le_bytes());
        webp.extend_from_slice(&body);
        assert_eq!(
            parse(&webp).orientation(),
            expected,
            "WebP orientation {orientation}"
        );
    }
}

#[test]
fn a_video_says_where_it_was_recorded() {
    let mov = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../hexscope-core/tests/fixtures/iphone.mov"),
    )
    .unwrap();
    let parsed = parse(&mov);
    assert_eq!(parsed.format(), "video");
    assert_eq!(parsed.dimensions(), vec![64, 48]);
    assert_eq!(parsed.location().len(), 4);
    let c = clean_copy(&mov, false);
    assert_eq!(c.error(), "");
    assert!(parse(&c.bytes()).location().is_empty());
}

#[test]
fn a_pdf_says_who_wrote_it() {
    let pdf = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../hexscope-core/tests/fixtures/report.pdf"),
    )
    .unwrap();
    let parsed = parse(&pdf);
    assert_eq!(parsed.format(), "pdf");
    assert!(parsed.facts().contains("Olena Koval"));
}

#[test]
fn a_png_still_carries_its_deflate_stream() {
    let parsed = parse(&fixture("basn2c08.png"));
    assert_eq!(parsed.format(), "png");
    assert_eq!(parsed.dimensions(), vec![32, 32]);
    assert!(!parsed.trace().is_empty() && !parsed.segments().is_empty());
}

#[test]
fn control_characters_cannot_break_the_join() {
    assert_eq!(sanitise("IH\u{1F}DR\n"), "IH\u{FFFD}DR\u{FFFD}");
    assert!(!sanitise("a\u{1F}b").contains(SEPARATOR));
}

#[test]
fn values_render_for_humans() {
    assert_eq!(display(&Some(Value::Bytes(1))), "1 byte");
    assert_eq!(display(&Some(Value::Bytes(13))), "13 bytes");
    assert_eq!(
        display(&Some(Value::Enum {
            raw: 6,
            name: "RGBA"
        })),
        "6 (RGBA)"
    );
    assert_eq!(display(&None), "");
}

#[test]
fn explain_agrees_with_steps() {
    let parsed = parse(&fixture("basn2c08.png"));
    let steps = every_step(&parsed);
    for (i, s) in steps.chunks(STEP_STRIDE).enumerate() {
        let e = parsed.explain(i as f64);
        assert_eq!(e[1], -1.0, "step {i} decodes");
        let n = e[2] as usize;
        assert_eq!(e.len(), 3 + n * PART_STRIDE);
        if n > 0 {
            assert_eq!(e[3 + 1], s[3], "step {i}: first part starts with the step");
            assert_eq!(
                e[3 + (n - 1) * PART_STRIDE + 2],
                s[4],
                "step {i}: last part ends with it"
            );
        }
    }
}

#[test]
fn tables_follow_the_block() {
    let parsed = parse(&fixture("basn2c08.png"));
    let t = parsed.tables(0.0);
    assert!(t[0] == 1.0 || t[0] == 2.0, "a coded block: {}", t[0]);
    assert!(t[4] > 0.0, "the literal/length table has groups");
}

#[test]
fn explaining_nothing_is_empty() {
    let parsed = parse(&fixture("basn2c08.png"));
    for index in [-1.0, f64::NAN, 1e12] {
        assert!(parsed.explain(index).is_empty(), "{index}");
        assert!(parsed.tables(index).is_empty(), "{index}");
    }
}

fn docx(path: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn every_deflated_entry_plays_back_to_its_bytes() {
    let bytes = docx("../hexscope-core/tests/fixtures/report.docx");
    let mut parsed = parse(&bytes);
    assert_eq!(parsed.format(), "zip");
    let doc = hexscope_core::zip::parse_zip(&bytes);
    let entries = parsed.entries();
    assert_eq!(entries.len(), doc.entries.len() * ENTRY_STRIDE);

    for (i, e) in doc.entries.iter().enumerate() {
        assert_eq!(entries[i * ENTRY_STRIDE + 5], 1.0, "{} is playable", e.name);
        assert!(parsed.select_entry(i as u32), "{}", e.name);
        assert_eq!(parsed.stream_header(), 0.0);
        assert_eq!(parsed.segments(), [e.data.start as f64, e.data.len as f64]);
        let want = hexscope_core::zip::extract(&bytes, e, u64::MAX).unwrap();
        assert_eq!(rebuild(&every_step(&parsed)), want, "{}", e.name);
        assert_eq!(parsed.inflated(), want);
    }
}

#[test]
fn zip_entry_rows_include_the_central_record_range() {
    let bytes = docx("../hexscope-core/tests/fixtures/report.docx");
    let parsed = parse(&bytes);
    let doc = hexscope_core::zip::parse_zip(&bytes);
    let entries = parsed.entries();

    assert_eq!(entries.len(), doc.entries.len() * ENTRY_STRIDE);
    for (i, entry) in doc.entries.iter().enumerate() {
        let row = &entries[i * ENTRY_STRIDE..(i + 1) * ENTRY_STRIDE];
        if let Some(range) = entry.central_record {
            assert_eq!(
                row[7], range.start as f64,
                "{} starts at its record",
                entry.name
            );
            assert_eq!(row[8], range.len as f64, "{} spans its record", entry.name);
            assert_eq!(
                &bytes[range.start as usize..range.start as usize + 4],
                b"PK\x01\x02"
            );
        } else {
            assert_eq!(
                (row[7], row[8]),
                (-1.0, 0.0),
                "{} has no central record",
                entry.name
            );
        }
    }
}

#[test]
fn what_cannot_be_played_is_refused() {
    let bytes = docx("../../apps/web/public/samples/report.docx");
    let mut parsed = parse(&bytes);
    let entries = parsed.entries();
    let n = entries.len() / ENTRY_STRIDE;
    let stored = (0..n)
        .find(|&i| entries[i * ENTRY_STRIDE + 1] == 0.0)
        .expect("the sample stores its photo");
    assert_eq!(entries[stored * ENTRY_STRIDE + 5], 0.0);
    assert!(!parsed.select_entry(stored as u32));
    assert!(!parsed.select_entry(n as u32));
    assert!(parsed.trace().is_empty(), "a refused entry changes nothing");

    let png = parse(&fixture("basn2c08.png"));
    assert_eq!(png.stream_header(), 2.0);
    assert!(png.entries().is_empty());
}

#[test]
fn an_entry_opens_as_a_file_of_its_own() {
    let bytes = docx("../../apps/web/public/samples/report.docx");
    let mut parsed = parse(&bytes);
    let entries = parsed.entries();
    let n = entries.len() / ENTRY_STRIDE;
    let photo = (0..n)
        .find(|&i| entries[i * ENTRY_STRIDE + 1] == 0.0)
        .expect("the stored photo");
    assert_eq!(
        entries[photo * ENTRY_STRIDE + 6],
        1.0,
        "stored entries open"
    );

    let jpeg = parsed.extract_entry(photo as u32);
    assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
    assert_eq!(parsed.extract_error(), "");
    let inner = parse(&jpeg);
    assert_eq!(inner.format(), "jpeg");
    assert!(
        !inner.location().is_empty(),
        "the photo inside knows where it was taken"
    );

    let facts = parsed.facts();
    assert!(facts.contains("author\u{1F}Olena Koval"), "{facts}");

    assert!(parsed.extract_entry(n as u32).is_empty());
    assert_eq!(parsed.extract_error(), format!("there is no entry {n}"));
}

#[test]
fn every_node_carries_its_explanation() {
    let parsed = parse(&fixture("basn2c08.png"));
    let ids = parsed.doc_ids();
    assert_eq!(ids.len(), parsed.labels().split(SEPARATOR).count());
    let keys = parsed.doc_keys();
    let labels: Vec<String> = parsed
        .labels()
        .split(SEPARATOR)
        .map(str::to_string)
        .collect();
    let ihdr = labels.iter().position(|l| l == "IHDR").unwrap();
    // The page reads the sentence by its key, from what `every` lists.
    let sentence = |key: u32| {
        hexscope_core::docs::every()
            .into_iter()
            .find(|(_, d)| d.key == key)
            .map(|(_, d)| d.text)
    };
    let header = sentence(keys[ids[ihdr] as usize]).unwrap();
    assert!(header.starts_with("The image header"), "{header}");
    assert!(ids.iter().all(|&i| i >= 0), "every PNG node is explained");
    // A palette of 256 colours is 256 nodes and one explanation.
    let palette = parse(&fixture("basn3p08.png"));
    let shared = palette.doc_keys().len();
    assert!(
        shared * 4 < palette.doc_ids().len(),
        "explanations are shared, not repeated"
    );

    // A damaged CRC: a problem that is damage.
    let mut bytes = fixture("basn2c08.png");
    bytes[29] ^= 0xFF;
    let parsed = parse(&bytes);
    let labels: Vec<String> = parsed
        .labels()
        .split(SEPARATOR)
        .map(str::to_string)
        .collect();
    let crc = labels
        .iter()
        .position(|l| l.starts_with("CRC mismatch"))
        .unwrap();
    assert_eq!(parsed.doc_concerns()[parsed.doc_ids()[crc] as usize], 1);
    assert_eq!(parsed.doc_keys().len(), parsed.doc_concerns().len());
}

#[test]
fn composition_covers_the_file_and_entropy_is_bounded() {
    let bytes = fixture("basn2c08.png");
    let parsed = parse(&bytes);
    let c = parsed.composition();
    assert_eq!(c.len() % SLICE_STRIDE, 0);
    let covered: f64 = c.chunks(SLICE_STRIDE).map(|s| s[1]).sum();
    assert_eq!(covered, bytes.len() as f64);
    assert!(
        c.chunks(SLICE_STRIDE).any(|s| s[2] == 0.0),
        "a PNG has picture bytes"
    );

    let h = entropy(&bytes, 1024);
    assert_eq!(h.len(), (bytes.len() / 256).max(1));
    assert!(h.iter().all(|&x| (0.0..=8.0).contains(&x)));
}

#[test]
fn a_clean_copy_of_the_sample_photo_reveals_nothing() {
    let photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    let c = clean_copy(&photo, false);
    assert_eq!(c.error(), "");
    assert!(c.removed().starts_with("EXIF"));
    let again = parse(&c.bytes());
    assert!(again.facts().is_empty());
    assert!(again.location().is_empty());

    let refused = clean_copy(b"\x1F\x8B\x08 not cleaned", false);
    assert!(refused.bytes().is_empty());
    assert!(refused.error().contains("WebAssembly modules only"));
    let bare = clean_copy(&fixture("basn2c08.png"), false);
    assert!(bare.error().contains("nothing in it"));
}

#[test]
fn verifies_the_clean_copy_using_the_source_parse() {
    let photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    let source = parse(&photo);
    let copy = clean_copy(&photo, false);
    let bytes = copy.bytes();

    let report = source.verify_copy(&bytes);

    assert!(report.contains("\"schema_version\":1"), "{report}");
    assert!(report.contains("\"kind\":\"location\""), "{report}");
    assert!(report.contains("\"removed\":["), "{report}");
    assert!(report.contains("\"present\":[]"), "{report}");
    // Every finding of the photo, the lens among them, is confirmed removed.
    assert!(
        report.contains("\"kind\":\"lens\",\"reason\":\"removed\""),
        "{report}"
    );
    assert!(report.contains("\"unchecked\":[]"), "{report}");
}

#[test]
fn reports_an_unchanged_copy_as_still_present() {
    let photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    let source = parse(&photo);

    let report = source.verify_copy(&photo);

    assert!(report.contains("\"reason\":\"still_present\""), "{report}");
    assert!(!report.contains("\"removed\":[{"), "{report}");
}

#[test]
fn skips_a_copy_over_ten_mib_without_reparsing_it() {
    let photo = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
    )
    .unwrap();
    let source = parse(&photo);
    let bytes = vec![0; 10 * 1024 * 1024 + 1];

    let report = source.verify_copy(&bytes);

    assert!(
        report.contains("\"reason\":\"verification_skipped\""),
        "{report}"
    );
    assert!(report.contains("\"unchecked\":[{"), "{report}");
}

#[test]
fn a_blacked_out_pdf_says_where_its_boxes_and_their_text_are() {
    let p = parse(&docx("../hexscope-core/tests/fixtures/redacted.pdf"));
    let b = p.blackouts();
    // One page, A4, three boxes over three pieces of text, three labels.
    assert_eq!(b[..8], [1.0, 0.0, 0.0, 595.0, 842.0, 3.0, 3.0, 3.0]);
    // Then page 2: the mark for redaction, the name under it, and the
    // words either side of it on the line.
    assert_eq!(b[44..52], [2.0, 0.0, 0.0, 595.0, 842.0, 1.0, 1.0, 2.0]);
    assert_eq!(b.len(), 44 + 8 + 4 + 4 + 2 * 4);
    let texts: Vec<String> = p
        .blackout_texts()
        .split(SEPARATOR)
        .map(String::from)
        .collect();
    assert_eq!(
        texts,
        [
            "Olena Koval",
            "+380 67 123 4567",
            "EUR 48,000",
            "Claimant:",
            "Phone:",
            "Settlement:",
            "Petro Ivanenko",
            "Signed in Kyiv on 3 March 2026 by",
            "."
        ]
    );
    // Each text lies inside the box drawn over it.
    for i in 0..3 {
        let bx = &b[8 + i * 4..8 + i * 4 + 4];
        let tx = &b[20 + i * 4..20 + i * 4 + 4];
        assert!(bx[0] <= tx[0] && tx[2] <= bx[2] + 1.0, "{bx:?} {tx:?}");
    }
    assert!(
        parse(&docx("../hexscope-core/tests/fixtures/report.pdf"))
            .blackouts()
            .is_empty()
    );
}
