import type { PublicationProfile } from "./preflight-model";
// Many files at once: a list that says, for each, what hexscope found — the
// same verdict a single file opens on, in a few words — so a folder of
// photos or documents can be checked before it is sent. Each opens as a
// file of its own, with a way back; all the clean copies save as one ZIP.
import type { VerdictLine } from "./verdict";
import { listed } from "./headline";
import { currentLocale, translateText } from "./i18n";
import { batchCoverageCsv, batchCoverageReport } from "./batch-coverage-report";
import { batchDiagnosticRows, batchDiagnosticsCsv, batchDiagnosticsReport } from "./batch-diagnostics-report";
import { phoneCanShare, saveAs, shareFile } from "./files";
import { cleanable, type BatchFormatCoverage, type BatchItem } from "./batch-model";

// The view and its queue controller form one public composition surface.
export { BatchController, type BatchAnalysis, type BatchControllerHooks, type BatchViewPort } from "./batch-controller";
export { cleanable } from "./batch-model";
export type { BatchFormatCoverage, BatchItem } from "./batch-model";

export type BatchShow = "all" | "reveals" | "problems" | "incomplete" | "policy";

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
  cleanable: number;
  problemCount: number;
  policyBlocked: number;
  policyUnchecked: number;
  policyChecked: number;
  coverageByFormat: BatchFormatCoverage[];
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
  const coverageIncomplete = item.coverage === "partial" || item.coverage === "unsupported";
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
  if (coverageIncomplete) {
    hasUnknownOrDamage = true;
    hasIncomplete = true;
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
    canClean: cleanable(item),
  };
}

/** Summarizes a batch and applies its current filter in one pass. */
export function summarizeBatch(
  items: BatchItem[],
  show: BatchShow,
  visibleLimit = Number.POSITIVE_INFINITY,
  formatFilter: string | null = null,
): BatchSummary {
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
    cleanable: 0,
    problemCount: 0,
    policyBlocked: 0,
    policyUnchecked: 0,
    policyChecked: 0,
    coverageByFormat: [],
    shownCount: 0,
    shown: [],
    anyPick: false,
  };
  const byFormat = new Map<string, BatchFormatCoverage>();

  for (let index = 0; index < items.length; index++) {
    const item = items[index]!;
    const facts = summarizeBatchItem(item);
    if (item.state === "done" && item.format && item.coverage) {
      let row = byFormat.get(item.format);
      if (!row) {
        row = { format: item.format, files: 0, complete: 0, partial: 0, unsupported: 0 };
        byFormat.set(item.format, row);
      }
      row.files++;
      row[item.coverage]++;
    }
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
    if (facts.canClean) {
      summary.cleanable++;
      if (!item.skip) summary.chosen++;
    }

    const blocked = (item.policy?.blocked_kinds.length ?? 0) > 0;
    if (item.policy) summary.policyChecked++;
    if (blocked) summary.policyBlocked++;
    if (item.policy?.unchecked || item.state === "failed") summary.policyUnchecked++;
    const matches = show === "all"
      || (show === "reveals" ? facts.reveals : show === "incomplete" ? facts.incomplete : show === "policy" ? blocked || item.policy?.unchecked || item.state === "failed" : facts.problem);
    if (matches && (formatFilter === null || item.format === formatFilter)) {
      summary.shownCount++;
      if (summary.shown.length < visibleLimit) summary.shown.push({ item, index });
      if (facts.canClean) summary.anyPick = true;
    }
  }
  summary.busy = summary.settled < items.length;
  summary.coverageByFormat = [...byFormat.values()].sort((a, b) => a.format < b.format ? -1 : a.format > b.format ? 1 : 0);
  return summary;
}

export interface BatchHooks {
  open(index: number): void;
  toggleReading(): void;
  retryFailed(): boolean;
  applyProfile?(profile: PublicationProfile): boolean;
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
const DIAGNOSTIC_PAGE = 50;

/** A report in sentences, each its own text so each is translated on its own. */
function sentences(text: string): Node[] {
  return text
    .trim()
    .split(/(?<=[.…])\s+/)
    .filter(Boolean)
    .flatMap((line, i) => [...(i ? [document.createTextNode(" ")] : []), el("span", undefined, line)]);
}

function diagnosticStatusLabel(status: string): string {
  switch (status) {
    case "read_failed": return "Read failed";
    case "partial": return "Partial";
    case "unsupported": return "Unsupported";
    case "damaged": return "Damaged";
    default: return "Needs attention";
  }
}

export class BatchView {
  private status = el("p", "batch-result");
  private show: BatchShow = "all";
  private formatFilter: string | null = null;
  private coverageOpen = false;
  private limit = PAGE;
  private items: BatchItem[] = [];
  private lastDrawn = 0;
  private pending = 0;
  private paused = false;
  private keepNames = false;
  private profileActive = false;
  private includeCopyFingerprints = false;

  constructor(
    private host: HTMLElement,
    private hooks: BatchHooks,
  ) {
    this.status.setAttribute("role", "status");
    this.status.setAttribute("aria-live", "polite");
    this.status.setAttribute("aria-atomic", "true");
  }

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
    if (this.items !== items) {
      this.show = "all";
      this.profileActive = false;
      this.formatFilter = null;
      this.limit = PAGE;
    }
    this.items = items;
    this.paused = paused;
    const wait = 150 - (performance.now() - this.lastDrawn);
    if (wait > 0 && items.some((i) => i.state === "waiting" || i.state === "reading")) {
      if (!this.pending) this.pending = window.setTimeout(() => this.draw(), wait);
      return;
    }
    this.draw();
  }

  /** Selects the cleanable members of one-based mosaic indexes and excludes other cleanable files. */
  selectMosaicGroup(indexes: readonly number[]): void {
    const selectedIndexes = new Set(
      indexes
        .filter((index) => Number.isInteger(index) && index > 0 && index <= this.items.length)
        .map((index) => index - 1),
    );
    if (!selectedIndexes.size) return;
    let selected = 0;
    for (let index = 0; index < this.items.length; index++) {
      const item = this.items[index]!;
      if (!cleanable(item)) continue;
      item.skip = !selectedIndexes.has(index);
      if (!item.skip) selected++;
    }
    const groupSize = selectedIndexes.size;
    this.result = currentLocale() === "uk"
      ? `Вибрано ${selected} з ${groupSize} фото групи для чистих копій. Решта файлів групи не мають доступної тут чистої копії. Інші файли виключено. Перевірте прапорці перед збереженням.`
      : `Selected ${selected} of ${groupSize} files in this group for clean copies. The remaining group files have no clean copy available here. Other files are excluded. Review the checkboxes before saving.`;
    this.draw();
  }

  /** Returns to a file from a mosaic group and gives its batch row focus. */
  focusItem(index: number): void {
    if (!Number.isInteger(index) || index < 0 || index >= this.items.length) return;
    this.show = "all";
    this.formatFilter = null;
    this.limit = Math.min(
      this.items.length,
      Math.max(this.limit, Math.ceil((index + 1) / PAGE) * PAGE),
    );
    this.draw();

    const row = this.host.querySelector<HTMLButtonElement>(`[data-batch-index="${index}"]`);
    if (!row || row.disabled) return;
    const behavior = window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth";
    row.scrollIntoView({ behavior, block: "center" });
    row.focus({ preventScroll: true });
    row.classList.add("is-mosaic-target");
    window.setTimeout(() => row.classList.remove("is-mosaic-target"), 1800);
  }

  private draw(): void {
    clearTimeout(this.pending);
    this.pending = 0;
    this.lastDrawn = performance.now();
    const active = document.activeElement;
    const focusedControl = active instanceof HTMLElement && this.host.contains(active)
      ? active.dataset.batchFocus
      : undefined;
    const items = this.items;
    const totals = summarizeBatch(items, this.show, this.limit, this.formatFilter);

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
    summary.setAttribute("role", "status");
    summary.setAttribute("aria-live", totals.busy ? "off" : "polite");
    summary.setAttribute("aria-atomic", "true");
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
    const failedCount = items.filter((item) => item.state === "failed").length;
    if (!totals.busy && failedCount > 0) {
      const retry = el("button", "btn batch-retry-failed", translateText("Retry failed files", currentLocale()));
      retry.type = "button";
      retry.title = translateText("Only failed files are read again; completed files stay untouched.", currentLocale());
      retry.addEventListener("click", () => this.hooks.retryFailed());
      head.append(retry);
    }
    if (totals.busy) {
      const toggle = el("button", "btn batch-toggle", translateText(this.paused ? "Resume reading" : "Pause reading", currentLocale()));
      toggle.type = "button";
      toggle.dataset.batchFocus = "reading-toggle";
      toggle.setAttribute("aria-pressed", String(this.paused));
      toggle.addEventListener("click", () => this.hooks.toggleReading());
      head.append(toggle);
    } else {
      head.append(save, keep, fingerprint);
      if (chosen > 0) {
        head.append(el("p", "batch-copy-limit", translateText(
          "In a batch, if the original or clean copy exceeds 10 MiB, read-back verification may be skipped; affected findings stay Not checked.",
          currentLocale(),
        )));
      }
    }
    head.append(this.status);
    if (!totals.busy && this.hooks.applyProfile) {
      const profile = el("button", "btn", currentLocale() === "uk" ? "Профіль для пакета" : "Batch publication profile");
      profile.onclick = () => {
        void import("./batch-policy-dialog").then(({ showBatchPolicyDialog }) => {
          if (this.items !== items) return;
          showBatchPolicyDialog((rules) => {
            if (this.items !== items) return false;
            this.profileActive = true;
            return this.hooks.applyProfile?.(rules) ?? false;
          });
        });
      };
      head.append(profile);
    }
    if (this.profileActive) {
      const uk = currentLocale() === "uk";
      head.append(el("p", "batch-policy-summary", uk
        ? `Профіль · оцінено ${totals.policyChecked} з ${items.length} · заблоковано ${totals.policyBlocked} · неперевірено ${totals.policyUnchecked}`
        : `Profile · evaluated ${totals.policyChecked} of ${items.length} · blocked ${totals.policyBlocked} · unchecked ${totals.policyUnchecked}`));
    }
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
      ["incomplete", "Incomplete", totals.incomplete],
    ];
    if (this.profileActive) filters.push(["policy", currentLocale() === "uk" ? "Проблеми за профілем" : "Profile issues", items.filter((item) => item.state === "failed" || item.policy?.unchecked || item.policy?.blocked_kinds.length).length]);
    const bar = el("div", "batch-filter");
    bar.setAttribute("role", "group");
    bar.setAttribute("aria-label", "Show");
    for (const [key, label, n] of filters) {
      const b = el("button", undefined, `${label} · ${n}`);
      b.dataset.batchFocus = `filter:${key}`;
      b.setAttribute("aria-pressed", String(this.show === key));
      b.addEventListener("click", () => {
        this.show = key;
        this.formatFilter = null;
        this.limit = PAGE;
        this.draw();
      });
      bar.append(b);
    }
    card.append(bar);
    if (this.formatFilter !== null) {
      const formatName = items.find((item) => item.format === this.formatFilter)?.kind ?? this.formatFilter;
      const activeFormat = el("div", "batch-format-active");
      activeFormat.append(
        el("span", undefined, translateText("Showing format:", currentLocale())),
        el("strong", undefined, formatName),
      );
      const clearFormat = el("button", "btn batch-format-clear", translateText("Clear format filter", currentLocale()));
      clearFormat.type = "button";
      clearFormat.dataset.batchFocus = "clear-format";
      clearFormat.addEventListener("click", () => {
        this.formatFilter = null;
        this.limit = PAGE;
        this.draw();
      });
      activeFormat.append(clearFormat);
      card.append(activeFormat);
    }
    const selection = el("div", "batch-selection");
    selection.setAttribute("role", "group");
    selection.setAttribute("aria-label", translateText("Clean-copy selection", currentLocale()));
    if (!totals.busy && totals.cleanable > 1) {
      const allSelected = totals.chosen === totals.cleanable;
      const bulk = el(
        "button",
        "batch-select-all",
        translateText(
          allSelected ? "Clear clean-copy selection" : `Select all ${plural(totals.cleanable, "cleanable file", "cleanable files")}`,
          currentLocale(),
        ),
      );
      bulk.type = "button";
      bulk.dataset.batchFocus = "bulk-selection";
      bulk.title = translateText(
        allSelected
          ? "Remove every cleanable file in this batch from the copy selection"
          : "Select every cleanable file in this batch, including files hidden by the current filter",
        currentLocale(),
      );
      bulk.addEventListener("click", () => {
        const selectAll = !allSelected;
        for (const item of this.items) {
          if (cleanable(item)) item.skip = !selectAll;
        }
        this.draw();
      });
      selection.append(bulk);
    }
    const list = el("ul", "batch-list");
    // With any tick in the list, every row keeps its place for one.
    for (const { item, index } of totals.shown) {
      const li = el("li");
      const row = el("button", "batch-row");
      row.dataset.batchIndex = String(index);
      row.disabled = item.state === "waiting" || item.state === "reading";
      row.title = row.disabled ? "Still reading" : "Open this file";
      // A stable position lets a private diagnostic report point back to this row.
      const nameGroup = el("span", "batch-name-group");
      const number = el("span", "batch-number", `#${index + 1}`);
      number.setAttribute("aria-label", `${translateText("File number", currentLocale())} ${index + 1}`);
      // In a folder, where it is in the folder.
      const name = el("span", "batch-name", item.file.webkitRelativePath || item.file.name);
      const meta = el("span", "batch-meta", [item.kind, size(item.file.size)].filter(Boolean).join(" · "));
      nameGroup.append(number, name);
      row.append(nameGroup, meta);
      if (item.headline) row.append(el("span", `batch-answer is-${item.headline.tone}`, listed(item.headline)));
      const found = tags(item);
      if (found.length > 0) {
        const box = el("span", "batch-tags");
        box.append(...found);
        row.append(box);
      }
      if (item.policy) {
        const uk = currentLocale() === "uk";
        const result = [
          item.policy.blocked_kinds.length ? `${uk ? "Заблоковано" : "Blocked"} · ${item.policy.blocked_kinds.join(", ")}` : "",
          item.policy.unchecked ? (uk ? "Не перевірено" : "Unchecked") : "",
        ].filter(Boolean).join(" · ");
        row.append(el("span", "batch-meta", result || (uk ? "Профіль пройдено" : "Profile passed")));
      }
      row.addEventListener("click", () => this.hooks.open(index));
      // Whether this one goes into the clean copies.
      if (cleanable(item)) {
        const pick = el("input", "batch-pick");
        pick.type = "checkbox";
        pick.dataset.batchFocus = `pick:${index}`;
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
    card.prepend(head);
    if (!totals.busy && items.length > 0) {
      const coverage = el("details", "batch-coverage");
      coverage.open = this.coverageOpen;
      coverage.addEventListener("toggle", () => { this.coverageOpen = coverage.open; });
      coverage.append(el("summary", undefined, translateText(
        totals.coverageByFormat.length ? "Coverage by format" : "Batch reports",
        currentLocale(),
      )));
      const rows = el("ul", "batch-coverage-list");
      if (totals.coverageByFormat.length > 0) {
        coverage.append(el("p", "batch-coverage-note", translateText("Select a format to show matching files.", currentLocale())));
      }
      for (const entry of totals.coverageByFormat) {
        const row = el("li", "batch-coverage-row");
        row.dataset.coverageFormat = entry.format;
        const name = items.find((item) => item.format === entry.format)?.kind || entry.format;
        const counts = [
          entry.complete ? `${entry.complete} ${translateText("complete", currentLocale())}` : "",
          entry.partial ? `${entry.partial} ${translateText("partial", currentLocale())}` : "",
          entry.unsupported ? `${entry.unsupported} ${translateText("unsupported", currentLocale())}` : "",
        ].filter(Boolean);
        const filter = el("button", "batch-coverage-filter");
        filter.type = "button";
        filter.dataset.batchFocus = `format:${entry.format}`;
        filter.setAttribute("aria-pressed", String(this.formatFilter === entry.format));
        filter.setAttribute("aria-label", `${translateText("Filter this batch to the format", currentLocale())} ${name}`);
        filter.append(el("span", "batch-coverage-name", name), el("span", "batch-coverage-counts", counts.join(" · ")));
        filter.addEventListener("click", () => {
          this.formatFilter = this.formatFilter === entry.format ? null : entry.format;
          this.limit = PAGE;
          this.draw();
        });
        row.append(filter);
        rows.append(row);
      }
      const exportButton = el("button", "btn batch-coverage-export", translateText("Save coverage report", currentLocale()));
      exportButton.type = "button";
      exportButton.addEventListener("click", () => {
        const report = batchCoverageReport(totals.coverageByFormat, totals.settled);
        saveAs("hexscope-batch-coverage.json", new Blob([report], { type: "application/json" }));
      });
      const csvExportButton = el("button", "btn batch-coverage-export-csv", translateText("Save coverage report as CSV", currentLocale()));
      csvExportButton.type = "button";
      csvExportButton.addEventListener("click", () => {
        const report = batchCoverageCsv(totals.coverageByFormat, totals.settled);
        saveAs("hexscope-batch-coverage.csv", new Blob([report], { type: "text/csv" }));
      });
      const csvReportName = "hexscope-batch-coverage.csv";
      const reportName = "hexscope-batch-coverage.json";
      const actions = el("div", "batch-coverage-actions");
      if (phoneCanShare(reportName)) {
        const shareButton = el("button", "btn btn-primary batch-coverage-share", translateText("Share coverage report", currentLocale()));
        shareButton.type = "button";
        shareButton.addEventListener("click", () => {
          const report = batchCoverageReport(totals.coverageByFormat, totals.settled);
          void shareFile(new File([report], reportName, { type: "application/json" }));
        });
        actions.append(shareButton);
      }
      if (phoneCanShare(csvReportName)) {
        const shareCsvButton = el("button", "btn btn-primary batch-coverage-share-csv", translateText("Share coverage report as CSV", currentLocale()));
        shareCsvButton.type = "button";
        shareCsvButton.addEventListener("click", () => {
          const report = batchCoverageCsv(totals.coverageByFormat, totals.settled);
          void shareFile(new File([report], csvReportName, { type: "text/csv" }));
        });
        actions.append(shareCsvButton);
      }
      actions.append(exportButton, csvExportButton);
      const privacyNote = el("p", "batch-coverage-note", translateText(
        "The report includes format names and read counts only; no file names or findings.",
        currentLocale(),
      ));
      if (totals.coverageByFormat.length > 0) coverage.append(rows);
      else coverage.append(el("p", "batch-coverage-note", translateText("No files could be read by format.", currentLocale())));
      coverage.append(actions, privacyNote);
      const diagnostics = batchDiagnosticsReport(items);
      if (diagnostics) {
        const localList = el("ul", "batch-diagnostic-list");
        const localEntries = batchDiagnosticRows(items);
        for (const entry of localEntries.slice(0, DIAGNOSTIC_PAGE)) {
          const item = items[entry.index - 1]!;
          const localRow = el("li", "batch-diagnostic-row");
          const description = el("span", "batch-diagnostic-description");
          description.append(
            el("span", "batch-number", `#${entry.index}`),
            el("span", "batch-diagnostic-name", item.file.webkitRelativePath || item.file.name),
            el("span", "batch-meta", `${item.kind} · ${translateText(diagnosticStatusLabel(entry.status), currentLocale())}`),
          );
          const showFile = el("button", "btn batch-diagnostic-open", translateText("Show this file in the batch", currentLocale()));
          showFile.type = "button";
          showFile.dataset.batchFocus = `diagnostic:${entry.index}`;
          showFile.addEventListener("click", () => this.focusItem(entry.index - 1));
          localRow.append(description, showFile);
          localList.append(localRow);
        }
        const localParts: Node[] = [
          el("summary", undefined, translateText("Find files needing attention", currentLocale())),
          localList,
        ];
        if (localEntries.length > DIAGNOSTIC_PAGE) {
          const lookup = el("form", "batch-diagnostic-lookup");
          const lookupLabel = el("label", undefined, translateText("Find file number", currentLocale()));
          const fileNumber = el("input");
          fileNumber.type = "number";
          fileNumber.min = "1";
          fileNumber.max = String(items.length);
          fileNumber.step = "1";
          fileNumber.required = true;
          fileNumber.setAttribute("aria-label", translateText("Find file number", currentLocale()));
          const lookupButton = el("button", "btn", translateText("Show this file in the batch", currentLocale()));
          lookupButton.type = "submit";
          lookup.append(lookupLabel, fileNumber, lookupButton);
          lookup.addEventListener("submit", (event) => {
            event.preventDefault();
            if (!fileNumber.checkValidity()) return;
            this.focusItem(Number(fileNumber.value) - 1);
          });
          localParts.push(
            lookup,
            el("p", "batch-coverage-note", translateText("Use a row number from the report to find any file in this batch.", currentLocale())),
          );
        }
        localParts.push(el("p", "batch-coverage-note", translateText("File names appear here on this device only; exported reports omit them.", currentLocale())));
        const localDetails = el("details", "batch-diagnostics-local");
        localDetails.append(...localParts);
        coverage.append(localDetails);
        const diagnosticsName = "hexscope-batch-diagnostics.json";
        const diagnosticActions = el("div", "batch-coverage-actions");
        if (phoneCanShare(diagnosticsName)) {
          const shareDiagnostics = el("button", "btn btn-primary", translateText("Share diagnostic report", currentLocale()));
          shareDiagnostics.type = "button";
          shareDiagnostics.addEventListener("click", () => {
            void shareFile(new File([diagnostics], diagnosticsName, { type: "application/json" }));
          });
          diagnosticActions.append(shareDiagnostics);
        }
        const diagnosticsCsvName = "hexscope-batch-diagnostics.csv";
        if (phoneCanShare(diagnosticsCsvName)) {
          const shareDiagnosticsCsv = el("button", "btn btn-primary", translateText("Share diagnostic report as CSV", currentLocale()));
          shareDiagnosticsCsv.type = "button";
          shareDiagnosticsCsv.addEventListener("click", () => {
            const csvDiagnostics = batchDiagnosticsCsv(items);
            if (!csvDiagnostics) return;
            void shareFile(new File([csvDiagnostics], diagnosticsCsvName, { type: "text/csv" }));
          });
          diagnosticActions.append(shareDiagnosticsCsv);
        }
        const saveDiagnostics = el("button", "btn", translateText("Save diagnostic report", currentLocale()));
        saveDiagnostics.type = "button";
        saveDiagnostics.addEventListener("click", () => {
          saveAs(diagnosticsName, new Blob([diagnostics], { type: "application/json" }));
        });
        const saveDiagnosticsCsv = el("button", "btn", translateText("Save diagnostic report as CSV", currentLocale()));
        saveDiagnosticsCsv.type = "button";
        saveDiagnosticsCsv.addEventListener("click", () => {
          const csvDiagnostics = batchDiagnosticsCsv(items);
          if (!csvDiagnostics) return;
          saveAs(diagnosticsCsvName, new Blob([csvDiagnostics], { type: "text/csv" }));
        });
        diagnosticActions.append(saveDiagnostics, saveDiagnosticsCsv);
        coverage.append(
          diagnosticActions,
          el("p", "batch-coverage-note", translateText(
            "The diagnostic report lists affected row numbers, formats and statuses only; it excludes file names, paths, contents and raw errors.",
            currentLocale(),
          )),
        );
      }
      card.append(coverage);
    }
    if (selection.childNodes.length) card.append(selection);
    card.append(list);
    if (totals.shownCount > this.limit) {
      const more = el("button", "btn", `Show ${Math.min(PAGE, totals.shownCount - this.limit)} more of ${totals.shownCount - this.limit}`);
      more.addEventListener("click", () => {
        this.limit += PAGE;
        this.draw();
      });
      card.append(more);
    }
    this.host.replaceChildren(card);
    if (focusedControl) {
      const target = Array.from(this.host.querySelectorAll<HTMLElement>("[data-batch-focus]"))
        .find((control) => control.dataset.batchFocus === focusedControl);
      target?.focus({ preventScroll: true });
    }
  }
}
