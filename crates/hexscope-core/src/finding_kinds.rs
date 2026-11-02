//! Stable finding codes and the words each interface uses for them.
//!
//! Parsers report a code and its value. This small catalogue gives the page,
//! command line and reports their words from that same code.

/// The user-facing words for one stable finding code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindingKind {
    pub code: &'static str,
    /// A short label for a detail row, if this kind has one.
    pub label: Option<&'static str>,
    /// The phrase used when grouping findings in a share summary.
    pub category: Option<&'static str>,
    /// The phrase used in the file verdict.
    pub reveal: Option<&'static str>,
    /// Position in the share summary, when this kind is a category.
    pub category_order: Option<usize>,
}

fn nonempty(value: &'static str) -> Option<&'static str> {
    (!value.is_empty()).then_some(value)
}

fn parse_row(line: &'static str) -> FindingKind {
    let mut columns = line.split('\t');
    let code = columns.next().expect("finding kind has a code");
    let label = nonempty(columns.next().unwrap_or(""));
    let category = nonempty(columns.next().unwrap_or(""));
    let reveal = nonempty(columns.next().unwrap_or(""));
    let category_order = columns.next().unwrap_or("").parse().ok();
    debug_assert!(columns.next().is_none());
    FindingKind {
        code,
        label,
        category,
        reveal,
        category_order,
    }
}

/// Every registered finding kind, with the share categories in display order.
pub fn every() -> impl Iterator<Item = FindingKind> {
    include_str!("finding-kinds.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(parse_row)
}

/// The words for a code, or None when a parser has added an unknown code.
pub fn by_code(code: &str) -> Option<FindingKind> {
    every().find(|kind| kind.code == code)
}
