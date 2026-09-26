//! `hexscope` on the command line: what files give away, before they are
//! sent or published — the same reading as the page, on your own machine,
//! for a folder at a time or a CI check.
//!
//! ```text
//! hexscope check [--fail-on WHAT] [--json] [--all] PATH...
//! hexscope clean [--in-place | --out DIR] PATH...
//! hexscope repair [--out DIR] PATH...
//! ```

#![forbid(unsafe_code)]

use hexscope_core::clean::clean;
use hexscope_core::docs::Concern;
use hexscope_core::repair::repair;
use hexscope_core::summary::{Summary, summarize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "hexscope — see what your files say about you

Usage:
  hexscope check [options] PATH...    what each file gives away, and what is wrong with it
  hexscope clean [options] PATH...    save copies without what they give away
  hexscope repair [options] PATH...   save copies of damaged files with what survived

PATH is a file or a folder, read with everything inside it.

check:
  --fail-on WHAT   exit 1 when a file has any of WHAT, a comma-separated list of
                   reveals, hidden, damage, oddity, or kinds of fact such as
                   location, serial, author, covered (default: reveals,hidden,damage);
                   none never fails
  --json           one JSON object per file, one per line
  --all            list files hexscope does not read, and files with nothing to say

clean:
  --in-place       replace each file with its clean copy
  --out DIR        write the copies into DIR (default: beside each file, as NAME-clean.EXT)

repair:
  --out DIR        write the copies into DIR (default: beside each file, as NAME-repaired.EXT)

Exit status: 0, nothing to fail on; 1, something to fail on; 2, a usage or file error.
In GitHub Actions, findings are also written as annotations on the files.

Nothing leaves your machine.";

/// Folders never looked into: tools' own, and dependencies.
const SKIP: [&str; 5] = [".git", "node_modules", "target", ".venv", "__pycache__"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("hexscope: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some((command, rest)) = args.split_first() else {
        println!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    match command.as_str() {
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        "-V" | "--version" => {
            println!("hexscope {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        "check" => check(rest),
        "clean" => copies(rest, Make::Clean),
        "repair" => copies(rest, Make::Repair),
        // A bare path checks it, as the page would.
        _ => check(args),
    }
}

/// The options that take a value, and those that do not, split from paths.
struct Options {
    fail_on: Vec<String>,
    json: bool,
    all: bool,
    in_place: bool,
    out: Option<PathBuf>,
    paths: Vec<PathBuf>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        fail_on: ["reveals", "hidden", "damage"].map(String::from).to_vec(),
        json: false,
        all: false,
        in_place: false,
        out: None,
        paths: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--fail-on" => {
                let v = it
                    .next()
                    .ok_or("--fail-on needs a list, such as reveals,damage")?;
                o.fail_on = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "none")
                    .collect();
            }
            "--json" => o.json = true,
            "--all" => o.all = true,
            "--in-place" => o.in_place = true,
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out needs a folder")?)),
            s if s.starts_with("--") => {
                return Err(format!("unknown option {s}; see hexscope --help"));
            }
            p => o.paths.push(PathBuf::from(p)),
        }
    }
    if o.paths.is_empty() {
        return Err("no files given; see hexscope --help".into());
    }
    Ok(o)
}

/// Every file under the paths, folders read through, in a stable order.
fn files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in paths {
        let meta = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
        if meta.is_dir() {
            walk(p, &mut out);
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Links are not followed: a loop of them would never end.
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if meta.is_dir() {
            if !SKIP.contains(&name) {
                walk(&p, out);
            }
        } else if meta.is_file() {
            out.push(p);
        }
    }
}

/// The groups a summary falls in, for `--fail-on`: its concerns, `reveals`
/// when it gives something away, and the kinds of what it gives away.
fn tags(s: &Summary) -> Vec<&str> {
    let mut t = Vec::new();
    for (concern, name) in [
        (Concern::Damage, "damage"),
        (Concern::Hidden, "hidden"),
        (Concern::Oddity, "oddity"),
    ] {
        if s.count(concern) > 0 {
            t.push(name);
        }
    }
    if !s.facts.is_empty() {
        t.push("reveals");
    }
    t.extend(s.facts.iter().map(|(k, _)| *k));
    t
}

fn concern_name(c: Concern) -> &'static str {
    match c {
        Concern::Damage => "damage",
        Concern::Hidden => "hidden",
        Concern::Oddity => "oddity",
    }
}

fn check(args: &[String]) -> Result<ExitCode, String> {
    let o = options(args)?;
    let github = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true");
    let mut failed = 0;
    let mut read = 0;
    for path in files(&o.paths)? {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let s = summarize(&data);
        let quiet = s.problems.is_empty() && s.facts.is_empty();
        if s.format == "unknown" && !o.all {
            continue;
        }
        read += 1;
        let tags = tags(&s);
        let fails = o.fail_on.iter().any(|f| tags.contains(&f.as_str()));
        failed += usize::from(fails);
        if o.json {
            println!("{}", json(&path, &s));
        } else if !quiet || o.all {
            print!("{}", human(&path, &s));
        }
        if github && fails {
            for line in annotations(&path, &s) {
                println!("{line}");
            }
        }
    }
    if !o.json {
        let fail_list = if o.fail_on.is_empty() {
            "nothing".to_string()
        } else {
            o.fail_on.join(", ")
        };
        eprintln!(
            "hexscope: read {read} {}; {failed} with something to fail on ({fail_list})",
            if read == 1 { "file" } else { "files" }
        );
    }
    Ok(if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn human(path: &Path, s: &Summary) -> String {
    let mut out = format!("{}  {}\n", path.display(), s.format);
    for p in &s.problems {
        let _ = writeln!(
            out,
            "  {:<8} {} (at {}, {} bytes)",
            concern_name(p.concern),
            p.label,
            p.offset,
            p.len
        );
    }
    for (kind, text) in &s.facts {
        let _ = writeln!(out, "  reveals  {kind}: {text}");
    }
    if s.problems.is_empty() && s.facts.is_empty() {
        out.push_str("  nothing found\n");
    }
    out
}

/// GitHub Actions workflow commands: one annotation per finding.
fn annotations(path: &Path, s: &Summary) -> Vec<String> {
    // The message may not hold a newline; `%` and line ends are escaped as
    // the runner expects.
    let esc = |m: &str| {
        m.replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    };
    let file = path
        .display()
        .to_string()
        .replace(',', "%2C")
        .replace(':', "%3A");
    let mut out = Vec::new();
    for p in &s.problems {
        let level = if p.concern == Concern::Oddity {
            "notice"
        } else {
            "error"
        };
        out.push(format!(
            "::{level} file={file},title=hexscope: {}::{}",
            concern_name(p.concern),
            esc(&p.label)
        ));
    }
    for (kind, text) in &s.facts {
        out.push(format!(
            "::warning file={file},title=hexscope: reveals {kind}::{}",
            esc(text)
        ));
    }
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json(path: &Path, s: &Summary) -> String {
    let problems: Vec<String> = s
        .problems
        .iter()
        .map(|p| {
            format!(
                "{{\"label\":{},\"concern\":\"{}\",\"offset\":{},\"length\":{}}}",
                json_str(&p.label),
                concern_name(p.concern),
                p.offset,
                p.len
            )
        })
        .collect();
    let facts: Vec<String> = s
        .facts
        .iter()
        .map(|(k, t)| format!("{{\"kind\":\"{k}\",\"text\":{}}}", json_str(t)))
        .collect();
    format!(
        "{{\"file\":{},\"format\":\"{}\",\"problems\":[{}],\"reveals\":[{}]}}",
        json_str(&path.display().to_string()),
        s.format,
        problems.join(","),
        facts.join(",")
    )
}

/// Which copy to make.
#[derive(Clone, Copy)]
enum Make {
    Clean,
    Repair,
}

/// Where a copy goes: beside the file, into `--out`, or over it.
fn target(path: &Path, o: &Options, suffix: &str) -> PathBuf {
    if o.in_place {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map_or_else(|| "file".into(), |s| s.to_string_lossy().into_owned());
    let name = match path.extension() {
        Some(e) => format!("{stem}-{suffix}.{}", e.to_string_lossy()),
        None => format!("{stem}-{suffix}"),
    };
    match &o.out {
        Some(dir) => dir.join(name),
        None => path.with_file_name(name),
    }
}

fn copies(args: &[String], kind: Make) -> Result<ExitCode, String> {
    let o = options(args)?;
    if let Some(dir) = &o.out {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if o.in_place && matches!(kind, Make::Repair) {
        return Err("repair keeps the damaged file: use --out, or the default beside it".into());
    }
    let (mut made, mut failed) = (0, 0);
    for path in files(&o.paths)? {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let result = match kind {
            Make::Clean => clean(&data)
                .map(|c| {
                    (
                        c.bytes,
                        c.removed.into_iter().map(|r| r.what).collect::<Vec<_>>(),
                    )
                })
                .map_err(|e| e.reason()),
            Make::Repair => repair(&data)
                .map(|r| (r.bytes, r.fixed))
                .map_err(|e| e.reason()),
        };
        match result {
            Ok((bytes, what)) => {
                let to = target(
                    &path,
                    &o,
                    if matches!(kind, Make::Clean) {
                        "clean"
                    } else {
                        "repaired"
                    },
                );
                std::fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
                println!("{} → {}", path.display(), to.display());
                for w in what {
                    println!("  {w}");
                }
                made += 1;
            }
            // A file with nothing to do is not a failure; one that could
            // not be done is said, and counted.
            Err(reason)
                if reason.starts_with("there is nothing")
                    || reason.starts_with("nothing in it") => {}
            Err(reason)
                if reason.starts_with("hexscope cleans")
                    || reason.starts_with("hexscope repairs") => {}
            Err(reason) => {
                eprintln!("hexscope: {}: not done, as {reason}", path.display());
                failed += 1;
            }
        }
    }
    eprintln!(
        "hexscope: {made} {} made",
        if made == 1 { "copy" } else { "copies" }
    );
    Ok(if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
