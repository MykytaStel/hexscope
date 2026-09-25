import "./style.css";
import { Drawer, formatBytes, type CleanResult } from "./drawer";
import { HexView } from "./hexview";
import { Minimap } from "./minimap";
import { Concern, FileModel, Kind } from "./model";
import { startDemo } from "./demo";
import { PictureView } from "./pixels";
import { Player } from "./player";
import { TreeView } from "./tree";
import { call, playerSource } from "./rpc";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let model: FileModel | null = null;
/** The documents above the one on screen: the file, then each entry opened. */
let levels: { model: FileModel; selected: number }[] = [];
let hover = -1;
let selected = -1;
let problemCursor = -1;
let loadId = 0;

const status = $("status");
const problemsBtn = $<HTMLButtonElement>("problems");
const locationBtn = $<HTMLButtonElement>("location");

const hex = new HexView($("hex"), {
  onHover: (id, offset) => {
    setHover(id);
    status.textContent = offset >= 0 && model ? describeOffset(offset) : "";
    picture.fromByte(offset);
    status.hidden = !status.textContent;
  },
  onSelect: select,
});
const minimap = new Minimap($("hex"), { onJump: (offset) => hex.scrollToOffset(offset) });
hex.onView = (start, end) => minimap.setView(start, end);
const tree = new TreeView($("tree"), { onHover: setHover, onSelect: (id) => point(id) });
const picture = new PictureView({
  locate: async (by, pos) => {
    const r = await call({ type: "locate", by, pos });
    return r.type === "located" ? r.step : new Float64Array(0);
  },
  blocks: async () => {
    const r = await call({ type: "blocks" });
    return r.type === "blocks" ? { map: r.map, note: r.note } : { map: new Float64Array(0), note: "" };
  },
  // The player owns the mark while it plays.
  onBytes: (start, end) => {
    if (document.body.classList.contains("is-playing")) return;
    hex.setHead(start, end);
    if (start >= 0) hex.revealOffset(start, false);
  },
  showBytes: (start, end) => {
    toBytes();
    // Its part of the file in the tree and the details, as well.
    if (model) select(model.nodeAt(start));
    hex.setHead(start, end);
    // Once the bytes are laid out, bring these into view.
    requestAnimationFrame(() => requestAnimationFrame(() => hex.scrollToOffset(start)));
  },
  onStep: (index) => {
    toBytes();
    void openPlayer().then(() => player.jump(index));
  },
});
// On a phone the file opens on its summary; the tree and the bytes are a
// tap away, and anything that points into the bytes goes there.
const narrow = matchMedia("(max-width: 900px)");
function setView(view: "summary" | "bytes"): void {
  document.body.dataset.view = view;
  for (const b of document.querySelectorAll<HTMLButtonElement>("#viewswitch button")) {
    b.setAttribute("aria-pressed", String(b.dataset.view === view));
  }
}
function toBytes(): void {
  if (narrow.matches) setView("bytes");
}
for (const b of document.querySelectorAll<HTMLButtonElement>("#viewswitch button")) {
  b.addEventListener("click", () => setView(b.dataset.view === "bytes" ? "bytes" : "summary"));
}
setView("summary");

const drawer = new Drawer(
  $("drawer"),
  (id) => {
    toBytes();
    point(id);
  },
  (entry) => void openPlayer(entry),
  (entry) => void openEntry(entry),
  setHover,
  {
    clean: cleanCopy,
    open: (bytes) => {
      const name = cleanName(model?.name ?? "file");
      void load(new File([bytes as BlobPart], name));
    },
  },
  (m) => picture.element(m),
);

/** "photo.jpg" → "photo-clean.jpg" */
function cleanName(name: string): string {
  const base = name.split("/").pop() ?? name;
  const dot = base.lastIndexOf(".");
  return dot > 0 ? `${base.slice(0, dot)}-clean${base.slice(dot)}` : `${base}-clean`;
}

/** Makes the copy in the worker and hands it to the browser as a download. */
async function cleanCopy(): Promise<CleanResult> {
  const m = model;
  if (!m) return { bytes: new Uint8Array(0), removed: [], orientation: 0, error: "no file is open" };
  const r = await call({ type: "clean", bytes: m.bytes.slice() });
  if (r.type !== "cleaned") {
    return { bytes: new Uint8Array(0), removed: [], orientation: 0, error: r.type === "error" ? r.message : "unexpected reply" };
  }
  if (!r.error) {
    const url = URL.createObjectURL(new Blob([r.bytes.slice() as BlobPart]));
    const a = document.createElement("a");
    a.href = url;
    a.download = cleanName(m.name);
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 10_000);
  }
  return { bytes: r.bytes, removed: r.removed, orientation: r.orientation, error: r.error };
}
const playBtn = $<HTMLButtonElement>("play");
const player = new Player($("drawer"), {
  onHead: (start, end, follow) => {
    hex.setHead(start, end);
    if (follow) hex.revealOffset(start, false);
  },
  onClose: closePlayer,
});

/** The ZIP entry holding the selection, or -1. */
function selectedEntry(): number {
  return model ? model.entryOf(selected) : -1;
}

/** Whether there is a stream to play: a PNG's, or the selected ZIP entry's. */
function canPlay(): boolean {
  if (!model) return false;
  if (model.file.format !== "zip") return model.playable;
  const i = selectedEntry();
  return i >= 0 && model.entry(i).playable;
}

async function openPlayer(entry = selectedEntry()): Promise<void> {
  if (!model) return;
  if (model.file.format === "zip") {
    if (entry < 0 || !model.entry(entry).playable) return;
    const m = model;
    closePlayer();
    const r = await call({ type: "selectEntry", index: entry });
    if (m !== model || r.type !== "stream" || !r.playable) return;
    m.setStream(r.segments, r.trace, r.idatBytes);
  }
  if (!model.playable) return;
  document.body.classList.add("is-playing");
  await player.open(model, playerSource);
}

function closePlayer(): void {
  player.close();
  hex.setHead(-1, -1);
  document.body.classList.remove("is-playing");
}

function describeOffset(offset: number): string {
  if (!model) return "";
  const where = model
    .path(model.nodeAt(offset))
    .slice(1)
    .map((id) => model!.label(id))
    .join(" › ");
  return `0x${offset.toString(16).toUpperCase().padStart(8, "0")} · ${where || model.label(0)}`;
}

function setHover(id: number): void {
  if (id === hover) return;
  hover = id;
  hex.setHover(id);
  tree.setHover(id);
  drawer.showNode(model, id >= 0 ? id : selected, id < 0 && selected >= 0);
}

function select(id: number): void {
  selected = id;
  hex.setSelected(id);
  minimap.setSelected(id);
  tree.setSelected(id);
  if (id >= 0) hex.reveal(id);
  drawer.showNode(model, id, id >= 0);
  // While playing, the button is also how the player closes: keep it.
  playBtn.hidden = !canPlay() && !player.isOpen;
}

/** Selects a node another view led to, and lights up its bytes. */
function point(id: number): void {
  select(id);
  hex.flash(id);
}

function updateProblems(): void {
  const n = model?.problems.length ?? 0;
  problemsBtn.hidden = n === 0;
  if (!model || n === 0) return;
  // Coloured by how worried to be, as the verdict is: a corrupt checksum is
  // read fine but still damage.
  const damaged = model.problems.some(
    (id) => model!.kind(id) === Kind.Error || model!.doc(id)?.concern === Concern.Damage,
  );
  problemsBtn.dataset.severity = damaged ? "error" : "warning";
  problemsBtn.textContent = `${n} ${n === 1 ? "problem" : "problems"}`;
  problemsBtn.title = "Jump to the next problem (N)";
}

function nextProblem(): void {
  if (!model || model.problems.length === 0) return;
  problemCursor = (problemCursor + 1) % model.problems.length;
  select(model.problems[problemCursor]);
}

function showFileInfo(m: FileModel): void {
  const info = $("fileinfo");
  const chips: string[] = [];
  const f = m.file;
  if (f.format === "png" && f.ihdr) {
    const [w, h, depth, color] = f.ihdr;
    const names: Record<number, string> = { 0: "Grey", 2: "RGB", 3: "Palette", 4: "Grey+α", 6: "RGBA" };
    chips.push("PNG", `${w}×${h}`, `${names[color] ?? `type ${color}`} ${depth}-bit`);
  } else if (f.format === "png") {
    chips.push("PNG");
  } else if (f.format === "jpeg") {
    chips.push("JPEG");
    if (f.dimensions) chips.push(`${f.dimensions[0]}×${f.dimensions[1]}`);
    if (f.facts.length > 0 || f.location) chips.push("EXIF");
  } else if (f.format === "heif") {
    chips.push(m.value(0).split(" · ")[0]);
    if (f.dimensions) chips.push(`${f.dimensions[0]}×${f.dimensions[1]}`);
    if (f.facts.length > 0 || f.location) chips.push("EXIF");
  } else if (f.format === "video") {
    // "QuickTime · 64×48 · 1.0 s"
    chips.push(...m.value(0).split(" · "));
    if (f.location) chips.push("Location");
  } else if (f.format === "pdf") {
    // "PDF 1.7 · 12 objects · 2 revisions"
    chips.push(...m.value(0).split(" · "));
  } else if (f.format === "zip") {
    chips.push("ZIP", m.value(0));
  } else if (f.format === "wasm") {
    // "WebAssembly · 82 functions · “hello”"
    chips.push(...m.value(0).split(" · "));
  } else {
    chips.push("Unrecognised format");
  }
  chips.push(formatBytes(m.bytes.length), `parsed in ${f.parseMs.toFixed(1)} ms`);

  // The way back up: each archive above this document, then this one.
  const name = document.createElement("span");
  name.className = "filename";
  levels.forEach((level, depth) => {
    const crumb = document.createElement("button");
    crumb.className = "crumb-back";
    crumb.textContent = level.model.name;
    crumb.title = "Back to this file (Backspace goes up one)";
    crumb.addEventListener("click", () => void back(depth));
    const sep = document.createElement("span");
    sep.className = "crumb-sep";
    sep.textContent = "›";
    name.append(crumb, sep);
  });
  name.append(m.name);
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = chips.join("  ·  ");
  info.replaceChildren(name, meta);

  // A location is not damage, so it gets its own badge rather than joining
  // the problems count — but it is the first thing a person should see.
  locationBtn.hidden = !f.location;
}

async function load(file: File): Promise<void> {
  const id = ++loadId;
  closePlayer();
  document.body.dataset.state = "loading";
  $("fileinfo").textContent = `Parsing ${file.name}…`;
  $("load-error").hidden = true;

  // The main thread keeps its own view of the bytes for drawing; parsing
  // happens entirely in the worker.
  const [response, buffer] = await Promise.all([call({ type: "parse", file }), file.arrayBuffer()]);
  if (id !== loadId) return; // a newer file was dropped meanwhile

  if (response.type !== "parsed") {
    const message = response.type === "error" ? response.message : "unexpected reply";
    loadFailed(`Could not read ${file.name}: ${message}`);
    return;
  }

  levels = [];
  show(new FileModel(response.result, new Uint8Array(buffer), file.name));
  arrive();
}

/** Puts a document on screen, with nothing selected. */
function show(m: FileModel): void {
  model = m;
  hover = -1;
  selected = -1;
  problemCursor = -1;
  document.body.dataset.state = "ready";
  drawer.nested = levels.length;

  hex.setModel(m);
  minimap.setModel(m);
  tree.setModel(m);
  drawer.showFile(m);
  drawer.showNode(m, -1, false);
  showFileInfo(m);
  setView("summary");
  updateProblems();
  playBtn.hidden = !canPlay();
}

/** Opens on the answer to the question the person came with: a damaged file
 * on its damage, a photo that records a place on that place. */
function arrive(): void {
  if (!model) return;
  if (model.problems.length > 0) nextProblem();
  else if (model.file.location) select(model.file.location.node);
}

/** Opens a ZIP entry as a document of its own, one level down. */
async function openEntry(entry: number): Promise<void> {
  const parent = model;
  if (!parent) return;
  const name = parent.label(parent.entry(entry).node);
  closePlayer();
  const r = await call({ type: "open", index: entry });
  if (model !== parent) return;
  if (r.type !== "opened") {
    drawer.showNote(r.type === "error" ? r.message : "The entry could not be opened.");
    return;
  }
  levels.push({ model: parent, selected });
  show(new FileModel(r.result, r.bytes, name));
  arrive();
}

/** Returns to the document at `depth` in the breadcrumbs, as it was left. */
async function back(depth: number): Promise<void> {
  if (depth >= levels.length) return;
  closePlayer();
  await call({ type: "back", depth });
  const level = levels[depth];
  levels = levels.slice(0, depth);
  show(level.model);
  if (level.selected >= 0) select(level.selected);
}

async function loadSample(path: string, name: string): Promise<void> {
  // Loading from the moment it is asked for: the landing demo shares the
  // worker, and must not parse its own file over this one.
  document.body.dataset.state = "loading";
  $("fileinfo").textContent = `Opening ${name}…`;
  try {
    const res = await fetch(path);
    if (!res.ok) throw new Error(`the sample answered ${res.status}`);
    await load(new File([await res.blob()], name));
  } catch (e) {
    loadFailed(`Could not open ${name}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Back to where it was after a file failed to open, saying why. */
function loadFailed(message: string): void {
  document.body.dataset.state = model ? "ready" : "empty";
  $("fileinfo").textContent = message;
  // On the landing page the top bar is out of the way: say it by the button.
  const landing = $("load-error");
  landing.textContent = message;
  landing.hidden = !!model;
  // An empty page shows the demo again.
  if (!model) setTimeout(() => void startDemo($("demo")), 0);
}

// --- inputs -------------------------------------------------------------

for (const id of ["picker", "picker-empty"]) {
  $<HTMLInputElement>(id).addEventListener("change", (e) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // so choosing the same file again still fires
    if (file) void load(file);
  });
}
for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-sample]")) {
  btn.addEventListener("click", async () => {
    await loadSample(btn.dataset.sample!, btn.dataset.name!);
    // "Watch compression work" goes straight to the player.
    if (btn.dataset.then === "play" && canPlay()) void openPlayer();
  });
}

// The guides link to a sample by name, as ?sample=photo.jpg. Only the
// samples on this page can be opened this way; the name picks a button.
function openLinkedSample(): void {
  const name = new URLSearchParams(location.search).get("sample");
  if (!name) return;
  history.replaceState(null, "", location.pathname);
  const btn = [...document.querySelectorAll<HTMLButtonElement>("[data-sample]")].find((b) => b.dataset.name === name);
  btn?.click();
}

// A file shared to the installed app (Android's share sheet) arrives through
// the service worker, which keeps it in a cache for this page to pick up.
async function openShared(): Promise<void> {
  if (!new URLSearchParams(location.search).has("shared") || !("caches" in window)) return;
  history.replaceState(null, "", location.pathname);
  const cache = await caches.open("hexscope-shared");
  const res = await cache.match("shared-file");
  if (!res) return;
  const name = decodeURIComponent(res.headers.get("x-file-name") ?? "shared file");
  document.body.dataset.state = "loading";
  await cache.delete("shared-file");
  await load(new File([await res.blob()], name));
}

// Offline after the first visit, and installable. Only in production: in
// development the cache would serve stale modules.
if (import.meta.env.PROD && "serviceWorker" in navigator) {
  void navigator.serviceWorker.register("./sw.js");
}
problemsBtn.addEventListener("click", () => {
  toBytes();
  nextProblem();
  hex.flash(selected);
});
locationBtn.addEventListener("click", () => {
  toBytes();
  if (model?.file.location) point(model.file.location.node);
});
playBtn.addEventListener("click", () => {
  if (player.isOpen) return closePlayer();
  toBytes();
  void openPlayer();
});

let dragDepth = 0;
window.addEventListener("dragenter", (e) => {
  e.preventDefault();
  if (++dragDepth === 1) document.body.classList.add("is-dragging");
});
window.addEventListener("dragleave", () => {
  if (--dragDepth === 0) document.body.classList.remove("is-dragging");
});
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", (e) => {
  e.preventDefault();
  dragDepth = 0;
  document.body.classList.remove("is-dragging");
  const file = e.dataTransfer?.files[0];
  if (file) void load(file);
});

window.addEventListener("keydown", (e) => {
  if (e.target instanceof HTMLInputElement && e.target.type !== "range") return;
  if (player.handleKey(e)) {
    e.preventDefault();
    return;
  }
  if (e.key === "Escape") {
    if (player.isOpen) closePlayer();
    else select(-1);
  }
  if (e.key === "n" || e.key === "N") nextProblem();
  if (e.key === "Backspace" && levels.length > 0) {
    e.preventDefault();
    void back(levels.length - 1);
  }
  if ((e.key === "p" || e.key === "P") && canPlay() && !player.isOpen) void openPlayer();
});

document.body.dataset.state = "empty";
// What a link asked to open goes first; only a page still empty shows the demo.
openLinkedSample();
void openShared();
setTimeout(() => void startDemo($("demo")), 0);
