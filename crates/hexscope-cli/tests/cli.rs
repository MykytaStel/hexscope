//! The command line, run as a person would run it.

use serde_json::json;
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
fn check_reads_audio_tags_and_reports_uninspected_id3_metadata() {
    fn write_tag(path: &std::path::Path, id: &[u8; 4], payload: &[u8]) {
        let mut body = Vec::new();
        body.extend_from_slice(id);
        body.extend_from_slice(&[0, 0, 0, payload.len() as u8, 0, 0]);
        body.extend_from_slice(payload);
        let size = body.len();
        let mut tag = b"ID3\x04\0\0".to_vec();
        tag.extend_from_slice(&[
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        tag.extend_from_slice(&body);
        std::fs::write(path, tag).unwrap();
    }

    let dir = std::env::temp_dir().join(format!("hexscope-audio-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let complete = dir.join("complete.mp3");
    let partial = dir.join("partial.mp3");
    write_tag(&complete, b"TIT2", b"\x03song");
    write_tag(&partial, b"TXXX", b"\x03custom\0value");

    let complete_path = complete.to_str().unwrap();
    let partial_path = partial.to_str().unwrap();
    let (code, out, err) = run(&["check", partial_path]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("metadata not fully inspected"), "{out}");

    let (code, out, err) = run(&[
        "check",
        "--json",
        "--fail-on",
        "none",
        complete_path,
        partial_path,
    ]);
    assert_eq!(code, 0, "{err}");
    let rows: Vec<serde_json::Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2, "{out}");
    assert_eq!(rows[0]["format"], "mp3");
    assert!(rows[0].get("complete").is_none());
    assert!(rows[0]["reveals"].to_string().contains("song"));
    assert_eq!(rows[1]["complete"], false);
    assert!(rows[1]["reveals"].as_array().unwrap().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clean_mp3_copies_audio_frames_and_verifies_the_removed_title() {
    let dir = std::env::temp_dir().join(format!("hexscope-clean-mp3-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("song.mp3");
    let output = dir.join("song-clean.mp3");
    let mut body = b"TIT2".to_vec();
    body.extend_from_slice(&5u32.to_be_bytes());
    body.extend_from_slice(&[0, 0, 0]);
    body.extend_from_slice(b"song");
    let size = body.len();
    let mut mp3 = b"ID3\x03\0\0".to_vec();
    mp3.extend_from_slice(&[
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ]);
    mp3.extend_from_slice(&body);
    let audio = [0xff, 0xfb, 0x90, 0x64, 1, 2, 3, 4];
    mp3.extend_from_slice(&audio);
    std::fs::write(&source, mp3).unwrap();

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        source.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let receipt: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(receipt["operation_state"], "written");
    assert!(receipt["removed"].to_string().contains("title"));
    assert_eq!(receipt["unchecked"].as_array().unwrap().len(), 0);
    assert_eq!(std::fs::read(output).unwrap(), audio);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clean_flac_removes_vorbis_comments_and_verifies_the_retained_stream() {
    let dir = std::env::temp_dir().join(format!("hexscope-clean-flac-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("song.flac");
    let output = dir.join("song-clean.flac");

    let vendor = b"test encoder";
    let field = b"TITLE=Song";
    let mut comments = Vec::new();
    comments.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    comments.extend_from_slice(vendor);
    comments.extend_from_slice(&1u32.to_le_bytes());
    comments.extend_from_slice(&(field.len() as u32).to_le_bytes());
    comments.extend_from_slice(field);
    let frames = [0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4];
    let mut flac = b"fLaC".to_vec();
    flac.extend_from_slice(&[0, 0, 0, 34]);
    flac.extend_from_slice(&[0; 34]);
    flac.push(0x84);
    flac.extend_from_slice(&[
        ((comments.len() >> 16) & 0xff) as u8,
        ((comments.len() >> 8) & 0xff) as u8,
        (comments.len() & 0xff) as u8,
    ]);
    flac.extend_from_slice(&comments);
    flac.extend_from_slice(&frames);
    std::fs::write(&source, flac).unwrap();

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--json",
        "--out",
        output.to_str().unwrap(),
        source.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let receipt: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(receipt["operation_state"], "written");
    assert!(receipt["removed"].to_string().contains("title"));
    assert_eq!(receipt["unchecked"].as_array().unwrap().len(), 0);
    let cleaned = std::fs::read(output).unwrap();
    assert!(
        cleaned.ends_with(&frames),
        "compressed audio frames stay byte for byte"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

fn sha256_hex(bytes: &[u8]) -> String {
    hexscope_core::crypto::sha2::sha256(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    std::fs::write(dir.join("memo.psd"), b"8BPS").unwrap();
    std::fs::write(dir.join("a.HTML"), "<p>a</p>").unwrap();
    std::fs::write(dir.join("b.html"), "<p>b</p>").unwrap();
    let folder = dir.to_str().unwrap();

    let (code, _, err) = run(&["check", "--fail-on", "none", folder]);
    assert_eq!(code, 0);
    assert!(err.contains("read 1 file;"), "{err}");
    assert!(
        err.contains("3 not read") && err.contains("(2 .html, 1 .psd)"),
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

#[test]
fn clean_receipt_is_value_free_and_checks_exact_copy_bytes() {
    let dir = std::env::temp_dir().join(format!("hexscope-clean-receipt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("secret-source-name.jpg");
    let output = dir.join("clean-copy.jpg");
    let receipt_path = dir.join("verification.json");
    std::fs::copy(fixture("photo.jpg"), &input).unwrap();
    let source_values = hexscope_core::summary::summarize(&std::fs::read(&input).unwrap()).facts;

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--receipt",
        receipt_path.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(output.is_file());
    let receipt_text = std::fs::read_to_string(&receipt_path).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&receipt_text).unwrap();
    assert_eq!(receipt["schema"], "hexscope.clean-copy-receipt");
    assert_eq!(receipt["version"], 1);
    assert_eq!(receipt["hexscope_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(receipt["copy"]["format"], "jpeg");
    let copy_bytes = std::fs::read(&output).unwrap();
    assert_eq!(receipt["copy"]["sha256"], sha256_hex(&copy_bytes));
    assert_eq!(
        receipt["verification"]["removed"].as_array().unwrap().len(),
        8
    );
    assert!(
        receipt["verification"]["present"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        receipt["verification"]["unchecked"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!receipt_text.contains(input.to_str().unwrap()));
    assert!(!receipt_text.contains(output.to_str().unwrap()));
    assert!(!receipt_text.contains("secret-source-name.jpg"));
    for (_, value) in source_values {
        assert!(!receipt_text.contains(&value), "receipt exposed {value}");
    }

    let (code, out, err) = run(&[
        "receipt",
        "verify",
        output.to_str().unwrap(),
        receipt_path.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("bytes match"), "{out}");
    assert!(out.contains("does not verify who made it"), "{out}");
    assert!(out.contains("does not certify safety"), "{out}");

    let mut changed = copy_bytes.clone();
    changed[0] ^= 1;
    std::fs::write(&output, changed).unwrap();
    let (code, out, err) = run(&[
        "receipt",
        "verify",
        output.to_str().unwrap(),
        receipt_path.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("do not match"), "{out}");

    let other = dir.join("different-copy.jpg");
    std::fs::copy(fixture("photo.jpg"), &other).unwrap();
    let (code, out, err) = run(&[
        "receipt",
        "verify",
        other.to_str().unwrap(),
        receipt_path.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "{out}{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn receipt_verify_rejects_malformed_unsupported_and_unfingerprinted_receipts() {
    let dir = std::env::temp_dir().join(format!("hexscope-receipt-invalid-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join("copy.jpg");
    std::fs::copy(fixture("photo.jpg"), &copy).unwrap();
    let valid = json!({
        "schema": "hexscope.clean-copy-receipt",
        "version": 1,
        "hexscope_version": "0.8.0",
        "copy": {"format":"jpeg", "sha256":sha256_hex(&std::fs::read(&copy).unwrap())},
        "verification": {"schema_version":1,"removed":[],"present":[],"unchecked":[]}
    });
    let mut cases = Vec::new();
    cases.push(("malformed", "{".to_string()));
    let mut unknown = valid.clone();
    unknown["schema"] = json!("hexscope.unknown-receipt");
    cases.push(("unknown", unknown.to_string()));
    let mut future = valid.clone();
    future["version"] = json!(99);
    cases.push(("future", future.to_string()));
    let mut extra = valid.clone();
    extra["extra"] = json!("must reject");
    cases.push(("extra", extra.to_string()));
    let mut missing = valid.clone();
    missing["copy"]["sha256"] = serde_json::Value::Null;
    missing["copy"]["fingerprint_reason"] = json!("size_limit");
    cases.push(("missing-digest", missing.to_string()));
    let mut wrong_group = serde_json::from_str::<serde_json::Value>(&valid.to_string()).unwrap();
    wrong_group["verification"]["removed"] =
        json!([{"kind":"location","reason":"still_present","unexpected":false}]);
    cases.push(("wrong-reason-group", wrong_group.to_string()));
    let mut wrong_flag = serde_json::from_str::<serde_json::Value>(&valid.to_string()).unwrap();
    wrong_flag["verification"]["present"] =
        json!([{"kind":"location","reason":"unexpected_output","unexpected":false}]);
    cases.push(("unexpected-output-flag", wrong_flag.to_string()));
    let mut unexpected_flag =
        serde_json::from_str::<serde_json::Value>(&valid.to_string()).unwrap();
    unexpected_flag["verification"]["present"] =
        json!([{"kind":"location","reason":"still_present","unexpected":true}]);
    cases.push(("unexpected-flag", unexpected_flag.to_string()));

    for (name, contents) in cases {
        let receipt = dir.join(format!("{name}.json"));
        std::fs::write(&receipt, contents).unwrap();
        let (code, out, err) = run(&[
            "receipt",
            "verify",
            copy.to_str().unwrap(),
            receipt.to_str().unwrap(),
        ]);
        assert_eq!(code, 2, "{name}: {out}{err}");
        let expected_error = if name == "missing-digest" {
            "no copy fingerprint"
        } else {
            "malformed or unsupported receipt"
        };
        assert!(err.contains(expected_error), "{name}: {out}{err}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn clean_receipt_requires_verification_one_input_and_non_aliasing_destinations() {
    let dir = std::env::temp_dir().join(format!("hexscope-receipt-options-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let first = dir.join("first.jpg");
    let second = dir.join("second.jpg");
    std::fs::copy(fixture("photo.jpg"), &first).unwrap();
    std::fs::copy(fixture("photo.jpg"), &second).unwrap();
    let original = std::fs::read(&first).unwrap();
    let output = dir.join("copy.jpg");
    let receipt = dir.join("receipt.json");

    let (code, out, err) = run(&[
        "clean",
        "--receipt",
        receipt.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("--receipt requires --verify"), "{out}{err}");
    assert!(!output.exists());

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--receipt",
        receipt.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        first.to_str().unwrap(),
        second.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("--receipt accepts exactly one input file"),
        "{out}{err}"
    );
    assert!(!output.exists());

    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--receipt",
        first.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("--receipt cannot overwrite an input or clean copy"),
        "{out}{err}"
    );
    assert_eq!(std::fs::read(&first).unwrap(), original);
    assert!(!output.exists());

    std::fs::write(&output, b"existing output must stay intact").unwrap();
    let output_before = std::fs::read(&output).unwrap();
    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--receipt",
        output.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("clean-copy output already exists"),
        "{out}{err}"
    );
    assert_eq!(std::fs::read(&output).unwrap(), output_before);

    std::fs::write(&receipt, b"existing receipt must stay intact").unwrap();
    let receipt_before = std::fs::read(&receipt).unwrap();
    let new_output = dir.join("new-copy.jpg");
    let (code, out, err) = run(&[
        "clean",
        "--verify",
        "--receipt",
        receipt.to_str().unwrap(),
        "--out",
        new_output.to_str().unwrap(),
        first.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("receipt destination already exists"),
        "{out}{err}"
    );
    assert_eq!(std::fs::read(&receipt).unwrap(), receipt_before);
    assert!(!new_output.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn batch_default_receipt_bytes_remain_v1_and_hashes_are_opt_in() {
    let dir = std::env::temp_dir().join(format!(
        "hexscope-batch-copy-receipts-{}",
        std::process::id()
    ));
    let inputs = dir.join("inputs");
    let output = dir.join("default");
    std::fs::create_dir_all(&inputs).unwrap();
    std::fs::copy(fixture("photo.jpg"), inputs.join("secret-name.jpg")).unwrap();

    let (code, out, err) = run(&[
        "batch",
        "--out",
        output.to_str().unwrap(),
        inputs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}{err}");
    let receipt_bytes = std::fs::read(output.join("hexscope-report.json")).unwrap();
    let expected_default = json!({
        "schema":"hexscope.batch-receipt",
        "version":1,
        "total":1,
        "written":1,
        "failed":0,
        "removed":8,
        "present":0,
        "unchecked":0,
        "files":[{
            "index":1,
            "operation_state":"written",
            "verification":{
                "schema_version":1,
                "removed":[
                    {"kind":"location","reason":"removed","unexpected":false},
                    {"kind":"camera","reason":"removed","unexpected":false},
                    {"kind":"lens","reason":"removed","unexpected":false},
                    {"kind":"serial","reason":"removed","unexpected":false},
                    {"kind":"owner","reason":"removed","unexpected":false},
                    {"kind":"taken","reason":"removed","unexpected":false},
                    {"kind":"software","reason":"removed","unexpected":false},
                    {"kind":"thumbnail","reason":"removed","unexpected":false}
                ],
                "present":[],"unchecked":[]
            }
        }]
    });
    assert_eq!(
        receipt_bytes,
        serde_json::to_vec_pretty(&expected_default).unwrap()
    );
    let default_text = String::from_utf8_lossy(&receipt_bytes);
    assert!(!default_text.contains("sha256"));
    assert!(!default_text.contains("secret-name.jpg"));
    assert!(!default_text.contains(dir.to_str().unwrap()));

    let opt_inputs = dir.join("opt-in-inputs");
    let opt_output = dir.join("opt-in-output");
    std::fs::create_dir_all(&opt_inputs).unwrap();
    std::fs::copy(fixture("photo.jpg"), opt_inputs.join("a-private-photo.jpg")).unwrap();
    std::fs::write(
        opt_inputs.join("z-unsupported-secret.txt"),
        "PRIVATE_TEXT_SENTINEL",
    )
    .unwrap();
    let (code, out, err) = run(&[
        "batch",
        "--include-copy-hashes",
        "--out",
        opt_output.to_str().unwrap(),
        opt_inputs.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "{out}{err}");
    let text = std::fs::read_to_string(opt_output.join("hexscope-report.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["schema"], "hexscope.batch-clean-copy-receipt");
    assert_eq!(report["version"], 1);
    let files = report["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    let written = files
        .iter()
        .find(|row| row["operation_state"] == "written")
        .unwrap();
    let clean_copy = opt_output.join("000001-clean.jpg");
    assert_eq!(
        written["receipt"]["copy"]["sha256"],
        sha256_hex(&std::fs::read(clean_copy).unwrap())
    );
    let failed = files
        .iter()
        .find(|row| row["operation_state"] == "not_created")
        .unwrap();
    assert!(failed["receipt"].is_null());
    assert!(!text.contains("a-private-photo.jpg"));
    assert!(!text.contains("z-unsupported-secret.txt"));
    assert!(!text.contains(opt_inputs.to_str().unwrap()));
    assert!(!text.contains("PRIVATE_TEXT_SENTINEL"));
    std::fs::remove_dir_all(&dir).unwrap();
}
