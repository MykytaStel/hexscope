//! Local policy profiles and value-free clean-copy plans.
use hexscope_core::{
    policy::Policy,
    summary::summary_of,
    verification::{planned_removal_kinds, snapshot},
};
use serde_json::{Value, json};
use std::{path::PathBuf, process::ExitCode};

pub fn run(args: &[String], policy_mode: bool) -> Result<ExitCode, String> {
    let (policy, paths) = if policy_mode {
        let (profile, paths) = args
            .split_first()
            .ok_or("policy needs a profile JSON and input paths")?;
        let bytes = std::fs::File::open(profile).map_err(|e| e.to_string())?;
        use std::io::Read;
        let mut body = Vec::new();
        bytes
            .take(65_537)
            .read_to_end(&mut body)
            .map_err(|e| e.to_string())?;
        if body.len() > 65_536 {
            return Err("policy profile exceeds 64 KiB".into());
        }
        let value: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        if value["schema"] != "hexscope.policy" || value["version"] != 1 {
            return Err("unsupported policy profile".into());
        }
        let require_complete = value["require_complete"]
            .as_bool()
            .ok_or("require_complete must be boolean")?;
        let kinds = value["blocked_kinds"]
            .as_array()
            .ok_or("blocked_kinds must be an array")?;
        if kinds.len() > 128 {
            return Err("too many policy rules".into());
        }
        let blocked_kinds = kinds
            .iter()
            .map(|kind| {
                let kind = kind.as_str().ok_or("policy rule must be a finding kind")?;
                if kind.is_empty()
                    || kind.len() > 64
                    || !kind.bytes().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
                    })
                {
                    return Err("invalid policy finding kind");
                }
                Ok(kind.to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        (
            Some(Policy {
                blocked_kinds,
                require_complete,
            }),
            paths,
        )
    } else {
        (None, args)
    };
    if paths.is_empty() || paths.iter().any(|path| path.starts_with('-')) {
        return Err("provide input file or folder paths".into());
    }
    let mut failed = false;
    for (index, path) in super::files(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())?
        .into_iter()
        .enumerate()
    {
        let data = super::input::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let summary = summary_of(&hexscope_core::parse(&data));
        if let Some(policy) = &policy {
            let result = policy.evaluate(&summary);
            failed |= !result.passed();
            let mut result: Value =
                serde_json::from_str(&result.to_json()).map_err(|e| e.to_string())?;
            result["index"] = json!(index + 1);
            println!("{result}");
        } else {
            let source = snapshot(&summary);
            println!(
                "{}",
                json!({"schema":"hexscope.copy-plan", "version":1, "index":index+1, "format":summary.format, "inspection_status":summary.inspection_status().as_str(), "planned_removal_kinds":planned_removal_kinds(&source), "verified":false})
            );
        }
    }
    Ok(ExitCode::from(u8::from(failed)))
}
