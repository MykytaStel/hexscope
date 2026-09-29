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
import { dismissable } from "./dialogs";

/** Parts listed per group, at most. */
const MAX_LISTED = 200;
/** Bytes of a part compared directly; longer parts compare by a sample. */
const MAX_HASHED = 1 << 16;

/** Parses a file in a worker made for it, then lets the worker go. */
export async function parseAside(file: File): Promise<FileModel> {
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  try {
    const response = await new Promise<WorkerResponse>((resolve) => {
      worker.addEventListener("message", (e: MessageEvent<WorkerResponse>) => resolve(e.data), { once: true });
      worker.postMessage({ id: 1, type: "parse", file } as WorkerRequest);
    });
    if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
    return new FileModel(response.result, response.bytes, file.name);
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
  /** Whether a large movie's media was compared by its length alone. */
  mediaByLength: boolean;
}

/** Whether any of `[start, start + len)` is media the page has not read. */
const unread = (m: FileModel, start: number, len: number) => m.missing !== null && m.missing.wanted(start, start + len).length > 0;

/** The first offset below `n` where the bytes differ, a large movie's media aside; `n` when there is none. */
function diffAt(a: FileModel, b: FileModel, n: number): number {
  const media = [...(a.missing?.ranges ?? []), ...(b.missing?.ranges ?? [])].sort((x, y) => x[0] - y[0]);
  let i = 0;
  for (const [from, to] of [...media, [n, n]]) {
    for (const stop = Math.min(from, n); i < stop; i++) if (a.bytes[i] !== b.bytes[i]) return i;
    i = Math.max(i, to);
  }
  return n;
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
    mediaByLength: a.missing !== null || b.missing !== null,
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
    const read = !unread(a, a.start(ia), a.len(ia)) && !unread(b, b.start(ib), b.len(ib));
    if (va !== vb || (leaf && (a.len(ia) !== b.len(ib) || (read && digest(a, ia) !== digest(b, ib))))) {
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
  const i = diffAt(a, b, n);
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

/** Bytes shown per row around the first difference: narrow enough for a phone. */
const ROW = 8;

/**
 * The bytes around where the two first differ, one file's row over the
 * other's, the differing bytes marked; and the part of the first file the
 * offset falls in.
 */
function firstDifference(a: FileModel, b: FileModel, at: number): HTMLElement {
  const box = el("div", "compare-bytes");
  const part = a.nodeAt(at);
  if (part > 0) {
    box.append(el("p", "hint", `In ${a.name}, that is in ${a.path(part).map((id) => a.label(id)).join(" › ")}.`));
  }
  const grid = el("div", "compare-rows");
  grid.setAttribute("role", "img");
  grid.setAttribute("aria-label", `Bytes of both files from offset 0x${at.toString(16).toUpperCase()}, the differing ones marked`);
  const from = Math.max(0, Math.floor(at / ROW) * ROW - ROW);
  const to = Math.min(Math.max(a.bytes.length, b.bytes.length), from + 4 * ROW);
  const hex = (n: number) => n.toString(16).toUpperCase().padStart(2, "0");
  for (let row = from; row < to; row += ROW) {
    for (const [side, m, o] of [["A", a, b], ["B", b, a]] as const) {
      const line = el("div", "compare-row");
      line.append(el("span", "compare-side", side), el("span", "compare-offset", row.toString(16).toUpperCase().padStart(6, "0")));
      const bytes = el("span", "compare-hex");
      const text = el("span", "compare-ascii");
      for (let i = row; i < row + ROW; i++) {
        const v = i < m.bytes.length ? m.bytes[i] : -1;
        const differs = i >= at && v !== (i < o.bytes.length ? o.bytes[i] : -1);
        const cell = el(differs ? "mark" : "span", undefined, v < 0 ? "  " : hex(v));
        const ch = el(differs ? "mark" : "span", undefined, v < 0 ? " " : v >= 0x20 && v < 0x7f ? String.fromCharCode(v) : "·");
        bytes.append(cell);
        text.append(ch);
      }
      line.append(bytes, text);
      grid.append(line);
    }
  }
  box.append(grid, el("p", "hint", `A is ${a.name}, B is ${b.name}; past an end, blank.`));
  return box;
}

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
        `${cap(b.name)}: ${plural(c.problems.b, "problem", "problems")}${c.problems.damageB ? `, ${c.problems.damageB} damage` : ""}.`,
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
  if (c.mediaByLength) structure.append(el("p", "hint", "The picture and sound of a large movie are compared by their length, not byte by byte."));
  if (c.firstDiff >= 0) structure.append(firstDifference(a, b, c.firstDiff));
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
  dismissable(dialog);
  dialog.showModal();
}
