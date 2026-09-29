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

mod movie;

use hexscope_core::Document;
use hexscope_core::clean::{CleanOptions, clean_video_gapped, clean_with};
use hexscope_core::docs::Concern;
use hexscope_core::repair::repair;
use hexscope_core::summary::{Summary, summarize, summary_of};
use hexscope_core::video::parse_video_gapped;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "hexscope — see what your files say about you

Usage:
  hexscope check [options] PATH...    what each file gives away, and what is wrong with it
  hexscope clean [options] PATH...    save copies without what they give away
  hexscope repair [options] PATH...   save copies of damaged files with what survived
  hexscope redact --text WORDS PATH... save PDFs with WORDS taken out of their pages

PATH is a file or a folder, read with everything inside it.

check:
  --fail-on WHAT   exit 1 when a file has any of WHAT, a comma-separated list of
                   reveals, hidden, damage, oddity, or kinds of fact such as
                   location, serial, author, covered (default: reveals,hidden,damage);
                   none never fails
  --json           one JSON object per file, one per line
  --sarif          one SARIF 2.1.0 log for all of them, for GitHub code scanning
  --all            list files hexscope does not read, and files with nothing to say

clean:
  --in-place       replace each file with its clean copy
  --notes          also empty Excel's and PowerPoint's comments and speaker notes
  --out DIR        write the copies into DIR (default: beside each file, as NAME-clean.EXT);
                   for a single file, --out can name the copy itself: --out copy.jpg

redact (PDF):
  --text WORDS     black out every place WORDS appear; give it more than once for more
  --out DIR        write the copies into DIR (default: beside each file, as NAME-redacted.EXT)

repair:
  --out DIR        write the copies into DIR (default: beside each file, as NAME-repaired.EXT);
                   for a single file, --out can name the copy itself

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
        "redact" => copies(rest, Make::Redact),
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
    notes: bool,
    sarif: bool,
    /// What `redact` blacks out, each wherever it appears.
    texts: Vec<String>,
    out: Option<PathBuf>,
    paths: Vec<PathBuf>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        fail_on: ["reveals", "hidden", "damage"].map(String::from).to_vec(),
        json: false,
        all: false,
        in_place: false,
        notes: false,
        sarif: false,
        texts: Vec::new(),
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
            "--sarif" => o.sarif = true,
            "--all" => o.all = true,
            "--in-place" => o.in_place = true,
            "--notes" => o.notes = true,
            "--text" => o.texts.push(
                it.next()
                    .ok_or("--text needs the words to black out")?
                    .clone(),
            ),
            "--out" => {
                o.out = Some(PathBuf::from(
                    it.next()
                        .ok_or("--out needs a folder, or a name for the copy")?,
                ))
            }
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
    let mut results: Vec<(String, String)> = Vec::new();
    for path in files(&o.paths)? {
        let failed_to_read = |e: std::io::Error| format!("{}: {e}", path.display());
        let s = match movie::read(&path).map_err(failed_to_read)? {
            Some(m) => summary_of(&Document::Video(parse_video_gapped(&m.given, &m.gaps))),
            None => summarize(&std::fs::read(&path).map_err(failed_to_read)?),
        };
        let quiet = s.problems.is_empty() && s.facts.is_empty();
        if s.format == "unknown" && !o.all {
            continue;
        }
        read += 1;
        let tags = tags(&s);
        let fails = o.fail_on.iter().any(|f| tags.contains(&f.as_str()));
        failed += usize::from(fails);
        if o.sarif {
            results.extend(sarif_results(&path, &s));
        } else if o.json {
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
    if o.sarif {
        println!("{}", sarif(&results));
    }
    if !o.json && !o.sarif {
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

/// A finding as a SARIF 2.1.0 result: damage an error, what is hidden or
/// given away a warning, a broken rule a note; the part's bytes as a region.
fn sarif_results(path: &Path, s: &Summary) -> Vec<(String, String)> {
    let uri = json_str(&path.display().to_string().replace('\\', "/"));
    let mut out = Vec::new();
    for p in &s.problems {
        let (rule, level) = match p.concern {
            Concern::Damage => ("damage", "error"),
            Concern::Hidden => ("hidden", "warning"),
            Concern::Oddity => ("oddity", "note"),
        };
        out.push((rule.to_string(), format!(
            "{{\"ruleId\":\"{rule}\",\"level\":\"{level}\",\"message\":{{\"text\":{}}},\"locations\":[{{\"physicalLocation\":{{\"artifactLocation\":{{\"uri\":{uri}}},\"region\":{{\"byteOffset\":{},\"byteLength\":{}}}}}}}]}}",
            json_str(&p.label),
            p.offset,
            p.len
        )));
    }
    for (kind, text) in &s.facts {
        out.push((format!("reveals/{kind}"), format!(
            "{{\"ruleId\":\"reveals/{kind}\",\"level\":\"warning\",\"message\":{{\"text\":{}}},\"locations\":[{{\"physicalLocation\":{{\"artifactLocation\":{{\"uri\":{uri}}}}}}}]}}",
            json_str(&format!("reveals {kind}: {text}"))
        )));
    }
    out
}

/// One SARIF log for every file read, for code scanning.
fn sarif(results: &[(String, String)]) -> String {
    // Every rule a result names, described once.
    let mut ids: Vec<&str> = results.iter().map(|(id, _)| id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    let rules: Vec<String> = ids
        .iter()
        .map(|id| {
            let text = match *id {
                "damage" => "Part of the file is damaged".to_string(),
                "hidden" => "Something is hidden or disguised".to_string(),
                "oddity" => "A rule of the format is broken".to_string(),
                other => format!("The file reveals {}", other.trim_start_matches("reveals/")),
            };
            format!(
                "{{\"id\":\"{id}\",\"shortDescription\":{{\"text\":{}}}}}",
                json_str(&text)
            )
        })
        .collect();
    format!(
        "{{\"version\":\"2.1.0\",\"$schema\":\"https://json.schemastore.org/sarif-2.1.0.json\",\"runs\":[{{\"tool\":{{\"driver\":{{\"name\":\"hexscope\",\"version\":\"{}\",\"informationUri\":\"https://github.com/MykytaStel/hexscope\",\"rules\":[{}]}}}},\"results\":[{}]}}]}}",
        env!("CARGO_PKG_VERSION"),
        rules.join(","),
        results
            .iter()
            .map(|(_, r)| r.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Which copy to make.
#[derive(Clone, Copy)]
enum Make {
    Clean,
    Repair,
    Redact,
}

/// The copy's own name, when `--out` gives one: a single file, and a path
/// with an extension that is not a folder already.
fn out_file(o: &Options) -> Option<PathBuf> {
    match (&o.out, o.paths.as_slice()) {
        (Some(out), [one]) if one.is_file() && !out.is_dir() && out.extension().is_some() => {
            Some(out.clone())
        }
        _ => None,
    }
}

/// Writes a copy so that it is there whole or not at all: into a file
/// beside it first, then renamed over it in one step. With `--in-place`
/// that is the person's own file — a full disk or a Ctrl-C halfway must
/// not leave it cut short. The copy keeps the permissions of what it
/// replaces.
fn write_whole(
    to: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = to
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = to
        .file_name()
        .map_or_else(|| "copy".into(), |n| n.to_string_lossy().into_owned());
    let part = dir.join(format!(".{name}.hexscope-{}.part", std::process::id()));
    let written = File::create(&part)
        .and_then(|f| {
            let mut out = BufWriter::new(f);
            write(&mut out)?;
            out.into_inner()?.sync_all()
        })
        .and_then(|()| match std::fs::metadata(to) {
            Ok(meta) => std::fs::set_permissions(&part, meta.permissions()),
            Err(_) => Ok(()),
        })
        .and_then(|()| std::fs::rename(&part, to));
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written
}

/// Where a copy goes: beside the file, into `--out`, or over it.
fn target(path: &Path, o: &Options, suffix: &str) -> PathBuf {
    if o.in_place {
        return path.to_path_buf();
    }
    if let Some(file) = out_file(o) {
        return file;
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

/// Where each of `texts` appears on a PDF's pages, as boxes to black out:
/// the page's letters in order, a space put where the gap between two says
/// there is one, matched ignoring case and how words are spaced — as the
/// page's own search does.
fn places(data: &[u8], texts: &[String]) -> Vec<(u32, [f64; 4])> {
    const PAD: f64 = 0.6;
    let mut out = Vec::new();
    for page in hexscope_core::pdf::page_texts(data) {
        // Each character, lower case, and the glyph it came from.
        let mut chars: Vec<(char, Option<usize>)> = Vec::new();
        let g = &page.glyphs;
        for (i, (area, text)) in g.iter().enumerate() {
            if let Some((prev, _)) = i.checked_sub(1).and_then(|p| g.get(p)) {
                let h = (area[3] - area[1]).max(prev[3] - prev[1]).max(0.01);
                let same_line = (area[1] - prev[1]).abs() < 0.3 * h;
                let gap = area[0] - prev[2];
                let spaced = chars.last().is_some_and(|c| c.0.is_whitespace())
                    || text.starts_with(char::is_whitespace);
                if (!same_line || gap > 0.2 * h) && !spaced && !text.is_empty() {
                    chars.push((' ', None));
                }
            }
            for c in text.chars() {
                let lower = c.to_lowercase().next().unwrap_or(c);
                chars.push((if lower.is_whitespace() { ' ' } else { lower }, Some(i)));
            }
        }
        for query in texts {
            let words: Vec<Vec<char>> = query
                .split_whitespace()
                .map(|w| {
                    w.chars()
                        .map(|c| c.to_lowercase().next().unwrap_or(c))
                        .collect()
                })
                .collect();
            let Some(first) = words.first() else { continue };
            let mut at = 0;
            while at < chars.len() {
                let word_at = |k: usize, w: &[char]| {
                    w.iter()
                        .enumerate()
                        .all(|(j, c)| chars.get(k + j).is_some_and(|x| x.0 == *c))
                };
                if !word_at(at, first) {
                    at += 1;
                    continue;
                }
                let mut end = at + first.len();
                let mut ok = true;
                for w in words.iter().skip(1) {
                    let mut k = end;
                    while chars.get(k).is_some_and(|c| c.0 == ' ') {
                        k += 1;
                    }
                    if k == end || !word_at(k, w) {
                        ok = false;
                        break;
                    }
                    end = k + w.len();
                }
                if !ok {
                    at += 1;
                    continue;
                }
                // The glyphs matched, a box per line they run along.
                let mut boxes: Vec<[f64; 4]> = Vec::new();
                let mut last = None;
                for (_, glyph) in chars.get(at..end).unwrap_or(&[]) {
                    let Some(i) = glyph.filter(|i| Some(*i) != last) else {
                        continue;
                    };
                    last = Some(i);
                    let Some(([l, b, r, t], _)) = g.get(i) else {
                        continue;
                    };
                    let h = t - b;
                    match boxes.last_mut() {
                        Some(bx)
                            if (bx[1] - b).abs() < 0.3 * h.max(0.01) + PAD
                                && l - bx[2] < 1.5 * h =>
                        {
                            *bx = [
                                bx[0].min(l - PAD),
                                bx[1].min(b - PAD),
                                bx[2].max(r + PAD),
                                bx[3].max(t + PAD),
                            ];
                        }
                        _ => boxes.push([l - PAD, b - PAD, r + PAD, t + PAD]),
                    }
                }
                out.extend(boxes.into_iter().map(|b| (page.page, b)));
                at = end;
            }
        }
    }
    out
}

enum Done {
    Made,
    Failed,
    Nothing,
}

/// Writes a copy that was made, or says why none was.
fn finish(
    path: &Path,
    o: &Options,
    kind: Make,
    result: Result<(Vec<u8>, Vec<String>), &str>,
    write: impl FnOnce(&[u8], &mut BufWriter<File>) -> std::io::Result<()>,
) -> Result<Done, String> {
    match result {
        Ok((bytes, what)) => {
            let to = target(
                path,
                o,
                match kind {
                    Make::Clean => "clean",
                    Make::Repair => "repaired",
                    Make::Redact => "redacted",
                },
            );
            write_whole(&to, |out| write(&bytes, out))
                .map_err(|e| format!("{}: {e}", to.display()))?;
            println!("{} → {}", path.display(), to.display());
            for w in what {
                println!("  {w}");
            }
            Ok(Done::Made)
        }
        // A file with nothing to do is not a failure; one that could not be
        // done is said, and counted.
        Err(reason)
            if reason.starts_with("there is nothing")
                || reason.starts_with("nothing in it")
                || reason.starts_with("hexscope cleans")
                || reason.starts_with("hexscope repairs")
                || reason.starts_with("hexscope redacts") =>
        {
            Ok(Done::Nothing)
        }
        Err(reason) => {
            eprintln!("hexscope: {}: not done, as {reason}", path.display());
            Ok(Done::Failed)
        }
    }
}

fn copies(args: &[String], kind: Make) -> Result<ExitCode, String> {
    let o = options(args)?;
    match (&o.out, out_file(&o)) {
        (Some(dir), None) => {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        (_, Some(file)) => {
            if let Some(parent) = file.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
        }
        _ => {}
    }
    if matches!(kind, Make::Redact) && o.texts.is_empty() {
        return Err("redact needs what to black out: --text \"a name\"".into());
    }
    if o.in_place && matches!(kind, Make::Repair) {
        return Err("repair keeps the damaged file: use --out, or the default beside it".into());
    }
    let (mut made, mut failed) = (0, 0);
    for path in files(&o.paths)? {
        if matches!(kind, Make::Clean)
            && let Some(m) = movie::read(&path).map_err(|e| format!("{}: {e}", path.display()))?
        {
            let result = clean_video_gapped(&m.given, &m.gaps)
                .map(|c| (c.bytes, c.removed.into_iter().map(|r| r.what).collect()))
                .map_err(|e| e.reason());
            let write = |copy: &[u8], out: &mut BufWriter<File>| m.write_copy(copy, &path, out);
            match finish(&path, &o, kind, result, write)? {
                Done::Made => made += 1,
                Done::Failed => failed += 1,
                Done::Nothing => {}
            }
            continue;
        }
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let result = match kind {
            Make::Clean => clean_with(
                &data,
                CleanOptions {
                    comments_and_notes: o.notes,
                },
            )
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
            Make::Redact => {
                let places = places(&data, &o.texts);
                if !hexscope_core::pdf::is_pdf(&data) {
                    Err("hexscope redacts PDFs")
                } else if places.is_empty() {
                    Err("none of it is on the pages as text")
                } else {
                    hexscope_core::clean::redact(&data, &places, &[])
                        .map(|c| {
                            let mut what = vec![format!(
                                "{} {} blacked out",
                                places.len(),
                                if places.len() == 1 { "place" } else { "places" }
                            )];
                            what.extend(c.removed.into_iter().map(|r| r.what));
                            (c.bytes, what)
                        })
                        .map_err(|e| e.reason())
                }
            }
        };
        let write = |copy: &[u8], out: &mut BufWriter<File>| out.write_all(copy);
        match finish(&path, &o, kind, result, write)? {
            Done::Made => made += 1,
            Done::Failed => failed += 1,
            Done::Nothing => {}
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
