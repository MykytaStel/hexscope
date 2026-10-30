// Many files at once: a list that says, for each, what hexscope found — the
// same verdict a single file opens on, in a few words — so a folder of
// photos or documents can be checked before it is sent. Each opens as a
// file of its own, with a way back; all the clean copies save as one ZIP.
import type { VerdictLine } from "./verdict";
import { listed, type Headline } from "./headline";
import { currentLocale, translateText } from "./i18n";

// The view and its queue controller form one public composition surface.
export { BatchController, type BatchAnalysis, type BatchControllerHooks, type BatchViewPort } from "./batch-controller";

export interface BatchItem {
  file: File;
  state: "waiting" | "reading" | "done" | "failed";
  /** Shown beside the name: "JPEG", "PDF 1.7". */
  kind: string;
  lines: VerdictLine[];
  /** The answer a single file opens on; null until it is read. */
  headline: Headline | null;
  /** What it gives away, as categories: "where it was taken". */
  reveals: string[];
  /** The clean copy's name, for the ZIP. */
  cleanName: string;
  /** Machine format code from the parser, used by portable receipts. */
  format?: string;
  note: string;
  /** Left out of the clean copies, by choice. */
  skip: boolean;
  copyStatus?: string;
  photoSignal?: import("./photo-mosaic").PhotoSignal;
}

/** Whether a file has something a clean copy takes out. */
export function cleanable(i: BatchItem): boolean {
  return i.state === "done" && (i.reveals.length > 0 || i.lines.some((l) => l.kind === "hidden"));
}

export type BatchShow = "all" | "reveals" | "problems";

export interface BatchSummary {
  settled: number;
  busy: boolean;
  revealing: number;
  damaged: number;
  incomplete: number;
  hidden: number;
  flagged: number;
  dangerous: boolean;
  chosen: number;
  problemCount: number;
  shownCount: number;
  shown: { item: BatchItem; index: number }[];
  anyPick: boolean;
}

interface BatchItemSummary {
  settled: boolean;
  reveals: boolean;
  damaged: boolean;
  incomplete: boolean;
  hidden: boolean;
  flagged: boolean;
  dangerous: boolean;
  problem: boolean;
  canClean: boolean;
}

function summarizeBatchItem(item: BatchItem): BatchItemSummary {
  const failed = item.state === "failed";
  let hasUnknownOrDamage = false;
  let hasDamage = false;
  let hasHidden = false;
  let hasMisnamed = false;
  let hasIncomplete = false;
  for (const line of item.lines) {
    if (line.kind === "unknown" || line.kind === "warning") {
      hasUnknownOrDamage = true;
      hasIncomplete = true;
    }
    if (line.kind === "damage") {
      hasUnknownOrDamage = true;
      hasDamage = true;
    }
    if (line.kind === "hidden") hasHidden = true;
    if (line.kind === "misnamed") hasMisnamed = true;
  }
  const reveals = item.reveals.length > 0;
  return {
    settled: item.state === "done" || failed,
    reveals,
    damaged: hasDamage,
    incomplete: failed || hasIncomplete,
    hidden: hasHidden,
    flagged: hasUnknownOrDamage || failed || item.headline?.tone === "danger" || item.headline?.tone === "warning",
    dangerous: failed || item.headline?.tone === "danger",
    problem: failed || hasDamage || hasHidden || hasMisnamed || hasIncomplete,
    canClean: item.state === "done" && (reveals || hasHidden),
  };
}

/** Summarizes a batch and applies its current filter in one pass. */
export function summarizeBatch(items: BatchItem[], show: BatchShow, visibleLimit = Number.POSITIVE_INFINITY): BatchSummary {
  const summary: BatchSummary = {
    settled: 0,
    busy: false,
    revealing: 0,
    damaged: 0,
    incomplete: 0,
    hidden: 0,
    flagged: 0,
    dangerous: false,
    chosen: 0,
    problemCount: 0,
    shownCount: 0,
    shown: [],
    anyPick: false,
  };

  for (let index = 0; index < items.length; index++) {
    const item = items[index]!;
    const facts = summarizeBatchItem(item);
    if (facts.settled) summary.settled++;
    if (facts.reveals) summary.revealing++;
    if (facts.damaged) summary.damaged++;
    if (facts.incomplete) summary.incomplete++;
    if (facts.hidden) summary.hidden++;
    if (facts.flagged) {
      summary.flagged++;
      if (facts.dangerous) summary.dangerous = true;
    }
    if (facts.problem) summary.problemCount++;
    if (facts.canClean && !item.skip) summary.chosen++;

    const matches = show === "all" || (show === "reveals" ? facts.reveals : facts.problem);
    if (matches) {
      summary.shownCount++;
      if (summary.shown.length < visibleLimit) summary.shown.push({ item, index });
      if (facts.canClean) summary.anyPick = true;
    }
  }
  summary.busy = summary.settled < items.length;
  return summary;
}

export interface BatchHooks {
  open(index: number): void;
  toggleReading(): void;
  /** Makes the copies of the files chosen; `keepNames` names each as the original. */
  saveClean(keepNames: boolean, includeCopyFingerprints: boolean): void;
  /** Whether the clean copies can go to the share sheet, one file each: a phone. */
  canShareCopies(): boolean;
  mosaic?(items: BatchItem[]): void;
  filmRoll?(items: BatchItem[]): void;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function size(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** Worst first: damage, a wrong name, something hidden, what it reveals. */
function tags(item: BatchItem): HTMLElement[] {
  if (item.state === "waiting" || item.state === "reading") {
    return [el("span", "tag is-waiting", item.state === "reading" ? "Reading…" : "Waiting")];
  }
  if (item.state === "failed") return [el("span", "tag is-damage", item.note || "Could not be read")];
  const out: HTMLElement[] = [];
  if (item.copyStatus) out.push(el("span", "tag", item.copyStatus));
  const has = (k: VerdictLine["kind"]) => item.lines.some((l) => l.kind === k);
  if (has("unknown")) out.push(el("span", "tag is-unknown", "Not a format hexscope reads"));
  if (has("warning")) out.push(el("span", "tag is-unknown", "Partial check"));
  if (has("damage")) out.push(el("span", "tag is-damage", "Damaged"));
  if (has("misnamed")) out.push(el("span", "tag is-oddity", "Wrong extension"));
  if (has("hidden")) out.push(el("span", "tag is-hidden", "Something hidden"));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  for (const r of item.reveals.slice(0, 4)) out.push(el("span", "tag is-reveals", cap(r)));
  if (item.reveals.length > 4) out.push(el("span", "tag is-reveals", `+${item.reveals.length - 4} more`));
  return out;
}

/** Rows drawn at a time: a folder of thousands would make the page crawl. */
const PAGE = 300;

/** A report in sentences, each its own text so each is translated on its own. */
function sentences(text: string): Node[] {
  return text
    .trim()
    .split(/(?<=[.…])\s+/)
    .filter(Boolean)
    .flatMap((line, i) => [...(i ? [document.createTextNode(" ")] : []), el("span", undefined, line)]);
}

export class BatchView {
  private status = el("p", "batch-result");
  private show: BatchShow = "all";
  private limit = PAGE;
  private items: BatchItem[] = [];
  private lastDrawn = 0;
  private pending = 0;
  private paused = false;
  private keepNames = false;
  private includeCopyFingerprints = false;

  constructor(
    private host: HTMLElement,
    private hooks: BatchHooks,
  ) {}

  /** Says how saving the clean copies went, under the button. */
  set result(text: string) {
    this.status.replaceChildren(...sentences(text));
  }

  /** Says what is ready, with what to do with it. */
  offer(text: string, ...actions: HTMLButtonElement[]): void {
    const buttons = el("div", "batch-actions");
    buttons.append(...actions.map((a) => (a.classList.add("batch-action"), a)));
    this.status.replaceChildren(...sentences(text), buttons);
    this.host.querySelector(".batch-head .btn-clean")?.classList.replace("btn-clean", "btn-again");
  }

  /** Draws the list, at most a few times a second while files are read. */
  render(items: BatchItem[], paused = false): void {
    this.items = items;
    this.paused = paused;
    const wait = 150 - (performance.now() - this.lastDrawn);
    if (wait > 0 && items.some((i) => i.state === "waiting" || i.state === "reading")) {
      if (!this.pending) this.pending = window.setTimeout(() => this.draw(), wait);
      return;
    }
    this.draw();
  }

  private draw(): void {
    clearTimeout(this.pending);
    this.pending = 0;
    this.lastDrawn = performance.now();
    const items = this.items;
    const totals = summarizeBatch(items, this.show, this.limit);

    const card = el("div", "batch-card");
    const head = el("div", "batch-head");
    const label = el("p", "batch-label", plural(items.length, "file", "files"));
    const tone = totals.busy ? "neutral" : totals.dangerous ? "danger" : totals.flagged ? "warning" : "ok";
    const title = el(
      "h2",
      `batch-title is-${tone}`,
      totals.busy
        ? this.paused
          ? translateText("Reading paused", currentLocale())
          : `Reading ${totals.settled + 1} of ${items.length}…`
        : totals.flagged
          ? `${totals.flagged} of ${plural(items.length, "file needs", "files need")} a look`
          : `Nothing personal found in ${items.length === 1 ? "this file" : `these ${items.length} files`}`,
    );
    const summary = el("p", "batch-summary");
    if (totals.busy) {
      summary.textContent = "Nothing leaves this tab.";
    } else {
      const parts = [
        totals.revealing ? `${totals.revealing} ${totals.revealing === 1 ? "reveals" : "reveal"} something about you` : "",
        totals.hidden ? `${totals.hidden} ${totals.hidden === 1 ? "hides" : "hide"} something` : "",
        totals.damaged ? `${totals.damaged} damaged` : "",
        totals.incomplete ? `${totals.incomplete} not fully checked` : "",
      ].filter(Boolean);
      // Each part its own text, so each is translated on its own.
      if (parts.length) summary.append(...parts.flatMap((part, i) => [...(i ? [" · "] : []), el("span", undefined, part)]), ".");
      else summary.textContent = "No personal metadata found by the available checks. Visible content has not been cleared.";
    }
    const phone = this.hooks.canShareCopies();
    const chosen = totals.chosen;
    const copies = plural(chosen, "clean copy", "clean copies");
    // Once the copies wait to be shared, sharing is the one thing to do: making them again steps back.
    const offered = this.status.querySelector(".batch-actions") !== null;
    const save = el("button", `btn ${offered ? "btn-again" : "btn-clean"}`, totals.busy || chosen === 0 ? (phone ? "Make clean copies" : "Save clean copies (.zip)") : phone ? `Make ${copies}` : `Save ${copies} (.zip)`);
    save.disabled = totals.busy || chosen === 0;
    save.title = phone
      ? "Makes the copies in this tab, to share or save: nothing is uploaded"
      : "Makes the copies in this tab and saves them as one ZIP: nothing is uploaded";
    save.addEventListener("click", () => this.hooks.saveClean(this.keepNames, this.includeCopyFingerprints));
    // "photo.jpg" rather than "photo-clean.jpg": in their own ZIP they need no mark.
    const keep = el("label", "batch-keep");
    const box = el("input");
    box.type = "checkbox";
    box.checked = this.keepNames;
    box.addEventListener("change", () => (this.keepNames = box.checked));
    keep.append(box, el("span", undefined, "Keep the original names"));
    const fingerprint = el("label", "batch-keep batch-fingerprints");
    const fingerprintBox = el("input");
    fingerprintBox.type = "checkbox";
    fingerprintBox.checked = this.includeCopyFingerprints;
    fingerprintBox.addEventListener("change", () => (this.includeCopyFingerprints = fingerprintBox.checked));
    fingerprint.title = translateText(
      "Direct phone sharing sends copies only; Save as a ZIP includes the receipt and fingerprints.",
      currentLocale(),
    );
    fingerprint.append(
      fingerprintBox,
      el("span", undefined, translateText("Include stable SHA-256 copy fingerprints in ZIP receipt (up to 10 MiB per copy)", currentLocale())),
    );
    fingerprint.hidden = totals.busy || chosen === 0;
    // Copies shared to a chat go one by one, under the names the chat shows: the choice is for a ZIP.
    keep.hidden = totals.busy || chosen === 0 || phone;
    head.append(label, title, summary);
    if (totals.busy) {
      const toggle = el("button", "btn batch-toggle", translateText(this.paused ? "Resume reading" : "Pause reading", currentLocale()));
      toggle.type = "button";
      toggle.setAttribute("aria-pressed", String(this.paused));
      toggle.addEventListener("click", () => this.hooks.toggleReading());
      head.append(toggle);
    } else {
      head.append(save, keep, fingerprint);
    }
    head.append(this.status);
    // The tools for a whole roll of photos, out of the way of the one thing most come for.
    const tools: HTMLButtonElement[] = [];
    if (!totals.busy && items.length <= 1000 && this.hooks.mosaic) {
      const mosaic = el("button", "btn", "Photo privacy mosaic");
      mosaic.addEventListener("click", () => this.hooks.mosaic?.(items));
      tools.push(mosaic);
    }
    if (!totals.busy && this.hooks.filmRoll) {
      const roll = el("button", "btn", "Process film scans");
      roll.addEventListener("click", () => this.hooks.filmRoll?.(items));
      tools.push(roll);
    }
    if (tools.length) {
      const more = el("details", "batch-tools");
      more.append(el("summary", undefined, "More tools"), ...tools);
      head.append(more);
    }

    const filters: [BatchShow, string, number][] = [
      ["all", "All", items.length],
      ["reveals", "Give something away", totals.revealing],
      ["problems", "Needs attention", totals.problemCount],
    ];
    const bar = el("div", "batch-filter");
    bar.setAttribute("role", "group");
    bar.setAttribute("aria-label", "Show");
    for (const [key, label, n] of filters) {
      const b = el("button", undefined, `${label} · ${n}`);
      b.setAttribute("aria-pressed", String(this.show === key));
      b.addEventListener("click", () => {
        this.show = key;
        this.limit = PAGE;
        this.draw();
      });
      bar.append(b);
    }
    const list = el("ul", "batch-list");
    // With any tick in the list, every row keeps its place for one.
    for (const { item, index } of totals.shown) {
      const li = el("li");
      const row = el("button", "batch-row");
      row.disabled = item.state === "waiting" || item.state === "reading";
      row.title = row.disabled ? "Still reading" : "Open this file";
      // In a folder, where it is in the folder.
      const name = el("span", "batch-name", item.file.webkitRelativePath || item.file.name);
      const meta = el("span", "batch-meta", [item.kind, size(item.file.size)].filter(Boolean).join(" · "));
      row.append(name, meta);
      if (item.headline) row.append(el("span", `batch-answer is-${item.headline.tone}`, listed(item.headline)));
      const found = tags(item);
      if (found.length > 0) {
        const box = el("span", "batch-tags");
        box.append(...found);
        row.append(box);
      }
      row.addEventListener("click", () => this.hooks.open(index));
      // Whether this one goes into the clean copies.
      if (cleanable(item)) {
        const pick = el("input", "batch-pick");
        pick.type = "checkbox";
        pick.checked = !item.skip;
        pick.setAttribute("aria-label", `Make a clean copy of ${item.file.name}`);
        pick.title = "Make a clean copy of this one";
        pick.addEventListener("change", () => {
          item.skip = !pick.checked;
          this.draw();
        });
        li.classList.add("has-pick");
        li.append(pick);
      } else if (totals.anyPick) {
        li.classList.add("has-pick");
        li.append(el("span", "batch-pick"));
      }
      li.append(row);
      list.append(li);
    }
    card.append(head, bar, list);
    if (totals.shownCount > this.limit) {
      const more = el("button", "btn", `Show ${Math.min(PAGE, totals.shownCount - this.limit)} more of ${totals.shownCount - this.limit}`);
      more.addEventListener("click", () => {
        this.limit += PAGE;
        this.draw();
      });
      card.append(more);
    }
    this.host.replaceChildren(card);
  }
}
