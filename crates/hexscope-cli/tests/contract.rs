//! What docs/cli.md promises, held to the program: every test file in the
//! repository through `check --json` and `--sarif`, the copies' reports, and
//! the exit status, against the lists that page keeps. A new option, format,
//! kind or reason fails here until the page lists it; a field added to or
//! taken from an object fails until the page and this test agree.

use hexscope_core::verification::VerificationReason;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_hexscope");

/// Where the repository's test files are.
const FOLDERS: [&str; 4] = [
    "crates/hexscope-core/tests/fixtures",
    "crates/hexscope-core/tests/corpus",
    "crates/hexscope-cli/tests/fixtures",
    "apps/web/public/samples",
];

fn repo(path: &str) -> String {
    format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))
}

fn hexscope(args: &[&str], github: bool) -> (i32, String, String) {
    let mut command = Command::new(BIN);
    command.args(args).env_remove("GITHUB_ACTIONS");
    if github {
        command.env("GITHUB_ACTIONS", "true");
    }
    let out = command.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// What the page lists between `<!-- NAME:start -->` and `<!-- NAME:end -->`:
/// each `code` span there.
fn listed(name: &str) -> BTreeSet<String> {
    let page = fs::read_to_string(repo("docs/cli.md")).unwrap();
    let start = format!("<!-- {name}:start -->");
    let section = page
        .split_once(&start)
        .and_then(|(_, rest)| rest.split_once(&format!("<!-- {name}:end -->")))
        .unwrap_or_else(|| panic!("docs/cli.md has no {start}"))
        .0;
    section
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn keys(o: &Map<String, Value>) -> BTreeSet<&str> {
    o.keys().map(String::as_str).collect()
}

fn set<'a>(names: &[&'a str]) -> BTreeSet<&'a str> {
    names.iter().copied().collect()
}

fn object(v: &Value) -> &Map<String, Value> {
    v.as_object()
        .unwrap_or_else(|| panic!("not an object: {v}"))
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hexscope-contract-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn every_test_file(first: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = first.iter().map(|a| a.to_string()).collect();
    args.extend(FOLDERS.iter().map(|f| repo(f)));
    args
}

#[test]
fn the_page_lists_every_command_and_option_the_help_does() {
    let (code, help, _) = hexscope(&["--help"], false);
    assert_eq!(code, 0);
    let options = |text: &str| -> BTreeSet<String> {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .filter(|w| w.len() > 2 && w.starts_with("--"))
            .map(str::to_string)
            .collect()
    };
    let documented: BTreeSet<String> = listed("options")
        .iter()
        .filter(|s| s.starts_with("--"))
        .flat_map(|s| options(s))
        .collect();
    assert_eq!(options(&help), documented);

    let commands = |lines: Vec<&str>| -> BTreeSet<String> {
        lines
            .iter()
            .filter_map(|l| l.trim().strip_prefix("hexscope "))
            .filter_map(|rest| rest.split_whitespace().next())
            .filter(|w| w.chars().all(|c| c.is_ascii_lowercase()))
            .map(str::to_string)
            .collect()
    };
    let usage = help
        .split_once("Usage:")
        .unwrap()
        .1
        .split_once("\n\n")
        .unwrap()
        .0;
    let page: Vec<String> = listed("commands").into_iter().collect();
    assert_eq!(
        commands(usage.lines().collect()),
        commands(page.iter().map(String::as_str).collect())
    );
}

#[test]
fn version_is_the_crates() {
    let (code, out, _) = hexscope(&["--version"], false);
    assert_eq!(code, 0);
    assert_eq!(out, format!("hexscope {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn check_json_has_the_fields_formats_and_kinds_the_page_lists() {
    let formats = listed("formats");
    let kinds = listed("kinds");
    for format in ["mp3", "flac", "ogg"] {
        assert!(
            formats.contains(format),
            "docs/cli.md does not list {format}"
        );
    }
    for kind in ["title", "artist", "album", "audio-tag"] {
        assert!(kinds.contains(kind), "docs/cli.md does not list {kind}");
    }
    let args = every_test_file(&["check", "--json", "--all", "--fail-on", "none"]);
    let (code, out, err) = hexscope(&args.iter().map(String::as_str).collect::<Vec<_>>(), false);
    assert_eq!(code, 0, "{err}");
    let (mut files, mut misnamed) = (0, 0);
    for line in out.lines() {
        let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        let o = object(&v);
        let mut expected = set(&["file", "format", "problems", "reveals"]);
        if o.contains_key("complete") {
            expected.insert("complete");
            assert_eq!(o["complete"], Value::Bool(false), "{line}");
        }
        if o.contains_key("misnamed") {
            expected.insert("misnamed");
            misnamed += 1;
            let m = object(&o["misnamed"]);
            assert_eq!(keys(m), set(&["ext", "is", "fits"]), "{line}");
            assert!(m.values().all(Value::is_string), "{line}");
            assert!(m["fits"].as_str().unwrap().starts_with('.'), "{line}");
        }
        assert_eq!(keys(o), expected, "{line}");
        assert!(o["file"].is_string(), "{line}");
        let format = o["format"].as_str().unwrap();
        assert!(
            formats.contains(format),
            "docs/cli.md does not list the format {format}"
        );
        for p in o["problems"].as_array().unwrap() {
            let p = object(p);
            assert_eq!(
                keys(p),
                set(&["label", "concern", "offset", "length"]),
                "{line}"
            );
            assert!(p["label"].is_string(), "{line}");
            let concern = p["concern"].as_str().unwrap();
            assert!(["damage", "hidden", "oddity"].contains(&concern), "{line}");
            assert!(p["offset"].is_u64() && p["length"].is_u64(), "{line}");
        }
        for r in o["reveals"].as_array().unwrap() {
            let r = object(r);
            assert_eq!(keys(r), set(&["kind", "text"]), "{line}");
            let kind = r["kind"].as_str().unwrap();
            assert!(
                kinds.contains(kind),
                "docs/cli.md does not list the kind {kind}"
            );
            assert!(r["text"].is_string(), "{line}");
        }
        files += 1;
    }
    assert!(files > 200, "only {files} files were read");
    assert!(
        misnamed > 0,
        "no test file is misnamed, so that field went unchecked"
    );
}

#[test]
fn sarif_has_the_rules_and_levels_the_page_lists() {
    let kinds = listed("kinds");
    let args = every_test_file(&["check", "--sarif", "--fail-on", "none"]);
    let (code, out, err) = hexscope(&args.iter().map(String::as_str).collect::<Vec<_>>(), false);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 1);
    let log: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(keys(object(&log)), set(&["version", "$schema", "runs"]));
    assert_eq!(log["version"], "2.1.0");
    let runs = log["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(keys(object(&runs[0])), set(&["tool", "results"]));
    let driver = object(&runs[0]["tool"]["driver"]);
    assert_eq!(
        keys(driver),
        set(&["name", "version", "informationUri", "rules"])
    );
    assert_eq!(driver["name"], "hexscope");
    assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"));
    let mut rules = BTreeSet::new();
    for rule in driver["rules"].as_array().unwrap() {
        let rule = object(rule);
        assert_eq!(keys(rule), set(&["id", "shortDescription"]));
        assert!(rule["shortDescription"]["text"].is_string());
        assert!(
            rules.insert(rule["id"].as_str().unwrap().to_string()),
            "a rule twice"
        );
    }
    let results = runs[0]["results"].as_array().unwrap();
    assert!(results.len() > 100);
    for r in results {
        let o = object(r);
        assert_eq!(
            keys(o),
            set(&["ruleId", "level", "message", "locations"]),
            "{r}"
        );
        let id = o["ruleId"].as_str().unwrap();
        assert!(rules.contains(id), "{id} is not among the rules");
        let (level, region) = match id {
            "damage" => ("error", true),
            "hidden" => ("warning", true),
            "oddity" => ("note", true),
            "misnamed" => ("note", false),
            _ => {
                let kind = id
                    .strip_prefix("reveals/")
                    .unwrap_or_else(|| panic!("rule {id}"));
                assert!(
                    kinds.contains(kind),
                    "docs/cli.md does not list the kind {kind}"
                );
                ("warning", false)
            }
        };
        assert_eq!(o["level"], level, "{r}");
        assert!(o["message"]["text"].is_string(), "{r}");
        let locations = o["locations"].as_array().unwrap();
        assert_eq!(locations.len(), 1, "{r}");
        let at = object(&locations[0]["physicalLocation"]);
        assert!(at["artifactLocation"]["uri"].is_string(), "{r}");
        if region {
            assert_eq!(keys(at), set(&["artifactLocation", "region"]), "{r}");
            let region = object(&at["region"]);
            assert_eq!(keys(region), set(&["byteOffset", "byteLength"]), "{r}");
            assert!(region.values().all(Value::is_u64), "{r}");
        } else {
            assert_eq!(keys(at), set(&["artifactLocation"]), "{r}");
        }
    }
}

#[test]
fn in_github_actions_a_report_on_standard_output_stays_whole() {
    let photo = repo("apps/web/public/samples/photo.jpg");
    let (code, out, err) = hexscope(&["check", "--sarif", &photo], true);
    assert_eq!(code, 1);
    serde_json::from_str::<Value>(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
    assert!(err.contains("::warning file="), "{err}");

    let (code, out, err) = hexscope(&["check", "--json", &photo], true);
    assert_eq!(code, 1);
    for line in out.lines() {
        serde_json::from_str::<Value>(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    }
    assert!(err.contains("::warning file="), "{err}");

    // Without them, the commands are the output, and carry no values.
    let (code, out, _) = hexscope(&["check", &photo], true);
    assert_eq!(code, 1);
    assert!(out.lines().all(|l| l.starts_with("::")), "{out}");
    assert!(out.contains("title=hexscope: reveals location"), "{out}");
}

/// A verification object: the groups, each item a kind and a listed reason.
fn verification(o: &Map<String, Value>, reasons: &BTreeSet<String>) {
    assert_eq!(o["schema_version"], 1);
    for group in ["removed", "present", "unchecked"] {
        for item in o[group].as_array().unwrap() {
            let item = object(item);
            assert_eq!(keys(item), set(&["kind", "reason", "unexpected"]));
            assert!(item["kind"].is_string() && item["unexpected"].is_boolean());
            let reason = item["reason"].as_str().unwrap();
            assert!(
                reasons.contains(reason),
                "docs/cli.md does not list the reason {reason}"
            );
        }
    }
}

#[test]
fn clean_verify_json_has_the_fields_the_page_lists() {
    let reasons = listed("reasons");
    let dir = temp("verify");
    fs::write(dir.join("notes.txt"), b"nothing to clean").unwrap();
    let (code, out, err) = hexscope(
        &[
            "clean",
            "--verify",
            "--json",
            "--out",
            dir.join("out").to_str().unwrap(),
            &repo("apps/web/public/samples/photo.jpg"),
            &repo("apps/web/public/samples/report.pdf"),
            dir.join("notes.txt").to_str().unwrap(),
        ],
        false,
    );
    assert_eq!(code, 1, "the text file has no copy: {err}");
    let mut states = BTreeSet::new();
    for line in out.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        let o = object(&v);
        assert_eq!(
            keys(o),
            set(&[
                "source",
                "output",
                "operation_state",
                "schema_version",
                "removed",
                "present",
                "unchecked"
            ]),
            "{line}"
        );
        let state = o["operation_state"].as_str().unwrap();
        assert_eq!(o["output"].is_null(), state == "not_created", "{line}");
        states.insert(state.to_string());
        verification(o, &reasons);
    }
    assert_eq!(out.lines().count(), 3);
    assert_eq!(states, ["not_created", "written"].map(String::from).into());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_reason_is_listed() {
    use VerificationReason as R;
    let all = [
        R::Removed,
        R::StillPresent,
        R::ValueChangedSameKind,
        R::NoStableValue,
        R::CoverageIncomplete,
        R::ParseIncomplete,
        R::UnexpectedOutput,
        R::NoComparableFindings,
        R::VerificationSkipped,
    ];
    // A reason added to the core stops this from compiling until it is in
    // `all` above, and so on the page.
    for r in all {
        match r {
            R::Removed
            | R::StillPresent
            | R::ValueChangedSameKind
            | R::NoStableValue
            | R::CoverageIncomplete
            | R::ParseIncomplete
            | R::UnexpectedOutput
            | R::NoComparableFindings
            | R::VerificationSkipped => {}
        }
    }
    let codes: BTreeSet<String> = all.iter().map(|r| r.code().to_string()).collect();
    assert_eq!(codes, listed("reasons"));
}

#[test]
fn the_receipts_have_the_fields_the_page_lists() {
    let reasons = listed("reasons");
    let dir = temp("receipts");
    let photos = dir.join("photos");
    fs::create_dir_all(&photos).unwrap();
    for name in ["photo.jpg", "photo.png"] {
        fs::copy(
            repo(&format!("crates/hexscope-core/tests/fixtures/{name}")),
            photos.join(name),
        )
        .unwrap();
    }

    let out = dir.join("batch");
    let (_, _, err) = hexscope(
        &[
            "batch",
            "--out",
            out.to_str().unwrap(),
            photos.to_str().unwrap(),
        ],
        false,
    );
    let receipt: Value =
        serde_json::from_slice(&fs::read(out.join("hexscope-report.json")).unwrap())
            .unwrap_or_else(|e| panic!("{e}: {err}"));
    let o = object(&receipt);
    assert_eq!(
        keys(o),
        set(&[
            "schema",
            "version",
            "total",
            "written",
            "failed",
            "removed",
            "present",
            "unchecked",
            "files"
        ])
    );
    assert_eq!(o["schema"], "hexscope.batch-receipt");
    assert_eq!(o["version"], 1);
    for file in o["files"].as_array().unwrap() {
        let file = object(file);
        assert_eq!(
            keys(file),
            set(&["index", "operation_state", "verification"])
        );
        let v = object(&file["verification"]);
        assert_eq!(
            keys(v),
            set(&["schema_version", "removed", "present", "unchecked"])
        );
        verification(v, &reasons);
    }

    let (code, json, err) = hexscope(&["mosaic", "--json", photos.to_str().unwrap()], false);
    assert_eq!(code, 0, "{err}");
    let mosaic: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        keys(object(&mosaic)),
        set(&[
            "schema",
            "version",
            "total",
            "serial_groups",
            "location_groups",
            "located_files",
            "local_date_order",
            "utc_date_order",
            "location_radius_metres",
            "inferences",
            "input_total",
            "not_checked_indexes",
        ])
    );
    assert_eq!(mosaic["schema"], "hexscope.photo-mosaic");
    assert_eq!(mosaic["inferences"], false);

    let recipe = dir.join("recipe.json");
    fs::write(
        &recipe,
        r#"{"schema":"hexscope.film-recipe","version":1,"algorithm":"srgb-density-v1","settings":{"mode":"color-negative","crop":[0.1,0.1,0.1,0.1],"exposure":0,"contrast":1,"baseColor":null}}"#,
    )
    .unwrap();
    let roll = dir.join("roll");
    fs::create_dir_all(&roll).unwrap();
    fs::copy(
        repo("apps/web/public/samples/film-negative.png"),
        roll.join("1.png"),
    )
    .unwrap();
    let film = dir.join("film");
    let (code, _, err) = hexscope(
        &[
            "film",
            "--recipe",
            recipe.to_str().unwrap(),
            "--out",
            film.to_str().unwrap(),
            roll.to_str().unwrap(),
        ],
        false,
    );
    assert_eq!(code, 0, "{err}");
    let report: Value =
        serde_json::from_slice(&fs::read(film.join("hexscope-film-report.json")).unwrap()).unwrap();
    let o = object(&report);
    assert_eq!(
        keys(o),
        set(&[
            "schema",
            "version",
            "algorithm",
            "total",
            "written",
            "failed",
            "files"
        ])
    );
    assert_eq!(o["schema"], "hexscope.film-roll");
    assert_eq!(
        keys(object(&o["files"][0])),
        set(&[
            "index",
            "operation_state",
            "width",
            "height",
            "source_depth",
            "output_depth",
            "limited",
            "profile_present",
            "color_assumption",
            "bytes_readback",
            "sha256",
            "metadata_verification",
        ])
    );
    fs::remove_dir_all(dir).unwrap();
}

/// The exit status table, row by row.
#[test]
fn exit_status_is_as_the_page_says() {
    let dir = temp("exit");
    let photo = repo("apps/web/public/samples/photo.jpg");
    let pdf = repo("crates/hexscope-cli/tests/fixtures/redacted.pdf");
    let lease = repo("apps/web/public/samples/lease.odt");
    let text = dir.join("notes.txt");
    fs::write(&text, b"nothing to clean").unwrap();
    let text = text.to_str().unwrap();
    let out = dir.join("out");
    let out = out.to_str().unwrap();
    let missing = Path::new(out).join("missing.jpg");
    let missing = missing.to_str().unwrap();
    let cases: [(&[&str], i32); 21] = [
        (&[], 2),
        (&["--help"], 0),
        (&["--version"], 0),
        (&["check", "--fail-on", "none", &photo], 0),
        (&["check", "--fail-on", "serial,location", &lease], 0),
        (&["check", &photo], 1),
        (&[&photo], 1),
        (&["check", "--fail-on", "location", &photo], 1),
        (&["check", "--bogus", &photo], 2),
        (&["check", missing], 2),
        (&["check", "--verify", &photo], 2),
        (&["clean", "--out", out, text], 0),
        (&["clean", "--verify", "--out", out, text], 1),
        (&["repair", "--in-place", &photo], 2),
        (&["redact", "--out", out, &pdf], 2),
        (
            &["redact", "--text", "not on any page", "--out", out, &pdf],
            1,
        ),
        (&["batch", &photo], 2),
        (&["film", "--out", out, &photo], 2),
        (&["mosaic", "--bogus", &photo], 2),
        (&["mosaic", text], 1),
        (&["menu", "--bogus"], 2),
    ];
    for (args, expected) in cases {
        let (code, _, err) = hexscope(args, false);
        assert_eq!(code, expected, "hexscope {}: {err}", args.join(" "));
    }
    fs::remove_dir_all(dir).unwrap();
}
