// "Report a problem with this file": the file's layout as text, for a bug
// report — its format, and each part's name, offset and size, with the
// problems first — and none of its content. Values are left out, and names
// an archive's entries carry are replaced by their number. The person sees
// the whole text before copying it.
import { Kind, type FileModel } from "./model";

/** The commit the site was built from, set by the build. */
declare const __BUILD__: string;

/** Levels of the tree listed, and lines at most. */
const DEPTH = 3;
const MAX_LINES = 400;
const ISSUE = "https://github.com/MykytaStel/hexscope/issues/new?template=bug.yml";

const at = (n: number) => `@0x${n.toString(16).toUpperCase().padStart(8, "0")}`;

/** A node's name as the report shows it: an archive entry by its number. */
function name(m: FileModel, id: number): string {
  if (m.file.format === "zip") {
    const e = m.entryOf(id);
    if (e >= 0 && m.entry(e).node === id) return `entry ${e}`;
    // The central directory's records are named after their files too.
    const parent = m.file.parents[id];
    if (parent >= 0 && m.label(parent) === "central directory") {
      return `record ${Array.prototype.indexOf.call(m.children(parent), id)}`;
    }
  }
  let label = m.label(id);
  // Two archive problems quote an entry's name: keep what they say, not whom.
  if (m.file.format === "zip") {
    label = label
      .replace(/^overlaps .*?: the same bytes/, "overlaps another entry: the same bytes")
      .replace(/^(name differs from the central directory's):.*$/, "$1");
  }
  return label.length > 80 ? `${label.slice(0, 80)}…` : label;
}

export function structureReport(m: FileModel): string {
  const f = m.file;
  const lines = [
    "hexscope structure report — the file's layout, none of its content",
    `build ${typeof __BUILD__ === "string" ? __BUILD__ : "dev"}`,
    `format ${f.format} · ${m.bytes.length.toLocaleString("en")} bytes · parsed in ${f.parseMs.toFixed(1)} ms`,
    "",
  ];
  const kinds: Record<number, string> = { [Kind.Warning]: "warning", [Kind.Error]: "error" };
  lines.push(`problems: ${m.problems.length}`);
  for (const id of m.problems.slice(0, 50)) {
    lines.push(`  ${kinds[m.kind(id)] ?? "?"}  ${at(m.start(id))} +${m.len(id)}  ${name(m, id)}`);
  }
  if (m.problems.length > 50) lines.push(`  … ${m.problems.length - 50} more`);
  lines.push("", `tree, ${DEPTH} levels, values left out:`);
  let listed = 0;
  let skipped = 0;
  const walk = (id: number, depth: number) => {
    if (listed >= MAX_LINES) {
      skipped++;
      return;
    }
    listed++;
    lines.push(`${"  ".repeat(depth + 1)}${at(m.start(id))} +${m.len(id)}  ${name(m, id)}`);
    if (depth + 1 >= DEPTH) return;
    for (const c of m.children(id)) walk(c, depth + 1);
  };
  walk(0, 0);
  if (skipped > 0) lines.push(`  … ${skipped} more parts at these levels`);
  return lines.join("\n");
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Shows the report in a dialog, to read, copy, and take to an issue. */
export function openReport(m: FileModel): void {
  const dialog = el("dialog", "report");
  const text = el("textarea", "report-text");
  text.readOnly = true;
  text.value = structureReport(m);
  text.rows = 14;
  const copy = el("button", "btn btn-primary", "Copy");
  copy.addEventListener("click", async () => {
    text.select();
    try {
      await navigator.clipboard.writeText(text.value);
    } catch {
      document.execCommand("copy");
    }
    copy.textContent = "Copied";
  });
  const issue = el("a", "btn", "Open an issue on GitHub ↗");
  issue.href = ISSUE;
  issue.target = "_blank";
  issue.rel = "noopener noreferrer";
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => dialog.remove());
  const actions = el("div", "report-actions");
  actions.append(copy, issue, close);
  dialog.append(
    el("h2", undefined, "Report a problem with this file"),
    el(
      "p",
      "hint",
      "This is the file's layout — its parts, where they are and how big — and none of what it says. Read it before you share it: nothing is sent unless you paste it somewhere yourself.",
    ),
    text,
    actions,
  );
  document.body.append(dialog);
  dialog.showModal();
}
