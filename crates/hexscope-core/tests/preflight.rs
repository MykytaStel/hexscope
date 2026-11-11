use hexscope_core::{
    coverage::CoverageStatus,
    model::{ByteRange, NodeKind, ParseTree},
    summary::summarize,
    verification::{planned_removal_kinds, snapshot},
};

#[test]
fn plan_uses_only_complete_supported_capabilities() {
    let summary = summarize(include_bytes!("fixtures/photo.jpg"));
    assert_eq!(summary.inspection_status(), CoverageStatus::Complete);
    let mut source = snapshot(&summary);
    assert!(planned_removal_kinds(&source).contains(&"location"));
    source.complete = false;
    assert!(planned_removal_kinds(&source).is_empty());
    assert_eq!(
        summarize(b"hello").inspection_status(),
        CoverageStatus::Unsupported
    );
}

#[test]
fn shared_node_budget_keeps_one_error_and_bounded_storage() {
    let mut tree = ParseTree::with_node_limit(3);
    let root = tree.add(
        None,
        "root",
        ByteRange::new(0, 100),
        NodeKind::Container,
        None,
    );
    for _ in 0..100 {
        tree.add(
            Some(root),
            "field",
            ByteRange::new(0, 1),
            NodeKind::Field,
            None,
        );
    }
    assert_eq!(tree.len(), 4);
    assert_eq!(tree.get(3).kind, NodeKind::Error);
    assert_eq!(tree.get(root).children, [1, 2, 3]);
}
