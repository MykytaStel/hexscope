#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let doc = hexscope_core::png::parse_png(data);
    // The only assertion that matters: we got here without panicking, and the
    // tree always describes something.
    assert!(!doc.tree.is_empty());
});
