#![no_main]

use libfuzzer_sys::fuzz_target;

// A clean copy is a file people will share: it must read back as the same
// format, clean again without panicking, and compare against its source.
fuzz_target!(|data: &[u8]| {
    let Ok(copy) = hexscope_core::clean::clean(data) else {
        return;
    };
    let source = hexscope_core::summary::summarize(data);
    let output = hexscope_core::summary::summarize(&copy.bytes);
    assert_eq!(
        source.format, output.format,
        "a clean copy keeps its format"
    );
    assert!(!hexscope_core::parse(&copy.bytes).tree().is_empty());
    if output.complete {
        let second = hexscope_core::clean::clean(&copy.bytes);
        assert!(second.is_ok() || second == Err(hexscope_core::clean::CleanError::NothingToRemove), "a complete clean copy remains cleanable");
    }
    let source = hexscope_core::verification::snapshot(&source);
    let output = hexscope_core::verification::snapshot(&output);
    let report = hexscope_core::verification::compare(&source, &output);
    for removed in &report.removed {
        assert!(source.complete && output.complete);
        assert!(hexscope_core::verification::planned_removal_kinds(&source).contains(&removed.kind.as_str()));
        let before = source.findings.iter().filter(|finding| finding.kind == removed.kind).count();
        let after = output.findings.iter().filter(|finding| finding.kind == removed.kind).count();
        let claimed = report.removed.iter().filter(|finding| finding.kind == removed.kind).count();
        assert!(claimed <= before.saturating_sub(after), "removed claims cannot exceed observed disappearance");
    }
});
