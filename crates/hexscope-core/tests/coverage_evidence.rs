//! The evidence behind each row of the removal contract: real files of each
//! format, cleaned and read back, have every covered finding confirmed
//! removed, and none left unchecked for want of coverage.

use hexscope_core::{
    clean::clean,
    summary::summarize,
    verification::{VERIFIED_REMOVAL, VerificationReason, compare, snapshot},
};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Cleans `name` and returns the kinds confirmed removed, after checking that
/// each covered finding of the original is among them.
fn confirmed(name: &str, format: &str) -> Vec<String> {
    let original = fixture(name);
    let before = summarize(&original);
    assert_eq!(before.format, format, "{name}");
    assert!(before.complete, "{name}: {:?}", before.problems);
    let copy = clean(&original).unwrap();
    let after = summarize(&copy.bytes);
    assert!(
        after.complete,
        "{name}: the copy reads with {:?}",
        after.problems
    );
    let report = compare(&snapshot(&before), &snapshot(&after));
    let covered = VERIFIED_REMOVAL
        .iter()
        .find(|(f, _)| *f == format)
        .unwrap()
        .1;
    for (kind, _) in before.facts.iter().filter(|(k, _)| covered.contains(k)) {
        assert!(
            report.removed.iter().any(|i| i.kind == *kind),
            "{name}: {kind}: {report:?}"
        );
    }
    assert!(
        !report
            .unchecked
            .iter()
            .any(|i| i.reason == VerificationReason::CoverageIncomplete
                && covered.contains(&i.kind.as_str())),
        "{name}: {report:?}"
    );
    report.removed.into_iter().map(|i| i.kind).collect()
}

#[test]
fn photos_lose_every_covered_finding() {
    let all = [
        "location", "camera", "lens", "serial", "owner", "taken", "software",
    ];
    for (name, format, extra) in [
        ("photo.jpg", "jpeg", Some("thumbnail")),
        ("photo.png", "png", None),
        ("cwebp-photo.webp", "webp", Some("thumbnail")),
        ("photo.heic", "heif", None),
    ] {
        let removed = confirmed(name, format);
        for kind in all.iter().chain(extra.iter()) {
            assert!(
                removed.iter().any(|k| k == kind),
                "{name} lacks {kind} as evidence"
            );
        }
    }
}

#[test]
fn ai_pictures_lose_what_made_them_and_the_prompt() {
    let removed = confirmed("ai-picture.png", "png");
    for kind in ["ai", "prompt"] {
        assert!(
            removed.iter().any(|k| k == kind),
            "ai-picture.png lacks {kind} as evidence"
        );
    }
}

#[test]
fn videos_lose_place_camera_owner_time_and_software() {
    let mut seen: Vec<String> = Vec::new();
    for name in ["iphone.mov", "android.mp4", "voice.m4a"] {
        seen.extend(confirmed(name, "video"));
    }
    for kind in ["location", "camera", "owner", "taken", "software"] {
        assert!(
            seen.iter().any(|k| k == kind),
            "no video fixture shows {kind} removed"
        );
    }
}

#[test]
fn documents_lose_their_properties_and_a_macs_notes() {
    let docx = confirmed("report.docx", "zip");
    for kind in ["title", "author", "editor", "created", "company"] {
        assert!(docx.iter().any(|k| k == kind), "report.docx: {kind}");
    }
    let mac = confirmed("mac-report.zip", "zip");
    assert_eq!(mac, ["downloaded", "tags"]);
    let doc = confirmed("plan.doc", "office97");
    for kind in ["title", "author"] {
        assert!(doc.iter().any(|k| k == kind), "plan.doc: {kind}");
    }
}
