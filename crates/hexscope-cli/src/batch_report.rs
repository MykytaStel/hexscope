//! Folder copies with an index-only receipt. Errors remain per-file.
use hexscope_core::{
    clean::clean,
    summary::summarize,
    verification::{compare, snapshot},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

pub const MAX_FILES: usize = 1000;
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;

pub fn collect(paths: &[PathBuf], exclude: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    fn walk(
        path: &Path,
        exclude: Option<&Path>,
        files: &mut BTreeSet<PathBuf>,
    ) -> Result<(), String> {
        let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "{}: symbolic links are not followed",
                path.display()
            ));
        }
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        if exclude.is_some_and(|excluded| path.starts_with(excluded)) {
            return Ok(());
        }
        if meta.is_dir() {
            if matches!(
                path.file_name().and_then(|v| v.to_str()),
                Some(".git" | "node_modules" | "target" | ".venv" | "__pycache__")
            ) {
                return Ok(());
            }
            for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
                walk(&entry.map_err(|e| e.to_string())?.path(), exclude, files)?;
            }
        } else if meta.is_file() {
            files.insert(path);
        }
        if files.len() > MAX_FILES {
            return Err("a batch is limited to 1000 files".into());
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    for path in paths {
        walk(path, exclude, &mut files)?;
    }
    Ok(files.into_iter().collect())
}

pub fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES {
        return Err("file exceeds 50 MiB limit".into());
    }
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_BYTES {
        return Err("file exceeds 50 MiB limit".into());
    }
    Ok(data)
}

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut out = None;
    let mut paths = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a directory")?)),
            arg if arg.starts_with('-') => return Err(format!("unknown batch option: {arg}")),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        return Err("batch needs input files or folders".into());
    }
    let out = out.ok_or("batch needs --out DIR")?;
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let out = fs::canonicalize(out).map_err(|e| e.to_string())?;
    let files = collect(&paths, Some(&out))?;
    if files.is_empty() {
        return Err("no input files outside the output folder".into());
    }
    let receipt = out.join("hexscope-report.json");
    if fs::symlink_metadata(&receipt).is_ok() {
        return Err("output report already exists; use a new output folder".into());
    }
    let targets = files
        .iter()
        .enumerate()
        .map(|(i, path)| {
            out.join(format!(
                "{:06}-clean{}",
                i + 1,
                path.extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default()
            ))
        })
        .collect::<Vec<_>>();
    if targets.iter().any(|p| p.exists()) {
        return Err("an output copy already exists; use a new output folder".into());
    }
    // Reserve a new receipt before processing. create_new also rejects dangling
    // symlinks and entries planted after preflight, without following them.
    let mut receipt_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&receipt)
        .map_err(|e| format!("cannot create output report: {e}"))?;
    let mut rows = Vec::new();
    let mut written = 0;
    let mut failed = 0;
    let mut removed = 0;
    let mut present = 0;
    let mut unchecked = 0;
    for (index, (source, target)) in files.iter().zip(&targets).enumerate() {
        let result = (|| -> Result<Value, String> {
            let data = bounded_read(source)?;
            let before = snapshot(&summarize(&data));
            let copy = clean(&data).map_err(|e| e.reason().to_owned())?;
            // create_new cannot replace another copy or a newly planted symlink.
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(|e| e.to_string())?;
            if let Err(e) = file.write_all(&copy.bytes).and_then(|()| file.sync_all()) {
                let _ = fs::remove_file(target);
                return Err(e.to_string());
            }
            written += 1;
            let actual = bounded_read(target)?;
            let report = compare(&before, &snapshot(&summarize(&actual)));
            removed += report.removed.len();
            present += report.present.len();
            unchecked += report.unchecked.len();
            Ok(
                json!({"index":index+1,"operation_state":"written","verification":serde_json::from_str::<Value>(&report.to_json()).map_err(|e|e.to_string())?}),
            )
        })();
        match result {
            Ok(row) => rows.push(row),
            Err(error) => {
                failed += 1;
                eprintln!("{}: {error}", source.display());
                rows.push(json!({"index":index+1,"operation_state":if targets[index].exists(){"written"}else{"not_created"},"verification":{"schema_version":1,"removed":[],"present":[],"unchecked":[{"kind":"file","reason":"verification_skipped","unexpected":false}]}}));
                unchecked += 1;
            }
        }
    }
    let report = json!({"schema":"hexscope.batch-receipt","version":1,"total":files.len(),"written":written,"failed":failed,"removed":removed,"present":present,"unchecked":unchecked,"files":rows});
    receipt_file
        .write_all(&serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .and_then(|()| receipt_file.sync_all())
        .map_err(|e| e.to_string())?;
    eprintln!(
        "{written} copies written; {failed} not completed; findings: {removed} removed, {present} present, {unchecked} unchecked. Receipt: hexscope-report.json"
    );
    Ok(if failed > 0 || present > 0 || unchecked > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
