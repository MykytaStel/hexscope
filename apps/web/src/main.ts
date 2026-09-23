import "./style.css";
import { Drawer, formatBytes } from "./drawer";
import { HexView } from "./hexview";
import { FileModel, Kind } from "./model";
import { TreeView } from "./tree";
import type { WorkerRequest, WorkerResponse } from "./worker";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

let model: FileModel | null = null;
let hover = -1;
let selected = -1;
let problemCursor = -1;
let requestId = 0;

const status = $("status");
const problemsBtn = $<HTMLButtonElement>("problems");

const hex = new HexView($("hex"), {
  onHover: (id, offset) => {
    setHover(id);
    status.textContent = offset >= 0 && model ? describeOffset(offset) : "";
    status.hidden = !status.textContent;
  },
  onSelect: select,
});
const tree = new TreeView($("tree"), { onHover: setHover, onSelect: select });
const drawer = new Drawer($("drawer"));

function describeOffset(offset: number): string {
  if (!model) return "";
  const where = model
    .path(model.nodeAt(offset))
    .slice(1)
    .map((id) => model!.label(id))
    .join(" › ");
  return `0x${offset.toString(16).toUpperCase().padStart(8, "0")} · ${where || "PNG"}`;
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
  if (f.ihdr) {
    const [w, h, depth, color] = f.ihdr;
    const names: Record<number, string> = { 0: "Grey", 2: "RGB", 3: "Palette", 4: "Grey+α", 6: "RGBA" };
    chips.push(`PNG`, `${w}×${h}`, `${names[color] ?? `type ${color}`} ${depth}-bit`);
  } else {
    chips.push(m.kind(1) === Kind.Error ? "Unknown format" : "PNG");
  }
  chips.push(formatBytes(m.bytes.length), `parsed in ${f.parseMs.toFixed(1)} ms`);

  const name = document.createElement("span");
  name.className = "filename";
  name.textContent = m.name;
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = chips.join("  ·  ");
  info.replaceChildren(name, meta);
}

async function load(file: File): Promise<void> {
  const id = ++requestId;
  document.body.dataset.state = "loading";
  $("fileinfo").textContent = `Parsing ${file.name}…`;

  const parsed = new Promise<WorkerResponse>((resolve) => {
    const onMessage = (e: MessageEvent<WorkerResponse>) => {
      if (e.data.id !== id) return;
      worker.removeEventListener("message", onMessage);
      resolve(e.data);
    };
    worker.addEventListener("message", onMessage);
    worker.postMessage({ id, file } satisfies WorkerRequest);
  });
  // The main thread keeps its own view of the bytes for drawing; parsing
  // happens entirely in the worker.
  const [response, buffer] = await Promise.all([parsed, file.arrayBuffer()]);
  if (id !== requestId) return; // a newer file was dropped meanwhile

  if (!response.ok) {
    document.body.dataset.state = model ? "ready" : "empty";
    $("fileinfo").textContent = `Could not read ${file.name}: ${response.message}`;
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

  // A damaged file opens on its damage: that is the question the user came with.
  if (model.problems.length > 0) nextProblem();
}

async function loadSample(path: string, name: string): Promise<void> {
  const res = await fetch(path);
  await load(new File([await res.blob()], name, { type: "image/png" }));
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
  if (e.target instanceof HTMLInputElement) return;
  if (e.key === "Escape") select(-1);
  if (e.key === "n" || e.key === "N") nextProblem();
});

document.body.dataset.state = "empty";
