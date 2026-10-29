// A list of archive rows where only the rows in view exist in the DOM: a
// folder of a hundred thousand files costs the page the same as one of ten.
import type { ArchiveRow } from "./archive-index";

/** Rows drawn beyond the visible window, so fast scrolling shows no gaps. */
export const OVERSCAN = 8;

/** Rows [from, to) to draw for this scroll position. */
export function windowOf(scrollTop: number, viewport: number, rowHeight: number, count: number, overscan = OVERSCAN): [number, number] {
  const from = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const to = Math.min(count, Math.ceil((scrollTop + viewport) / rowHeight) + overscan);
  return [Math.min(from, to), to];
}

export interface ArchiveListHooks {
  /** The listbox's accessible name. */
  label: string;
  /** Fills one row's element with what the row says. */
  draw(row: ArchiveRow, into: HTMLElement): void;
  /** A click on a row, or Enter on the active one. */
  choose(row: ArchiveRow): void;
  /** Backspace or the left arrow: the folder above. */
  up(): void;
}

let lists = 0;

export class ArchiveList {
  private readonly box: HTMLDivElement;
  private readonly rowsHost: HTMLDivElement;
  private readonly prefix = `archive-row-${++lists}-`;
  /** Fingers need more room than a pointer. */
  private readonly coarse = matchMedia("(pointer: coarse)");
  private rows: ArchiveRow[] = [];
  private activeRow = -1;
  private chosen: number | null = null;
  /** Kept from the last scroll: a hidden list reads 0. */
  private top = 0;
  private frame = 0;

  constructor(host: HTMLElement, private readonly hooks: ArchiveListHooks) {
    this.box = document.createElement("div");
    this.box.className = "archive-list";
    this.box.setAttribute("role", "listbox");
    this.box.setAttribute("aria-label", hooks.label);
    this.box.tabIndex = 0;
    this.rowsHost = document.createElement("div");
    this.rowsHost.className = "archive-list-rows";
    this.box.append(this.rowsHost);
    host.append(this.box);

    this.box.addEventListener("scroll", () => {
      this.top = this.box.scrollTop;
      this.schedule();
    }, { passive: true });
    new ResizeObserver(() => this.schedule()).observe(this.box);
    this.coarse.addEventListener("change", () => this.layout());
    this.box.addEventListener("click", (e) => {
      const el = (e.target as HTMLElement | null)?.closest<HTMLElement>("[data-row]");
      if (!el) return;
      this.activeRow = Number(el.dataset.row);
      this.hooks.choose(this.rows[this.activeRow]);
      this.draw();
    });
    this.box.addEventListener("keydown", (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (this.key(e.key)) {
        e.preventDefault();
        e.stopPropagation();
      }
    });
  }

  get scrollTop(): number {
    return this.top;
  }

  get active(): number {
    return this.activeRow;
  }

  /** The rows to list; `keep` puts the scroll and the active row back where they were. */
  setRows(rows: ArchiveRow[], keep?: { scrollTop: number; active: number }): void {
    this.rows = rows;
    this.activeRow = Math.min(keep?.active ?? (rows.length ? 0 : -1), rows.length - 1);
    this.layout();
    this.box.scrollTop = this.top = keep?.scrollTop ?? 0;
    this.draw();
  }

  focus(): void {
    this.box.focus();
  }

  /** The entry shown in the details, marked as the list's selection. */
  markChosen(index: number | null): void {
    this.chosen = index;
    this.draw();
  }

  private get rowHeight(): number {
    return this.coarse.matches ? 44 : 36;
  }

  private layout(): void {
    this.rowsHost.style.height = `${this.rows.length * this.rowHeight}px`;
    this.draw();
  }

  private schedule(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.draw();
    });
  }

  private draw(): void {
    const h = this.rowHeight;
    const [from, to] = windowOf(this.box.scrollTop, this.box.clientHeight, h, this.rows.length);
    const frag = document.createDocumentFragment();
    for (let i = from; i < to; i++) {
      const row = this.rows[i];
      const el = document.createElement("div");
      el.className = "archive-row";
      el.id = this.prefix + i;
      el.dataset.row = String(i);
      el.setAttribute("role", "option");
      el.setAttribute("aria-setsize", String(this.rows.length));
      el.setAttribute("aria-posinset", String(i + 1));
      el.setAttribute("aria-selected", String(row.kind === "entry" && row.index === this.chosen));
      el.style.transform = `translateY(${i * h}px)`;
      el.style.height = `${h}px`;
      if (i === this.activeRow) el.classList.add("is-active");
      this.hooks.draw(row, el);
      frag.append(el);
    }
    this.rowsHost.replaceChildren(frag);
    if (this.activeRow >= from && this.activeRow < to) this.box.setAttribute("aria-activedescendant", this.prefix + this.activeRow);
    else this.box.removeAttribute("aria-activedescendant");
  }

  /** Moves the active row and keeps it in view. */
  private moveTo(index: number): void {
    if (this.rows.length === 0) return;
    this.activeRow = Math.max(0, Math.min(this.rows.length - 1, index));
    const h = this.rowHeight;
    const top = this.activeRow * h;
    if (top < this.box.scrollTop) this.box.scrollTop = top;
    else if (top + h > this.box.scrollTop + this.box.clientHeight) this.box.scrollTop = top + h - this.box.clientHeight;
    this.top = this.box.scrollTop;
    this.draw();
  }

  /** Whether the key was one the list uses. */
  private key(key: string): boolean {
    const page = Math.max(1, Math.floor(this.box.clientHeight / this.rowHeight) - 1);
    const at = Math.max(0, this.activeRow);
    switch (key) {
      case "ArrowDown":
        this.moveTo(this.activeRow < 0 ? 0 : at + 1);
        return true;
      case "ArrowUp":
        this.moveTo(at - 1);
        return true;
      case "Home":
        this.moveTo(0);
        return true;
      case "End":
        this.moveTo(this.rows.length - 1);
        return true;
      case "PageDown":
        this.moveTo(at + page);
        return true;
      case "PageUp":
        this.moveTo(at - page);
        return true;
      case "Enter":
      case " ":
        if (this.rows[at]) this.hooks.choose(this.rows[at]);
        return true;
      case "ArrowRight":
        if (this.rows[at]?.kind === "folder") this.hooks.choose(this.rows[at]);
        return true;
      case "ArrowLeft":
      case "Backspace":
        this.hooks.up();
        return true;
    }
    return false;
  }
}
