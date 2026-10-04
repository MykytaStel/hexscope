//! The command line, run as a person would run it.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_hexscope");

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn core_fixture(name: &str) -> String {
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

#[test]
fn clean_verify_pdf_metadata_removes_compact_info_and_xmp() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-pdf-compact-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("compact.pdf");
    let output = dir.join("compact-clean.pdf");
    std::fs::copy(core_fixture("compact.pdf"), &input).unwrap();
    let source_values = hexscope_core::summary::summarize(&std::fs::read(&input).unwrap()).facts;

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);

    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("\"operation_state\":\"written\""), "{out}");
    let removed = out
        .split("\"removed\":")
        .nth(1)
        .unwrap_or_default()
        .split(",\"present\":")
        .next()
        .unwrap_or_default();
    assert!(removed.starts_with("[{"), "{out}");
    for kind in [
        "author",
        "title",
        "application",
        "producer",
        "created",
        "history",
    ] {
        assert!(removed.contains(&format!("\"kind\":\"{kind}\"")), "{out}");
    }
    assert!(out.contains("\"present\":[]"), "{out}");
    assert!(out.contains("\"unchecked\":[]"), "{out}");
    for (_, value) in source_values {
        assert!(
            !out.contains(&value),
            "verification exposed a finding value: {value}"
        );
    }
    assert!(output.is_file());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_verify_pdf_metadata_leaves_revision_findings_unchecked() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-pdf-report-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("report.pdf");
    let output = dir.join("report-clean.pdf");
    std::fs::copy(core_fixture("report.pdf"), &input).unwrap();
    let source_values = hexscope_core::summary::summarize(&std::fs::read(&input).unwrap()).facts;

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);

    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("\"operation_state\":\"written\""), "{out}");
    let removed = out
        .split("\"removed\":")
        .nth(1)
        .unwrap_or_default()
        .split(",\"present\":")
        .next()
        .unwrap_or_default();
    for kind in [
        "author",
        "title",
        "application",
        "producer",
        "created",
        "modified",
        "history",
    ] {
        assert!(removed.contains(&format!("\"kind\":\"{kind}\"")), "{out}");
    }
    let unchecked = out.split("\"unchecked\":").nth(1).unwrap_or_default();
    for kind in ["updates", "earlier"] {
        assert!(unchecked.contains(&format!("\"kind\":\"{kind}\"")), "{out}");
    }
    assert!(
        unchecked.contains("\"reason\":\"coverage_incomplete\""),
        "{out}"
    );
    for (_, value) in source_values {
        assert!(
            !out.contains(&value),
            "verification exposed a finding value: {value}"
        );
    }
    assert!(output.is_file());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_verify_checks_the_written_copy_and_emits_value_free_json() {
    let dir = std::env::temp_dir().join(format!("hexscope-clean-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("verified.jpg");
    let source = std::fs::read(fixture("photo.jpg")).unwrap();
    let source_values = hexscope_core::summary::summarize(&source).facts;

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        &fixture("photo.jpg"),
    ]);

    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("\"schema_version\":1"), "{out}");
    assert!(out.contains("\"operation_state\":\"written\""), "{out}");
    assert!(out.contains("\"kind\":\"location\""), "{out}");
    assert!(out.contains("\"reason\":\"coverage_incomplete\""), "{out}");
    for (_, value) in source_values {
        assert!(
            !out.contains(&value),
            "verification exposed a finding value: {value}"
        );
    }
    let checked = hexscope_core::summary::summarize(&std::fs::read(&output).unwrap());
    assert!(!checked.facts.iter().any(|(kind, _)| *kind == "location"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_verify_human_output_names_kinds_without_values() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-human-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("human.jpg");
    let source_values =
        hexscope_core::summary::summarize(&std::fs::read(fixture("photo.jpg")).unwrap()).facts;

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--out",
        output.to_str().unwrap(),
        &fixture("photo.jpg"),
    ]);

    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("Removed:\n    Location"), "{out}");
    assert!(out.contains("    Camera\n"), "{out}");
    assert!(out.contains("    Camera serial\n"), "{out}");
    assert!(out.contains("    Owner\n"), "{out}");
    assert!(
        out.contains("Not checked:\n    Lens — Hexscope cannot yet confirm whether this JPEG finding was removed."),
        "{out}"
    );
    assert!(out.contains("Still present:\n    (none)"), "{out}");
    assert!(!out.contains("coverage_incomplete"), "{out}");
    assert!(!out.contains(" (removed)"), "{out}");
    for (_, value) in source_values {
        assert!(
            !out.contains(&value),
            "verification exposed a finding value: {value}"
        );
    }
    assert!(output.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_verify_handles_batch_outputs_and_in_place_replacement() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-batch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let photo = fixture("photo.jpg");
    let no_metadata = fixture("pngsuite/basn2c08.png");

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        dir.to_str().unwrap(),
        &photo,
        &no_metadata,
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(
        out.lines()
            .any(|line| line.contains("\"operation_state\":\"written\"")),
        "{out}"
    );
    assert!(
        out.lines()
            .any(|line| line.contains("\"operation_state\":\"not_created\"")),
        "{out}"
    );
    assert!(out.contains("\"output\":null"), "{out}");
    assert!(dir.join("photo-clean.jpg").exists());

    let in_place = dir.join("in-place.jpg");
    std::fs::copy(&photo, &in_place).unwrap();
    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--in-place",
        in_place.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("\"source\":\""), "{out}");
    assert!(out.contains("\"output\":\""), "{out}");
    assert!(
        !hexscope_core::summary::summarize(&std::fs::read(&in_place).unwrap())
            .facts
            .iter()
            .any(|(kind, _)| *kind == "location")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_verify_returns_one_when_cleaning_made_no_copy() {
    let (code, out, _) = run(&[
        "clean",
        "--verify",
        "--json",
        &fixture("pngsuite/basn2c08.png"),
    ]);

    assert_eq!(code, 1);
    assert!(out.contains("\"operation_state\":\"not_created\""), "{out}");
    assert!(out.contains("\"output\":null"), "{out}");
    assert!(out.contains("\"reason\":\"verification_skipped\""), "{out}");
}

#[test]
fn clean_verify_returns_one_when_directory_contains_no_files() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-empty-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let (code, out, err) = run(&["clean", "--verify", "--json", dir.to_str().unwrap()]);

    assert_eq!(code, 1, "{out}{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("0 copies made"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn verify_is_only_an_option_for_clean() {
    let (code, _, err) = run(&["check", "--verify", &fixture("photo.jpg")]);
    assert_eq!(code, 2);
    assert!(err.contains("only with clean"), "{err}");

    let (code, _, err) = run(&["repair", "--verify", &fixture("photo.jpg")]);
    assert_eq!(code, 2);
    assert!(err.contains("only with clean"), "{err}");

    let (code, _, err) = run(&[
        "redact",
        "--verify",
        "--text",
        "secret",
        &fixture("photo.jpg"),
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("only with clean"), "{err}");
}

#[test]
fn clean_verify_reports_an_unwritable_destination_as_io_error() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-write-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let parent = dir.join("not-a-directory");
    std::fs::write(&parent, b"occupied").unwrap();
    let output = parent.join("blocked.jpg");

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        &fixture("photo.jpg"),
    ]);

    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("not-a-directory"), "{err}");
    assert!(!err.contains("unknown option --verify"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A phone's video with its picture and sound grown to `size` bytes.
fn long_video(size: usize) -> Vec<u8> {
    let src = std::fs::read(fixture("iphone.mov")).unwrap();
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= src.len() {
        let len = u32::from_be_bytes(src[at..at + 4].try_into().unwrap()) as usize;
        if &src[at + 4..at + 8] == b"mdat" {
            out.extend_from_slice(&(8 + size as u32).to_be_bytes());
            out.extend_from_slice(&src[at + 4..at + len]);
            out.extend((len..8 + size).map(|i| i as u8));
        } else {
            out.extend_from_slice(&src[at..at + len]);
        }
        at += len;
    }
    out
}

#[test]
fn a_long_video_is_checked_and_cleaned_without_its_media_in_memory() {
    let dir = std::env::temp_dir().join(format!("hexscope-long-video-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let video = dir.join("long.mov");
    let data = long_video(80 << 20);
    std::fs::write(&video, &data).unwrap();

    let (code, out, _) = run(&["check", video.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.contains("reveals  location:"), "{out}");

    let (code, out, err) = run(&["clean", video.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}{err}");
    let copy = std::fs::read(dir.join("long-clean.mov")).unwrap();
    assert_eq!(copy.len(), data.len());
    let media = 36..data.len() - 1389;
    assert!(copy[media.clone()] == data[media]);
    let (code, out, _) = run(&["check", dir.join("long-clean.mov").to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    std::fs::remove_dir_all(&dir).unwrap();
}
