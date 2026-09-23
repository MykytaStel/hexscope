import { FileModel, Kind } from "./model";

export interface TreeCallbacks {
  onHover(id: number): void;
  onSelect(id: number): void;
}

/** Collapsible structure tree. Only expanded branches exist in the DOM. */
export class TreeView {
  private readonly list: HTMLDivElement;
  private model: FileModel | null = null;
  private readonly expanded = new Set<number>();
  private rows = new Map<number, HTMLElement>();
  private hover = -1;
  private selected = -1;

  constructor(
    host: HTMLElement,
    private readonly cb: TreeCallbacks,
  ) {
    this.list = document.createElement("div");
    this.list.className = "tree";
    this.list.setAttribute("role", "tree");
    host.append(this.list);

    this.list.addEventListener("mouseover", (e) => {
      const id = this.idFrom(e.target);
      if (id !== null) this.cb.onHover(id);
    });
    this.list.addEventListener("mouseleave", () => this.cb.onHover(-1));
    this.list.addEventListener("click", (e) => {
      const id = this.idFrom(e.target);
      if (id === null) return;
      if ((e.target as HTMLElement).closest(".twisty")) {
        this.toggle(id);
      } else {
        this.cb.onSelect(id);
      }
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
    this.render();
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
    this.rows.get(this.selected)?.classList.remove("is-selected");
    this.selected = id;
    if (id < 0) return;

    // Open the ancestors so the selection is actually on screen.
    let changed = false;
    for (const a of m.path(id).slice(0, -1)) {
      if (!this.expanded.has(a)) {
        this.expanded.add(a);
        changed = true;
      }
    }
    if (changed) this.render();
    const row = this.rows.get(id);
    row?.classList.add("is-selected");
    row?.scrollIntoView({ block: "nearest" });
  }

  private toggle(id: number): void {
    if (this.expanded.has(id)) this.expanded.delete(id);
    else this.expanded.add(id);
    this.render();
  }

  private idFrom(target: EventTarget | null): number | null {
    const row = (target as HTMLElement | null)?.closest<HTMLElement>(".row");
    return row ? Number(row.dataset.id) : null;
  }

  private render(): void {
    const m = this.model;
    this.rows = new Map();
    const frag = document.createDocumentFragment();
    if (m) this.renderNode(m, 0, frag);
    this.list.replaceChildren(frag);
    this.rows.get(this.hover)?.classList.add("is-hover");
    this.rows.get(this.selected)?.classList.add("is-selected");
  }

  private renderNode(m: FileModel, id: number, into: DocumentFragment): void {
    const row = document.createElement("div");
    row.className = "row";
    row.dataset.id = String(id);
    row.dataset.tint = m.tint(id);
    row.setAttribute("role", "treeitem");
    row.style.setProperty("--depth", String(m.depth[id]));

    const kind = m.kind(id);
    if (kind === Kind.Warning) row.classList.add("is-warning");
    if (kind === Kind.Error) row.classList.add("is-error");

    const open = this.expanded.has(id);
    const twisty = document.createElement("span");
    twisty.className = "twisty";
    if (m.hasChildren(id)) {
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
    row.title = `${m.label(id)}${m.value(id) ? ` = ${m.value(id)}` : ""}`;
    into.append(row);
    this.rows.set(id, row);

    if (open) for (const child of m.children(id)) this.renderNode(m, child, into);
  }
}
