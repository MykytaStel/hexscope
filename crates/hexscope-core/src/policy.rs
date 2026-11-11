//! Local, value-free publication rules, shared by the browser and CLI.
use crate::{
    summary::Summary,
    verification::{VerificationSnapshot, json_quote},
};

#[derive(Debug, Clone)]
pub struct Policy {
    pub blocked_kinds: Vec<String>,
    pub require_complete: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PolicyResult {
    pub blocked_kinds: Vec<String>,
    pub unchecked: bool,
}

impl Policy {
    pub fn evaluate(&self, summary: &Summary) -> PolicyResult {
        self.evaluate_findings(
            summary.format,
            summary.complete,
            summary.facts.iter().map(|(kind, _)| *kind),
        )
    }

    pub fn evaluate_snapshot(&self, source: &VerificationSnapshot) -> PolicyResult {
        self.evaluate_findings(
            &source.format,
            source.complete,
            source.findings.iter().map(|item| item.kind.as_str()),
        )
    }

    fn evaluate_findings<'a>(
        &self,
        format: &str,
        complete: bool,
        kinds: impl Iterator<Item = &'a str>,
    ) -> PolicyResult {
        let observed: Vec<_> = kinds.collect();
        let mut blocked_kinds = self
            .blocked_kinds
            .iter()
            .filter(|kind| observed.contains(&kind.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        blocked_kinds.sort_unstable();
        blocked_kinds.dedup();
        PolicyResult {
            blocked_kinds,
            unchecked: self.blocked_kinds.iter().any(|kind| {
                kind.starts_with("qr") || crate::finding_kinds::by_code(kind).is_none()
            }) || self.require_complete && (!complete || format == "unknown"),
        }
    }
}

impl PolicyResult {
    /// A rule passing is not a safety certification.
    pub fn passed(&self) -> bool {
        self.blocked_kinds.is_empty() && !self.unchecked
    }

    pub fn to_json(&self) -> String {
        let kinds = self
            .blocked_kinds
            .iter()
            .map(|kind| json_quote(kind))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"hexscope.policy-result\",\"version\":1,\"passed\":{},\"unchecked\":{},\"blocked_kinds\":[{}]}}",
            self.passed(),
            self.unchecked,
            kinds
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_is_unchecked_even_when_its_byte_tree_is_complete() {
        let policy = Policy {
            blocked_kinds: vec!["location".into()],
            require_complete: true,
        };
        let unknown = crate::summary::summarize(b"hello");
        assert!(!policy.evaluate(&unknown).passed());
        let source = crate::summary::summarize(include_bytes!("../tests/fixtures/photo.jpg"));
        let result = policy.evaluate(&source);
        assert_eq!(result.blocked_kinds, ["location"]);
        assert_eq!(
            result,
            policy.evaluate_snapshot(&crate::verification::snapshot(&source))
        );
        assert!(!result.to_json().contains("latitude"));
        for kind in ["qrlink", "unregistered_kind"] {
            let unsupported = Policy {
                blocked_kinds: vec![kind.into()],
                require_complete: false,
            };
            assert!(unsupported.evaluate(&source).unchecked);
            assert!(!unsupported.evaluate(&source).passed());
        }
    }
}
