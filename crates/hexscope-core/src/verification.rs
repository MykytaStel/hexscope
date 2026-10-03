//! Conservative classification of parser-backed findings in a cleaned copy.

use crate::summary::Summary;

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

    for original in &source.findings {
        let exact = original.value.as_ref().and_then(|value| {
            output.findings.iter().enumerate().find_map(|(i, finding)| {
                (!used_output[i]
                    && source.format == output.format
                    && original.kind == finding.kind
                    && original.scope == finding.scope
                    && finding.value.as_ref() == Some(value))
                .then_some(i)
            })
        });
        if let Some(i) = exact {
            used_output[i] = true;
            report
                .present
                .push(item(original, VerificationReason::StillPresent, false));
            continue;
        }

        let changed = original.value.as_ref().and_then(|value| {
            output.findings.iter().enumerate().find_map(|(i, finding)| {
                (!used_output[i]
                    && source.format == output.format
                    && original.kind == finding.kind
                    && original.scope == finding.scope
                    && finding
                        .value
                        .as_ref()
                        .is_some_and(|output_value| output_value != value))
                .then_some(i)
            })
        });
        if let Some(i) = changed {
            used_output[i] = true;
            report.present.push(item(
                original,
                VerificationReason::ValueChangedSameKind,
                false,
            ));
            continue;
        }

        let reason = if original.value.is_none() {
            Some(VerificationReason::NoStableValue)
        } else if !source.complete || !output.complete {
            Some(VerificationReason::ParseIncomplete)
        } else if !covered(&source.format, &original.kind, &original.scope) {
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
            report
                .present
                .push(item(finding, VerificationReason::UnexpectedOutput, true));
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
