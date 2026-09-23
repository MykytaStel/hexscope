import "./style.css";
import { Drawer, formatBytes } from "./drawer";
import { HexView } from "./hexview";
import { FileModel, Kind } from "./model";
import { Player } from "./player";
import { TreeView } from "./tree";
import type { WorkerRequest, WorkerResponse } from "./worker";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

// One outstanding promise per request id; responses can arrive in any order.
let nextId = 0;
const waiting = new Map<number, (r: WorkerResponse) => void>();
worker.addEventListener("message", (e: MessageEvent<WorkerResponse>) => {
  waiting.get(e.data.id)?.(e.data);
  waiting.delete(e.data.id);
});

type Req = WorkerRequest extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;

function call(req: Req): Promise<WorkerResponse> {
  const id = ++nextId;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    worker.postMessage({ ...req, id } as WorkerRequest);
  });
}

let model: FileModel | null = null;
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
    status.hidden = !status.textContent;
  },
  onSelect: select,
});
const tree = new TreeView($("tree"), { onHover: setHover, onSelect: select });
const drawer = new Drawer($("drawer"), select, (entry) => void openPlayer(entry));
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
  await player.open(model, {
    steps: async (from, count) => {
      const r = await call({ type: "steps", from, count });
      return r.type === "steps" ? r.steps : new Float64Array(0);
    },
    inflated: async () => {
      const r = await call({ type: "inflated" });
      return r.type === "inflated" ? r.bytes : new Uint8Array(0);
    },
    explain: async (index, knownBlock) => {
      const r = await call({ type: "explain", index, knownBlock });
      return r.type === "explain" ? { parts: r.parts, tables: r.tables } : { parts: new Float64Array(0), tables: null };
    },
  });
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
  tree.setSelected(id);
  if (id >= 0) hex.reveal(id);
  drawer.showNode(model, id, id >= 0);
  // While playing, the button is also how the player closes: keep it.
  playBtn.hidden = !canPlay() && !player.isOpen;
}

function updateProblems(): void {
  const n = model?.problems.length ?? 0;
  problemsBtn.hidden = n === 0;
  if (!model || n === 0) return;
  const errors = model.problems.filter((id) => model!.kind(id) === Kind.Error).length;
  problemsBtn.dataset.severity = errors > 0 ? "error" : "warning";
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
  } else if (f.format === "zip") {
    chips.push("ZIP", m.value(0));
  } else {
    chips.push("Unrecognised format");
  }
  chips.push(formatBytes(m.bytes.length), `parsed in ${f.parseMs.toFixed(1)} ms`);

  const name = document.createElement("span");
  name.className = "filename";
  name.textContent = m.name;
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

  // The main thread keeps its own view of the bytes for drawing; parsing
  // happens entirely in the worker.
  const [response, buffer] = await Promise.all([call({ type: "parse", file }), file.arrayBuffer()]);
  if (id !== loadId) return; // a newer file was dropped meanwhile

  if (response.type !== "parsed") {
    document.body.dataset.state = model ? "ready" : "empty";
    const message = response.type === "error" ? response.message : "unexpected reply";
    $("fileinfo").textContent = `Could not read ${file.name}: ${message}`;
    return;
  }

  model = new FileModel(response.result, new Uint8Array(buffer), file.name);
  hover = -1;
  selected = -1;
  problemCursor = -1;
  document.body.dataset.state = "ready";

  hex.setModel(model);
  tree.setModel(model);
  drawer.showFile(model);
  drawer.showNode(model, -1, false);
  showFileInfo(model);
  updateProblems();
  playBtn.hidden = !canPlay();

  // Open on the answer to the question the person came with: a damaged file
  // on its damage, a photo that records a place on that place.
  if (model.problems.length > 0) nextProblem();
  else if (model.file.location) select(model.file.location.node);
}

async function loadSample(path: string, name: string): Promise<void> {
  const res = await fetch(path);
  await load(new File([await res.blob()], name));
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
  btn.addEventListener("click", () => void loadSample(btn.dataset.sample!, btn.dataset.name!));
}
problemsBtn.addEventListener("click", nextProblem);
locationBtn.addEventListener("click", () => {
  if (model?.file.location) select(model.file.location.node);
});
playBtn.addEventListener("click", () => (player.isOpen ? closePlayer() : void openPlayer()));

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
  if ((e.key === "p" || e.key === "P") && canPlay() && !player.isOpen) void openPlayer();
});

document.body.dataset.state = "empty";
