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
