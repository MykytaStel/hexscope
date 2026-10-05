use crate::batch_report::{bounded_read, collect};
use hexscope_core::{mosaic, summary::summary_of};
use std::{path::PathBuf, process::ExitCode};
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut paths = Vec::new();
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            s if s.starts_with('-') => return Err(format!("unknown mosaic option: {s}")),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        return Err("mosaic needs input files or folders".into());
    }
    let files = collect(&paths, None)?;
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
        println!("{} complete photos; {failed} not checked", report.total);
        println!("Repeated stored serial values: {:?}", report.serial_groups);
        println!(
            "Stored GPS points within 100m of first file: {:?}",
            report.location_groups
        );
        println!(
            "Recorded dates without a time zone: {:?}",
            report.local_date_order
        );
        println!(
            "Recorded dates with normalized time zones: {:?}",
            report.utc_date_order
        );
        println!(
            "Indexes identify input files. Matching facts are not proof of identity, home or a route. JSON omits raw metadata and filenames."
        );
    }
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
