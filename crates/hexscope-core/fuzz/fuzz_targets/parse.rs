#![no_main]

use libfuzzer_sys::fuzz_target;

// Every format through the dispatcher: PNG, JPEG with EXIF, and unknown.
fuzz_target!(|data: &[u8]| {
    let doc = hexscope_core::parse(data);
    assert!(!doc.tree().is_empty());
});
