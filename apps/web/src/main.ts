import "./style.css";
import { Drawer, type CleanResult } from "./drawer";
import { formatBytes } from "./dom";
import { HexView } from "./hexview";
import { Minimap } from "./minimap";
import { Concern, FileModel, Kind } from "./model";
import { themeButton } from "./theme";
import { ActionBar } from "./actionbar";
import { startDemo } from "./demo";
import { PictureView } from "./pixels";
import { Player } from "./player";
import { TreeView } from "./tree";
import { call, playerSource } from "./rpc";
import { verdict } from "./verdict";
import { headline } from "./headline";
import { BatchView } from "./batch";
import { BatchController, type BatchAnalysis } from "./batch-controller";
import { WorkspaceController, type WorkspaceLevel } from "./workspace-controller";
import { categories } from "./share";
import { openShortcuts } from "./shortcuts";
import { dismissTour, maybeTour, resetTour } from "./tour";
import { SearchBar } from "./search";
import { announce } from "./announce";
import { compare, parseAside, showComparison } from "./compare";
import { recentFiles, remember } from "./recent";
import { MAX_FILE, cleanName, kindOf, kindWord, phoneCanShare, repairedName, saveAs, tooLarge } from "./files";
import { cleanCopy, redactCopy, repairCopy } from "./copies";
import { showLegend } from "./legend";
import { wireInputs } from "./inputs";
import { currentLocale, installLocale, languageButton, translateText } from "./i18n";

installLocale();
document.querySelector(".topbar .actions")?.prepend(languageButton());

let batchController: BatchController;
let workspace: WorkspaceController;

for (const b of document.querySelectorAll<HTMLButtonElement>("[data-shortcuts]")) b.addEventListener("click", openShortcuts);
// "Take the tour": on the sample photo, from the start.
for (const b of document.querySelectorAll<HTMLButtonElement>("[data-tour]")) {
  b.addEventListener("click", () => {
    resetTour();
    batchController.clear();
    void loadSample("samples/photo.jpg", "photo.jpg");
  });
}

// On a phone, what to do about the file stays at hand.
new ActionBar().watch(document.getElementById("drawer")!);

// Light, dark or the system's, beside Open.
document.querySelector('[data-opens="picker"]')?.before(themeButton());

// The file pickers: buttons that open hidden inputs — wherever they are,
// the summary's own included, drawn after the page loaded.
document.addEventListener("click", (e) => {
  const button = (e.target as Element | null)?.closest?.<HTMLButtonElement>("button[data-opens]");
  if (button) document.getElementById(button.dataset.opens ?? "")?.click();
  // A file opened earlier in this tab, from the summary's list.
  const again = (e.target as Element | null)?.closest?.<HTMLButtonElement>("button[data-recent]");
  const file = again ? recentFiles()[Number(again.dataset.recent)] : undefined;
  if (file) {
    batchController.clear();
    void workspace.openFile(file);
  }
});

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const appNavigation = $<HTMLElement>("app-nav");
const comparePicker = $<HTMLInputElement>("compare-picker");
const noFile = async (): Promise<CleanResult> => ({ copy: new Blob([]), name: "", saved: false, removed: [], orientation: 0, error: "no file is open" });

let hover = -1;
let selected = -1;
let problemCursor = -1;

const status = $("status");
status.setAttribute("role", "status");
status.setAttribute("aria-live", "off");
status.setAttribute("aria-atomic", "true");
let statusOffset = -1;
const touchPointer = matchMedia("(hover: none)");

function renderStatus(): void {
  const model = typeof workspace === "undefined" ? null : workspace?.model;
  const message = statusOffset >= 0 && model
    ? describeOffset(statusOffset)
    : touchPointer.matches
      ? "Tap a byte"
      : "Hover a byte or row; click to pin";
  status.textContent = translateText(message, currentLocale());
}

function setStatusOffset(offset: number): void {
  statusOffset = offset;
  renderStatus();
}

renderStatus();
window.addEventListener("hexscope:locale", renderStatus);
touchPointer.addEventListener("change", renderStatus);
const problemsBtn = $<HTMLButtonElement>("problems");
const locationBtn = $<HTMLButtonElement>("location");

const hex = new HexView($("hex-body"), {
  onHover: (id, offset, keyboard = false) => {
    status.setAttribute("aria-live", keyboard ? "polite" : "off");
    setHover(id);
    setStatusOffset(offset >= 0 && workspace.model ? offset : -1);
    picture.fromByte(offset);
  },
  onSelect: (id, offset, keyboard = false) => {
    status.setAttribute("aria-live", keyboard ? "polite" : "off");
    if (offset !== undefined) setStatusOffset(offset);
    select(id, offset);
  },
  onNeed: (start, end) => {
    const m = workspace.model;
    if (!m?.missing || !m.source) return;
    void m.missing.fill(m.bytes, m.source, start, end).then((read) => {
      if (read && m === workspace.model) hex.redraw();
    });
  },
});
const minimap = new Minimap($("hex-body"), { onJump: (offset) => hex.scrollToOffset(offset) });
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
    if (workspace.model) select(workspace.model.nodeAt(start));
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
/** The view a wide screen opens on: the answer, unless the person chose the bytes. */
const VIEW = "hexscope.view";
function wideView(): "summary" | "bytes" {
  try {
    return localStorage.getItem(VIEW) === "bytes" ? "bytes" : "summary";
  } catch {
    return "summary";
  }
}
function setView(view: "summary" | "bytes"): void {
  document.body.dataset.view = view;
  for (const b of document.querySelectorAll<HTMLButtonElement>("#viewswitch button")) {
    b.setAttribute("aria-pressed", String(b.dataset.view === view));
  }
}

type WorkspaceView = "overview" | "metadata" | "content" | "structure" | "bytes";

const contentTargetSelector = [
  ".reveals .reveal-hero",
  ".reveals .blackout-figure",
  '.reveals .reveal-list dd[data-kind="hiddentext"]',
  '.reveals .reveal-list dd[data-kind="covered"]',
  '.reveals .reveal-list dd[data-kind="deleted"]',
  '.reveals .reveal-list dd[data-kind="earlier"]',
  '.reveals .reveal-list dd[data-kind="attachments"] .attachment-open',
].join(", ");

function syncAppNavigation(): void {
  const model = typeof workspace === "undefined" ? null : workspace?.model;
  const ready = document.body.dataset.state === "ready" && !!model;
  appNavigation.hidden = !ready;
  if (!ready) return;

  const hasMetadata = !!$("drawer").querySelector(".reveals");
  const hasContent = !!$("drawer").querySelector(contentTargetSelector);
  const current = document.body.dataset.appView;
  for (const button of appNavigation.querySelectorAll<HTMLButtonElement>("button")) {
    if (button.dataset.appView) {
      const view = button.dataset.appView as WorkspaceView;
      button.disabled = view === "metadata" ? !hasMetadata : view === "content" ? !hasContent : false;
      if (view === current) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    } else if (button.dataset.appAction === "compression") {
      const playing = document.body.classList.contains("is-playing");
      button.disabled = !playing && !canPlay();
      button.setAttribute("aria-pressed", String(playing));
    }
  }
}

function setWorkspaceView(view: WorkspaceView, scrollToSection = true): void {
  document.body.dataset.appView = view;
  setView(view === "structure" || view === "bytes" ? "bytes" : "summary");
  syncAppNavigation();

  if (!scrollToSection) return;
  const file = $("drawer").querySelector<HTMLElement>(".drawer-file");
  const target = view === "metadata"
    ? $("drawer").querySelector<HTMLElement>(".reveals")
    : view === "content"
      ? $("drawer").querySelector<HTMLElement>(contentTargetSelector)
      : null;
  if (view === "overview") {
    requestAnimationFrame(() => file?.scrollTo({ top: 0, behavior: "smooth" }));
  } else if (target) {
    requestAnimationFrame(() => target.scrollIntoView({ behavior: "smooth", block: "start" }));
  }
}

appNavigation.addEventListener("click", (event) => {
  const button = (event.target as Element | null)?.closest<HTMLButtonElement>("button");
  if (!button || button.disabled) return;
  if (button.dataset.appView) {
    setWorkspaceView(button.dataset.appView as WorkspaceView);
    return;
  }
  if (button.dataset.appAction === "compression") {
    if (document.body.classList.contains("is-playing")) closePlayer();
    else if (canPlay()) void openPlayer();
  } else if (button.dataset.appAction === "compare") {
    comparePicker.click();
  }
});

comparePicker.addEventListener("change", () => {
  const file = comparePicker.files?.[0];
  comparePicker.value = "";
  if (file) void compareWith(file);
});

let tourDismissedByNavigation = false;
/** The view a file opens on: a phone always starts on the summary. */
function openingView(): "summary" | "bytes" {
  return narrow.matches ? "summary" : wideView();
}
function toBytes(): void {
  if (document.body.dataset.appView !== "bytes") setWorkspaceView("bytes", false);
}
for (const b of document.querySelectorAll<HTMLButtonElement>("#viewswitch button")) {
  b.addEventListener("click", () => {
    const view = b.dataset.view === "bytes" ? "bytes" : "summary";
    tourDismissedByNavigation = true;
    dismissTour();
    setWorkspaceView(view === "bytes" ? "bytes" : "overview", false);
    // On a wide screen the choice is remembered for the next file.
    if (!narrow.matches) {
      try {
        localStorage.setItem(VIEW, view);
      } catch {
        // Nowhere to keep it.
      }
    }
  });
}
document.body.dataset.appView = "overview";
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
    clean: (notes) => (workspace.model ? cleanCopy(workspace.model, notes) : noFile()),
    open: (copy) => void workspace.openFile(new File([copy], workspace.model ? cleanName(workspace.model) : "file-clean")),
    save: saveAs,
    openInside: (bytes, name) => void workspace.openInside(bytes, name),
    pages: async () => {
      const r = await call({ type: "pageTexts", bytes: workspace.model ? workspace.model.bytes.slice() : new Uint8Array(0) });
      return r.type === "pageTexts" ? r.pages : [];
    },
    pictures: async (page) => {
      const r = await call({ type: "pagePictures", bytes: workspace.model ? workspace.model.bytes.slice() : new Uint8Array(0), page });
      return r.type === "pagePictures" ? r.pictures : [];
    },
    redact: (areas, selections) => (workspace.model ? redactCopy(workspace.model, areas, selections) : noFile()),
    repair: () => (workspace.model ? repairCopy(workspace.model) : Promise.resolve({ bytes: new Uint8Array(0), name: "", fixed: [], error: "no file is open" })),
    compare: (other) => void compareWith(other),
    openRepaired: (bytes) => void workspace.openFile(new File([bytes as BlobPart], workspace.model ? repairedName(workspace.model) : "file-repaired")),
  },
  (m) => picture.element(m),
);

/** Compares the file on screen with another, parsed aside. */
async function compareWith(other: File): Promise<void> {
  const a = workspace.model;
  if (!a) return;
  try {
    const b = await parseAside(other);
    showComparison(a, b, compare(a, b));
  } catch (e) {
    drawer.showNote(`Could not read ${other.name}: ${e instanceof Error ? e.message : String(e)}`);
  }
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
  return workspace.model ? workspace.model.entryOf(selected) : -1;
}

/** Whether there is a stream to play: a PNG's, or the selected ZIP entry's. */
function canPlay(): boolean {
  const model = workspace.model;
  if (!model) return false;
  if (model.file.format !== "zip") return model.playable;
  const i = selectedEntry();
  return i >= 0 && model.entry(i).playable;
}

async function openPlayer(entry = selectedEntry()): Promise<void> {
  const model = workspace.model;
  if (!model) return;
  if (model.file.format === "zip") {
    if (entry < 0 || !model.entry(entry).playable) return;
    const m = model;
    closePlayer();
    const r = await call({ type: "selectEntry", index: entry });
    if (m !== workspace.model || r.type !== "stream" || !r.playable) return;
    m.setStream(r.segments, r.trace, r.idatBytes);
  }
  if (!model.playable) return;
  document.body.classList.add("is-playing");
  syncAppNavigation();
  await player.open(model, playerSource);
}

function closePlayer(): void {
  player.close();
  hex.setHead(-1, -1);
  document.body.classList.remove("is-playing");
  syncAppNavigation();
}

function describeOffset(offset: number): string {
  const model = workspace.model;
  if (!model) return "";
  const where = model
    .path(model.nodeAt(offset))
    .slice(1)
    .map((id) => translateText(model!.label(id), currentLocale()))
    .join(" › ");
  return `0x${offset.toString(16).toUpperCase().padStart(8, "0")} · ${where || model.label(0)}`;
}

function setHover(id: number): void {
  if (id === hover) return;
  hover = id;
  hex.setHover(id);
  tree.setHover(id);
  drawer.showNode(workspace.model, id >= 0 ? id : selected, id < 0 && selected >= 0);
}

function select(id: number, offset?: number): void {
  const model = workspace.model;
  selected = id;
  hex.setSelected(id, offset);
  minimap.setSelected(id);
  tree.setSelected(id);
  if (id >= 0) hex.reveal(id);
  drawer.showNode(model, id, id >= 0);
  // While playing, the button is also how the player closes: keep it.
  playBtn.hidden = !canPlay() && !player.isOpen;
  syncAppNavigation();
}

/** Selects a node without scrolling to it: the view is already where it should be. */
function selectInPlace(id: number): void {
  const model = workspace.model;
  selected = id;
  hex.setSelected(id);
  minimap.setSelected(id);
  tree.setSelected(id);
  drawer.showNode(model, id, id >= 0);
}

// Find in the file: each match marked in the bytes, its part selected.
const search = new SearchBar($("hex-find"), {
  bytes: () => workspace.model?.bytes ?? null,
  parts: () => (workspace.model?.missing ? workspace.model.missing.read(workspace.model.bytes.length) : null),
  show: (start, end) => {
    hex.setHead(start, end);
    const model = workspace.model;
    if (start < 0 || !model) return;
    toBytes();
    selectInPlace(model.nodeAt(start));
    hex.revealOffset(start, false);
  },
});
{
  const open = document.createElement("button");
  open.className = "search-open";
  open.title = "Find in the file (/)";
  open.setAttribute("aria-label", "Find in the file");
  open.innerHTML = '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="m10.5 10.5 3.5 3.5"/></svg>';
  open.addEventListener("click", () => search.open());
  $("hex-find").append(open);
}

/** Selects a node another view led to, and lights up its bytes. */
function point(id: number): void {
  select(id);
  hex.flash(id);
}

function updateProblems(): void {
  const model = workspace.model;
  const n = model?.problems.length ?? 0;
  problemsBtn.hidden = n === 0;
  if (!model || n === 0) return;
  // Coloured by how worried to be, as the verdict is: a corrupt checksum is
  // read fine but still damage.
  const damaged = model.problems.some(
    (id) => model!.kind(id) === Kind.Error || model!.doc(id)?.concern === Concern.Damage,
  );
  problemsBtn.dataset.severity = damaged ? "error" : "warning";
  // Damage is a problem; something hidden or a rule bent is a finding.
  const word = document.createElement("span");
  word.className = "problems-word";
  word.textContent = damaged ? (n === 1 ? " problem" : " problems") : n === 1 ? " finding" : " findings";
  problemsBtn.replaceChildren(String(n), word);
  problemsBtn.setAttribute("aria-label", `${n}${word.textContent}: jump to the next`);
  problemsBtn.title = `Jump to the next ${damaged ? "problem" : "finding"} (N)`;
}

function nextProblem(): void {
  const model = workspace.model;
  if (!model || model.problems.length === 0) return;
  problemCursor = (problemCursor + 1) % model.problems.length;
  select(model.problems[problemCursor]);
}

function showFileInfo(m: FileModel, levels: readonly WorkspaceLevel[]): void {
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
  } else if (f.format === "webp" || f.format === "gif") {
    chips.push(...m.value(0).split(" · "));
    if (f.facts.length > 0 || f.location) chips.push("Metadata");
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
    // A document by what it is; an archive by what is in it.
    const kind = kindOf(m);
    chips.push(...(kind === "ZIP" ? ["ZIP", m.value(0)] : [kind]));
  } else if (f.format === "office97" || f.format === "cfb") {
    chips.push(m.value(0).charAt(0).toUpperCase() + m.value(0).slice(1));
  } else if (f.format === "eml" || f.format === "msg") {
    chips.push(f.format === "msg" ? "Outlook message" : "Email");
    if (f.attachments.length > 0) chips.push(f.attachments.length === 1 ? "1 attachment" : `${f.attachments.length} attachments`);
  } else if (f.format === "wasm") {
    // "WebAssembly · 82 functions · “hello”"
    chips.push(...m.value(0).split(" · "));
  } else {
    chips.push("Unrecognised format");
  }
  chips.push(formatBytes(m.bytes.length), `parsed in ${f.parseMs.toFixed(1)} ms`);

  // The way back up: the list of files, each archive above this document, then this one.
  // The page's heading while a file is open: its name, and the way to it.
  const name = document.createElement("h1");
  name.className = "filename";
  const batch = batchController.items;
  if (batch) {
    const crumb = document.createElement("button");
    crumb.className = "crumb-back";
    crumb.textContent = `${batch.length} files`;
    crumb.title = "Back to the list (Backspace)";
    crumb.addEventListener("click", showBatch);
    const sep = document.createElement("span");
    sep.className = "crumb-sep";
    sep.textContent = "›";
    name.append(crumb, sep);
  }
  levels.forEach((level, depth) => {
    const crumb = document.createElement("button");
    crumb.className = "crumb-back";
    crumb.textContent = level.model.name;
    crumb.title = "Back to this file (Backspace goes up one)";
    crumb.addEventListener("click", () => void workspace.back(depth));
    const sep = document.createElement("span");
    sep.className = "crumb-sep";
    sep.textContent = "›";
    name.append(crumb, sep);
  });
  name.append(m.name);
  // The summary says what the file is in a word; the bytes, every detail.
  const meta = document.createElement("span");
  meta.className = "meta";
  const plain = document.createElement("span");
  plain.className = "meta-plain";
  plain.textContent = `${kindWord(m)}  ·  ${formatBytes(m.bytes.length)}`;
  const full = document.createElement("span");
  full.className = "meta-full";
  full.textContent = chips.join("  ·  ");
  meta.append(plain, full);
  info.replaceChildren(name, meta);

  // A location is not damage, so it gets its own badge rather than joining
  // the problems count — but it is the first thing a person should see.
  locationBtn.hidden = !f.location;
}

workspace = new WorkspaceController({
  maxFileBytes: MAX_FILE,
  getSelected: () => selected,
  services: {
    async parse(file) {
      const response = await call({ type: "parse", file });
      if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
      return new FileModel(response.result, response.bytes, file.name, file);
    },
    async openEntry(index, name) {
      const response = await call({ type: "open", index });
      if (response.type !== "opened") throw new Error(response.type === "error" ? response.message : "The entry could not be opened.");
      return new FileModel(response.result, response.bytes, name);
    },
    async openBytes(bytes, name) {
      const response = await call({ type: "openBytes", bytes });
      if (response.type !== "opened") throw new Error(response.type === "error" ? response.message : "It could not be opened.");
      return new FileModel(response.result, response.bytes, name);
    },
    async back(depth) {
      const response = await call({ type: "back", depth });
      if (response.type === "error") throw new Error(response.message);
      if (response.type !== "back") throw new Error("Could not return to the previous file.");
    },
  },
  view: {
    loading(name) {
      document.body.dataset.state = "loading";
      $("fileinfo").textContent = `Parsing ${name}…`;
      $("load-error").hidden = true;
    },
    show: renderFile,
    select,
    failed: loadFailed,
    note: (message) => drawer.showNote(message),
    remember,
    afterOpen(model) {
      arrive();
      // Only a standalone file gets the first-open tour; nested entries do not.
      if (model.source) {
        setTimeout(() => {
          if (!tourDismissedByNavigation && document.body.dataset.view === "summary") maybeTour();
        }, 350);
      }
    },
    clearPlayer: closePlayer,
  },
});

// --- many files ------------------------------------------------------------

const batchView = new BatchView($("batch"), {
  canShareCopies: phoneCanShare,
  open: (i) => batchController.open(i),
  saveClean: (keepNames) => void batchController.saveClean(keepNames),
  mosaic: (items) => void import("./photo-mosaic").then((m) => m.showMosaic(items)).catch((e) => announce(String(e))),
  filmRoll: (items) => void import("./film-roll-ui").then((m) => m.showFilmRoll(items.map((i) => i.file))).catch((e) => announce(String(e))),
});
batchController = new BatchController(batchView, {
  analyze: analyzeBatchFile,
  openFile: (file) => void workspace.openFile(file),
  saveClean: (...args) => import("./batchclean").then((m) => m.cleanCopies(...args)),
  announce,
});

/** Starts reading many files, and lists them. */
function startBatch(files: File[]): void {
  workspace.clear();
  batchController.start(files);
  showBatch();
}

/** Back to the list of files. */
function showBatch(): void {
  const items = batchController.items;
  if (!items) return;
  closePlayer();
  document.body.dataset.state = "batch";
  $("fileinfo").textContent = `${items.length} files`;
  $("load-error").hidden = true;
  locationBtn.hidden = true;
  problemsBtn.hidden = true;
  playBtn.hidden = true;
  batchController.resume();
  syncAppNavigation();
}

/** Reads and summarizes one file for the batch list. */
async function analyzeBatchFile(file: File): Promise<BatchAnalysis> {
  if (file.size > MAX_FILE) throw new Error(tooLarge(file));
  const response = await call({ type: "parse", file });
  if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
  const parsed = new FileModel(response.result, response.bytes, file.name, file);
  await (await import("./qrfacts")).addCodeFacts(parsed);
  return {
    kind: kindOf(parsed),
    lines: verdict(parsed),
    headline: headline(parsed),
    reveals: categories(parsed),
    cleanName: cleanName(parsed),
    ...( ["jpeg", "png", "webp", "heif", "gif"].includes(parsed.file.format) && !parsed.file.kinds.some((k) => k === Kind.Warning || k === Kind.Error) ? { photoSignal: {
      serial: parsed.file.facts.find((f) => f.kind === "serial")?.text ?? null,
      location: parsed.file.location ? [parsed.file.location.latitude, parsed.file.location.longitude] as [number, number] : null,
      taken: parsed.file.facts.find((f) => f.kind === "taken")?.text ?? null,
    } } : {}),
  };
}

/** Puts a document on screen, with nothing selected. */
function renderFile(m: FileModel, levels: readonly WorkspaceLevel[]): void {
  hover = -1;
  selected = -1;
  problemCursor = -1;
  tourDismissedByNavigation = false;
  setStatusOffset(-1);
  document.body.dataset.state = "ready";
  drawer.nested = levels.length;

  hex.setModel(m);
  showLegend($("hex-legend"), m, point);
  search.reset();
  minimap.setModel(m);
  tree.setModel(m);
  drawer.showFile(m);
  drawer.showNode(m, -1, false);
  showFileInfo(m, levels);
  setWorkspaceView(openingView() === "bytes" ? "bytes" : "overview", false);
  updateProblems();
  playBtn.hidden = !canPlay();
  syncAppNavigation();
  void lookForCodes(m);
}

/** QR codes, looked for once the file is on screen: its card is drawn again with what they say. */
async function lookForCodes(m: FileModel): Promise<void> {
  const { addCodeFacts, placeFilmScanCard } = await import("./qrfacts");
  const update = await addCodeFacts(m, true);
  if (!update || m !== workspace.model) return;
  // A clean copy made meanwhile keeps its card; local image analysis refreshes it when ready.
  const drawerEl = $("drawer");
  const redraw = update === "codes" && !drawerEl.querySelector(".before-after, .clean-list");
  const focused = drawerEl.contains(document.activeElement);
  if (redraw) drawer.showFile(m);
  await placeFilmScanCard(drawerEl, m);
  syncAppNavigation();
  if (redraw && focused) drawer.focusVerdict();
}

/** Opens on the answer to the question the person came with: a damaged file
 * on its damage, a photo that records a place on that place. */
function arrive(): void {
  const model = workspace.model;
  if (!model) return;
  if (model.problems.length > 0) nextProblem();
  else if (model.file.location) select(model.file.location.node);
  // Heard, and where the keyboard starts: what was found.
  const lines = verdict(model).map((l) => l.text);
  announce(`${model.name} is open. ${lines.join(" ")}`);
  drawer.focusVerdict();
}

/** Opens a ZIP entry as a document of its own, one level down. */
function openEntry(entry: number): void {
  const parent = workspace.model;
  if (!parent) return;
  // A PDF's attachment by its name; an archive's entry by its node's.
  const name =
    ["pdf", "eml", "msg"].includes(parent.file.format)
      ? (parent.file.attachments[entry] ?? "attachment")
      : parent.label(parent.entry(entry).node);
  void workspace.openEntry(entry, name);
}

async function loadSample(path: string, name: string): Promise<void> {
  // Loading from the moment it is asked for: the landing demo shares the
  // worker, and must not parse its own file over this one.
  document.body.dataset.state = "loading";
  $("fileinfo").textContent = `Opening ${name}…`;
  let res: Response | null = null;
  // A dropped connection gets a second try before it is said.
  for (let attempt = 0; attempt < 2 && !res; attempt++) {
    if (attempt > 0) await new Promise((r) => setTimeout(r, 800));
    res = await fetch(path).catch(() => null);
  }
  if (!res) {
    loadFailed(
      navigator.onLine
        ? `Could not download the sample ${name}: the connection dropped. Try again in a moment.`
        : `Could not download the sample ${name}: you are offline, and it has not been saved for offline use yet. Your own files still open.`,
    );
    return;
  }
  if (!res.ok) {
    loadFailed(`Could not download the sample ${name}: the site answered ${res.status}. Try again in a moment.`);
    return;
  }
  try {
    await workspace.openFile(new File([await res.blob()], name));
  } catch (e) {
    loadFailed(`Could not open ${name}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Back to where it was after a file failed to open, saying why. */
function loadFailed(message: string): void {
  const model = workspace.model;
  if (batchController.items && !model) {
    showBatch();
    $("fileinfo").textContent = message;
    return;
  }
  document.body.dataset.state = model ? "ready" : "empty";
  syncAppNavigation();
  $("fileinfo").textContent = message;
  // On the landing page the top bar is out of the way: say it by the button.
  const landing = $("load-error");
  landing.textContent = message;
  landing.hidden = !!model;
  // An empty page shows the demo again.
  if (!model) setTimeout(() => void startDemo($("demo")), 0);
}

// --- inputs -------------------------------------------------------------

for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-sample]")) {
  btn.addEventListener("click", async () => {
    batchController.clear();
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
  batchController.clear();
  await workspace.openFile(new File([await res.blob()], name));
}

// Files opened with the installed app from the computer's own file manager.
interface LaunchParams {
  files: FileSystemFileHandle[];
}
function openLaunched(): void {
  const queue = (window as Window & { launchQueue?: { setConsumer(f: (p: LaunchParams) => void): void } }).launchQueue;
  queue?.setConsumer(async ({ files }) => {
    if (files.length > 0) openFiles(await Promise.all(files.map((f) => f.getFile())));
  });
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
  if (workspace.model?.file.location) point(workspace.model.file.location.node);
});
playBtn.addEventListener("click", () => {
  if (player.isOpen) return closePlayer();
  toBytes();
  void openPlayer();
});

// Every way a file comes in.
wireInputs({ open: (files) => openFiles(files), fail: loadFailed });

/** One file opens; several are listed. */
function openFiles(files: File[]): void {
  if (files.length > 1) {
    startBatch(files);
  } else if (files.length === 1) {
    batchController.clear();
    void workspace.openFile(files[0]);
  }
}

window.addEventListener("keydown", (e) => {
  if (e.target instanceof HTMLInputElement && e.target.type !== "range") return;
  // A dialog open over the page takes the keys; Escape closes it by itself.
  if (document.querySelector("dialog[open]")) return;
  if (player.handleKey(e)) {
    e.preventDefault();
    return;
  }
  if (e.key === "Escape") {
    if (player.isOpen) closePlayer();
    else select(-1);
  }
  // Letters only bare: Cmd-N and Ctrl-P belong to the browser.
  const bare = !e.metaKey && !e.ctrlKey && !e.altKey;
  if (e.key === "?" && !e.metaKey && !e.ctrlKey) openShortcuts();
  if (bare && (e.key === "o" || e.key === "O")) $<HTMLInputElement>(document.body.dataset.state === "empty" ? "picker-empty" : "picker").click();
  if (bare && (e.key === "n" || e.key === "N")) nextProblem();
  // The bytes are drawn, not text: the browser's own find cannot see them.
  if (document.body.dataset.state === "ready" && (e.key === "/" || ((e.metaKey || e.ctrlKey) && e.key === "f"))) {
    e.preventDefault();
    search.open();
  }
  if (e.key === "Backspace" && workspace.levels.length > 0) {
    e.preventDefault();
    void workspace.back(workspace.levels.length - 1);
  } else if (e.key === "Backspace" && batchController.items && document.body.dataset.state === "ready") {
    e.preventDefault();
    showBatch();
  }
  if (bare && (e.key === "p" || e.key === "P") && canPlay() && !player.isOpen) void openPlayer();
});

document.body.dataset.state = "empty";
// What a link asked to open goes first; only a page still empty shows the demo.
openLinkedSample();
void openShared();
openLaunched();
setTimeout(() => void startDemo($("demo")), 0);

$("film-roll-open").addEventListener("click", () => void import("./film-roll-ui").then((m) => m.showFilmRoll()).catch((e) => announce(String(e))));
