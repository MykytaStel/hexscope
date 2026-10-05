// Many files at once: a list that says, for each, what hexscope found — the
// same verdict a single file opens on, in a few words — so a folder of
// photos or documents can be checked before it is sent. Each opens as a
// file of its own, with a way back; all the clean copies save as one ZIP.
import type { VerdictLine } from "./verdict";
import { listed, type Headline } from "./headline";

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

export interface BatchHooks {
  open(index: number): void;
  /** Makes the copies of the files chosen; `keepNames` names each as the original. */
  saveClean(keepNames: boolean): void;
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
  if (has("damage")) out.push(el("span", "tag is-damage", "Damaged"));
  if (has("misnamed")) out.push(el("span", "tag is-oddity", "Wrong extension"));
  if (has("hidden")) out.push(el("span", "tag is-hidden", "Something hidden"));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  for (const r of item.reveals.slice(0, 4)) out.push(el("span", "tag is-reveals", cap(r)));
  if (item.reveals.length > 4) out.push(el("span", "tag is-reveals", `+${item.reveals.length - 4} more`));
  return out;
}

/** Which files the list shows. */
type Show = "all" | "reveals" | "problems";

/** Rows drawn at a time: a folder of thousands would make the page crawl. */
const PAGE = 300;

export class BatchView {
  private status = el("p", "batch-result");
  private show: Show = "all";
  private limit = PAGE;
  private items: BatchItem[] = [];
  private lastDrawn = 0;
  private pending = 0;
  private keepNames = false;

  constructor(
    private host: HTMLElement,
    private hooks: BatchHooks,
  ) {}

  /** Says how saving the clean copies went, under the button. */
  set result(text: string) {
    this.status.textContent = text;
  }

  /** Says what is ready, with what to do with it. */
  offer(text: string, ...actions: HTMLButtonElement[]): void {
    this.status.replaceChildren(text, ...actions.map((a) => (a.classList.add("batch-action"), a)));
  }

  /** Draws the list, at most a few times a second while files are read. */
  render(items: BatchItem[]): void {
    this.items = items;
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
    const done = items.filter((i) => i.state === "done" || i.state === "failed");
    const busy = done.length < items.length;
    const revealing = items.filter((i) => i.reveals.length > 0).length;
    const damaged = items.filter((i) => i.state === "failed" || i.lines.some((l) => l.kind === "damage")).length;
    const hidden = items.filter((i) => i.lines.some((l) => l.kind === "hidden")).length;

    const card = el("div", "batch-card");
    const head = el("div", "batch-head");
    const label = el("p", "batch-label", plural(items.length, "file", "files"));
    const flagged = items.filter((i) => i.lines.some((l) => l.kind === "unknown" || l.kind === "damage") || i.state === "failed" || i.headline?.tone === "danger" || i.headline?.tone === "warning");
    const tone = busy ? "neutral" : flagged.some((i) => i.state === "failed" || i.headline?.tone === "danger") ? "danger" : flagged.length ? "warning" : "ok";
    const title = el(
      "h2",
      `batch-title is-${tone}`,
      busy
        ? `Reading ${done.length + 1} of ${items.length}…`
        : flagged.length
          ? `${flagged.length} of ${plural(items.length, "file needs", "files need")} a look`
          : `Nothing personal found in ${items.length === 1 ? "this file" : `these ${items.length} files`}`,
    );
    const summary = el("p", "batch-summary");
    if (busy) {
      summary.textContent = "Nothing leaves this tab.";
    } else {
      const parts = [
        revealing ? `${revealing} ${revealing === 1 ? "reveals" : "reveal"} something about you` : "",
        hidden ? `${hidden} ${hidden === 1 ? "hides" : "hide"} something` : "",
        damaged ? `${damaged} damaged` : "",
      ].filter(Boolean);
      summary.textContent = parts.length ? `${parts.join(" · ")}.` : "No personal metadata found by the available checks. Visible content has not been cleared.";
    }
    const phone = this.hooks.canShareCopies();
    const chosen = items.filter((i) => cleanable(i) && !i.skip).length;
    const copies = plural(chosen, "clean copy", "clean copies");
    const save = el("button", "btn btn-clean", busy || chosen === 0 ? (phone ? "Make clean copies" : "Save clean copies (.zip)") : phone ? `Make ${copies}` : `Save ${copies} (.zip)`);
    save.disabled = busy || chosen === 0;
    save.title = phone
      ? "Makes the copies in this tab, to share or save: nothing is uploaded"
      : "Makes the copies in this tab and saves them as one ZIP: nothing is uploaded";
    save.addEventListener("click", () => this.hooks.saveClean(this.keepNames));
    // "photo.jpg" rather than "photo-clean.jpg": in their own ZIP they need no mark.
    const keep = el("label", "batch-keep");
    const box = el("input");
    box.type = "checkbox";
    box.checked = this.keepNames;
    box.addEventListener("change", () => (this.keepNames = box.checked));
    keep.append(box, " Keep the original names");
    keep.hidden = busy || chosen === 0;
    head.append(label, title, summary, save, keep, this.status);
    if (!busy && items.length <= 1000 && this.hooks.mosaic) {
      const mosaic = el("button", "btn", "Photo privacy mosaic");
      mosaic.addEventListener("click", () => this.hooks.mosaic?.(items)); head.append(mosaic);
    }
    if (!busy && this.hooks.filmRoll) {
      const roll = el("button", "btn", "Film roll"); roll.addEventListener("click", () => this.hooks.filmRoll?.(items)); head.append(roll);
    }

    const isProblem = (i: BatchItem) =>
      i.state === "failed" || i.lines.some((l) => l.kind === "damage" || l.kind === "hidden" || l.kind === "misnamed");
    const filters: [Show, string, number][] = [
      ["all", "All", items.length],
      ["reveals", "Give something away", revealing],
      ["problems", "Damaged or hiding something", items.filter(isProblem).length],
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
    const shown = items
      .map((item, i) => ({ item, i }))
      .filter(({ item }) => this.show === "all" || (this.show === "reveals" ? item.reveals.length > 0 : isProblem(item)));

    const list = el("ul", "batch-list");
    // With any tick in the list, every row keeps its place for one.
    const anyPick = shown.some(({ item }) => cleanable(item));
    shown.slice(0, this.limit).forEach(({ item, i }) => {
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
      row.addEventListener("click", () => this.hooks.open(i));
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
      } else if (anyPick) {
        li.classList.add("has-pick");
        li.append(el("span", "batch-pick"));
      }
      li.append(row);
      list.append(li);
    });
    card.append(head, bar, list);
    if (shown.length > this.limit) {
      const more = el("button", "btn", `Show ${Math.min(PAGE, shown.length - this.limit)} more of ${shown.length - this.limit}`);
      more.addEventListener("click", () => {
        this.limit += PAGE;
        this.draw();
      });
      card.append(more);
    }
    this.host.replaceChildren(card);
  }
}
