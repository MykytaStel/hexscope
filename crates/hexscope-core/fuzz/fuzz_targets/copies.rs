#![no_main]

use hexscope_core::video::Gap;
use libfuzzer_sys::fuzz_target;

// The copies hexscope makes of a file — clean, repaired, a PDF with areas
// blacked out, a movie given without its media — never panic, whatever the
// file. The areas and gaps come from the input's first bytes, the file from
// the rest.
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
    let gaps: Vec<Gap> = head
        .chunks(4)
        .map(|c| {
            let b = |i: usize| u64::from(*c.get(i).unwrap_or(&0));
            Gap {
                at: b(0) * 4,
                len: b(1) << (b(2) % 64),
            }
        })
        .filter(|g| g.len > 0)
        .collect();
    let _ = hexscope_core::video::parse_video_gapped(file, &gaps);
    let _ = hexscope_core::clean::clean_video_gapped(file, &gaps);
});
