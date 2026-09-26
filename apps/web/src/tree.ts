import { FileModel, Kind } from "./model";

export interface TreeCallbacks {
  onHover(id: number): void;
  onSelect(id: number): void;
}

const ROW_H = 26;
const PAD_Y = 8;
/** Rows rendered beyond the visible window, so fast scrolling shows no gaps. */
const OVERSCAN = 12;

/**
 * Collapsible structure tree. Only the rows in view exist in the DOM: the
 * expanded tree is flattened into a list, and a window of it is drawn at the
 * scroll position. A file with tens of thousands of chunks costs the same to
 * scroll as one with ten.
 */
export class TreeView {
  private readonly list: HTMLDivElement;
  private model: FileModel | null = null;
  private readonly expanded = new Set<number>();
  /** The expanded tree in display order. */
  private visible: number[] = [];
  private rows = new Map<number, HTMLElement>();
  private hover = -1;
  private selected = -1;
  private frame = 0;

  constructor(
    private readonly host: HTMLElement,
    private readonly cb: TreeCallbacks,
  ) {
    this.list = document.createElement("div");
    this.list.className = "tree";
    this.list.setAttribute("role", "tree");
    this.list.setAttribute("aria-label", "Structure");
    // One stop for the keyboard; the arrows move within, as in a file manager.
    this.list.tabIndex = 0;
    this.list.addEventListener("keydown", (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (this.key(e.key)) {
        e.preventDefault();
        e.stopPropagation();
      }
    });
    host.append(this.list);

    host.addEventListener("scroll", () => this.schedule(), { passive: true });
    new ResizeObserver(() => this.schedule()).observe(host);
    this.list.addEventListener("mouseover", (e) => {
      const id = this.idFrom(e.target);
      if (id !== null) this.cb.onHover(id);
    });
    this.list.addEventListener("mouseleave", () => this.cb.onHover(-1));
    this.list.addEventListener("click", (e) => {
      const id = this.idFrom(e.target);
      if (id === null) return;
      if ((e.target as HTMLElement).closest(".twisty")) this.toggle(id);
      else this.cb.onSelect(id);
    });
  }

  setModel(model: FileModel | null): void {
    this.model = model;
    this.expanded.clear();
    this.hover = -1;
    this.selected = -1;
    if (model) {
      this.expanded.add(0);
      // Open whatever tells the story fastest: the header, and anything broken.
      for (const id of model.children(0)) {
        if (model.label(id) === "IHDR") this.expanded.add(id);
      }
      for (const id of model.problems) for (const a of model.path(id)) this.expanded.add(a);
    }
    this.host.scrollTop = 0;
    this.relayout();
  }

  setHover(id: number): void {
    if (id === this.hover) return;
    this.rows.get(this.hover)?.classList.remove("is-hover");
    this.hover = id;
    this.rows.get(id)?.classList.add("is-hover");
  }

  setSelected(id: number): void {
    const m = this.model;
    if (!m) return;
    const was = this.rows.get(this.selected);
    was?.classList.remove("is-selected");
    was?.setAttribute("aria-selected", "false");
    this.selected = id;
    if (id < 0) {
      this.list.removeAttribute("aria-activedescendant");
      return;
    }

    // Open the ancestors so the selection is actually in the list.
    let changed = false;
    for (const a of m.path(id).slice(0, -1)) {
      if (!this.expanded.has(a)) {
        this.expanded.add(a);
        changed = true;
      }
    }
    if (changed) this.relayout();
    this.scrollToRow(this.visible.indexOf(id));
    const row = this.rows.get(id);
    row?.classList.add("is-selected");
    row?.setAttribute("aria-selected", "true");
    if (row) this.list.setAttribute("aria-activedescendant", row.id);
  }

  /** Moves through the tree by key; whether the key was one it uses. */
  private key(key: string): boolean {
    const m = this.model;
    if (!m || this.visible.length === 0) return false;
    const at = Math.max(0, this.visible.indexOf(this.selected));
    const id = this.visible[at];
    const go = (to: number) => {
      if (to !== undefined && to >= 0) this.cb.onSelect(to);
    };
    // Nothing selected yet: the first arrow lands on the top.
    if (this.selected < 0 && key.startsWith("Arrow")) {
      go(this.visible[0]);
      return true;
    }
    switch (key) {
      case "ArrowDown":
        go(this.visible[Math.min(at + 1, this.visible.length - 1)]);
        return true;
      case "ArrowUp":
        go(this.visible[Math.max(at - 1, 0)]);
        return true;
      case "Home":
        go(this.visible[0]);
        return true;
      case "End":
        go(this.visible[this.visible.length - 1]);
        return true;
      case "ArrowRight":
        if (!m.hasChildren(id)) return true;
        if (!this.expanded.has(id)) this.toggle(id);
        else go(m.children(id)[0]);
        return true;
      case "ArrowLeft":
        if (this.expanded.has(id) && m.hasChildren(id)) this.toggle(id);
        else if (id !== 0) go(m.file.parents[id]);
        return true;
      case "Enter":
      case " ":
        if (m.hasChildren(id)) this.toggle(id);
        return true;
    }
    return false;
  }

  private toggle(id: number): void {
    if (this.expanded.has(id)) this.expanded.delete(id);
    else this.expanded.add(id);
    this.relayout();
  }

  private idFrom(target: EventTarget | null): number | null {
    const row = (target as HTMLElement | null)?.closest<HTMLElement>(".row");
    return row ? Number(row.dataset.id) : null;
  }

  private scrollToRow(index: number): void {
    if (index < 0) return;
    const top = PAD_Y + index * ROW_H;
    const view = this.host;
    if (top < view.scrollTop || top + ROW_H > view.scrollTop + view.clientHeight) {
      view.scrollTop = Math.max(0, top - view.clientHeight / 3);
    }
    this.draw();
  }

  /** Re-flattens the expanded tree after it changes shape. */
  private relayout(): void {
    const m = this.model;
    this.visible = [];
    if (m) {
      // Iterative walk: deep trees must not overflow the call stack.
      const stack = [0];
      while (stack.length > 0) {
        const id = stack.pop()!;
        this.visible.push(id);
        if (this.expanded.has(id)) {
          const kids = m.children(id);
          for (let i = kids.length - 1; i >= 0; i--) stack.push(kids[i]);
        }
      }
    }
    this.list.style.height = `${this.visible.length * ROW_H + PAD_Y * 2}px`;
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
    const m = this.model;
    const first = Math.max(0, Math.floor((this.host.scrollTop - PAD_Y) / ROW_H) - OVERSCAN);
    const last = Math.min(
      this.visible.length,
      Math.ceil((this.host.scrollTop + this.host.clientHeight) / ROW_H) + OVERSCAN,
    );

    this.rows = new Map();
    const frag = document.createDocumentFragment();
    if (m) {
      for (let i = first; i < last; i++) {
        const row = this.row(m, this.visible[i]);
        row.style.top = `${PAD_Y + i * ROW_H}px`;
        frag.append(row);
      }
    }
    this.list.replaceChildren(frag);
    if (this.rows.has(this.selected)) this.list.setAttribute("aria-activedescendant", `tree-row-${this.selected}`);
    else this.list.removeAttribute("aria-activedescendant");
  }

  private row(m: FileModel, id: number): HTMLElement {
    const row = document.createElement("div");
    row.className = "row";
    row.dataset.id = String(id);
    row.dataset.tint = m.tint(id);
    row.setAttribute("role", "treeitem");
    row.id = `tree-row-${id}`;
    row.setAttribute("aria-level", String(m.depth[id] + 1));
    row.setAttribute("aria-selected", String(id === this.selected));
    row.style.setProperty("--depth", String(m.depth[id]));

    const kind = m.kind(id);
    if (kind === Kind.Warning) row.classList.add("is-warning");
    if (kind === Kind.Error) row.classList.add("is-error");
    if (id === this.hover) row.classList.add("is-hover");
    if (id === this.selected) row.classList.add("is-selected");

    const twisty = document.createElement("span");
    twisty.className = "twisty";
    if (m.hasChildren(id)) {
      const open = this.expanded.has(id);
      twisty.textContent = open ? "▾" : "▸";
      row.setAttribute("aria-expanded", String(open));
    }

    const swatch = document.createElement("span");
    swatch.className = "swatch";
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = m.label(id);
    const value = document.createElement("span");
    value.className = "value";
    value.textContent = m.value(id);

    row.append(twisty, swatch, label, value);
    const doc = m.doc(id);
    row.title = `${m.label(id)}${m.value(id) ? ` = ${m.value(id)}` : ""}${doc ? `\n${doc.text}` : ""}`;
    this.rows.set(id, row);
    return row;
  }
}
