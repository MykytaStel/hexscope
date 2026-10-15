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

/// An attachment and text under a black box are findings read whole: the
/// copy is read completely, its properties are confirmed gone, and the
/// hidden text it no longer holds stays unchecked only because the contract
/// does not cover it yet.
#[test]
fn removed_hidden_pdf_findings_outside_the_contract_stay_unchecked() {
    let source_bytes = include_bytes!("fixtures/redacted.pdf");
    let source = summarize(source_bytes);
    assert!(source.facts.iter().any(|(kind, _)| *kind == "covered"));
    assert!(source.facts.iter().any(|(kind, _)| *kind == "hiddentext"));

    let cleaned = clean::clean(source_bytes).unwrap();
    let output = summarize(&cleaned.bytes);
    assert!(
        !output
            .facts
            .iter()
            .any(|(kind, _)| *kind == "covered" || *kind == "hiddentext")
    );
    assert!(output.facts.iter().any(|(kind, _)| *kind == "attachments"));

    assert!(source.complete && output.complete);
    let report = compare(&snapshot(&source), &snapshot(&output));

    for kind in ["title", "producer"] {
        assert!(
            report.removed.iter().any(|item| item.kind == kind),
            "{kind}: {report:?}"
        );
    }
    assert!(report.present.iter().any(|item| {
        item.kind == "attachments" && item.reason == VerificationReason::StillPresent
    }));
    for kind in ["covered", "hiddentext"] {
        assert!(
            report.unchecked.iter().any(|item| {
                item.kind == kind && item.reason == VerificationReason::CoverageIncomplete
            }),
            "{kind}: {:?}",
            report.unchecked
        );
    }
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
