//! What a PDF does, besides being read: scripts a viewer runs, programs or
//! files it asks to open, answers a form sends away, a site it goes to when
//! opened, and where its links go (ISO 32000-1 §12.6). Said in words for the
//! person about to open it; nothing is run or visited.

use super::facts::{cap, decode, insert_update, text};
use super::lexer::Obj;
use super::{Ctx, ObjRec};
use crate::eml::links::host;
use crate::model::NodeId;
use crate::zip::DocumentFact;

/// Scripts quoted, at most; each quoted this long.
const MAX_QUOTED: usize = 2;
const MAX_SNIPPET: usize = 80;
/// Sites listed by name, at most.
const MAX_SITES: usize = 4;

/// A string, or a file specification's name (7.11.3).
fn file_name(ctx: &Ctx, o: &Obj) -> Option<String> {
    match o {
        Obj::Str(s) => Some(text(s)),
        Obj::Dict(_) => ["UF", "F"].iter().find_map(|k| match o.get(k) {
            Some(Obj::Str(s)) => Some(text(s)),
            _ => None,
        }),
        Obj::Ref(n, _) => latest(ctx, *n).and_then(|r| file_name(ctx, &r.value)),
        _ => None,
    }
}

fn latest(ctx: &Ctx, n: u32) -> Option<&ObjRec> {
    ctx.objects.iter().rev().find(|o| o.num == n)
}

/// A script's text, from a string or a stream, on one line and short.
fn script(data: &[u8], ctx: &Ctx, o: &Obj, budget: &mut u64) -> Option<String> {
    let raw = match o {
        Obj::Str(s) => text(s),
        Obj::Ref(n, _) => {
            let rec = latest(ctx, *n)?;
            String::from_utf8_lossy(&decode(data, rec, ctx.crypt.as_ref(), budget)?).into_owned()
        }
        _ => return None,
    };
    let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = line.chars().take(MAX_SNIPPET).collect();
    (!short.is_empty()).then(|| {
        if short.len() < line.len() {
            format!("{short}…")
        } else {
            short
        }
    })
}

/// `a, b and 2 more`.
fn listed(items: &[String], max: usize) -> String {
    let shown = items
        .iter()
        .take(max)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match items.len().saturating_sub(max) {
        0 => shown,
        n => format!("{shown} and {n} more"),
    }
}

pub(super) fn check(data: &[u8], ctx: &Ctx, facts: &mut Vec<DocumentFact>) {
    let mut budget = super::facts::MAX_DECODED_TOTAL;
    let (mut scripts, mut launches, mut sends, mut sites) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut links = 0usize;
    // Where each kind was first met, to point at.
    let mut nodes: Vec<(&'static str, NodeId)> = Vec::new();
    // The latest version of each object — what a viewer uses — in the
    // order the file has them.
    // By number, the last met first within each; one of each; file order.
    let mut order: Vec<(u32, usize)> = ctx
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.num, usize::MAX - i))
        .collect();
    order.sort_unstable();
    order.dedup_by_key(|(n, _)| *n);
    let mut current: Vec<usize> = order.into_iter().map(|(_, i)| usize::MAX - i).collect();
    current.sort_unstable();
    for rec in current.into_iter().map(|i| &ctx.objects[i]) {
        // An action is an object of its own, or written into a link's /A.
        let inline = match rec.value.get("A") {
            Some(a @ Obj::Dict(_)) => Some(a),
            _ => None,
        };
        for d in std::iter::once(&rec.value).chain(inline) {
            match d.get("S").and_then(Obj::name) {
                Some("JavaScript") => {
                    if let Some(s) = d
                        .get("JS")
                        .and_then(|js| script(data, ctx, js, &mut budget))
                        && !scripts.contains(&s)
                    {
                        scripts.push(s);
                    }
                    nodes.push(("scripts", rec.node));
                }
                Some("Launch") => {
                    let f = d
                        .get("F")
                        .or_else(|| d.get("Win").and_then(|w| w.get("F")))
                        .and_then(|f| file_name(ctx, f))
                        .unwrap_or_else(|| "a program".into());
                    if !launches.contains(&f) {
                        launches.push(f);
                    }
                    nodes.push(("launch", rec.node));
                }
                Some("SubmitForm") => {
                    if let Some(to) = d.get("F").and_then(|f| file_name(ctx, f)) {
                        let to = host(&to).unwrap_or(to);
                        if !sends.contains(&to) {
                            sends.push(to);
                        }
                    }
                    nodes.push(("submits", rec.node));
                }
                Some("URI") => {
                    if let Some(Obj::Str(u)) = d.get("URI")
                        && let Some(h) = host(&text(u))
                    {
                        links += 1;
                        if !sites.contains(&h) {
                            sites.push(h);
                        }
                        nodes.push(("weblinks", rec.node));
                    }
                }
                _ => {}
            }
        }
    }
    let node_of = |kind: &str| nodes.iter().find(|(k, _)| *k == kind).map(|(_, n)| *n);

    // What it does when opened: the catalog's OpenAction (12.6.2).
    let root = ctx.trailers.iter().rev().find_map(|t| match t.get("Root") {
        Some(&Obj::Ref(n, _)) => Some(n),
        _ => None,
    });
    let open = root
        .and_then(|n| latest(ctx, n))
        .and_then(|c| c.value.get("OpenAction").cloned())
        .and_then(|a| match a {
            Obj::Ref(n, _) => latest(ctx, n).map(|r| (r.value.clone(), r.node)),
            d @ Obj::Dict(_) => root.and_then(|n| latest(ctx, n)).map(|c| (d, c.node)),
            _ => None,
        });
    let on_open = open.and_then(|(action, node)| {
        let what = match action.get("S").and_then(Obj::name) {
            Some("JavaScript") => "runs a script".to_string(),
            Some("URI") => match action.get("URI") {
                Some(Obj::Str(u)) => format!("goes to {}", host(&text(u))?),
                _ => return None,
            },
            Some("Launch") => "asks to start a program".to_string(),
            _ => return None,
        };
        Some((what, node))
    });
    if let Some((what, node)) = on_open {
        insert_update(
            facts,
            DocumentFact {
                kind: "opens",
                text: cap(format!("it {what}")),
                node,
            },
        );
    }
    if let Some(node) = node_of("scripts") {
        let text = if scripts.is_empty() {
            "JavaScript, for the viewer to run".to_string()
        } else {
            let q: Vec<String> = scripts
                .iter()
                .take(MAX_QUOTED)
                .map(|s| format!("“{s}”"))
                .collect();
            let more = scripts.len().saturating_sub(MAX_QUOTED);
            format!(
                "JavaScript, for the viewer to run: {}{}",
                q.join("; "),
                if more > 0 {
                    format!("; and {more} more")
                } else {
                    String::new()
                }
            )
        };
        insert_update(
            facts,
            DocumentFact {
                kind: "scripts",
                text,
                node,
            },
        );
    }
    if let Some(node) = node_of("launch") {
        let q: Vec<String> = launches.iter().map(|l| format!("“{l}”")).collect();
        insert_update(
            facts,
            DocumentFact {
                kind: "launch",
                text: format!("asks to open {}", listed(&q, MAX_QUOTED)),
                node,
            },
        );
    }
    if let Some(node) = node_of("submits") {
        let text = if sends.is_empty() {
            "sends its form's answers away when submitted".to_string()
        } else {
            format!("sends its form's answers to {}", listed(&sends, MAX_SITES))
        };
        insert_update(
            facts,
            DocumentFact {
                kind: "submits",
                text,
                node,
            },
        );
    }
    if let Some(node) = node_of("weblinks") {
        let text = format!(
            "{links} {} to {}",
            if links == 1 { "link," } else { "links," },
            listed(&sites, MAX_SITES)
        );
        insert_update(
            facts,
            DocumentFact {
                kind: "weblinks",
                text,
                node,
            },
        );
    }
}
