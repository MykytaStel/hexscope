//! Conservative classification of parser-backed findings in a cleaned copy.

use crate::summary::Summary;
use std::cmp::Ordering;

/// One parser-backed fact. The value and scope are kept in memory only; they
/// are never copied into [`VerificationReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationFinding {
    pub kind: String,
    pub scope: String,
    pub value: Option<String>,
}

/// Facts observed in one parsed file and whether its parser returned a clean
/// structural read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationSnapshot {
    pub format: String,
    pub complete: bool,
    pub findings: Vec<VerificationFinding>,
}

/// Stable machine-readable explanation for one result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationReason {
    Removed,
    StillPresent,
    ValueChangedSameKind,
    NoStableValue,
    CoverageIncomplete,
    ParseIncomplete,
    UnexpectedOutput,
    NoComparableFindings,
    VerificationSkipped,
}

impl VerificationReason {
    /// The versioned report's stable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::StillPresent => "still_present",
            Self::ValueChangedSameKind => "value_changed_same_kind",
            Self::NoStableValue => "no_stable_value",
            Self::CoverageIncomplete => "coverage_incomplete",
            Self::ParseIncomplete => "parse_incomplete",
            Self::UnexpectedOutput => "unexpected_output",
            Self::NoComparableFindings => "no_comparable_findings",
            Self::VerificationSkipped => "verification_skipped",
        }
    }
}

/// A value-free report item, grouped by its status in the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationItem {
    pub kind: String,
    pub reason: VerificationReason,
    pub unexpected: bool,
}

/// A versioned, value-free result of comparing a source with its copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub schema_version: u8,
    pub removed: Vec<VerificationItem>,
    pub present: Vec<VerificationItem>,
    pub unchecked: Vec<VerificationItem>,
}

impl VerificationReport {
    /// A failed-to-run or deliberately bounded check. It is never a clear result.
    pub fn skipped() -> Self {
        Self {
            schema_version: 1,
            removed: Vec::new(),
            present: Vec::new(),
            unchecked: vec![VerificationItem {
                kind: "file".into(),
                reason: VerificationReason::VerificationSkipped,
                unexpected: false,
            }],
        }
    }

    /// JSON contains only stable kinds, reasons, statuses, and the schema version.
    pub fn to_json(&self) -> String {
        fn quote(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => {
                        use std::fmt::Write as _;
                        let _ = write!(out, "\\u{:04x}", c as u32);
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }

        fn group(items: &[VerificationItem]) -> String {
            let items = items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"kind\":{},\"reason\":{},\"unexpected\":{}}}",
                        quote(&item.kind),
                        quote(item.reason.code()),
                        item.unexpected
                    )
                })
                .collect::<Vec<_>>();
            format!("[{}]", items.join(","))
        }

        format!(
            "{{\"schema_version\":{},\"removed\":{},\"present\":{},\"unchecked\":{}}}",
            self.schema_version,
            group(&self.removed),
            group(&self.present),
            group(&self.unchecked)
        )
    }
}

/// Copies parser facts into the classifier's owned in-memory representation.
pub fn snapshot(summary: &Summary) -> VerificationSnapshot {
    VerificationSnapshot {
        format: summary.format.to_owned(),
        complete: summary.complete,
        findings: summary
            .facts
            .iter()
            .map(|(kind, value)| VerificationFinding {
                kind: (*kind).to_owned(),
                scope: "file".into(),
                value: Some(value.clone()),
            })
            .collect(),
    }
}

/// Compares observations as a multiset. A source finding is only called
/// removed when its exact format/kind/scope is explicitly covered and both
/// parses are complete.
pub fn compare(source: &VerificationSnapshot, output: &VerificationSnapshot) -> VerificationReport {
    let mut report = VerificationReport {
        schema_version: 1,
        removed: Vec::new(),
        present: Vec::new(),
        unchecked: Vec::new(),
    };
    let mut used_output = vec![false; output.findings.len()];
    let mut source_status = vec![None; source.findings.len()];

    if source.format == output.format {
        let source_order = sorted_indices(&source.findings);
        let output_order = sorted_indices(&output.findings);

        // Join exact keys as sorted multisets. Each item advances once, so
        // duplicate findings remain distinct without rescanning the copy.
        let (mut s, mut o) = (0, 0);
        while s < source_order.len() && o < output_order.len() {
            let left = &source.findings[source_order[s]];
            let right = &output.findings[output_order[o]];
            match key_order(left, right) {
                Ordering::Less => {
                    s = key_end(&source_order, s, &source.findings);
                }
                Ordering::Greater => {
                    o = key_end(&output_order, o, &output.findings);
                }
                Ordering::Equal => {
                    let next_s = key_end(&source_order, s, &source.findings);
                    let next_o = key_end(&output_order, o, &output.findings);
                    let reason = if stable_value(left).is_some() {
                        VerificationReason::StillPresent
                    } else {
                        VerificationReason::NoStableValue
                    };
                    for (&source_index, &output_index) in
                        source_order[s..next_s].iter().zip(&output_order[o..next_o])
                    {
                        source_status[source_index] = Some(reason);
                        used_output[output_index] = true;
                    }
                    s = next_s;
                    o = next_o;
                }
            }
        }

        // Pair stable leftovers first so unknown values cannot consume a
        // provable same-kind residual. Each item still advances at most once.
        let (mut s, mut o) = (0, 0);
        while s < source_order.len() && o < output_order.len() {
            let left = &source.findings[source_order[s]];
            let right = &output.findings[output_order[o]];
            match scope_order(left, right) {
                Ordering::Less => {
                    s = scope_end(&source_order, s, &source.findings);
                }
                Ordering::Greater => {
                    o = scope_end(&output_order, o, &output.findings);
                }
                Ordering::Equal => {
                    let next_s = scope_end(&source_order, s, &source.findings);
                    let next_o = scope_end(&output_order, o, &output.findings);
                    let (mut si, mut oi) = (s, o);
                    while si < next_s && oi < next_o {
                        let source_index = source_order[si];
                        let output_index = output_order[oi];
                        if source_status[source_index].is_some()
                            || stable_value(&source.findings[source_index]).is_none()
                        {
                            si += 1;
                        } else if used_output[output_index]
                            || stable_value(&output.findings[output_index]).is_none()
                        {
                            oi += 1;
                        } else {
                            source_status[source_index] =
                                Some(VerificationReason::ValueChangedSameKind);
                            used_output[output_index] = true;
                            si += 1;
                            oi += 1;
                        }
                    }

                    let has_unknown_output = output_order[o..next_o].iter().any(|&index| {
                        !used_output[index] && stable_value(&output.findings[index]).is_none()
                    });
                    if has_unknown_output {
                        let mut unmatched_sources = 0;
                        for &index in &source_order[s..next_s] {
                            if source_status[index].is_none() {
                                source_status[index] = Some(VerificationReason::NoStableValue);
                                unmatched_sources += 1;
                            }
                        }
                        for &index in &output_order[o..next_o] {
                            if unmatched_sources == 0 {
                                break;
                            }
                            if !used_output[index]
                                && stable_value(&output.findings[index]).is_none()
                            {
                                used_output[index] = true;
                                unmatched_sources -= 1;
                            }
                        }
                    }
                    s = next_s;
                    o = next_o;
                }
            }
        }
    }

    for (i, original) in source.findings.iter().enumerate() {
        if let Some(reason) = source_status[i] {
            if reason == VerificationReason::NoStableValue {
                report.unchecked.push(item(original, reason, false));
            } else {
                report.present.push(item(original, reason, false));
            }
            continue;
        }

        let reason = if stable_value(original).is_none() {
            Some(VerificationReason::NoStableValue)
        } else if !source.complete || !output.complete {
            Some(VerificationReason::ParseIncomplete)
        } else if source.format != output.format
            || !covered(&source.format, &original.kind, &original.scope)
        {
            Some(VerificationReason::CoverageIncomplete)
        } else {
            None
        };
        match reason {
            Some(reason) => report.unchecked.push(item(original, reason, false)),
            None => report
                .removed
                .push(item(original, VerificationReason::Removed, false)),
        }
    }

    for (i, finding) in output.findings.iter().enumerate() {
        if !used_output[i] {
            if stable_value(finding).is_none() {
                report
                    .unchecked
                    .push(item(finding, VerificationReason::NoStableValue, false));
            } else {
                report
                    .present
                    .push(item(finding, VerificationReason::UnexpectedOutput, true));
            }
        }
    }

    if source.findings.is_empty() && output.findings.is_empty() {
        let reason = if source.complete && output.complete {
            VerificationReason::NoComparableFindings
        } else {
            VerificationReason::ParseIncomplete
        };
        report.unchecked.push(VerificationItem {
            kind: "file".into(),
            reason,
            unexpected: false,
        });
    }

    report
}

fn item(
    finding: &VerificationFinding,
    reason: VerificationReason,
    unexpected: bool,
) -> VerificationItem {
    VerificationItem {
        kind: finding.kind.clone(),
        reason,
        unexpected,
    }
}

fn key_order(left: &VerificationFinding, right: &VerificationFinding) -> Ordering {
    (&left.kind, &left.scope, &left.value).cmp(&(&right.kind, &right.scope, &right.value))
}

fn stable_value(finding: &VerificationFinding) -> Option<&str> {
    finding
        .value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

fn sorted_indices(findings: &[VerificationFinding]) -> Vec<usize> {
    let mut order: Vec<_> = (0..findings.len()).collect();
    let len = order.len();
    let mut start = len / 2;
    while start > 0 {
        start -= 1;
        sift_down(&mut order, start, len, findings);
    }
    let mut end = len;
    while end > 1 {
        end -= 1;
        order.swap(0, end);
        sift_down(&mut order, 0, end, findings);
    }
    order
}

fn sift_down(order: &mut [usize], start: usize, end: usize, findings: &[VerificationFinding]) {
    let mut root = start;
    while root < end / 2 {
        let left = root * 2 + 1;
        let mut child = left;
        if left + 1 < end
            && key_order(&findings[order[left]], &findings[order[left + 1]]) == Ordering::Less
        {
            child += 1;
        }
        if key_order(&findings[order[root]], &findings[order[child]]) != Ordering::Less {
            break;
        }
        order.swap(root, child);
        root = child;
    }
}

fn scope_order(left: &VerificationFinding, right: &VerificationFinding) -> Ordering {
    (&left.kind, &left.scope).cmp(&(&right.kind, &right.scope))
}

fn key_end(order: &[usize], start: usize, findings: &[VerificationFinding]) -> usize {
    let key = &findings[order[start]];
    let mut end = start + 1;
    while end < order.len() && key_order(key, &findings[order[end]]) == Ordering::Equal {
        end += 1;
    }
    end
}

fn scope_end(order: &[usize], start: usize, findings: &[VerificationFinding]) -> usize {
    let key = &findings[order[start]];
    let mut end = start + 1;
    while end < order.len() && scope_order(key, &findings[order[end]]) == Ordering::Equal {
        end += 1;
    }
    end
}

fn covered(format: &str, kind: &str, scope: &str) -> bool {
    format == "jpeg"
        && scope == "file"
        && matches!(kind, "camera" | "serial" | "owner" | "location")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(kind: &str, scope: &str, value: Option<&str>) -> VerificationFinding {
        VerificationFinding {
            kind: kind.into(),
            scope: scope.into(),
            value: value.map(str::to_owned),
        }
    }

    fn snapshot(
        format: &str,
        complete: bool,
        findings: Vec<VerificationFinding>,
    ) -> VerificationSnapshot {
        VerificationSnapshot {
            format: format.into(),
            complete,
            findings,
        }
    }

    #[test]
    fn verification_compares_duplicate_findings_without_collapsing_occurrences() {
        let source = snapshot(
            "jpeg",
            true,
            vec![
                finding("camera", "file", Some("camera A")),
                finding("camera", "file", Some("camera A")),
            ],
        );
        let output = snapshot(
            "jpeg",
            true,
            vec![finding("camera", "file", Some("camera A"))],
        );

        let report = compare(&source, &output);

        assert_eq!(report.present.len(), 1);
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.removed[0].reason, VerificationReason::Removed);
        assert!(report.unchecked.is_empty());
    }

    #[test]
    fn verification_matches_unordered_values_before_pairing_changed_findings() {
        let source = snapshot(
            "jpeg",
            true,
            vec![
                finding("camera", "file", Some("camera Z")),
                finding("camera", "file", Some("camera A")),
                finding("location", "file", Some("place A")),
                finding("serial", "file", None),
            ],
        );
        let output = snapshot(
            "jpeg",
            true,
            vec![
                finding("location", "file", Some("place B")),
                finding("camera", "file", Some("camera B")),
                finding("owner", "file", Some("someone")),
                finding("camera", "file", Some("camera A")),
            ],
        );

        let report = compare(&source, &output);

        assert_eq!(
            report
                .present
                .iter()
                .map(|item| (item.kind.as_str(), item.reason))
                .collect::<Vec<_>>(),
            [
                ("camera", VerificationReason::ValueChangedSameKind),
                ("camera", VerificationReason::StillPresent),
                ("location", VerificationReason::ValueChangedSameKind),
                ("owner", VerificationReason::UnexpectedOutput),
            ]
        );
        assert_eq!(report.unchecked.len(), 1);
        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::NoStableValue
        );
        assert!(report.removed.is_empty());
    }

    #[test]
    fn verification_handles_thousands_of_same_kind_findings() {
        let source = snapshot(
            "jpeg",
            true,
            (0..4096)
                .map(|i| VerificationFinding {
                    kind: "camera".into(),
                    scope: "file".into(),
                    value: Some(format!("source-{i}")),
                })
                .collect(),
        );
        let output = snapshot(
            "jpeg",
            true,
            (0..4096)
                .rev()
                .map(|i| VerificationFinding {
                    kind: "camera".into(),
                    scope: "file".into(),
                    value: Some(format!("copy-{i}")),
                })
                .collect(),
        );

        let report = compare(&source, &output);

        assert_eq!(report.present.len(), 4096);
        assert!(
            report
                .present
                .iter()
                .all(|item| item.reason == VerificationReason::ValueChangedSameKind)
        );
        assert!(report.removed.is_empty());
        assert!(report.unchecked.is_empty());
    }

    #[test]
    fn verification_treats_a_changed_value_of_the_same_kind_as_present() {
        let source = snapshot(
            "jpeg",
            true,
            vec![finding("camera", "file", Some("camera A"))],
        );
        let output = snapshot(
            "jpeg",
            true,
            vec![finding("camera", "file", Some("camera B"))],
        );

        let report = compare(&source, &output);

        assert!(report.removed.is_empty());
        assert_eq!(
            report.present[0].reason,
            VerificationReason::ValueChangedSameKind
        );
    }

    #[test]
    fn verification_marks_uncovered_and_incomplete_findings_unchecked() {
        let uncovered = compare(
            &snapshot("jpeg", true, vec![finding("lens", "file", Some("lens A"))]),
            &snapshot("jpeg", true, vec![]),
        );
        assert_eq!(
            uncovered.unchecked[0].reason,
            VerificationReason::CoverageIncomplete
        );

        let incomplete = compare(
            &snapshot(
                "jpeg",
                false,
                vec![finding("camera", "file", Some("camera A"))],
            ),
            &snapshot("jpeg", true, vec![]),
        );
        assert_eq!(
            incomplete.unchecked[0].reason,
            VerificationReason::ParseIncomplete
        );

        let wrong_scope = compare(
            &snapshot(
                "jpeg",
                true,
                vec![finding("location", "region", Some("somewhere"))],
            ),
            &snapshot("jpeg", true, vec![]),
        );
        assert_eq!(
            wrong_scope.unchecked[0].reason,
            VerificationReason::CoverageIncomplete
        );

        let wrong_format = compare(
            &snapshot(
                "png",
                true,
                vec![finding("camera", "file", Some("camera A"))],
            ),
            &snapshot("png", true, vec![]),
        );
        assert_eq!(
            wrong_format.unchecked[0].reason,
            VerificationReason::CoverageIncomplete
        );
    }

    #[test]
    fn verification_does_not_call_a_finding_removed_when_output_format_changes() {
        let report = compare(
            &snapshot(
                "jpeg",
                true,
                vec![finding("camera", "file", Some("camera A"))],
            ),
            &snapshot("png", true, vec![]),
        );

        assert!(report.removed.is_empty());
        assert_eq!(report.unchecked.len(), 1);
        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::CoverageIncomplete
        );
    }

    #[test]
    fn verification_marks_a_finding_without_a_stable_value_unchecked() {
        let report = compare(
            &snapshot("jpeg", true, vec![finding("camera", "file", None)]),
            &snapshot("jpeg", true, vec![]),
        );

        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::NoStableValue
        );
    }

    #[test]
    fn verification_does_not_call_a_source_finding_removed_when_output_has_same_kind_without_value()
    {
        let report = compare(
            &snapshot(
                "jpeg",
                true,
                vec![finding("camera", "file", Some("camera A"))],
            ),
            &snapshot("jpeg", true, vec![finding("camera", "file", None)]),
        );

        assert!(report.removed.is_empty());
        assert!(report.present.is_empty());
        assert_eq!(report.unchecked.len(), 1);
        assert!(
            report
                .unchecked
                .iter()
                .all(|item| item.reason == VerificationReason::NoStableValue)
        );
        assert!(
            report
                .unchecked
                .iter()
                .all(|item| item.kind == "camera" && !item.unexpected)
        );
    }

    #[test]
    fn verification_pairs_known_values_before_unknown_same_kind_values() {
        let report = compare(
            &snapshot(
                "jpeg",
                true,
                vec![
                    finding("camera", "file", None),
                    finding("camera", "file", Some("camera A")),
                ],
            ),
            &snapshot(
                "jpeg",
                true,
                vec![finding("camera", "file", Some("camera B"))],
            ),
        );

        assert!(report.removed.is_empty());
        assert_eq!(report.present.len(), 1);
        assert_eq!(
            report.present[0].reason,
            VerificationReason::ValueChangedSameKind
        );
        assert_eq!(report.unchecked.len(), 1);
        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::NoStableValue
        );
    }

    #[test]
    fn verification_does_not_call_an_output_finding_without_value_unexpected() {
        let report = compare(
            &snapshot("jpeg", true, vec![]),
            &snapshot("jpeg", true, vec![finding("camera", "file", None)]),
        );

        assert!(report.present.is_empty());
        assert!(report.removed.is_empty());
        assert_eq!(report.unchecked.len(), 1);
        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::NoStableValue
        );
        assert!(!report.unchecked[0].unexpected);
    }

    #[test]
    fn verification_reports_output_only_findings_as_unexpected() {
        let report = compare(
            &snapshot("jpeg", true, vec![]),
            &snapshot(
                "jpeg",
                true,
                vec![finding("camera", "file", Some("camera A"))],
            ),
        );

        assert_eq!(report.present.len(), 1);
        assert_eq!(
            report.present[0].reason,
            VerificationReason::UnexpectedOutput
        );
        assert!(report.present[0].unexpected);
        assert!(report.unchecked.is_empty());
    }

    #[test]
    fn verification_does_not_call_an_empty_comparison_clear() {
        let report = compare(
            &snapshot("jpeg", true, vec![]),
            &snapshot("jpeg", true, vec![]),
        );

        assert_eq!(report.unchecked[0].kind, "file");
        assert_eq!(
            report.unchecked[0].reason,
            VerificationReason::NoComparableFindings
        );
    }

    #[test]
    fn verification_serialized_report_contains_no_finding_values_or_scopes() {
        let report = compare(
            &snapshot(
                "jpeg",
                true,
                vec![finding(
                    "camera",
                    "private-scope-sentinel",
                    Some("private-value-sentinel"),
                )],
            ),
            &snapshot("jpeg", true, vec![]),
        );

        let json = report.to_json();

        assert!(json.contains("\"schema_version\":1"));
        assert!(!json.contains("private-value-sentinel"));
        assert!(!json.contains("private-scope-sentinel"));
    }

    #[test]
    fn verification_summary_is_incomplete_when_it_has_a_warning_or_error() {
        use crate::document::Document;
        use crate::model::{ByteRange, NodeKind, ParseTree};
        use crate::summary::summary_of;

        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "unknown",
            ByteRange::new(0, 1),
            NodeKind::Container,
            None,
        );
        tree.warning(root, "warning", ByteRange::new(0, 1));

        assert!(!summary_of(&Document::Unknown(tree)).complete);
    }
}
