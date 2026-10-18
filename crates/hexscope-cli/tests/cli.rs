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

fn pdf_with_unreadable_page_content() -> Vec<u8> {
    let page_content = b"BT (hidden page text) Tj ET";
    let objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec(),
        [
            format!(
                "<< /Filter /MadeUpDecode /Length {} >>\nstream\n",
                page_content.len()
            )
            .as_bytes(),
            page_content,
            b"\nendstream",
        ]
        .concat(),
    ];
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f\r\n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n\r\n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
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

#[cfg(unix)]
#[test]
fn github_annotations_escape_untrusted_paths_and_do_not_print_finding_values() {
    use std::os::unix::ffi::OsStrExt;

    let dir = std::env::temp_dir().join(format!("hexscope-github-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let name = "photo%\r\n::warning file=owned,title=Injected.jpg";
    let path = dir.join(std::ffi::OsStr::from_bytes(name.as_bytes()));
    std::fs::copy(fixture("photo.jpg"), &path).unwrap();
    let output = Command::new(BIN)
        .args(["check", "--fail-on", "location", path.to_str().unwrap()])
        .env("GITHUB_ACTIONS", "true")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{stdout}{stderr}");
    assert!(
        stdout.contains("%25%0D%0A%3A%3Awarning file=owned%2Ctitle=Injected.jpg"),
        "{stdout}"
    );
    assert!(!stdout.contains("\n::warning file=owned"), "{stdout}");
    assert!(!stdout.contains("48.8584"), "{stdout}");
    assert!(!stdout.contains("HX-000042"), "{stdout}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn check_reports_symlinks_in_folders_instead_of_silently_skipping_them() {
    use std::os::unix::fs::symlink;

    let dir = std::env::temp_dir().join(format!("hexscope-folder-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("nested")).unwrap();
    std::fs::copy(fixture("photo.jpg"), dir.join("nested/photo.jpg")).unwrap();
    symlink(fixture("photo.jpg"), dir.join("nested/photo-link.jpg")).unwrap();

    let (code, out, err) = run(&["check", dir.to_str().unwrap()]);

    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("symbolic links are not followed"), "{err}");
    assert!(!err.contains("read 1 file"), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
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
    assert!(
        err.contains("1 not read") && err.contains("(1 .txt)"),
        "{err}"
    );

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

/// Files of a format hexscope does not read are counted and named by kind,
/// never passed over in silence: a check that skipped them is not complete.
#[test]
fn files_it_does_not_read_are_counted_by_kind() {
    let dir = std::env::temp_dir().join(format!("hexscope-unread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture("photo.jpg"), dir.join("photo.jpg")).unwrap();
    std::fs::write(dir.join("memo.mp3"), b"ID3\x03\x00\x00\x00\x00\x00\x00").unwrap();
    std::fs::write(dir.join("a.HTML"), "<p>a</p>").unwrap();
    std::fs::write(dir.join("b.html"), "<p>b</p>").unwrap();
    let folder = dir.to_str().unwrap();

    let (code, _, err) = run(&["check", "--fail-on", "none", folder]);
    assert_eq!(code, 0);
    assert!(err.contains("read 1 file;"), "{err}");
    assert!(
        err.contains("3 not read") && err.contains("(2 .html, 1 .mp3)"),
        "{err}"
    );

    let (_, out, _) = run(&["check", "--all", "--fail-on", "none", folder]);
    assert_eq!(
        out.matches("not read: hexscope does not read this format")
            .count(),
        3,
        "{out}"
    );

    for machine in ["--json", "--sarif"] {
        let out = Command::new(BIN)
            .args(["check", machine, "--fail-on", "none", folder])
            .env("GITHUB_ACTIONS", "true")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stdout.contains("::notice"), "{stdout}");
        assert!(
            stderr.contains("::notice title=hexscope: not read::3 files"),
            "{stderr}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// In GitHub Actions the run's page gets a table: a row per file with
/// findings, kinds and not values, the files not read counted.
#[test]
fn github_step_summary_lists_every_file_with_findings_without_values() {
    let dir = std::env::temp_dir().join(format!("hexscope-summary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture("photo.jpg"), dir.join("photo.jpg")).unwrap();
    std::fs::write(dir.join("a|b.psd"), b"8BPS").unwrap();
    let summary = dir.join("summary.md");
    std::fs::write(&summary, "earlier step\n").unwrap();
    let out = Command::new(BIN)
        .args([
            "check",
            "--fail-on",
            "location",
            dir.join("photo.jpg").to_str().unwrap(),
            dir.join("a|b.psd").to_str().unwrap(),
        ])
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_STEP_SUMMARY", &summary)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = std::fs::read_to_string(&summary).unwrap();
    assert!(text.starts_with("earlier step\n### hexscope\n"), "{text}");
    assert!(
        text.contains("Read 1 file; 1 with something to fail on (location); 1 not read"),
        "{text}"
    );
    let row = text.lines().find(|l| l.contains("photo.jpg")).unwrap();
    assert!(
        row.contains("| jpeg | location, camera,") && row.ends_with("| yes |"),
        "{row}"
    );
    assert!(
        !text.contains("48.858"),
        "a value reached the summary: {text}"
    );

    // Outside Actions, nothing is written.
    std::fs::remove_file(&summary).unwrap();
    let (_, _, _) = run(&["check", dir.join("photo.jpg").to_str().unwrap()]);
    assert!(!summary.exists());
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
    let cleaned = hexscope_core::summary::summarize(&std::fs::read(&output).unwrap());
    for kind in ["updates", "earlier"] {
        assert!(
            !cleaned.facts.iter().any(|(found, _)| *found == kind),
            "the old revision finding {kind} should be absent from the written copy"
        );
    }
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

    // Every finding of the photo is confirmed removed: nothing to fail on.
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.contains("\"schema_version\":1"), "{out}");
    assert!(out.contains("\"operation_state\":\"written\""), "{out}");
    assert!(
        out.contains("\"kind\":\"location\",\"reason\":\"removed\""),
        "{out}"
    );
    assert!(!out.contains("\"reason\":\"coverage_incomplete\""), "{out}");
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

    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Removed:\n    Location"), "{out}");
    assert!(out.contains("    Camera\n"), "{out}");
    assert!(out.contains("    Camera serial\n"), "{out}");
    assert!(out.contains("    Owner\n"), "{out}");
    assert!(out.contains("    Lens\n"), "{out}");
    assert!(out.contains("Not checked:\n    (none)"), "{out}");
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
    assert_eq!(code, 0, "{out}{err}");
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
fn clean_verify_returns_one_when_a_finding_is_not_checked() {
    // A PDF's earlier versions are outside what a copy is confirmed to lose.
    let dir = std::env::temp_dir().join(format!("hexscope-verify-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        dir.join("report.pdf").to_str().unwrap(),
        &core_fixture("report.pdf"),
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"reason\":\"coverage_incomplete\""), "{out}");
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
fn clean_verify_skips_verification_when_a_pdf_page_stream_cannot_be_decoded() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-clean-verify-incomplete-page-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("incomplete.pdf");
    let output = dir.join("cleaned.pdf");
    std::fs::write(&input, pdf_with_unreadable_page_content()).unwrap();

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
    assert!(out.contains("\"operation_state\":\"not_created\""), "{out}");
    assert!(out.contains("\"reason\":\"verification_skipped\""), "{out}");
    assert!(out.contains("\"output\":null"), "{out}");
    assert!(err.contains("PDF checks are incomplete"), "{err}");
    assert!(
        !output.exists(),
        "an unreadable page must not produce a copy"
    );
    std::fs::remove_dir_all(&dir).unwrap();
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

#[test]
fn a_name_that_says_another_format_is_named_and_can_fail_the_check() {
    let dir = std::env::temp_dir().join(format!("hexscope-misnamed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("contract.pdf");
    std::fs::copy(core_fixture("report.docx"), &pdf).unwrap();
    let pdf = pdf.to_str().unwrap();

    let (code, out, _) = run(&["check", pdf]);
    assert_eq!(code, 1, "it still reveals its author");
    assert!(
        out.contains("misnamed named .pdf, but it is a Word document"),
        "{out}"
    );
    assert!(out.contains("renaming it to .docx"), "{out}");

    let (_, json, _) = run(&["check", "--json", pdf]);
    assert!(
        json.contains(r#""misnamed":{"ext":"pdf","is":"a Word document","fits":".docx"}"#),
        "{json}"
    );

    let (code, _, _) = run(&[
        "check",
        "--fail-on",
        "misnamed",
        &core_fixture("report.docx"),
    ]);
    assert_eq!(code, 0, "a name that fits does not fail");
    let (code, _, _) = run(&["check", "--fail-on", "misnamed", pdf]);
    assert_eq!(code, 1);
    std::fs::remove_dir_all(&dir).ok();
}

/// "Clean with hexscope" goes where this system's file manager looks for it,
/// runs this very program on what is chosen, and goes away again.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn menu_adds_clean_with_hexscope_and_removes_it() {
    let home = std::env::temp_dir().join(format!("hexscope-menu-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let menu = |args: &[&str]| {
        let out = Command::new(BIN)
            .args(args)
            .env("HOME", &home)
            .env("XDG_DATA_HOME", home.join("data"))
            .output()
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    };
    let (code, said) = menu(&["menu"]);
    assert_eq!(code, 0, "{said}");
    let script = if cfg!(target_os = "macos") {
        home.join("Library/Services/Clean with hexscope.workflow/Contents/document.wflow")
    } else {
        home.join("data/nautilus/scripts/Clean with hexscope")
    };
    let text = std::fs::read_to_string(&script).unwrap();
    assert!(text.contains("clean --verify"), "{text}");
    assert!(
        text.contains(BIN),
        "the entry runs this program by its full path"
    );
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "GNOME Files runs only executable scripts"
        );
    }
    let (code, said) = menu(&["menu", "--remove"]);
    assert_eq!(code, 0);
    assert!(said.starts_with("Removed"), "{said}");
    assert!(!script.exists());
    let (code, said) = menu(&["menu", "--remove"]);
    assert_eq!(code, 0);
    assert!(said.contains("was not there"), "{said}");
    std::fs::remove_dir_all(home).unwrap();
}
