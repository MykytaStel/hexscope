//! Apply one calibrated recipe to a folder, using the same core as web.
use crate::batch_report::{bounded_read, collect};
use hexscope_raster::{Output, process, read_recipe};
use serde_json::json;
use std::{fs, io::Write, path::PathBuf, process::ExitCode};
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut recipe = None;
    let mut out = None;
    let mut paths = Vec::new();
    let mut format = Output::Jpeg;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--recipe" => recipe = Some(PathBuf::from(it.next().ok_or("--recipe needs a file")?)),
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a folder")?)),
            "--format" => {
                format = match it.next().map(String::as_str) {
                    Some("jpeg") => Output::Jpeg,
                    Some("tiff16") => Output::Tiff16,
                    _ => return Err("--format accepts jpeg or tiff16".into()),
                }
            }
            arg if arg.starts_with('-') => return Err(format!("unknown film option: {arg}")),
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let recipe = recipe.ok_or("film needs --recipe JSON")?;
    if fs::metadata(&recipe).map_err(|e| e.to_string())?.len() > 16_384 {
        return Err("recipe exceeds 16 KiB".into());
    }
    let mut settings = read_recipe(&fs::read_to_string(recipe).map_err(|e| e.to_string())?)?;
    let out = out.ok_or("film needs --out DIR")?;
    if paths.is_empty() {
        return Err("film needs input files or folders".into());
    }
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let out = fs::canonicalize(out).map_err(|e| e.to_string())?;
    let files = collect(&paths, Some(&out))?;
    if files.is_empty() {
        return Err("no input files outside the output folder".into());
    }
    let extension = match format {
        Output::Jpeg => "jpg",
        Output::Tiff16 => "tif",
    };
    let report_path = out.join("hexscope-film-report.json");
    let targets = (0..files.len())
        .map(|i| out.join(format!("{:06}-film.{extension}", i + 1)))
        .collect::<Vec<_>>();
    if report_path.exists() || targets.iter().any(|p| p.exists()) {
        return Err("output exists; use a new folder".into());
    }
    let mut rows = Vec::new();
    let mut written = 0;
    for (index, path) in files.iter().enumerate() {
        let result = (|| -> Result<serde_json::Value, String> {
            let source = bounded_read(path)?;
            let rendered = process(
                &source,
                &settings,
                format,
                if matches!(format, Output::Jpeg) {
                    4096
                } else {
                    30_000
                },
            )?;
            if settings.base.is_none() {
                settings.base = Some(rendered.base);
            }
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&targets[index])
                .map_err(|e| e.to_string())?;
            if let Err(e) = output
                .write_all(&rendered.bytes)
                .and_then(|()| output.sync_all())
            {
                let _ = fs::remove_file(&targets[index]);
                return Err(e.to_string());
            }
            let actual = fs::read(&targets[index]).map_err(|e| e.to_string())?;
            if actual != rendered.bytes {
                return Err("written bytes differ from encoded copy".into());
            }
            let sha = hexscope_core::crypto::sha2::sha256(&actual)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            written += 1;
            Ok(
                json!({"index":index+1,"operation_state":"written","width":rendered.width,"height":rendered.height,"source_depth":rendered.source_depth,"output_depth":if matches!(format,Output::Tiff16){16}else{8},"limited":rendered.limited,"profile_present":rendered.profile_present,"color_assumption":"srgb_encoded_samples","bytes_readback":"matched","sha256":sha,"metadata_verification":"not_checked"}),
            )
        })();
        match result {
            Ok(row) => rows.push(row),
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                rows.push(json!({"index":index+1,"operation_state":if targets[index].exists(){"written"}else{"not_created"},"check":"unavailable"}));
            }
        }
    }
    let receipt = json!({"schema":"hexscope.film-roll","version":1,"algorithm":"srgb-density-v1","total":files.len(),"written":written,"failed":files.len()-written,"files":rows});
    let mut report = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report_path)
        .map_err(|e| e.to_string())?;
    report
        .write_all(
            serde_json::to_string_pretty(&receipt)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
    eprintln!(
        "{written}/{} film copies; one base calibration reused. sRGB assumption; ICC profiles are not color-managed. Originals unchanged.",
        files.len()
    );
    Ok(if written == files.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
