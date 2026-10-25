// The analyzer: the views of an open file, the workspace that opens it, the
// batch list and the player. main.ts loads it when a file is asked for, so
// the landing page does not load it before there is file intent.
import { explain } from "./explanations";
import { Drawer, type CleanResult } from "./drawer";
import { formatBytes } from "./dom";
import { HexView } from "./hexview";
import { Minimap } from "./minimap";
import { Concern, FileModel, Kind } from "./model";
import { ActionBar } from "./actionbar";
import { startDemo } from "./demo";
import { PictureView } from "./pixels";
import { TreeView } from "./tree";
import { call, onWorkerReset, playerSource } from "./rpc";
import { verdict } from "./verdict";
import { headline } from "./headline";
import { BatchController, BatchView, type BatchAnalysis } from "./batch";
import { WorkspaceController, type MovieProgress, type WorkspaceLevel } from "./workspace-controller";
import { categories } from "./share";
import { dismissTour, maybeTour, resetTour } from "./tour";
import { SearchBar } from "./search";
import { announce } from "./announce";
import { recentFiles, remember } from "./recent";
import { MAX_FILE, cleanName, kindOf, kindWord, phoneCanShare, repairedName, saveAs, siteUrl, tooLarge } from "./files";
import { cleanCopy, redactCopy, repairCopy } from "./copies";
import { showLegend } from "./legend";
import { currentLocale, loadUkrainian, translateText, withUkrainian } from "./i18n";

// What each part of a file is, in Ukrainian, for the Structure and Bytes
// views: it comes with the analyzer, and only for a reader in Ukrainian.
withUkrainian(() => import("./docs-i18n"));

let batchController: BatchController;
let workspace: WorkspaceController;

/** "Take the tour": on the sample photo, from the start. */
export function startTour(): void {
  resetTour();
  batchController.clear();
  void loadSample("samples/photo.jpg", "photo.jpg");
}

// On a phone, what to do about the file stays at hand.
new ActionBar().watch(document.getElementById("drawer")!);

document.addEventListener("click", (e) => {
  // A file opened earlier in this tab, from the summary's list.
  const again = (e.target as Element | null)?.closest?.<HTMLButtonElement>("button[data-recent]");
  const file = again ? recentFiles()[Number(again.dataset.recent)] : undefined;
  if (file) {
    batchController.clear();
    void workspace.openFile(file);
  }
});

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const readProgress = document.createElement("span");
readProgress.className = "file-read-progress";
readProgress.setAttribute("role", "progressbar");
readProgress.setAttribute("aria-valuemin", "0");
readProgress.setAttribute("aria-valuemax", "100");
const readAnnouncement = document.createElement("span");
readAnnouncement.className = "visually-hidden file-read-announcement";
readAnnouncement.setAttribute("role", "status");
readAnnouncement.setAttribute("aria-live", "polite");
readAnnouncement.setAttribute("aria-atomic", "true");
const cancelRead = document.createElement("button");
cancelRead.type = "button";
cancelRead.className = "btn fileinfo-cancel";
cancelRead.addEventListener("click", () => workspace?.cancelOpen());
const appNavigation = $<HTMLElement>("app-nav");
const comparePicker = $<HTMLInputElement>("compare-picker");
const noFile = async (): Promise<CleanResult> => ({ copy: new Blob([]), name: "", saved: false, removed: [], orientation: 0, error: "no file is open" });

let hover = -1;
let selected = -1;
let problemCursor = -1;
let playerPending = false;

const status = $("status");
status.setAttribute("role", "status");
status.setAttribute("aria-live", "off");
status.setAttribute("aria-atomic", "true");
let statusOffset = -1;
const touchPointer = matchMedia("(hover: none)");

const statusFile = $("statusbar-file");

/** The bar under the bytes: the byte under the pointer, then the part chosen and the file's size. */
function renderStatus(): void {
  const model = typeof workspace === "undefined" ? null : workspace?.model;
  const message = statusOffset >= 0 && model
    ? describeOffset(statusOffset)
    : touchPointer.matches
      ? "Tap a byte"
      : "Hover a byte or row; click to pin";
  status.textContent = translateText(message, currentLocale());
  if (!model) {
    statusFile.textContent = "";
    return;
  }
  const part = typeof selected === "number" && selected > 0
    ? `${translateText(model.label(selected), currentLocale())} · ${translateText(`${model.len(selected).toLocaleString("en")} bytes`, currentLocale())} · `
    : "";
  statusFile.textContent = `${part}${formatBytes(model.bytes.length)}`;
}

function setStatusOffset(offset: number): void {
  statusOffset = offset;
  if (offset >= 0) tools?.setOffset(offset);
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
    point(id, offset);
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
// What the bytes read as, their hashes and their entropy: a module of its
// own, loaded the first time its panel under the bytes is opened.
let tools: import("./bytetools").ByteTools | null = null;
let view: [number, number] = [0, 0];
const toolsBox = $<HTMLDetailsElement>("hex-tools");
toolsBox.addEventListener("toggle", () => {
  if (!toolsBox.open || tools) return;
  void import("./bytetools").then(({ ByteTools }) => {
    tools = new ByteTools($("hex-tools-body"), { jump: (offset) => hex.scrollToOffset(offset) });
    const m = workspace.model;
    tools.setModel(m);
    tools.setView(...view);
    if (statusOffset >= 0) tools.setOffset(statusOffset);
    if (m && selected > 0) tools.setPart(m.start(selected), m.len(selected), m.label(selected));
  });
});
hex.onView = (start, end) => {
  minimap.setView(start, end);
  view = [start, end];
  tools?.setView(start, end);
};
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
    if (workspace.model) point(workspace.model.nodeAt(start));
    hex.setHead(start, end);
    // Once the bytes are laid out, bring these into view.
    requestAnimationFrame(() => requestAnimationFrame(() => hex.scrollToOffset(start)));
  },
  onStep: (index) => {
    toBytes();
    void openPlayer(selectedEntry(), index);
  },
});
// On a phone the file opens on its summary; the tree and the bytes are a
// tap away, and anything that points into the bytes goes there.
const narrow = matchMedia("(max-width: 900px)");

// On a phone the bar keeps the views of the answer; the structure and the
// bytes, for the curious, wait under More, which says when one of them is open.
const expertViews = ["structure", "bytes"].map((v) => appNavigation.querySelector<HTMLElement>(`[data-app-view="${v}"]`)!);
const moreViews = appNavigation.querySelector<HTMLElement>(".app-nav-more-actions")!;
function placeExpertViews(): void {
  if (narrow.matches) moreViews.prepend(...expertViews);
  else appNavigation.querySelector('[data-app-view="content"]')?.after(...expertViews);
}
placeExpertViews();
narrow.addEventListener("change", placeExpertViews);
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
  if (!appNavigation.dataset.icons) {
    appNavigation.dataset.icons = "loading";
    void import("./app-workspace-ui")
      .then(({ installAppNavigationIcons, createMobileInspector }) => {
        installAppNavigationIcons(appNavigation);
        createMobileInspector($("drawer"), appNavigation);
      })
      .catch(() => { delete appNavigation.dataset.icons; });
  }
  if (!narrow.matches) appNavigation.querySelector<HTMLDetailsElement>(".app-nav-more")!.open = true;

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
      const playing = document.body.classList.contains("is-playing") || playerPending;
      button.disabled = !playing && !canPlay();
      button.setAttribute("aria-pressed", String(playing));
    }
  }
  // More says it holds the view open now, to the eye and to a screen reader.
  const more = appNavigation.querySelector(".app-nav-more > summary");
  if (moreViews.querySelector('[aria-current="page"]')) more?.setAttribute("aria-current", "page");
  else more?.removeAttribute("aria-current");
}
narrow.addEventListener("change", () => {
  appNavigation.querySelector<HTMLDetailsElement>(".app-nav-more")!.open = !narrow.matches;
  if (!narrow.matches && document.body.dataset.state === "ready") syncAppNavigation();
});

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
  if (view === "metadata") {
    // The view of every field: the extra ones too, open.
    for (const more of $("drawer").querySelectorAll<HTMLDetailsElement>(".metadata-groups details")) more.open = true;
  }
  if (view === "overview" || view === "metadata") {
    requestAnimationFrame(() => file?.scrollTo({ top: 0, behavior: "smooth" }));
  } else if (target) {
    requestAnimationFrame(() => target.scrollIntoView({ behavior: "smooth", block: "start" }));
  }
}

appNavigation.addEventListener("click", (event) => {
  const button = (event.target as Element | null)?.closest<HTMLButtonElement>("button");
  if (!button || button.disabled) return;
  // A phone's More closes on whatever was chosen in it.
  if (narrow.matches) button.closest<HTMLDetailsElement>(".app-nav-more")?.removeAttribute("open");
  if (button.dataset.appView) {
    if (playerPending) closePlayer();
    tourDismissedByNavigation = true;
    dismissTour();
    setWorkspaceView(button.dataset.appView as WorkspaceView);
    return;
  }
  tourDismissedByNavigation = true;
  dismissTour();
  button.closest<HTMLDetailsElement>(".app-nav-more")?.removeAttribute("open");
  if (button.dataset.appAction === "compression") {
    if (document.body.classList.contains("is-playing") || playerPending) closePlayer();
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
  (start, len) => {
    if (len <= 0) return;
    toBytes();
    hex.setHead(start, start + len);
    hex.focusAt(start);
    setStatusOffset(start);
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
    const { compare, parseAside, showComparison } = await import("./compare");
    const b = await parseAside(other);
    showComparison(a, b, compare(a, b));
  } catch (e) {
    drawer.showNote(`Could not read ${other.name}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

const playBtn = $<HTMLButtonElement>("play");
const playerLoadError = $<HTMLElement>("player-load-error");
type DeflatePlayer = import("./player").Player;
let player: DeflatePlayer | null = null;
let playerModule: Promise<typeof import("./player")> | null = null;
let playerRequest = 0;

function clearPlayerLoadError(): void {
  playerLoadError.hidden = true;
  playerLoadError.replaceChildren();
}

function showPlayerLoadError(message: string, retryLoad: () => void): void {
  const text = document.createElement("p");
  text.className = "player-load-error-message";
  text.textContent = message;

  const retry = document.createElement("button");
  retry.type = "button";
  retry.className = "btn";
  retry.dataset.retryPlayer = "true";
  retry.textContent = translateText("Try again", currentLocale());
  retry.addEventListener("click", () => {
    clearPlayerLoadError();
    retryLoad();
  });

  const dismiss = document.createElement("button");
  dismiss.type = "button";
  dismiss.className = "btn btn-dismiss";
  dismiss.setAttribute("aria-label", translateText("Close", currentLocale()));
  dismiss.textContent = "×";
  dismiss.addEventListener("click", clearPlayerLoadError);

  const actions = document.createElement("div");
  actions.className = "player-load-error-actions";
  actions.append(retry, dismiss);

  playerLoadError.replaceChildren(text, actions);
  playerLoadError.hidden = false;
}

function loadPlayerModule(): Promise<typeof import("./player")> {
  if (!playerModule) {
    playerModule = import("./player").catch((error) => {
      playerModule = null;
      throw error;
    });
  }
  return playerModule;
}

function createPlayer(PlayerClass: typeof import("./player").Player): DeflatePlayer {
  return new PlayerClass($("drawer"), {
    onHead: (start, end, follow) => {
      hex.setHead(start, end);
      if (follow) hex.revealOffset(start, false);
    },
    onClose: closePlayer,
  });
}

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

function retryPlayerRequest(model: FileModel, entry: number, step: number | undefined): void {
  if (model !== workspace.model) return;
  void openPlayer(entry, step);
}

async function loadPlayerClass(
  model: FileModel,
  entry: number,
  step: number | undefined,
  request: number,
): Promise<typeof import("./player").Player | null> {
  try {
    return (await loadPlayerModule()).Player;
  } catch {
    if (request !== playerRequest || model !== workspace.model) return null;
    showPlayerLoadError(translateText("Compression player could not be loaded. Try again.", currentLocale()), () =>
      retryPlayerRequest(model, entry, step),
    );
    return null;
  }
}

async function preparePlayerClass(
  model: FileModel,
  isZip: boolean,
  entry: number,
  step: number | undefined,
  request: number,
): Promise<typeof import("./player").Player | null> {
  if (isZip) {
    const r = await call({ type: "selectEntry", index: entry });
    if (request !== playerRequest || model !== workspace.model || r.type !== "stream" || !r.playable) return null;
    model.setStream(r.segments, r.trace, r.idatBytes);
  }
  if (!model.playable) return null;
  return loadPlayerClass(model, entry, step, request);
}

function finishPlayerRequest(request: number): void {
  if (request !== playerRequest || !playerPending) return;
  playerPending = false;
  syncAppNavigation();
}

async function openPlayer(entry = selectedEntry(), step?: number): Promise<DeflatePlayer | null> {
  const model = workspace.model;
  if (!model) return null;
  const isZip = model.file.format === "zip";
  if (isZip ? entry < 0 || !model.entry(entry).playable : !model.playable) return null;
  clearPlayerLoadError();
  if (isZip) closePlayer();
  const request = ++playerRequest;
  playerPending = true;
  syncAppNavigation();

  let PlayerClass: typeof import("./player").Player | null;
  try {
    PlayerClass = await preparePlayerClass(model, isZip, entry, step, request);
  } catch (error) {
    finishPlayerRequest(request);
    throw error;
  }
  finishPlayerRequest(request);
  if (!PlayerClass || request !== playerRequest || model !== workspace.model) return null;
  player ??= createPlayer(PlayerClass);
  document.body.classList.add("is-playing");
  syncAppNavigation();
  await player.open(model, playerSource);
  if (step !== undefined) player.jump(step);
  return player;
}

function closePlayer(): void {
  playerRequest++;
  playerPending = false;
  player?.close();
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
  if (model && id > 0) tools?.setPart(model.start(id), model.len(id), model.label(id));
  drawer.showNode(model, id, id >= 0);
  // While playing, the button is also how the player closes: keep it.
  playBtn.hidden = !canPlay() && !player?.isOpen;
  syncAppNavigation();
  renderStatus();
}

/** Selects a node without scrolling to it: the view is already where it should be. */
function selectInPlace(id: number): void {
  const model = workspace.model;
  selected = id;
  if (model && id > 0) tools?.setPart(model.start(id), model.len(id), model.label(id));
  hex.setSelected(id);
  minimap.setSelected(id);
  tree.setSelected(id);
  drawer.showNode(model, id, id >= 0);
  appNavigation.dataset.inspectorRequested = "true";
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
    setStatusOffset(start);
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
function point(id: number, offset?: number): void {
  select(id, offset);
  appNavigation.dataset.inspectorRequested = "true";
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
  appNavigation.dataset.inspectorRequested = "true";
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
  } else if (f.format === "tiff") {
    chips.push("TIFF");
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
    // A document by what it is; an archive by what is in it.
    const kind = kindOf(m);
    chips.push(...(kind === "ZIP" ? ["ZIP", m.value(0)] : [kind]));
  } else if (f.format === "rtf") {
    chips.push("Rich Text");
  } else if (f.format === "svg") {
    chips.push("SVG");
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
  // Each its own text, so each is translated on its own.
  plain.replaceChildren(kindWord(m), "  ·  ", formatBytes(m.bytes.length));
  const full = document.createElement("span");
  full.className = "meta-full";
  // Each its own text, so each is translated on its own: "parsed in 2 ms" is not a whole line.
  full.replaceChildren(...chips.flatMap((chip, i) => (i > 0 ? [document.createTextNode("  ·  "), chipText(chip)] : [chipText(chip)])));
  meta.append(plain, full);
  info.replaceChildren(name, meta);

  // A location is not damage, so it gets its own badge rather than joining
  // the problems count — but it is the first thing a person should see.
  locationBtn.hidden = !f.location;
}

function chipText(text: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.textContent = text;
  return span;
}

workspace = new WorkspaceController({
  maxFileBytes: MAX_FILE,
  getSelected: () => selected,
  services: {
    async parse(file, options) {
      // A reader in Ukrainian gets the answer in Ukrainian: the dictionary, if not here yet, comes beside the parser.
      const [response] = await Promise.all([
        call({ type: "parse", file }, undefined, { signal: options.signal, onProgress: options.onProgress }),
        currentLocale() === "uk" ? loadUkrainian().catch(() => undefined) : undefined,
      ]);
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
    progress: showMovieProgress,
    show: renderFile,
    select,
    failed: loadFailed,
    cancelled(model, levels) {
      document.body.dataset.state = "ready";
      syncAppNavigation();
      showFileInfo(model, levels);
      readAnnouncement.textContent = translateText("Reading was cancelled.", currentLocale());
      $("fileinfo").append(readAnnouncement);
    },
    note: (message) => drawer.showNote(message),
    remember,
    afterOpen(model) {
      arrive();
      if (followLink(model)) return;
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

function showMovieProgress(progress: MovieProgress): void {
  const locale = currentLocale();
  const phase = {
    scan: "structure",
    ranges: "metadata",
    entropy: "byte map",
  } as const;
  const percentage = Math.min(100, Math.floor((progress.loaded / Math.max(1, progress.total)) * 100));
  const label = progress.stage === "parse"
    ? "Parsing video metadata…"
    : `Reading video ${phase[progress.stage]}: ${percentage}%`;
  const localizedLabel = translateText(label, locale);
  readProgress.textContent = localizedLabel;
  readProgress.setAttribute("aria-label", translateText("Video read progress", locale));
  readProgress.setAttribute("aria-valuetext", localizedLabel);
  if (progress.stage === "parse") readProgress.removeAttribute("aria-valuenow");
  else readProgress.setAttribute("aria-valuenow", String(percentage));
  const announcementKey = progress.stage === "parse"
    ? `${locale}:${progress.stage}`
    : `${locale}:${progress.stage}:${Math.floor(percentage / 10) * 10}`;
  if (readAnnouncement.dataset.key !== announcementKey) {
    readAnnouncement.dataset.key = announcementKey;
    readAnnouncement.textContent = localizedLabel;
  }
  cancelRead.textContent = translateText("Cancel", locale);
  cancelRead.disabled = !progress.cancellable;
  const fileinfo = $("fileinfo");
  if (readProgress.parentElement !== fileinfo || readAnnouncement.parentElement !== fileinfo || cancelRead.parentElement !== fileinfo) {
    fileinfo.replaceChildren(readProgress, readAnnouncement, cancelRead);
  }
}

onWorkerReset(() => {
  const state = document.body.dataset.state;
  if (state === "ready" || (state === "loading" && workspace.isOpeningFile)) {
    void workspace.recoverAfterWorkerReset();
  }
});

// --- many files ------------------------------------------------------------

const batchView = new BatchView($("batch"), {
  canShareCopies: phoneCanShare,
  open: (i) => batchController.open(i),
  toggleReading: () => batchController.toggleReading(),
  saveClean: (keepNames, includeCopyFingerprints) => void batchController.saveClean(keepNames, includeCopyFingerprints),
  mosaic: (items) => void import("./photo-mosaic").then((m) => m.showMosaic(items)).catch((e) => announce(String(e))),
  filmRoll: (items) => void import("./film-roll-ui").then((m) => m.showFilmRoll(items.map((i) => i.file))).catch((e) => announce(String(e))),
});
batchController = new BatchController(batchView, {
  analyze: analyzeBatchFile,
  openFile: (file) => void workspace.openFile(file),
  saveClean: (items, keepNames, includeCopyFingerprints, output, isCurrent) =>
    import("./batchclean").then((m) => m.cleanCopies(items, keepNames, output, isCurrent, includeCopyFingerprints)),
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
async function analyzeBatchFile(file: File, signal: AbortSignal): Promise<BatchAnalysis> {
  if (file.size > MAX_FILE) throw new Error(tooLarge(file));
  const response = await call({ type: "parse", file }, undefined, { signal });
  if (response.type !== "parsed") throw new Error(response.type === "error" ? response.message : "unexpected reply");
  signal.throwIfAborted();
  const parsed = new FileModel(response.result, response.bytes, file.name, file);
  await (await import("./qrfacts")).addCodeFacts(parsed, false, signal);
  signal.throwIfAborted();
  return {
    kind: kindOf(parsed),
    format: parsed.file.format,
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
  clearPlayerLoadError();
  hover = -1;
  selected = -1;
  problemCursor = -1;
  tourDismissedByNavigation = false;
  setStatusOffset(-1);
  document.body.dataset.state = "ready";
  drawer.nested = levels.length;

  hex.setModel(m);
  tools?.setModel(m);
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
  // What each part is comes after the answer: the parser leaves it out.
  void explain(m.file.format).then(
    () => {
      if (m !== workspace.model) return;
      tree.refreshTitles();
      drawer.refreshNode();
    },
    () => {},
  );
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
  delete appNavigation.dataset.inspectorRequested;
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

export async function loadSample(path: string, name: string): Promise<void> {
  // Loading from the moment it is asked for: the landing demo shares the
  // worker, and must not parse its own file over this one.
  document.body.dataset.state = "loading";
  $("fileinfo").textContent = `Opening ${name}…`;
  let res: Response | null = null;
  // A dropped connection gets a second try before it is said.
  for (let attempt = 0; attempt < 2 && !res; attempt++) {
    if (attempt > 0) await new Promise((r) => setTimeout(r, 800));
    res = await fetch(siteUrl(path)).catch(() => null);
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
export function loadFailed(message: string): void {
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

/** Opens the sample a button names. */
export async function openSample(btn: HTMLButtonElement): Promise<void> {
  batchController.clear();
  await loadSample(btn.dataset.sample!, btn.dataset.name!);
  // "Watch compression work" goes straight to the player.
  if (btn.dataset.then === "play" && canPlay()) void openPlayer();
}

// The guides link to a sample by name, as ?sample=photo.jpg. Only the
// samples on this page can be opened this way; the name picks a button.
/** Where a link asked the sample to open: a view, and a part by its name. */
let linked: { view: WorkspaceView | null; part: string | null } | null = null;
const LINKABLE_VIEWS: readonly WorkspaceView[] = ["overview", "metadata", "content", "structure", "bytes"];

/** Opens the linked view and part once the sample is shown; whether a link asked for one. */
function followLink(model: FileModel): boolean {
  const asked = linked;
  linked = null;
  if (!asked || (!asked.view && !asked.part)) return false;
  const part = asked.part?.toLowerCase();
  const id = part ? model.file.labels.findIndex((label) => label.toLowerCase() === part) : -1;
  tourDismissedByNavigation = true;
  setWorkspaceView(asked.view ?? (id > 0 ? "bytes" : "overview"));
  if (id > 0) select(id);
  return true;
}

/** Opens the sample a link names, at the view and part it asks for. */
export function openLinkedSample(): void {
  const params = new URLSearchParams(location.search);
  const name = params.get("sample");
  if (!name) return;
  const view = params.get("view") as WorkspaceView | null;
  linked = { view: view && LINKABLE_VIEWS.includes(view) ? view : null, part: params.get("part") };
  history.replaceState(null, "", location.pathname);
  const btn = [...document.querySelectorAll<HTMLButtonElement>("[data-sample]")].find((b) => b.dataset.name === name);
  btn?.click();
}

// A share id names one complete submission, never a filename.
const SHARED_ERRORS: Record<string, string> = {
  empty: "The shared submission is empty. Choose files and try sharing again.",
  count: "The shared submission has more than 50 files. Share up to 50 files at a time.",
  size: "The shared submission is larger than 400 MiB. Share a smaller set of files.",
  storage: "The files could not be saved in this browser. Free up storage and share them again.",
  receive: "Could not receive these files. Try sharing them again.",
};
const SHARED_MISSING = "This shared file is no longer available. Share it again from the app that has it.";
const SHARED_OPEN_FAILED = "Could not open these shared files. Share them again from the app that has them.";

// A file shared to the installed app arrives through the service worker's
// local inbox. The legacy single-file key remains readable during migration.
export async function openShared(): Promise<void> {
  const params = new URLSearchParams(location.search);
  if (!params.has("shared") && !params.has("share-error")) return;
  const id = params.get("shared");
  const reason = params.get("share-error");
  history.replaceState(null, "", location.pathname);
  if (reason) {
    loadFailed(translateText(SHARED_ERRORS[reason] ?? SHARED_ERRORS.receive, currentLocale()));
    return;
  }
  document.body.dataset.state = "loading";
  try {
    const { receiveSharedFiles } = await import("./shared-inbox");
    const result = await receiveSharedFiles(id);
    if ("reason" in result) {
      const message = result.reason === "missing" ? SHARED_MISSING : SHARED_OPEN_FAILED;
      loadFailed(translateText(message, currentLocale()));
      return;
    }
    openFiles(result.files);
  } catch {
    loadFailed(translateText(SHARED_OPEN_FAILED, currentLocale()));
  }
}

problemsBtn.addEventListener("click", () => {
  toBytes();
  nextProblem();
  hex.flash(selected);
});
locationBtn.addEventListener("click", () => {
  const where = workspace.model?.file.location;
  // On a phone, where it was taken is the map and the place in the summary:
  // the bytes are a tab away for whoever wants them.
  if (where && narrow.matches) {
    setWorkspaceView("overview", false);
    if (drawer.showFact(where.node)) return;
  }
  toBytes();
  if (where) point(where.node);
});
playBtn.addEventListener("click", () => {
  if (player?.isOpen || playerPending) return closePlayer();
  toBytes();
  void openPlayer();
});

/** One file opens; several are listed. */
export function openFiles(files: File[]): void {
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
  if (player?.handleKey(e)) {
    e.preventDefault();
    return;
  }
  if (e.key === "Escape") {
    if (player?.isOpen || playerPending) closePlayer();
    else select(-1);
  }
  // Letters only bare: Cmd-N and Ctrl-P belong to the browser. "?" and O are main.ts's.
  const bare = !e.metaKey && !e.ctrlKey && !e.altKey;
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
  if (bare && (e.key === "p" || e.key === "P") && canPlay() && !playerPending && !player?.isOpen) void openPlayer();
});
