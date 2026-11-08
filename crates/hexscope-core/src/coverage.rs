//! Value-free counts of what a parser understood in a set of files.
//!
//! A scorecard keeps only format names, read status, input byte counts and
//! elapsed time. It never stores file names or finding values.

use crate::summary::Summary;
use std::collections::BTreeMap;
use std::time::Duration;

/// How completely hexscope could read one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageStatus {
    /// The format was recognized and the parser read all content it accounts for.
    Complete,
    /// The format was recognized, but some content could not be fully read.
    Partial,
    /// The format was not recognized or is not supported by this build.
    Unsupported,
}

impl CoverageStatus {
    /// Classify a parser summary without treating an unrecognized file as a
    /// complete read merely because its bytes have been accounted for.
    pub fn from_summary(summary: &Summary) -> Self {
        if summary.format == "unknown" {
            Self::Unsupported
        } else if summary.complete {
            Self::Complete
        } else {
            Self::Partial
        }
    }

    /// Stable, lowercase name for machine-readable reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Aggregate for one parsed format. `complete + partial + unsupported` is
/// always equal to `files`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatCoverage {
    pub format: &'static str,
    pub files: u64,
    pub complete: u64,
    pub partial: u64,
    pub unsupported: u64,
    pub bytes: u64,
    pub elapsed_micros: u64,
}

/// A deterministic, value-free report over a caller's local corpus.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CoverageScorecard {
    files: u64,
    bytes: u64,
    elapsed_micros: u64,
    formats: BTreeMap<&'static str, FormatCoverage>,
}

impl CoverageScorecard {
    /// Add the result for one input. No path, file bytes, or finding values
    /// are retained. Formats are returned in stable alphabetical order.
    pub fn record(&mut self, summary: &Summary, bytes: u64, elapsed: Duration) {
        let status = CoverageStatus::from_summary(summary);
        let elapsed_micros = elapsed.as_micros().min(u64::MAX as u128) as u64;
        let row = self
            .formats
            .entry(summary.format)
            .or_insert_with(|| FormatCoverage {
                format: summary.format,
                files: 0,
                complete: 0,
                partial: 0,
                unsupported: 0,
                bytes: 0,
                elapsed_micros: 0,
            });
        row.files = row.files.saturating_add(1);
        row.bytes = row.bytes.saturating_add(bytes);
        row.elapsed_micros = row.elapsed_micros.saturating_add(elapsed_micros);
        match status {
            CoverageStatus::Complete => row.complete = row.complete.saturating_add(1),
            CoverageStatus::Partial => row.partial = row.partial.saturating_add(1),
            CoverageStatus::Unsupported => row.unsupported = row.unsupported.saturating_add(1),
        }
        self.files = self.files.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        self.elapsed_micros = self.elapsed_micros.saturating_add(elapsed_micros);
    }

    pub const fn files(&self) -> u64 {
        self.files
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn elapsed_micros(&self) -> u64 {
        self.elapsed_micros
    }

    pub fn formats(&self) -> impl Iterator<Item = &FormatCoverage> {
        self.formats.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::summarize;

    #[test]
    fn aggregates_complete_partial_and_unsupported_files_without_values() {
        let photo = include_bytes!("../tests/fixtures/photo.jpg");
        let complete = summarize(photo);
        let partial = summarize(&[0xff, 0xd8, 0xff]);
        let unsupported = summarize(&[0, 1, 2, 3]);
        // The parser accounted for the fallback byte tree, but its format is
        // unsupported. Keep that distinction separate from Summary.complete.
        assert!(unsupported.complete);
        assert_eq!(
            CoverageStatus::from_summary(&unsupported),
            CoverageStatus::Unsupported
        );

        let mut scorecard = CoverageScorecard::default();
        scorecard.record(&complete, photo.len() as u64, Duration::from_micros(7));
        scorecard.record(&partial, 3, Duration::from_micros(11));
        scorecard.record(&unsupported, 4, Duration::from_micros(13));

        assert_eq!(scorecard.files(), 3);
        assert_eq!(scorecard.bytes(), photo.len() as u64 + 7);
        assert_eq!(scorecard.elapsed_micros(), 31);
        let rows: Vec<_> = scorecard.formats().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].format, "jpeg");
        assert_eq!(rows[0].files, 2);
        assert_eq!(rows[0].complete, 1);
        assert_eq!(rows[0].partial, 1);
        assert_eq!(rows[0].unsupported, 0);
        assert_eq!(rows[0].elapsed_micros, 18);
        assert_eq!(rows[1].format, "unknown");
        assert_eq!(rows[1].unsupported, 1);
        assert_eq!(CoverageStatus::Unsupported.as_str(), "unsupported");
        assert!(
            rows.iter()
                .all(|row| { row.files == row.complete + row.partial + row.unsupported })
        );
    }
}
