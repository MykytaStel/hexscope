#![no_main]

use libfuzzer_sys::fuzz_target;

// The copies hexscope makes of a file — clean, repaired, and a PDF with
// areas blacked out — never panic, whatever the file. The areas come from
// the input's first bytes, the file from the rest.
fuzz_target!(|data: &[u8]| {
    let (head, file) = data.split_at(data.len().min(16));
    let areas: Vec<(u32, [f64; 4])> = head
        .chunks(4)
        .map(|c| {
            let v = |i: usize| f64::from(*c.get(i).unwrap_or(&0)) * 4.0;
            (1, [v(0), v(1), v(0) + v(2), v(1) + v(3)])
        })
        .collect();
    let _ = hexscope_core::clean::clean(file);
    let _ = hexscope_core::repair::repair(file);
    let _ = hexscope_core::clean::redact(file, &areas, &[]);
    let _ = hexscope_core::pdf::page_texts(file);
});
