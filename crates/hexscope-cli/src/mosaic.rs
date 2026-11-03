use crate::batch_report::{bounded_read, collect};
use hexscope_core::{mosaic, summary::summary_of};
use serde_json::json;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::ExitCode,
};

fn format_order(indexes: &[usize]) -> String {
    if indexes.is_empty() {
        "none".into()
    } else {
        indexes
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut paths = Vec::new();
    let mut json = false;
    let mut clean_related = false;
    let mut out = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--clean-related" => clean_related = true,
            "--out" => out = Some(PathBuf::from(args.next().ok_or("--out needs a directory")?)),
            s if s.starts_with('-') => return Err(format!("unknown mosaic option: {s}")),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if out.is_some() && !clean_related {
        return Err("mosaic --out requires --clean-related".into());
    }
    if clean_related && out.is_none() {
        return Err("mosaic --clean-related needs --out DIR".into());
    }
    if paths.is_empty() {
        return Err("mosaic needs input files or folders".into());
    }
    let exclude = out
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display())))
        .transpose()?;
    let files = collect(&paths, exclude.as_deref())?;
    let mut signals = Vec::new();
    let mut failed = 0;
    let mut not_checked = Vec::new();
    for (index, path) in files.iter().enumerate() {
        match bounded_read(path) {
            Ok(bytes) => {
                let doc = hexscope_core::parse(&bytes);
                let summary = summary_of(&doc);
                if summary.complete
                    && matches!(summary.format, "jpeg" | "png" | "webp" | "heif" | "gif")
                {
                    if let Some(signal) = mosaic::from_document(index + 1, &doc) {
                        signals.push(signal);
                    }
                } else {
                    failed += 1;
                    not_checked.push(index + 1);
                    eprintln!("{}: not checked for photo mosaic", path.display());
                }
            }
            Err(e) => {
                failed += 1;
                not_checked.push(index + 1);
                eprintln!("{}: {e}", path.display());
            }
        }
    }
    let report = mosaic::analyze(&signals).map_err(String::from)?;
    let selected_indexes = report.related_file_indexes();
    if json {
        let mut result: serde_json::Value =
            serde_json::from_str(&report.to_json()).map_err(|e| e.to_string())?;
        result["input_total"] = serde_json::json!(files.len());
        result["not_checked_indexes"] = serde_json::json!(not_checked);
        println!("{result}");
    } else {
        for (i, path) in files.iter().enumerate() {
            println!("{}: {}", i + 1, path.display());
        }
        println!(
            "{} complete photos; {failed} not checked; {} files in {} relationship groups",
            report.total,
            report.related_file_count(),
            report.relationship_group_count(),
        );
        println!("Relationships:");
        let groups = report.relationship_groups();
        if groups.is_empty() {
            println!("  none");
        }
        for (index, group) in groups.iter().enumerate() {
            println!(
                "  group {} · {}: {}",
                index + 1,
                group.kind.label(),
                format_order(group.indexes),
            );
        }
        println!(
            "Group indexes identify input files. Matching facts are not proof of identity, home or a route. JSON omits raw metadata and filenames."
        );
    }
    if clean_related {
        if selected_indexes.is_empty() {
            eprintln!("No related photos found; no clean copies created.");
            return Ok(if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            });
        }
        let out = out.expect("--clean-related requires an output directory");
        fs::create_dir_all(&out).map_err(|error| error.to_string())?;
        let out = fs::canonicalize(out).map_err(|error| error.to_string())?;
        let selection_path = out.join("hexscope-mosaic-selection.json");
        let mut selection_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&selection_path)
            .map_err(|error| format!("cannot create mosaic selection receipt: {error}"))?;
        let in_progress = json!({
            "schema": "hexscope.photo-mosaic-selection",
            "version": 1,
            "state": "in_progress"
        });
        if let Err(error) = serde_json::to_writer_pretty(&mut selection_file, &in_progress) {
            drop(selection_file);
            let _ = fs::remove_file(&selection_path);
            return Err(error.to_string());
        }
        if let Err(error) = selection_file.sync_all() {
            drop(selection_file);
            let _ = fs::remove_file(&selection_path);
            return Err(error.to_string());
        }
        let selected_files = selected_indexes
            .iter()
            .map(|index| files[index - 1].clone())
            .collect::<Vec<_>>();
        let copy_result = crate::batch_report::run_files(&selected_files, &out, false);
        let copy_code = match copy_result {
            Ok(code) => code,
            Err(error) => {
                drop(selection_file);
                let _ = fs::remove_file(&selection_path);
                return Err(error);
            }
        };
        let selected_mapping = selected_indexes
            .iter()
            .enumerate()
            .map(|(batch_index, input_index)| {
                json!({ "input_index": input_index, "batch_index": batch_index + 1 })
            })
            .collect::<Vec<_>>();
        let selection = json!({
            "schema": "hexscope.photo-mosaic-selection",
            "version": 1,
            "state": "complete",
            "input_total": files.len(),
            "not_checked_indexes": not_checked,
            "selected": selected_mapping,
            "batch_receipt": "hexscope-report.json"
        });
        let result = (|| -> Result<(), String> {
            selection_file
                .set_len(0)
                .map_err(|error| error.to_string())?;
            selection_file
                .seek(SeekFrom::Start(0))
                .map_err(|error| error.to_string())?;
            serde_json::to_writer_pretty(&mut selection_file, &selection)
                .map_err(|error| error.to_string())?;
            selection_file
                .write_all(b"\n")
                .and_then(|()| selection_file.sync_all())
                .map_err(|error| error.to_string())
        })();
        if let Err(error) = result {
            return Err(format!(
                "could not finish mosaic selection receipt: {error}"
            ));
        }
        eprintln!("Mosaic selection receipt: hexscope-mosaic-selection.json");
        return Ok(if failed == 0 {
            copy_code
        } else {
            ExitCode::from(1)
        });
    }
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
