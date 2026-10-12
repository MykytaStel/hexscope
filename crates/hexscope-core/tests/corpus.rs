//! Files made the way people make them — by textutil, ffmpeg, sips,
//! cupsfilter and the Finder's compress (ditto), with made-up names in them —
//! and what hexscope says about each: its format, whether it read whole, its
//! problems and its findings. Any change to that shows up here as a diff of
//! the snapshot, to be read before it is accepted.

use hexscope_core::summary::summarize;
use std::fmt::Write as _;
use std::path::Path;

#[test]
fn what_hexscope_says_about_real_kinds_of_files() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut out = String::new();
    for name in names {
        let s = summarize(&std::fs::read(dir.join(&name)).unwrap());
        let _ = writeln!(
            out,
            "{name}: {}{}",
            s.format,
            if s.complete { "" } else { ", not read whole" }
        );
        for p in &s.problems {
            let _ = writeln!(out, "  ! {:?}: {}", p.concern, p.label);
        }
        for (kind, text) in &s.facts {
            let _ = writeln!(out, "  {kind}: {text}");
        }
    }
    insta::assert_snapshot!(out);
}
