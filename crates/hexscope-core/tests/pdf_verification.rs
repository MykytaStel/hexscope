use hexscope_core::{
    clean,
    summary::summarize,
    verification::{VerificationReason, compare, snapshot},
};

#[test]
fn hidden_text_that_survives_in_a_pdf_copy_is_reported_present() {
    let source_bytes = include_bytes!("fixtures/chrome-highlight.pdf");
    let output_bytes = source_bytes.to_vec();
    let source = summarize(source_bytes);
    let output = summarize(&output_bytes);
    assert!(source.facts.iter().any(|(kind, _)| *kind == "hiddentext"));

    let report = compare(&snapshot(&source), &snapshot(&output));

    assert!(report.removed.is_empty());
    let hidden = report
        .present
        .iter()
        .find(|item| item.kind == "hiddentext")
        .expect("surviving hidden text must be reported");
    assert_eq!(hidden.reason, VerificationReason::StillPresent);
    assert!(!hidden.unexpected);
}

/// A redacted PDF's copy, read whole, is confirmed to have lost the text
/// under its boxes and hidden in white, its note and who wrote it, the file
/// it carried, and its properties.
#[test]
fn a_redacted_pdfs_copy_is_confirmed_without_what_it_hid() {
    let source_bytes = include_bytes!("fixtures/redacted.pdf");
    let source = summarize(source_bytes);
    let cleaned = clean::clean(source_bytes).unwrap();
    let output = summarize(&cleaned.bytes);
    assert!(source.complete && output.complete);
    assert!(output.facts.is_empty(), "{:?}", output.facts);

    let report = compare(&snapshot(&source), &snapshot(&output));
    for kind in [
        "covered",
        "hiddentext",
        "comments",
        "attachments",
        "title",
        "producer",
    ] {
        assert!(
            report.removed.iter().any(|item| item.kind == kind),
            "{kind}: {report:?}"
        );
    }
    assert!(
        report.present.is_empty() && report.unchecked.is_empty(),
        "{report:?}"
    );
}

#[test]
fn pdf_findings_found_only_in_the_output_are_reported_as_unexpected() {
    let source = summarize(include_bytes!("fixtures/compact.pdf"));
    let output = summarize(include_bytes!("fixtures/report.pdf"));
    assert!(source.complete);
    assert!(output.complete);

    let report = compare(&snapshot(&source), &snapshot(&output));

    for kind in ["earlier", "updates"] {
        let unexpected = report
            .present
            .iter()
            .find(|item| item.kind == kind)
            .unwrap_or_else(|| panic!("output-only PDF finding {kind} was missed"));
        assert_eq!(
            unexpected.reason,
            VerificationReason::UnexpectedOutput,
            "{kind}"
        );
        assert!(unexpected.unexpected, "{kind}");
        assert!(
            !report.removed.iter().any(|item| item.kind == kind),
            "{kind}"
        );
    }
}
