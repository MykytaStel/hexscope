//! Cross-format contract for recognized damaged inputs and unknown formats.
//! These small fixtures measure inspection status, not decoder correctness.

use hexscope_core::coverage::{CoverageScorecard, CoverageStatus};
use hexscope_core::summary::summarize;
use std::time::Duration;

#[test]
fn damaged_recognized_formats_do_not_report_complete() {
    let cases: &[(&str, &[u8])] = &[
        ("png", b"\x89PNG\r\n\x1a\n"),
        ("jpeg", b"\xff\xd8\xff"),
        ("gif", b"GIF89a\x01\x00\x01\x00\x00\x00\x00"),
        ("webp", b"RIFF\x06\x00\x00\x00WEBP"),
        #[cfg(feature = "office")]
        ("zip", b"PK\x03\x04"),
        #[cfg(feature = "svg")]
        ("svg", b"<svg/><svg/>"),
    ];

    let mut scorecard = CoverageScorecard::default();
    for &(format, bytes) in cases {
        let summary = summarize(bytes);
        assert_eq!(summary.format, format);
        assert_eq!(
            summary.inspection_status(),
            CoverageStatus::Partial,
            "{format}: {:?}",
            summary.problems
        );
        assert!(!summary.complete, "{format}");
        scorecard.record(&summary, bytes.len() as u64, Duration::ZERO);
    }

    let unknown = summarize(b"\x00\x01\x02\x03");
    assert_eq!(unknown.inspection_status(), CoverageStatus::Unsupported);
    scorecard.record(&unknown, 4, Duration::ZERO);

    assert_eq!(scorecard.files(), cases.len() as u64 + 1);
    assert_eq!(
        scorecard.formats().map(|row| row.partial).sum::<u64>(),
        cases.len() as u64
    );
    assert_eq!(
        scorecard.formats().map(|row| row.unsupported).sum::<u64>(),
        1
    );
    assert!(scorecard.formats().all(|row| row.complete == 0));
}

#[test]
fn a_valid_photo_remains_complete() {
    let bytes = include_bytes!("fixtures/photo.jpg");
    let summary = summarize(bytes);
    assert_eq!(summary.format, "jpeg");
    assert_eq!(
        summary.inspection_status(),
        CoverageStatus::Complete,
        "{:?}",
        summary.problems
    );
}
