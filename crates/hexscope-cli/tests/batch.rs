use std::{fs, process::Command};

#[test]
fn batch_handles_duplicate_basenames_and_reports_unsupported_file() {
    let dir = std::env::temp_dir().join(format!("hexscope-batch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    let original = include_bytes!("../../hexscope-core/tests/fixtures/photo.png");
    fs::write(dir.join("a/photo.png"), original).unwrap();
    fs::write(dir.join("b/photo.png"), original).unwrap();
    fs::write(dir.join("unknown.bin"), b"not a known format").unwrap();
    let out = dir.join("copies");
    let result = Command::new(env!("CARGO_BIN_EXE_hexscope"))
        .args(["batch", "--out"])
        .arg(&out)
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{:?}", result);
    assert!(out.join("000001-clean.png").exists());
    assert!(out.join("000002-clean.png").exists());
    let receipt = fs::read_to_string(out.join("hexscope-report.json")).unwrap();
    assert!(receipt.contains("\"written\": 2"));
    assert!(receipt.contains("not_created"));
    assert!(!receipt.contains("HX-000042"));
    assert!(!receipt.contains("photo.png"));
    assert_eq!(fs::read(dir.join("a/photo.png")).unwrap(), original);
    // Existing output is excluded from traversal and a second run refuses to overwrite copies.
    let retry = Command::new(env!("CARGO_BIN_EXE_hexscope"))
        .args(["batch", "--out"])
        .arg(&out)
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!retry.status.success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn clean_rejects_duplicate_targets_before_writing() {
    let dir = std::env::temp_dir().join(format!("hexscope-collision-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::create_dir_all(dir.join("b")).unwrap();
    for folder in ["a", "b"] {
        fs::write(
            dir.join(folder).join("photo.png"),
            include_bytes!("../../hexscope-core/tests/fixtures/photo.png"),
        )
        .unwrap();
    }
    let out = dir.join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_hexscope"))
        .args(["clean", "--out"])
        .arg(&out)
        .arg(dir.join("a"))
        .arg(dir.join("b"))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(!out.join("photo-clean.png").exists());
    fs::remove_dir_all(dir).unwrap();
}
