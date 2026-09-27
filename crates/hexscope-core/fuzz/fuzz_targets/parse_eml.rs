#![no_main]

use libfuzzer_sys::fuzz_target;

// Emails straight to their parser, past the check that a file reads as one:
// header lines, MIME parts, encodings, and every attachment opened.
fuzz_target!(|data: &[u8]| {
    let doc = hexscope_core::eml::parse_eml(data);
    assert!(!doc.tree.is_empty());
    for i in 0..doc.attachments.len() {
        let _ = hexscope_core::eml::attachment_bytes(data, i);
    }
});
