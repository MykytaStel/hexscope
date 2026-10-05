use hexscope_core::{
    clean::clean,
    summary::summarize,
    verification::{compare, snapshot},
};

#[test]
fn png_and_webp_verify_real_metadata_removal() {
    for name in ["photo.png", "cwebp-photo.webp"] {
        let original = std::fs::read(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let before = summarize(&original);
        assert!(before.complete, "{name}: {:?}", before.problems);
        let cleaned = clean(&original).unwrap();
        let after = summarize(&cleaned.bytes);
        assert!(after.complete, "{name}: {:?}", after.problems);
        let report = compare(&snapshot(&before), &snapshot(&after));
        for kind in ["camera", "serial", "owner", "location"] {
            assert!(
                before.facts.iter().any(|(k, _)| *k == kind),
                "fixture {name} lacks {kind}"
            );
            assert!(
                report.removed.iter().any(|item| item.kind == kind),
                "{name}: {kind}: {report:?}"
            );
        }
        assert!(report.present.is_empty());
        // Other metadata categories intentionally retain incomplete coverage.
        assert!(
            report.unchecked.iter().all(
                |item| !["camera", "serial", "owner", "location"].contains(&item.kind.as_str())
            )
        );
        let mut damaged = original.clone();
        damaged.truncate(damaged.len() - 7);
        let damaged = snapshot(&summarize(&damaged));
        assert!(!damaged.complete);
        assert!(compare(&damaged, &snapshot(&after)).removed.is_empty());
    }
}

#[test]
fn webp_keeps_the_compressed_image_payload() {
    let original = include_bytes!("fixtures/cwebp-photo.webp");
    let cleaned = clean(original).unwrap();
    fn image(bytes: &[u8]) -> &[u8] {
        let mut at = 12;
        while at + 8 <= bytes.len() {
            let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            if &bytes[at..at + 4] == b"VP8 " || &bytes[at..at + 4] == b"VP8L" {
                return &bytes[at + 8..at + 8 + len];
            }
            at += 8 + len + (len & 1);
        }
        panic!("image chunk missing")
    }
    assert_eq!(image(original), image(&cleaned.bytes));
}
