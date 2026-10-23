#![no_main]

use libfuzzer_sys::fuzz_target;

// In CI, real and damaged format samples seed this dispatcher target.
fuzz_target!(|data: &[u8]| {
    let doc = hexscope_core::parse(data);
    assert!(!doc.tree().is_empty());
});
