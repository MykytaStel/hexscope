//! A small module to look inside: it counts the words in a text.

use std::slice;

/// Counts the words in `len` bytes at `ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn count_words(ptr: *const u8, len: usize) -> u32 {
    let text = unsafe { slice::from_raw_parts(ptr, len) };
    let text = std::str::from_utf8(text).expect("the text is not UTF-8");
    text.split_whitespace().count() as u32
}

/// The `n`-th word's length; panics past the last word.
#[unsafe(no_mangle)]
pub extern "C" fn word_length(ptr: *const u8, len: usize, n: usize) -> u32 {
    let text = unsafe { slice::from_raw_parts(ptr, len) };
    let words: Vec<&[u8]> = text.split(|b| b.is_ascii_whitespace()).filter(|w| !w.is_empty()).collect();
    words[n].len() as u32
}

// What it asks of the page that runs it: the time, and randomness.
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn date_now() -> f64;
    fn random_get(buf: *mut u8, len: usize) -> i32;
}

/// A word picked at random, as its index; the time it took goes to `took`.
#[unsafe(no_mangle)]
pub extern "C" fn pick_word(ptr: *const u8, len: usize, took: *mut f64) -> u32 {
    let start = unsafe { date_now() };
    let words = count_words(ptr, len).max(1);
    let mut r = [0u8; 4];
    unsafe { random_get(r.as_mut_ptr(), r.len()) };
    let pick = u32::from_le_bytes(r) % words;
    unsafe { *took = date_now() - start };
    pick
}
