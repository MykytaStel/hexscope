//! What a PDF carries besides its pages: the answers typed into its form
//! (12.7), kept as data that anything reading the file gets, whatever the
//! page shows; and the files attached to it (7.11.4), by name.

use super::Ctx;
use super::facts::{Found, cap, decrypt_strings, insert_update, resolve, text};
use super::lexer::Obj;
use crate::model::NodeId;
use crate::zip::DocumentFact;

/// Fields and attachments read, at most, and how deep a form's fields nest.
const MAX_FIELDS: usize = 500;
const MAX_DEPTH: usize = 12;
/// Answers and names quoted in a line, at most; the rest are counted.
const MAX_QUOTED: usize = 4;
const BUDGET: u64 = 16 * 1024 * 1024;

/// An object, its strings decrypted when the document is, and its node.
fn get(data: &[u8], ctx: &Ctx, obj: &Obj, budget: &mut u64) -> Option<(Obj, Option<NodeId>)> {
    match obj {
        Obj::Ref(n, _) => match resolve(data, ctx, *n, budget)? {
            Found::Top(rec) => Some((
                match &ctx.crypt {
                    Some(c) => decrypt_strings(&rec.value, c, rec.num, rec.gen_),
                    None => rec.value.clone(),
                },
                Some(rec.node),
            )),
            Found::Packed(o, node) => Some((o, Some(node))),
        },
        other => Some((other.clone(), None)),
    }
}

/// A value as a person would read it: a string's text, a name's name.
fn shown(v: &Obj) -> Option<String> {
    let t = match v {
        Obj::Str(s) => text(s),
        // Checkboxes and choices: /Off is no answer.
        Obj::Name(n) if n != "Off" => n.clone(),
        Obj::Array(items) => items
            .iter()
            .filter_map(|i| shown(&i.obj))
            .collect::<Vec<_>>()
            .join(", "),
        _ => return None,
    };
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

fn quote(items: &[String]) -> String {
    let q: Vec<String> = items.iter().take(MAX_QUOTED).cloned().collect();
    let more = items.len().saturating_sub(MAX_QUOTED);
    if more > 0 {
        format!("{} and {more} more", q.join(", "))
    } else {
        q.join(", ")
    }
}

pub(super) fn check(data: &[u8], ctx: &Ctx, facts: &mut Vec<DocumentFact>) {
    let mut budget = BUDGET;
    let Some(root) = ctx
        .trailers
        .iter()
        .rev()
        .find_map(|t| t.get("Root").cloned())
    else {
        return;
    };
    let Some((catalog, catalog_node)) = get(data, ctx, &root, &mut budget) else {
        return;
    };

    // The form: each field's full name and its answer, through the tree of
    // fields and their kids.
    let mut answers: Vec<String> = Vec::new();
    let mut node = None;
    if let Some((form, form_node)) = catalog
        .get("AcroForm")
        .and_then(|f| get(data, ctx, f, &mut budget))
        && let Some(Obj::Array(fields)) = form
            .get("Fields")
            .and_then(|f| get(data, ctx, f, &mut budget))
            .map(|x| x.0)
    {
        let mut stack: Vec<(Obj, String, usize)> = fields
            .iter()
            .rev()
            .map(|f| (f.obj.clone(), String::new(), 0))
            .collect();
        let mut seen = 0;
        while let Some((f, parent, depth)) = stack.pop() {
            seen += 1;
            if seen > MAX_FIELDS || depth > MAX_DEPTH {
                break;
            }
            let Some((field, field_node)) = get(data, ctx, &f, &mut budget) else {
                continue;
            };
            let part = field.get("T").and_then(shown).unwrap_or_default();
            let name = match (parent.is_empty(), part.is_empty()) {
                (true, _) => part,
                (false, true) => parent,
                (false, false) => format!("{parent}.{part}"),
            };
            if let Some(v) = field.get("V").and_then(shown) {
                node = node.or(field_node).or(form_node);
                answers.push(if name.is_empty() {
                    format!("“{v}”")
                } else {
                    format!("{name}: “{v}”")
                });
            }
            if let Some(Obj::Array(kids)) = field.get("Kids") {
                stack.extend(
                    kids.iter()
                        .rev()
                        .map(|k| (k.obj.clone(), name.clone(), depth + 1)),
                );
            }
        }
    }
    if !answers.is_empty() {
        let n = answers.len();
        let text = format!(
            "{n} {} filled in: {}",
            if n == 1 { "field" } else { "fields" },
            quote(&answers)
        );
        let node = node.or(catalog_node).unwrap_or(0);
        insert_update(
            facts,
            DocumentFact {
                kind: "form",
                text: cap(text),
                node,
            },
        );
    }

    // Attachments: the document's named files, and files attached to pages.
    let mut names: Vec<String> = Vec::new();
    let mut node = None;
    let mut push = |spec: &Obj, at: Option<NodeId>, budget: &mut u64| {
        let Some((spec, spec_node)) = get(data, ctx, spec, budget) else {
            return;
        };
        let name = spec
            .get("UF")
            .or(spec.get("F"))
            .and_then(shown)
            .or_else(|| shown(&spec));
        if let Some(n) = name
            && !names.contains(&n)
            && names.len() < MAX_FIELDS
        {
            node = node.or(spec_node).or(at);
            names.push(n);
        }
    };
    if let Some((tree, tree_node)) = catalog
        .get("Names")
        .and_then(|n| get(data, ctx, n, &mut budget))
        .and_then(|(n, _)| n.get("EmbeddedFiles").cloned())
        .and_then(|e| get(data, ctx, &e, &mut budget))
    {
        // A name tree: pairs of a name and a file spec, and kids that hold more.
        let mut stack = vec![(tree, 0usize)];
        let mut seen = 0;
        while let Some((t, depth)) = stack.pop() {
            seen += 1;
            if seen > MAX_FIELDS || depth > MAX_DEPTH {
                break;
            }
            if let Some(Obj::Array(pairs)) = t.get("Names") {
                for pair in pairs.chunks(2) {
                    if let [_, spec] = pair {
                        push(&spec.obj, tree_node, &mut budget);
                    }
                }
            }
            if let Some(Obj::Array(kids)) = t.get("Kids") {
                for k in kids {
                    if let Some((kid, _)) = get(data, ctx, &k.obj, &mut budget) {
                        stack.push((kid, depth + 1));
                    }
                }
            }
        }
    }
    for rec in ctx
        .objects
        .iter()
        .filter(|o| o.value.get("Subtype").and_then(Obj::name) == Some("FileAttachment"))
    {
        if let Some(fs) = rec.value.get("FS") {
            push(fs, Some(rec.node), &mut budget);
        }
    }
    if !names.is_empty() {
        let quoted: Vec<String> = names.iter().map(|n| format!("“{n}”")).collect();
        let n = names.len();
        let text = format!(
            "{n} {}: {}",
            if n == 1 { "file" } else { "files" },
            quote(&quoted)
        );
        insert_update(
            facts,
            DocumentFact {
                kind: "attachments",
                text: cap(text),
                node: node.unwrap_or(0),
            },
        );
    }
}
