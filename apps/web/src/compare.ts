// Two files side by side: what one gives away that the other does not, what
// is wrong with each, and — for whoever wants it — which parts of the
// structure only one has, or both have differently. Made for "what did the
// clean copy take out?" and "what changed between these two versions?".
//
// The second file is parsed by a worker of its own, so the one on screen
// keeps its place in the page's worker.
import { Concern, FileModel, Kind } from "./model";
import { categories } from "./share";
import type { WorkerRequest, WorkerResponse } from "./worker";

/** Parts listed per group, at most. */
const MAX_LISTED = 200;
/** Bytes of a part compared directly; longer parts compare by a sample. */
const MAX_HASHED = 1 << 16;

/** Parses a file in a worker made for it, then lets the worker go. */
export async function parseAside(file: File): Promise<FileModel> {
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  try {
    const [response, buffer] = await Promise.all([
      new Promise<WorkerResponse>((resolve) => {
        worker.addEventListener("message", (e: MessageEvent<WorkerResponse>) => resolve(e.data), { once: true });
        worker.postMessage({ id: 1, type: "parse", file } as WorkerRequest);
      }),
      file.arrayBuffer(),
    ]);
    if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
    return new FileModel(response.result, new Uint8Array(buffer), file.name);
  } finally {
    worker.terminate();
  }
}

/** A part's place in the tree, by labels; repeats numbered: `IFD0 › Make value`. */
function paths(m: FileModel): Map<string, number> {
  const out = new Map<string, number>();
  const name: string[] = new Array(m.file.parents.length);
  for (let id = 0; id < name.length; id++) {
    const p = m.file.parents[id];
    const base = id === 0 ? m.label(0) : `${name[p]} › ${m.label(id)}`;
    let key = base;
    for (let n = 2; out.has(key); n++) key = `${base} #${n}`;
    name[id] = key;
    out.set(key, id);
  }
  return out;
}

/** A short digest of a part's bytes: FNV-1a over them, or over a sample of a long one. */
function digest(m: FileModel, id: number): number {
  const start = m.start(id);
  const len = m.len(id);
  const step = len > MAX_HASHED ? Math.ceil(len / MAX_HASHED) : 1;
  let h = 0x811c9dc5 ^ len;
  for (let i = 0; i < len; i += step) h = Math.imul(h ^ m.bytes[start + i], 0x01000193);
  return h >>> 0;
}

export interface Comparison {
  gone: string[];
  added: string[];
  kept: string[];
  problems: { a: number; b: number; damageA: number; damageB: number };
  onlyA: string[];
  onlyB: string[];
  changed: { path: string; a: string; b: string }[];
  counts: { onlyA: number; onlyB: number; changed: number; same: number };
  /** The first offset where the bytes differ; -1 when one is the start of the other and they are the same length. */
  firstDiff: number;
}

export function compare(a: FileModel, b: FileModel): Comparison {
  const ca = categories(a);
  const cb = categories(b);
  const damage = (m: FileModel) =>
    m.problems.filter((id) => m.kind(id) === Kind.Error || m.doc(id)?.concern === Concern.Damage).length;
  const pa = paths(a);
  const pb = paths(b);
  const out: Comparison = {
    gone: ca.filter((c) => !cb.includes(c)),
    added: cb.filter((c) => !ca.includes(c)),
    kept: ca.filter((c) => cb.includes(c)),
    problems: { a: a.problems.length, b: b.problems.length, damageA: damage(a), damageB: damage(b) },
    onlyA: [],
    onlyB: [],
    changed: [],
    counts: { onlyA: 0, onlyB: 0, changed: 0, same: 0 },
    firstDiff: -1,
  };
  for (const [path, ia] of pa) {
    const ib = pb.get(path);
    if (ib === undefined) {
      out.counts.onlyA++;
      if (out.onlyA.length < MAX_LISTED) out.onlyA.push(path);
      continue;
    }
    const va = a.value(ia);
    const vb = b.value(ib);
    // A container differs by what is under it: its own bytes would count
    // every change below it again.
    const leaf = !a.hasChildren(ia) && !b.hasChildren(ib);
    if (va !== vb || (leaf && (a.len(ia) !== b.len(ib) || digest(a, ia) !== digest(b, ib)))) {
      out.counts.changed++;
      if (out.changed.length < MAX_LISTED) out.changed.push({ path, a: va, b: vb });
    } else {
      out.counts.same++;
    }
  }
  for (const path of pb.keys()) {
    if (pa.has(path)) continue;
    out.counts.onlyB++;
    if (out.onlyB.length < MAX_LISTED) out.onlyB.push(path);
  }
  const n = Math.min(a.bytes.length, b.bytes.length);
  let i = 0;
  while (i < n && a.bytes[i] === b.bytes[i]) i++;
  out.firstDiff = i === n && a.bytes.length === b.bytes.length ? -1 : i;
  return out;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

const plural = (n: number, one: string, many: string) => `${n.toLocaleString("en")} ${n === 1 ? one : many}`;
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** Shows a comparison in a dialog. */
export function showComparison(a: FileModel, b: FileModel, c: Comparison): void {
  const dialog = el("dialog", "report compare");
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => dialog.remove());

  const head = el("p", "compare-files");
  head.append(el("strong", undefined, a.name), ` (${a.file.format}, ${plural(a.bytes.length, "byte", "bytes")}) and `, el("strong", undefined, b.name), ` (${b.file.format}, ${plural(b.bytes.length, "byte", "bytes")})`);

  const reveals = el("section");
  reveals.append(el("h3", undefined, "What they give away"));
  const tags = (items: string[]) => {
    const box = el("div", "batch-tags");
    box.append(...items.map((t) => el("span", "tag is-reveals", cap(t))));
    return box;
  };
  if (c.gone.length + c.added.length + c.kept.length === 0) {
    reveals.append(el("p", "hint", "Neither gives anything away."));
  }
  if (c.gone.length) reveals.append(el("p", undefined, `Only ${a.name}:`), tags(c.gone));
  if (c.added.length) reveals.append(el("p", undefined, `Only ${b.name}:`), tags(c.added));
  if (c.kept.length) reveals.append(el("p", undefined, "Both:"), tags(c.kept));

  const problems = el("section");
  problems.append(
    el("h3", undefined, "What is wrong with them"),
    el(
      "p",
      undefined,
      `${a.name}: ${plural(c.problems.a, "problem", "problems")}${c.problems.damageA ? `, ${c.problems.damageA} damage` : ""}. ` +
        `${b.name}: ${plural(c.problems.b, "problem", "problems")}${c.problems.damageB ? `, ${c.problems.damageB} damage` : ""}.`,
    ),
  );

  const structure = el("section");
  structure.append(el("h3", undefined, "Their structure"));
  structure.append(
    el(
      "p",
      undefined,
      c.firstDiff < 0
        ? "The bytes are the same."
        : `${plural(c.counts.same, "part is", "parts are")} the same; ${plural(c.counts.changed, "part differs", "parts differ")}; ` +
            `${c.counts.onlyA.toLocaleString("en")} only in ${a.name}, ${c.counts.onlyB.toLocaleString("en")} only in ${b.name}. ` +
            `The bytes first differ at offset 0x${c.firstDiff.toString(16).toUpperCase()}.`,
    ),
  );
  const list = (title: string, items: string[], more: number) => {
    if (items.length === 0) return;
    const d = el("details");
    d.append(el("summary", undefined, `${title} (${more.toLocaleString("en")})`));
    const ul = el("ul", "compare-list");
    ul.append(...items.map((p) => el("li", undefined, p)));
    if (more > items.length) ul.append(el("li", "hint", `… and ${(more - items.length).toLocaleString("en")} more`));
    d.append(ul);
    structure.append(d);
  };
  list(`Only in ${a.name}`, c.onlyA, c.counts.onlyA);
  list(`Only in ${b.name}`, c.onlyB, c.counts.onlyB);
  list(
    "Different",
    c.changed.map((x) => (x.a !== x.b ? `${x.path}: ${x.a || "—"} → ${x.b || "—"}` : `${x.path}: its bytes`)),
    c.counts.changed,
  );

  dialog.append(el("h2", undefined, "Compare"), head, reveals, problems, structure, close);
  document.body.append(dialog);
  dialog.showModal();
}
