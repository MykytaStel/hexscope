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
