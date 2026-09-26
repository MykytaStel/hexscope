// Many files at once: a list that says, for each, what hexscope found — the
// same verdict a single file opens on, in a few words — so a folder of
// photos or documents can be checked before it is sent. Each opens as a
// file of its own, with a way back; all the clean copies save as one ZIP.
import type { VerdictLine } from "./verdict";

export interface BatchItem {
  file: File;
  state: "waiting" | "reading" | "done" | "failed";
  /** Shown beside the name: "JPEG", "PDF 1.7". */
  kind: string;
  lines: VerdictLine[];
  /** What it gives away, as categories: "where it was taken". */
  reveals: string[];
  /** The clean copy's name, for the ZIP. */
  cleanName: string;
  note: string;
}

export interface BatchHooks {
  open(index: number): void;
  saveClean(): void;
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
  const has = (k: VerdictLine["kind"]) => item.lines.some((l) => l.kind === k);
  if (has("unknown")) out.push(el("span", "tag is-unknown", "Not a format hexscope reads"));
  if (has("damage")) out.push(el("span", "tag is-damage", "Damaged"));
  if (has("misnamed")) out.push(el("span", "tag is-oddity", "Wrong extension"));
  if (has("hidden")) out.push(el("span", "tag is-hidden", "Something hidden"));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  for (const r of item.reveals.slice(0, 4)) out.push(el("span", "tag is-reveals", cap(r)));
  if (item.reveals.length > 4) out.push(el("span", "tag is-reveals", `+${item.reveals.length - 4} more`));
  if (out.length === 0) out.push(el("span", "tag is-healthy", "Nothing found"));
  return out;
}

export class BatchView {
  private status = el("p", "batch-result");

  constructor(
    private host: HTMLElement,
    private hooks: BatchHooks,
  ) {}

  /** Says how saving the clean copies went, under the button. */
  set result(text: string) {
    this.status.textContent = text;
  }

  render(items: BatchItem[]): void {
    const done = items.filter((i) => i.state === "done" || i.state === "failed");
    const busy = done.length < items.length;
    const revealing = items.filter((i) => i.reveals.length > 0).length;
    const damaged = items.filter((i) => i.state === "failed" || i.lines.some((l) => l.kind === "damage")).length;
    const hidden = items.filter((i) => i.lines.some((l) => l.kind === "hidden")).length;

    const card = el("div", "batch-card");
    const head = el("div", "batch-head");
    const title = el("h2", undefined, plural(items.length, "file", "files"));
    const summary = el("p", "batch-summary");
    if (busy) {
      summary.textContent = `Reading ${done.length + 1} of ${items.length}… Nothing leaves this tab.`;
    } else {
      const parts = [
        revealing ? `${revealing} ${revealing === 1 ? "reveals" : "reveal"} something about you` : "",
        hidden ? `${hidden} ${hidden === 1 ? "hides" : "hide"} something` : "",
        damaged ? `${damaged} damaged` : "",
      ].filter(Boolean);
      summary.textContent = parts.length ? `${parts.join(" · ")}.` : "None of them gives anything away.";
    }
    const save = el("button", "btn btn-clean", "Save clean copies (.zip)");
    save.disabled = busy || revealing + hidden === 0;
    save.title = "Makes every copy in this tab and saves them as one ZIP: nothing is uploaded";
    save.addEventListener("click", () => this.hooks.saveClean());
    head.append(title, summary, save, this.status);

    const list = el("ul", "batch-list");
    items.forEach((item, i) => {
      const li = el("li");
      const row = el("button", "batch-row");
      row.disabled = item.state === "waiting" || item.state === "reading";
      row.title = row.disabled ? "Still reading" : "Open this file";
      const name = el("span", "batch-name", item.file.name);
      const meta = el("span", "batch-meta", [item.kind, size(item.file.size)].filter(Boolean).join(" · "));
      const found = el("span", "batch-tags");
      found.append(...tags(item));
      row.append(name, meta, found);
      row.addEventListener("click", () => this.hooks.open(i));
      li.append(row);
      list.append(li);
    });
    card.append(head, list);
    this.host.replaceChildren(card);
  }
}
