//! The sentences and citations the browser's builds leave out, written for
//! the page to fetch: one JSON file per build, `{ "key": "sentence" }` or
//! `{ "key": ["sentence", "citation", "link"] }`, holding what that build
//! can give — `explain-media.json`, and `explain-pdf.json` and
//! `explain-docs.json` each with the media's too.
//!
//!   cargo run -p hexscope-wasm --example explanations -- apps/web/src/explanations

use std::fmt::Write as _;
use std::path::PathBuf;

fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("the folder to write into"));
    std::fs::create_dir_all(&dir).unwrap();
    let every = hexscope_core::docs::every();
    assert!(
        every.iter().all(|(_, d)| !d.text.is_empty()),
        "built without the explain feature: no sentences to write"
    );
    for build in ["media", "pdf", "docs"] {
        let mut docs: Vec<_> = every
            .iter()
            .filter(|(family, _)| *family == "media" || *family == build)
            .map(|(_, d)| (d.key, d.text, d.spec.map(|s| (s.cite, s.url))))
            .collect();
        docs.sort_unstable();
        docs.dedup();
        let mut out = String::from("{");
        for (i, (key, text, spec)) in docs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(out, "\n\"{key}\":");
            match spec {
                None => json_string(&mut out, text),
                Some((cite, url)) => {
                    out.push('[');
                    json_string(&mut out, text);
                    out.push(',');
                    json_string(&mut out, cite);
                    out.push(',');
                    json_string(&mut out, url);
                    out.push(']');
                }
            }
        }
        out.push_str("\n}\n");
        std::fs::write(dir.join(format!("explain-{build}.json")), out).unwrap();
    }
    // Written by the build, like the WebAssembly beside it: not kept in git.
    std::fs::write(dir.join(".gitignore"), "*\n").unwrap();
}
