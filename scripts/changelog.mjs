// The changelog's next version, from one file a change in changes/: pull
// requests that each add their own file never touch the same lines, so they
// no longer conflict in CHANGELOG.md.
//
//   node scripts/changelog.mjs          what the next version will say
//   node scripts/changelog.mjs --check  fails on a file it cannot read (CI)
//   node scripts/changelog.mjs 0.7.0    writes it into CHANGELOG.md as 0.7.0,
//                                       dated today, and empties changes/
//
// A file holds Keep a Changelog sections (### Added, ### Changed, ### Fixed…)
// with a list under each. Newest files come first, as in the rest of the log.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const dir = join(root, "changes");
const log = join(root, "CHANGELOG.md");
const SECTIONS = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

/** The sections of one file, each a list of items as written; or what is wrong with it. */
function read(name) {
  const sections = new Map();
  const errors = [];
  let current = null;
  let item = null;
  readFileSync(join(dir, name), "utf8")
    .split("\n")
    .forEach((line, i) => {
      const heading = /^### (.+)$/.exec(line);
      if (heading) {
        if (!SECTIONS.includes(heading[1])) errors.push(`line ${i + 1}: "### ${heading[1]}" is none of ${SECTIONS.join(", ")}`);
        current = heading[1];
        if (!sections.has(current)) sections.set(current, []);
        item = null;
      } else if (line.startsWith("- ")) {
        if (!current) errors.push(`line ${i + 1}: an item before any ### section`);
        else sections.get(current).push((item = [line]));
      } else if (line.trim() === "") {
        if (item) item.push(line);
      } else if (item && /^\s/.test(line)) {
        item.push(line);
      } else {
        errors.push(`line ${i + 1}: text outside a list item`);
      }
    });
  for (const [section, items] of sections) if (items.length === 0) errors.push(`### ${section} has no items`);
  if (sections.size === 0) errors.push("no ### section");
  const items = new Map([...sections].map(([s, list]) => [s, list.map((lines) => lines.join("\n").trimEnd())]));
  return { items, errors };
}

/** When the file was first committed, in seconds; a file not committed yet is the newest. */
function added(name) {
  try {
    const out = execFileSync("git", ["log", "--diff-filter=A", "--format=%ct", "-1", "--", join("changes", name)], {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    return out ? Number(out) : Infinity;
  } catch {
    return Infinity;
  }
}

const names = readdirSync(dir)
  .filter((n) => n.endsWith(".md") && n !== "README.md")
  .map((n) => ({ name: n, at: added(n) }))
  .sort((a, b) => b.at - a.at || a.name.localeCompare(b.name))
  .map((f) => f.name);
const files = names.map((name) => ({ name, ...read(name) }));
const broken = files.filter((f) => f.errors.length > 0);
for (const f of broken) for (const e of f.errors) console.error(`changes/${f.name}: ${e}`);
if (broken.length > 0) process.exit(1);

/** The next version's sections, in Keep a Changelog's order. */
function merged() {
  const out = [];
  for (const section of SECTIONS) {
    const items = files.flatMap((f) => f.items.get(section) ?? []);
    if (items.length > 0) out.push(`### ${section}\n\n${items.join("\n")}`);
  }
  return out.join("\n\n");
}

const arg = process.argv[2];
if (arg === "--check") {
  console.log(`changes/: ${files.length} file${files.length === 1 ? "" : "s"}, all readable.`);
} else if (arg === undefined) {
  console.log(merged() || "Nothing in changes/ yet.");
} else {
  if (!/^\d+\.\d+\.\d+$/.test(arg)) throw new Error(`"${arg}" is not a version like 0.7.0`);
  if (files.length === 0) throw new Error("changes/ is empty: nothing to release");
  const text = readFileSync(log, "utf8");
  if (text.includes(`## [${arg}]`)) throw new Error(`CHANGELOG.md already has ${arg}`);
  const next = text.indexOf("\n## [", text.indexOf("## [Unreleased]") + 1);
  const previous = /^## \[(\d+\.\d+\.\d+)\]/m.exec(text.slice(next + 1))[1];
  const day = new Date().toISOString().slice(0, 10);
  const url = "https://github.com/MykytaStel/hexscope/compare";
  const written =
    `${text.slice(0, next)}\n## [${arg}] — ${day}\n\n${merged()}\n${text.slice(next)}`.replace(
      `[Unreleased]: ${url}/v${previous}...HEAD`,
      `[Unreleased]: ${url}/v${arg}...HEAD\n[${arg}]: ${url}/v${previous}...v${arg}`,
    );
  writeFileSync(log, written);
  for (const name of names) rmSync(join(dir, name));
  console.log(`CHANGELOG.md: ${arg} — ${day}, from ${names.length} file${names.length === 1 ? "" : "s"} in changes/ (now removed).`);
}
