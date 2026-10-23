#![no_main]

use libfuzzer_sys::fuzz_target;

// Audio metadata is untrusted input. Keep the parser bounded, and make every
// reported node and fact point back into the original bytes.
fuzz_target!(|data: &[u8]| {
    let Some(doc) = hexscope_core::audio::parse(data) else {
        return;
    };
    assert!(!doc.tree.is_empty());
    for node in doc.tree.nodes() {
        assert!(node.range.start <= data.len() as u64);
        assert!(node.range.end() <= data.len() as u64);
    }
    for fact in &doc.facts {
        let node = doc
            .tree
            .try_get(fact.node)
            .expect("fact refers to a tree node");
        assert!(node.range.end() <= data.len() as u64);
    }
});
