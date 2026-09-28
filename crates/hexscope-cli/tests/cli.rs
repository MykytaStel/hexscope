//! The command line, run as a person would run it.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_hexscope");

fn fixture(name: &str) -> String {
    format!(
        "{}/../hexscope-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .env_remove("GITHUB_ACTIONS")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_names_what_a_photo_gives_away_and_fails_on_it() {
    let (code, out, err) = run(&["check", &fixture("photo.jpg")]);
    assert_eq!(code, 1, "{err}");
    assert!(out.contains("jpeg"), "{out}");
    assert!(out.contains("reveals  location:"), "{out}");
    assert!(
        err.contains("read 1 file; 1 with something to fail on"),
        "{err}"
    );
    // Told to fail only on damage, it passes.
    let (code, ..) = run(&["check", "--fail-on", "damage", &fixture("photo.jpg")]);
    assert_eq!(code, 0);
    // And a kind of fact can be named.
    let (code, ..) = run(&["check", "--fail-on", "location", &fixture("photo.jpg")]);
    assert_eq!(code, 1);
}

#[test]
fn json_is_one_object_per_file() {
    let (code, out, _) = run(&[
        "check",
        "--json",
        "--fail-on",
        "none",
        &fixture("redacted.pdf"),
    ]);
    assert_eq!(code, 0);
    let line = out.lines().next().unwrap();
    assert!(line.starts_with("{\"file\":"), "{line}");
    assert!(line.contains("\"format\":\"pdf\""), "{line}");
    assert!(line.contains("\"concern\":\"hidden\""), "{line}");
    assert!(line.contains("\"kind\":\"covered\""), "{line}");
    assert!(line.ends_with("]}"), "{line}");
}

#[test]
fn a_folder_is_read_through_and_unknown_files_are_left_out() {
    let dir = std::env::temp_dir().join(format!("hexscope-cli-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::copy(fixture("photo.jpg"), dir.join("sub/photo.jpg")).unwrap();
    std::fs::write(dir.join("notes.txt"), "just text").unwrap();
    let (code, out, err) = run(&["check", dir.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(
        out.contains("photo.jpg") && !out.contains("notes.txt"),
        "{out}"
    );
    assert!(err.contains("read 1 file"), "{err}");

    // Clean it beside itself, then check the copy: nothing to fail on.
    let (code, out, _) = run(&["clean", dir.join("sub/photo.jpg").to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    let copy = dir.join("sub/photo-clean.jpg");
    assert!(copy.exists(), "{out}");
    let (code, ..) = run(&["check", copy.to_str().unwrap()]);
    assert_eq!(code, 0);
    // One file, and --out naming the copy: the copy is that file.
    let named = dir.join("out").join("shared.jpg");
    let (code, out, _) = run(&[
        "clean",
        "--out",
        named.to_str().unwrap(),
        dir.join("sub/photo.jpg").to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(named.is_file(), "{out}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn repair_saves_what_survived() {
    let dir = std::env::temp_dir().join(format!("hexscope-cli-repair-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut png = std::fs::read(fixture("pngsuite/basn2c08.png")).unwrap();
    png[29] ^= 0xFF;
    std::fs::write(dir.join("broken.png"), &png).unwrap();
    let (code, out, _) = run(&["repair", dir.join("broken.png").to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("corrected a checksum"), "{out}");
    let (code, ..) = run(&[
        "check",
        "--fail-on",
        "damage",
        dir.join("broken-repaired.png").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn github_annotations_and_usage_errors() {
    let out = Command::new(BIN)
        .args(["check", &fixture("photo.jpg")])
        .env("GITHUB_ACTIONS", "true")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("::warning file="), "{text}");
    assert!(
        text.contains("title=hexscope: reveals location::"),
        "{text}"
    );
    let (code, _, err) = run(&["check", "--nope"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown option --nope"));
    let (code, _, err) = run(&["check", "/no/such/file"]);
    assert_eq!(code, 2, "{err}");
    let (code, out, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("hexscope "));
}

#[test]
fn redact_takes_words_off_a_pdfs_pages() {
    let dir = std::env::temp_dir().join(format!("hexscope-cli-redact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture("chrome-highlight.pdf"), dir.join("letter.pdf")).unwrap();
    std::fs::copy(fixture("photo.jpg"), dir.join("photo.jpg")).unwrap();
    let pdf = dir.join("letter.pdf");
    let (code, out, err) = run(&["redact", "--text", "settlement", pdf.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("1 place blacked out"), "{out}");
    let copy = dir.join("letter-redacted.pdf");
    let texts = hexscope_core::pdf::page_texts(&std::fs::read(&copy).unwrap());
    let words: String = texts
        .iter()
        .flat_map(|p| p.glyphs.iter().map(|g| g.1.as_str()))
        .collect();
    assert!(!words.to_lowercase().contains("settlement"), "{words}");
    assert!(!words.is_empty());

    // Words that are not there make no copy and say so; a photo in the
    // folder is left alone.
    let (code, _, err) = run(&["redact", "--text", "no such words", dir.to_str().unwrap()]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("none of it is on the pages as text"), "{err}");
    assert!(!err.contains("photo.jpg"), "{err}");
    let (code, _, err) = run(&["redact", pdf.to_str().unwrap()]);
    assert_eq!(code, 2, "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sarif_is_one_log_for_code_scanning() {
    let (code, out, _) = run(&[
        "check",
        "--sarif",
        &fixture("redacted.pdf"),
        &fixture("photo.jpg"),
    ]);
    assert_eq!(code, 1);
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.starts_with("{\"version\":\"2.1.0\""), "{out}");
    assert!(out.contains("\"ruleId\":\"reveals/location\""), "{out}");
    assert!(out.contains("\"id\":\"reveals/location\""), "{out}");
    assert!(out.contains("\"byteOffset\":"), "{out}");
    assert!(out.contains("photo.jpg"));
}

/// `clean --in-place` replaces the file with its clean copy in one step:
/// the photo's place is gone, its permissions are kept, and nothing is left
/// beside it.
#[test]
fn clean_in_place_replaces_the_file_whole() {
    let dir = std::env::temp_dir().join(format!("hexscope-in-place-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let photo = dir.join("photo.jpg");
    std::fs::copy(fixture("photo.jpg"), &photo).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&photo, std::fs::Permissions::from_mode(0o640)).unwrap();
    }
    let before = std::fs::read(&photo).unwrap();
    let (code, out, err) = run(&["clean", "--in-place", photo.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let after = std::fs::read(&photo).unwrap();
    assert!(after.len() < before.len());
    let (_, out, _) = run(&["check", "--fail-on", "none", photo.to_str().unwrap()]);
    assert!(!out.contains("location"), "{out}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&photo).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }
    let left: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left, [std::ffi::OsString::from("photo.jpg")]);
    std::fs::remove_dir_all(&dir).unwrap();
}
