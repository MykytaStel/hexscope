use std::{fs, process::Command};
#[test]
fn mosaic_export_preserves_unchecked_coverage_and_omits_personal_values() {
    let dir = std::env::temp_dir().join(format!("hexscope-mosaic-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for i in 1..=2 {
        fs::write(
            dir.join(format!("{i}.png")),
            include_bytes!("../../hexscope-core/tests/fixtures/photo.png"),
        )
        .unwrap();
    }
    fs::write(dir.join("unknown.bin"), b"unknown").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_hexscope"))
        .args(["mosaic", "--json"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["input_total"], 3);
    assert_eq!(report["not_checked_indexes"], serde_json::json!([3]));
    assert_eq!(report["serial_groups"], serde_json::json!([[1, 2]]));
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(!text.contains("HX-000042"));
    assert!(!text.contains(".png"));
    fs::remove_dir_all(dir).unwrap();
}
