use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn problems(tree: &ParseTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
        .map(|n| n.label.clone())
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn an_iphone_video_says_where_and_with_what() {
    let data = fixture("iphone.mov");
    assert!(is_video(&data));
    let doc = parse_video(&data);
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    assert_eq!(doc.kind, "QuickTime");
    assert_eq!((doc.width, doc.height), (Some(64), Some(48)));
    assert!(near(doc.duration.unwrap(), 1.0));
    let loc = doc.facts.location.unwrap();
    assert!(near(loc.latitude, 48.8584) && near(loc.longitude, 2.2945));
    assert_eq!(loc.altitude, Some(35.0));
    assert_eq!(
        doc.tree.get(loc.node).label,
        "com.apple.quicktime.location.ISO6709"
    );
    assert_eq!(doc.facts.camera.unwrap().text, "hexscope Sample Camera X1");
    assert_eq!(
        doc.facts.software.unwrap().text,
        "hexscope sample generator"
    );
    assert_eq!(doc.facts.taken.unwrap().text, "2026-06-14 18:32:07 +02:00");
    assert_eq!(date("2026-06-14T18:32:07Z"), "2026-06-14 18:32:07 UTC");
    assert_eq!(date("2026"), "2026");
    assert_eq!(date("2026-06-14T18:32:0é+0200"), "2026-06-14T18:32:0é+0200");
}

#[test]
fn an_android_video_and_a_3gpp_one_say_where() {
    for (name, box_label) in [("android.mp4", "©xyz"), ("loci.mp4", "loci")] {
        let doc = parse_video(&fixture(name));
        assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
        assert_eq!(doc.kind, "MP4", "{name}");
        let loc = doc.facts.location.unwrap();
        assert!(
            near(loc.latitude, 48.8584) && near(loc.longitude, 2.2945),
            "{name}: {loc:?}"
        );
        assert_eq!(doc.tree.get(loc.node).label, box_label, "{name}");
        // No other date: the movie header's creation time says when.
        assert_eq!(
            doc.facts.taken.unwrap().text,
            "2026-06-14 16:32:07 UTC",
            "{name}"
        );
    }
}

#[test]
fn places_are_read_the_way_iso_6709_writes_them() {
    assert_eq!(
        iso6709("+48.8584+002.2945+035.000/"),
        Some((48.8584, 2.2945, Some(35.0)))
    );
    assert_eq!(
        iso6709("-33.8568+151.2153/"),
        Some((-33.8568, 151.2153, None))
    );
    assert_eq!(iso6709("+48.8584-122.1/"), Some((48.8584, -122.1, None)));
    assert_eq!(iso6709("+91.0+000.0/"), None);
    assert_eq!(iso6709("nowhere"), None);
    assert_eq!(iso6709(""), None);
    assert_eq!(decimal("+048.8584"), Some(48.8584));
    assert_eq!(decimal("-7"), Some(-7.0));
    assert_eq!(decimal("1.2.3"), None);
    assert_eq!(decimal("+"), None);
    assert_eq!(decimal("1e5"), None);
}

#[test]
fn movie_time_counts_from_1904() {
    assert_eq!(movie_time(0), "1904-01-01 00:00:00 UTC");
    // 1970-01-01 is 2,082,844,800 seconds later.
    assert_eq!(movie_time(2_082_844_800), "1970-01-01 00:00:00 UTC");
    assert_eq!(
        movie_time(2_082_844_800 + 1_781_454_727),
        "2026-06-14 16:32:07 UTC"
    );
}

#[test]
fn every_cut_of_the_fixtures_is_survivable() {
    for name in ["iphone.mov", "android.mp4", "loci.mp4"] {
        let data = fixture(name);
        for cut in 0..data.len() {
            let doc = parse_video(&data[..cut]);
            for n in doc.tree.nodes() {
                assert!(
                    n.range.end() <= cut as u64,
                    "{name} cut at {cut}: {:?}",
                    n.label
                );
            }
            for s in &doc.scrub {
                let end = match *s {
                    Scrub::Free { at, len, .. } => at + len,
                    Scrub::Zero { range, .. } => range.end(),
                };
                assert!(end <= cut as u64, "{name} cut at {cut}");
            }
        }
    }
}

#[test]
fn heif_and_other_files_are_not_videos() {
    assert!(!is_video(b"\0\0\0\x08moo"));
    assert!(is_video(b"\0\0\0\x08wide"));
    assert!(!is_video(b"\0\0\x10\x00mdat"));
    assert!(!is_video(b"hello world"));
}

#[test]
fn a_clean_copy_keeps_its_size_and_loses_its_place() {
    for name in ["iphone.mov", "android.mp4", "loci.mp4"] {
        let data = fixture(name);
        let cleaned = crate::clean::clean(&data).unwrap();
        assert_eq!(cleaned.bytes.len(), data.len(), "{name}");
        assert_eq!(cleaned.removed[0].what, "the location", "{name}");
        let doc = parse_video(&cleaned.bytes);
        assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
        assert_eq!(doc.facts, PhotoFacts::default(), "{name}");
        assert!(doc.scrub.is_empty(), "{name}: {:?}", doc.scrub);
        // The picture's bytes are where they were.
        let mdat = |d: &VideoDocument| {
            let root = d.tree.root().unwrap();
            d.tree
                .get(root)
                .children
                .iter()
                .map(|&c| d.tree.get(c))
                .find(|n| n.label == "mdat")
                .unwrap()
                .range
        };
        let (a, b) = (mdat(&parse_video(&data)), mdat(&doc));
        assert_eq!(a, b, "{name}");
        assert_eq!(
            data[a.start as usize..a.end() as usize],
            cleaned.bytes[b.start as usize..b.end() as usize]
        );
    }
}

#[test]
fn sound_without_a_picture_is_audio() {
    let text = |name: &str| match parse_video(&fixture(name)).tree.get(0).value.clone() {
        Some(Value::Text(t)) => t,
        other => panic!("{other:?}"),
    };
    // A voice memo made by ffmpeg: one sound track, no picture.
    assert!(
        text("voice.m4a").starts_with("MP4 audio"),
        "{}",
        text("voice.m4a")
    );
    let doc = parse_video(&fixture("voice.m4a"));
    assert_eq!(doc.facts.owner.map(|f| f.text).as_deref(), Some("Olena"));
    assert!(
        text("iphone.mov").starts_with("QuickTime ·"),
        "{}",
        text("iphone.mov")
    );
    assert!(
        text("android.mp4").starts_with("MP4 ·"),
        "{}",
        text("android.mp4")
    );
}

/// The top-level boxes: where each starts, its header's length, its size.
fn top_boxes(data: &[u8]) -> Vec<(usize, usize, usize, [u8; 4])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= data.len() {
        let size32 = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let (size, header) = match size32 {
            1 => (
                u64::from_be_bytes(data[at + 8..at + 16].try_into().unwrap()) as usize,
                16,
            ),
            0 => (data.len() - at, 8),
            n => (n, 8),
        };
        out.push((at, header, size, data[at + 4..at + 8].try_into().unwrap()));
        at += size;
    }
    out
}

/// The file without its `mdat` bodies, and where they were left out.
fn without_media(data: &[u8]) -> (Vec<u8>, Vec<Gap>, Vec<std::ops::Range<usize>>) {
    let (mut given, mut gaps, mut bodies) = (Vec::new(), Vec::new(), Vec::new());
    let mut copied = 0;
    for (at, header, size, typ) in top_boxes(data) {
        if &typ != b"mdat" || size == header {
            continue;
        }
        given.extend_from_slice(&data[copied..at + header]);
        gaps.push(Gap {
            at: (given.len() - header) as u64,
            len: (size - header) as u64,
        });
        bodies.push(at + header..at + size);
        copied = at + size;
    }
    given.extend_from_slice(&data[copied..]);
    (given, gaps, bodies)
}

/// The tree in reading order, each node with its depth.
fn outline(tree: &ParseTree) -> Vec<(usize, String, ByteRange, NodeKind, Option<Value>)> {
    let mut out = Vec::new();
    let mut stack = vec![(0u32, 0usize)];
    while let Some((id, depth)) = stack.pop() {
        let n = tree.get(id);
        out.push((depth, n.label.clone(), n.range, n.kind, n.value.clone()));
        stack.extend(n.children.iter().rev().map(|&c| (c, depth + 1)));
    }
    out
}

/// The same movie with its `mdat` as a 64-bit box, and moved to the end
/// with a size of 0, "to the end of the file".
fn variants(data: &[u8]) -> Vec<Vec<u8>> {
    let boxes = top_boxes(data);
    let mdat = boxes.iter().find(|b| &b.3 == b"mdat").unwrap();
    let body = &data[mdat.0 + mdat.1..mdat.0 + mdat.2];
    let others: Vec<&[u8]> = boxes
        .iter()
        .filter(|b| &b.3 != b"mdat")
        .map(|b| &data[b.0..b.0 + b.2])
        .collect();
    let mut wide = Vec::new();
    for b in &boxes {
        if &b.3 == b"mdat" {
            wide.extend_from_slice(&[0, 0, 0, 1]);
            wide.extend_from_slice(b"mdat");
            wide.extend_from_slice(&(body.len() as u64 + 16).to_be_bytes());
            wide.extend_from_slice(body);
        } else {
            wide.extend_from_slice(&data[b.0..b.0 + b.2]);
        }
    }
    let mut last = others.concat();
    last.extend_from_slice(&[0, 0, 0, 0]);
    last.extend_from_slice(b"mdat");
    last.extend_from_slice(body);
    vec![wide, last]
}

#[test]
fn a_movie_read_without_its_media_is_the_movie_read_whole() {
    let mut movies: Vec<(String, Vec<u8>)> = Vec::new();
    for name in ["iphone.mov", "android.mp4", "loci.mp4", "voice.m4a"] {
        let data = fixture(name);
        for (i, v) in variants(&data).into_iter().enumerate() {
            movies.push((format!("{name}, variant {i}"), v));
        }
        movies.push((name.into(), data));
    }
    for (name, data) in movies {
        let whole = parse_video(&data);
        let (given, gaps, bodies) = without_media(&data);
        assert!(!gaps.is_empty(), "{name}");
        let gapped = parse_video_gapped(&given, &gaps);
        assert_eq!(outline(&gapped.tree), outline(&whole.tree), "{name}");
        assert_eq!(gapped.scrub, whole.scrub, "{name}");
        assert_eq!(
            gapped.facts.location.map(|l| l.node),
            whole.facts.location.map(|l| l.node),
            "{name}"
        );
        assert_eq!(gapped.duration, whole.duration, "{name}");

        // The clean copy, its bodies put back, is the whole file's.
        let whole_copy = crate::clean::clean(&data).map(|c| c.bytes);
        let copy = crate::clean::clean_video_gapped(&given, &gaps).map(|c| {
            let mut out = Vec::new();
            let mut from = 0;
            for (g, body) in gaps.iter().zip(&bodies) {
                let header = if c.bytes[g.at as usize..g.at as usize + 4] == [0, 0, 0, 1] {
                    16
                } else {
                    8
                };
                let end = g.at as usize + header;
                out.extend_from_slice(&c.bytes[from..end]);
                out.extend_from_slice(&data[body.clone()]);
                from = end;
            }
            out.extend_from_slice(&c.bytes[from..]);
            out
        });
        assert_eq!(copy, whole_copy, "{name}");
    }
}

#[test]
fn gaps_that_are_not_the_boxes_given_read_the_bytes_as_they_are() {
    let data = fixture("iphone.mov");
    let (given, gaps, _) = without_media(&data);
    let off = Gap {
        at: gaps[0].at,
        len: gaps[0].len + 1,
    };
    let doc = parse_video_gapped(&given, &[off]);
    assert_eq!(outline(&doc.tree), outline(&parse_video(&given).tree));
    let past_the_end = Gap {
        at: given.len() as u64,
        len: 5,
    };
    assert!(!parse_video_gapped(&given, &[past_the_end]).tree.is_empty());
    assert_eq!(
        crate::clean::clean_video_gapped(&given, &[off]).map(|c| c.bytes.len()),
        Err(crate::clean::CleanError::Damaged)
    );
}

#[test]
fn a_movie_larger_than_memory_is_read_by_its_headers() {
    let data = fixture("android.mp4");
    let (given, _, _) = without_media(&data);
    // Its media grown to 16 GB: a 64-bit box, given by its header alone.
    let boxes = top_boxes(&given);
    let mdat = boxes.iter().find(|b| &b.3 == b"mdat").unwrap();
    let len: u64 = 16 << 30;
    let mut huge = given[..mdat.0].to_vec();
    huge.extend_from_slice(&[0, 0, 0, 1]);
    huge.extend_from_slice(b"mdat");
    huge.extend_from_slice(&(len + 16).to_be_bytes());
    let gap = Gap {
        at: mdat.0 as u64,
        len,
    };
    huge.extend_from_slice(&given[mdat.0 + mdat.1..]);
    let doc = parse_video_gapped(&huge, &[gap]);
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    let whole = doc.tree.get(0).range;
    assert_eq!(whole.len, huge.len() as u64 + len);
    assert!(doc.facts.location.is_some());
    let copy = crate::clean::clean_video_gapped(&huge, &[gap]).unwrap();
    assert_eq!(copy.bytes.len(), huge.len());
}
