use std::{fs, process::Command};
#[test]
fn film_applies_one_recipe_to_folder_without_touching_sources() {
    let dir = std::env::temp_dir().join(format!("hexscope-film-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("input")).unwrap();
    let recipe = r#"{"schema":"hexscope.film-recipe","version":1,"algorithm":"srgb-density-v1","settings":{"mode":"color-negative","crop":[0.1,0.1,0.1,0.1],"exposure":0,"contrast":1,"baseColor":[220,140,85]}}"#;
    fs::write(dir.join("recipe.json"), recipe).unwrap();
    let bytes = include_bytes!("../../../apps/web/public/samples/film-negative.png");
    for i in 1..=2 {
        fs::write(dir.join(format!("input/{i}.png")), bytes).unwrap();
    }
    let out = dir.join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_hexscope"))
        .args(["film", "--recipe"])
        .arg(dir.join("recipe.json"))
        .arg("--out")
        .arg(&out)
        .arg("--format")
        .arg("tiff16")
        .arg(dir.join("input"))
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result);
    assert!(out.join("000001-film.tif").exists());
    assert!(out.join("000002-film.tif").exists());
    let report = fs::read_to_string(out.join("hexscope-film-report.json")).unwrap();
    assert!(report.contains("\"written\": 2"));
    assert!(!report.contains("recipe.json"));
    assert_eq!(fs::read(dir.join("input/1.png")).unwrap(), bytes);
    fs::remove_dir_all(dir).unwrap();
}
