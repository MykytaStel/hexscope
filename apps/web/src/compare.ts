// Two files side by side: what one gives away that the other does not, what
// is wrong with each, and — for whoever wants it — which parts of the
// structure only one has, or both have differently. Made for "what did the
// clean copy take out?" and "what changed between these two versions?".
//
// The second file is parsed by a worker of its own, so the one on screen
// keeps its place in the page's worker.
import { semanticDiff } from "./semantic-diff";
import { FACT_LABELS } from "./finding-kinds";
import { PARSE_DEADLINE_MS } from "./resource-budget";
import { Concern, FileModel, Kind } from "./model";
import { categories } from "./share";
import type { WorkerRequest, WorkerResponse } from "./worker";
import { dismissable } from "./dialogs";
import { currentLocale, translateText } from "./i18n";
import { decodePicture, drawn } from "./thumbnail";

/** Parts listed per group, at most. */
const MAX_LISTED = 200;
/** Bytes of a part compared directly; longer parts compare by a sample. */
const MAX_HASHED = 1 << 16;

/** Parses a file in a worker made for it, then lets the worker go. */
export async function parseAside(file: File): Promise<FileModel> {
  const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  try {
    const response = await new Promise<WorkerResponse>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Reading took too long; try a smaller file or use the command line tool")), PARSE_DEADLINE_MS);
      const finish = (response: WorkerResponse) => {
        clearTimeout(timer);
        resolve(response);
      };
      worker.addEventListener("message", (e: MessageEvent<WorkerResponse>) => {
        if (e.data.id !== 1 || e.data.type === "progress") return;
        finish(e.data);
      });
      worker.addEventListener("error", () => finish({ id: 1, type: "error", message: "The comparison reader stopped; try a smaller file" }));
      worker.addEventListener("messageerror", () => finish({ id: 1, type: "error", message: "The comparison reader could not return this file" }));
      try { worker.postMessage({ id: 1, type: "parse", file } as WorkerRequest); }
      catch (error) { clearTimeout(timer); reject(error); }
    });
    if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
    return new FileModel(response.result, response.bytes, file.name, file);
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

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const tr = (s: string) => translateText(s, currentLocale());
const number = (n: number) => n.toLocaleString(currentLocale());
const PREVIEW_BYTES = 16 * 1024 * 1024;
const PREVIEW_PIXELS = 40_000_000;
/** Two full-size bitmaps are decoded for the wipe; cap their combined peak memory. */
const WIPE_PIXELS = 12_000_000;

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
    box.append(el("p", "hint", `${tr("The first difference is in")} ${a.name}: ${a.path(part).map((id) => a.label(id)).join(" › ")}.`));
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
  box.append(grid, el("p", "hint", `${tr("File A")}: ${a.name} · ${tr("File B")}: ${b.name}. ${tr("Empty cells are past the end of a file.")}`));
  return box;
}

function section(title: string, className: string): HTMLElement {
  const box = el("section", `compare-section ${className}`);
  box.append(el("h3", undefined, tr(title)));
  return box;
}

function canPreview(model: FileModel, maxPixels = PREVIEW_PIXELS): boolean {
  const { format, dimensions } = model.file;
  return ["jpeg", "png", "heif", "webp", "gif"].includes(format) &&
    model.bytes.length <= PREVIEW_BYTES &&
    dimensions !== undefined && dimensions !== null &&
    dimensions[0] > 0 && dimensions[1] > 0 &&
    dimensions[0] * dimensions[1] <= maxPixels;
}

export function canCompareVisual(a: FileModel, b: FileModel): boolean {
  return canPreview(a, WIPE_PIXELS) && canPreview(b, WIPE_PIXELS);
}

function fileCard(model: FileModel, side: "a" | "b", mediaUrls: string[], showPreview = true): HTMLElement {
  const card = el("article", "compare-file");
  card.dataset.side = side;
  card.append(el("span", "compare-file-label", tr(side === "a" ? "File A" : "File B")));
  const format = model.file.format;
  if (showPreview && format === "video" && model.source) {
    const frame = el("div", "compare-preview");
    const player = el("video", "compare-video-preview");
    player.controls = true;
    player.playsInline = true;
    player.preload = "metadata";
    player.setAttribute("aria-label", tr(side === "a" ? "Preview of file A" : "Preview of file B"));
    const url = URL.createObjectURL(model.source);
    mediaUrls.push(url);
    player.src = url;
    frame.append(player);
    card.append(frame);
  } else if (showPreview && canPreview(model)) {
    const frame = el("div", "compare-preview");
    card.append(frame);
    void decodePicture(model)
      .then((bitmap) => {
        if (!bitmap) {
          frame.remove();
          return;
        }
        try {
          const shown = Math.max(frame.clientWidth, frame.clientHeight, 320) * (devicePixelRatio || 1);
          const preview = drawn(bitmap, format === "jpeg" ? model.file.orientation : 1, Math.min(2048, Math.ceil(shown)));
          preview.className = "compare-preview-image";
          preview.setAttribute("role", "img");
          preview.setAttribute("aria-label", tr(side === "a" ? "Preview of file A" : "Preview of file B"));
          frame.append(preview);
        } catch {
          frame.remove();
        } finally {
          bitmap.close();
        }
      })
      .catch(() => frame.remove());
  }
  card.append(
    el("strong", "compare-file-name", model.name),
    el("span", "compare-file-format", format.toUpperCase()),
    el("span", "compare-file-size", `${model.bytes.length.toLocaleString("en")} bytes`),
    el("span", "compare-file-fields", `${tr("Metadata fields")}: ${number(model.file.facts.length + Number(Boolean(model.file.location)))}`),
  );
  return card;
}

function isStill(model: FileModel): boolean {
  return ["jpeg", "png", "heif", "webp", "gif"].includes(model.file.format);
}

function motionPairNote(a: FileModel, b: FileModel): HTMLElement | null {
  if (!((isStill(a) && b.file.format === "video") || (isStill(b) && a.file.format === "video"))) return null;
  const note = section("Still + motion preview", "compare-motion-pair");
  note.append(el("p", "hint", tr("Choose the still photo and motion clip from the same Live Photo to inspect them side by side. The pairing identifier is not checked.")));
  return note;
}

/** A bounded, browser-rendered image wipe; this is a visual aid, not a pixel audit. */
function visualWipe(a: FileModel, b: FileModel): HTMLElement {
  const visualSection = section("Visual comparison", "compare-wipe-section");
  const frame = el("div", "compare-wipe");
  frame.setAttribute("role", "img");
  frame.setAttribute("aria-label", tr("File A and file B previews, split by the slider"));
  frame.style.setProperty("--wipe-position", "50%");
  const labels = el("div", "compare-wipe-labels");
  labels.append(el("span", undefined, tr("File A")), el("span", undefined, tr("File B")));
  frame.append(labels);
  const divider = el("span", "compare-wipe-divider");
  divider.setAttribute("aria-hidden", "true");
  frame.append(divider);

  const slider = el("input", "compare-wipe-control");
  slider.type = "range";
  slider.min = "0";
  slider.max = "100";
  slider.value = "50";
  slider.setAttribute("aria-label", tr("Reveal file A"));
  slider.setAttribute("aria-valuetext", `50% · ${tr("File A")}`);
  slider.addEventListener("input", () => {
    frame.style.setProperty("--wipe-position", `${slider.value}%`);
    slider.setAttribute("aria-valuetext", `${slider.value}% · ${tr("File A")}`);
  });

  const note = el("p", "compare-wipe-note", tr("These are scaled previews rendered by this browser. Colour and HDR appearance may differ from the original; metadata is compared separately."));
  visualSection.append(frame, slider, note);
  void Promise.all([decodePicture(a), decodePicture(b)])
    .then(([left, right]) => {
      if (!left || !right) {
        left?.close();
        right?.close();
        frame.replaceChildren(labels, el("p", "hint", tr("This browser could not decode both images for a visual comparison.")));
        return;
      }
      try {
        const cap = 1600;
        const leftCanvas = drawn(left, a.file.format === "jpeg" ? a.file.orientation : 1, cap);
        const rightCanvas = drawn(right, b.file.format === "jpeg" ? b.file.orientation : 1, cap);
        leftCanvas.className = "compare-wipe-image is-a";
        rightCanvas.className = "compare-wipe-image is-b";
        leftCanvas.setAttribute("aria-hidden", "true");
        rightCanvas.setAttribute("aria-hidden", "true");
        frame.append(rightCanvas, leftCanvas, divider);
      } catch {
        frame.replaceChildren(labels, el("p", "hint", tr("This browser could not prepare both images for a visual comparison.")));
      } finally {
        left.close();
        right.close();
      }
    })
    .catch(() => frame.replaceChildren(labels, el("p", "hint", tr("This browser could not decode both images for a visual comparison."))));
  return visualSection;
}

function metric(label: string, value: string | number): HTMLElement {
  const card = el("div", "compare-metric");
  card.append(el("span", "compare-metric-label", tr(label)), el("strong", "compare-metric-value", String(value)));
  return card;
}

/** Shows a comparison in a dialog. */
export function showComparison(a: FileModel, b: FileModel, c: Comparison): void {
  const dialog = el("dialog", "report compare");
  const mediaUrls: string[] = [];
  dialog.setAttribute("aria-labelledby", "compare-title");
  dialog.addEventListener("close", () => {
    for (const url of mediaUrls) URL.revokeObjectURL(url);
    dialog.remove();
  });

  const heading = el("header", "compare-header");
  const title = el("h2", undefined, tr("Compare"));
  title.id = "compare-title";
  heading.append(title);

  const pair = el("div", "compare-pair");
  const wipe = canCompareVisual(a, b);
  pair.append(fileCard(a, "a", mediaUrls, !wipe), el("span", "compare-vs", "vs."), fileCard(b, "b", mediaUrls, !wipe));

  const summary = el("div", "compare-summary");
  summary.setAttribute("aria-label", tr("Finding summary"));
  summary.append(metric("Findings only in A", c.gone.length), metric("Findings only in B", c.added.length), metric("Findings in both", c.kept.length));

  const reveals = section("What they give away", "compare-findings");
  const semantic = semanticDiff(a.file, b.file);
  const say = (en: string, uk: string) => currentLocale() === "uk" ? uk : en;
  if (semantic.changed.length) reveals.append(el("p", "compare-semantic", `${say("Stored values changed", "Збережені значення змінилися")}: ${semantic.changed.map((kind) => tr(FACT_LABELS[kind] ?? kind)).join(", ")}`));
  if (!semantic.complete) reveals.append(el("p", "hint", say("At least one inspection is incomplete. An absent finding does not prove removal.", "Щонайменше один аналіз неповний. Відсутність знахідки не доводить її видалення.")));
  reveals.append(el("p", "hint", c.firstDiff === -1 && !c.mediaByLength
    ? say("All file bytes match.", "Усі байти файлів збігаються.")
    : say("File bytes differ. Metadata comparison does not prove that the visible content is unchanged.", "Байти файлів різняться. Порівняння метаданих не доводить незмінність видимого вмісту.")));

  const findingGrid = el("div", "compare-finding-grid");
  const tags = (items: string[]) => {
    const box = el("div", "compare-tags");
    box.append(...items.map((t) => el("span", "tag is-reveals", tr(cap(t)))));
    return box;
  };
  if (c.gone.length + c.added.length + c.kept.length === 0) {
    reveals.append(el("p", "hint compare-empty", tr("Neither file gives anything away.")));
  } else {
    const findingColumn = (label: string, items: string[]) => {
      const column = el("details", "compare-finding-column");
      column.append(el("summary", undefined, `${tr(label)} · ${number(items.length)}`), tags(items));
      findingGrid.append(column);
    };
    if (c.gone.length) findingColumn("Only in A", c.gone);
    if (c.added.length) findingColumn("Only in B", c.added);
    if (c.kept.length) findingColumn("In both", c.kept);
  }
  if (findingGrid.childElementCount) reveals.append(findingGrid);

  const problems = section("File issues", "compare-problems");
  const problemGrid = el("div", "compare-problem-grid");
  const problemCard = (model: FileModel, side: "a" | "b", count: number, damage: number) => {
    const card = el("div", "compare-problem-file");
    const head = el("div", "compare-problem-head");
    head.append(
      el("span", "compare-problem-label", tr(side === "a" ? "File A" : "File B")),
      el("strong", "compare-problem-name", model.name),
    );
    const stats = el("div", "compare-problem-stats");
    stats.append(metric("Problems", count), metric("Damage", damage));
    card.append(head, stats);
    problemGrid.append(card);
  };
  problemCard(a, "a", c.problems.a, c.problems.damageA);
  problemCard(b, "b", c.problems.b, c.problems.damageB);
  problems.append(problemGrid);

  const structure = section("Structure", "compare-structure");
  const structureSummary = el("div", "compare-structure-summary");
  structureSummary.append(
    metric("Same parts", c.counts.same),
    metric("Changed parts", c.counts.changed),
    metric("Parts only in A", c.counts.onlyA),
    metric("Parts only in B", c.counts.onlyB),
  );
  structure.append(structureSummary);
  if (c.mediaByLength) structure.append(el("p", "hint", tr("The picture and sound of a large movie are compared by their length, not byte by byte.")));
  const list = (title: string, items: string[], more: number) => {
    if (items.length === 0) return;
    const d = el("details", "compare-list-detail");
    d.append(el("summary", undefined, `${tr(title)} · ${number(more)}`));
    const ul = el("ul", "compare-list");
    ul.append(...items.map((p) => el("li", undefined, p)));
    if (more > items.length) ul.append(el("li", "hint", `… and ${(more - items.length).toLocaleString("en")} more`));
    d.append(ul);
    structure.append(d);
  };
  list("Only in A", c.onlyA, c.counts.onlyA);
  list("Only in B", c.onlyB, c.counts.onlyB);
  list("Different", c.changed.map((x) => (x.a !== x.b ? `${x.path}: ${x.a || "—"} → ${x.b || "—"}` : `${x.path}: its bytes`)), c.counts.changed);
  if (c.firstDiff < 0) {
    structure.append(el("p", "hint compare-identical", tr("The bytes are the same.")));
  } else {
    const bytes = el("details", "compare-byte-detail");
    bytes.append(el("summary", undefined, `${tr("Inspect byte differences")} · 0x${c.firstDiff.toString(16).toUpperCase()}`));
    bytes.append(firstDifference(a, b, c.firstDiff));
    structure.append(bytes);
  }

  dialog.append(heading, pair);
  const motionNote = motionPairNote(a, b);
  if (motionNote) dialog.append(motionNote);
  if (wipe) dialog.append(visualWipe(a, b));
  dialog.append(summary, reveals, problems, structure);
  document.body.append(dialog);
  dismissable(dialog);
  const close = dialog.querySelector<HTMLButtonElement>(".dialog-x");
  if (close) {
    close.setAttribute("aria-label", tr("Close comparison"));
    close.title = tr("Close comparison");
    heading.append(close);
  }
  dialog.showModal();
}
