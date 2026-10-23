use crate::verification::{VerificationReport, json_quote};
use std::fmt;

const FORMATS: &[&str] = &[
    "jpeg", "png", "heif", "webp", "gif", "tiff", "video", "pdf", "zip", "wasm", "eml", "cfb",
    "msg", "office97", "rtf", "svg", "unknown",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptError {
    UnsupportedFormat,
    UnsupportedVerificationSchema,
    InvalidFingerprint,
}

impl fmt::Display for ReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat => f.write_str("unsupported output format"),
            Self::UnsupportedVerificationSchema => {
                f.write_str("unsupported verification report version")
            }
            Self::InvalidFingerprint => f.write_str("invalid copy fingerprint fields"),
        }
    }
}

impl std::error::Error for ReceiptError {}

/// Serializes a versioned, value-free record for one clean copy.
pub fn copy_receipt_json(
    format: &str,
    copy_sha256: Option<&str>,
    fingerprint_reason: Option<&str>,
    report: &VerificationReport,
) -> Result<String, ReceiptError> {
    if !FORMATS.contains(&format) {
        return Err(ReceiptError::UnsupportedFormat);
    }
    if report.schema_version != 1 {
        return Err(ReceiptError::UnsupportedVerificationSchema);
    }

    let (digest, reason) = match (copy_sha256, fingerprint_reason) {
        (Some(digest), None)
            if digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            (json_quote(digest), None)
        }
        (None, Some("size_limit")) => ("null".to_string(), Some("size_limit")),
        _ => return Err(ReceiptError::InvalidFingerprint),
    };

    let reason_field = reason.map_or_else(String::new, |reason| {
        format!(",\"fingerprint_reason\":{}", json_quote(reason))
    });
    Ok(format!(
        "{{\"schema\":\"hexscope.clean-copy-receipt\",\"version\":1,\"hexscope_version\":{},\"copy\":{{\"format\":{},\"sha256\":{}{}}},\"verification\":{}}}",
        json_quote(env!("CARGO_PKG_VERSION")),
        json_quote(format),
        digest,
        reason_field,
        report.to_json()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::{
        VerificationFinding, VerificationItem, VerificationReason, VerificationReport,
        VerificationSnapshot, compare,
    };

    fn report_with_all_statuses() -> VerificationReport {
        VerificationReport {
            schema_version: 1,
            removed: vec![VerificationItem {
                kind: "location".into(),
                reason: VerificationReason::Removed,
                unexpected: false,
            }],
            present: vec![VerificationItem {
                kind: "author".into(),
                reason: VerificationReason::StillPresent,
                unexpected: false,
            }],
            unchecked: vec![VerificationItem {
                kind: "custom".into(),
                reason: VerificationReason::CoverageIncomplete,
                unexpected: false,
            }],
        }
    }

    #[test]
    fn receipt_has_the_versioned_schema_and_all_value_free_status_groups() {
        let digest = "a".repeat(64);
        let receipt =
            copy_receipt_json("png", Some(&digest), None, &report_with_all_statuses()).unwrap();
        assert_eq!(
            receipt,
            format!(
                "{{\"schema\":\"hexscope.clean-copy-receipt\",\"version\":1,\"hexscope_version\":\"{}\",\"copy\":{{\"format\":\"png\",\"sha256\":\"{}\"}},\"verification\":{{\"schema_version\":1,\"removed\":[{{\"kind\":\"location\",\"reason\":\"removed\",\"unexpected\":false}}],\"present\":[{{\"kind\":\"author\",\"reason\":\"still_present\",\"unexpected\":false}}],\"unchecked\":[{{\"kind\":\"custom\",\"reason\":\"coverage_incomplete\",\"unexpected\":false}}]}}}}",
                env!("CARGO_PKG_VERSION"),
                digest
            )
        );
    }

    #[test]
    fn missing_digest_is_explicitly_marked_as_size_limited() {
        let receipt = copy_receipt_json(
            "png",
            None,
            Some("size_limit"),
            &VerificationReport::skipped(),
        )
        .unwrap();
        assert!(receipt.contains("\"sha256\":null"));
        assert!(receipt.contains("\"fingerprint_reason\":\"size_limit\""));
    }

    #[test]
    fn invalid_digest_reason_and_format_combinations_are_rejected() {
        let report = VerificationReport::skipped();
        for digest in ["abc", &"A".repeat(64)] {
            assert!(copy_receipt_json("png", Some(digest), None, &report).is_err());
        }
        assert!(copy_receipt_json("png", None, None, &report).is_err());
        assert!(copy_receipt_json("png", None, Some("other"), &report).is_err());
        assert!(
            copy_receipt_json("png", Some(&"a".repeat(64)), Some("size_limit"), &report).is_err()
        );
        assert!(
            copy_receipt_json("../private/file", Some(&"a".repeat(64)), None, &report).is_err()
        );
    }

    #[test]
    fn receipt_never_serializes_finding_values_scopes_or_paths() {
        let source = VerificationSnapshot {
            format: "png".into(),
            complete: true,
            findings: vec![
                VerificationFinding {
                    kind: "location".into(),
                    scope: "file".into(),
                    value: Some("COORDINATE_SENTINEL".into()),
                },
                VerificationFinding {
                    kind: "location".into(),
                    scope: "/Users/alice/private/photo.png".into(),
                    value: Some("PATH_SCOPE_SENTINEL".into()),
                },
                VerificationFinding {
                    kind: "author".into(),
                    scope: "secret-scope".into(),
                    value: Some("AUTHOR_SENTINEL".into()),
                },
                VerificationFinding {
                    kind: "custom".into(),
                    scope: "private-scope".into(),
                    value: Some("PRIVATE_METADATA_SENTINEL".into()),
                },
            ],
        };
        let output = VerificationSnapshot {
            format: "png".into(),
            complete: true,
            findings: vec![VerificationFinding {
                kind: "author".into(),
                scope: "secret-scope".into(),
                value: Some("AUTHOR_SENTINEL".into()),
            }],
        };
        let report = compare(&source, &output);
        assert!(!report.removed.is_empty());
        assert!(!report.present.is_empty());
        assert!(!report.unchecked.is_empty());

        let receipt = copy_receipt_json("png", Some(&"b".repeat(64)), None, &report).unwrap();
        for secret in [
            "/Users/alice/private/photo.png",
            "COORDINATE_SENTINEL",
            "AUTHOR_SENTINEL",
            "PRIVATE_METADATA_SENTINEL",
            "secret-scope",
            "private-scope",
        ] {
            assert!(!receipt.contains(secret), "receipt leaked {secret}");
        }
    }
}
