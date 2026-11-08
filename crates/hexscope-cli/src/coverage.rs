//! A local, value-free summary of parser coverage across files.

use hexscope_core::{
    coverage::CoverageScorecard,
    document::{Document, parse},
    summary::summary_of,
    video::parse_video_gapped,
};
use serde_json::json;
use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut json_output = false;
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--json" => json_output = true,
            arg if arg.starts_with('-') => {
                return Err(format!(
                    "unknown coverage option {arg}; see hexscope --help"
                ));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        return Err("coverage needs files or folders".into());
    }

    let mut scorecard = CoverageScorecard::default();
    for path in super::files(&paths)? {
        let input_bytes = fs::metadata(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .len();
        let start = Instant::now();
        let summary =
            match super::movie::read(&path).map_err(|e| format!("{}: {e}", path.display()))? {
                Some(movie) => summary_of(&Document::Video(parse_video_gapped(
                    &movie.given,
                    &movie.gaps,
                ))),
                None => {
                    let data = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    summary_of(&parse(&data))
                }
            };
        scorecard.record(&summary, input_bytes, start.elapsed());
    }

    if json_output {
        let formats: Vec<_> = scorecard
            .formats()
            .map(|row| {
                json!({
                    "format": row.format,
                    "files": row.files,
                    "complete": row.complete,
                    "partial": row.partial,
                    "unsupported": row.unsupported,
                    "bytes": row.bytes,
                    "elapsed_microseconds": row.elapsed_micros,
                })
            })
            .collect();
        println!(
            "{}",
            json!({
                "schema": "hexscope.format-coverage",
                "version": 1,
                "files": scorecard.files(),
                "bytes": scorecard.bytes(),
                "elapsed_microseconds": scorecard.elapsed_micros(),
                "formats": formats,
            })
        );
    } else {
        println!("Hexscope format coverage");
        println!(
            "{} files · {} bytes · {:.1} ms",
            scorecard.files(),
            scorecard.bytes(),
            scorecard.elapsed_micros() as f64 / 1_000.0,
        );
        println!("format\tfiles\tcomplete\tpartial\tunsupported\tbytes\tms");
        for row in scorecard.formats() {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{:.1}",
                row.format,
                row.files,
                row.complete,
                row.partial,
                row.unsupported,
                row.bytes,
                row.elapsed_micros as f64 / 1_000.0,
            );
        }
        println!("No file names or finding values are included.");
    }
    Ok(ExitCode::SUCCESS)
}
