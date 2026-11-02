#![no_main]

use libfuzzer_sys::fuzz_target;

// A clean copy is a file people will share: it must read back as the same
// format, clean again without panicking, and compare against its source.
fuzz_target!(|data: &[u8]| {
    let Ok(copy) = hexscope_core::clean::clean(data) else {
        return;
    };
    let source = hexscope_core::summary::summarize(data);
    let output = hexscope_core::summary::summarize(&copy.bytes);
    assert_eq!(
        source.format, output.format,
        "a clean copy keeps its format"
    );
    assert!(!hexscope_core::parse(&copy.bytes).tree().is_empty());
    let _ = hexscope_core::clean::clean(&copy.bytes);
    let source = hexscope_core::verification::snapshot(&source);
    let output = hexscope_core::verification::snapshot(&output);
    let _ = hexscope_core::verification::compare(&source, &output);
});
