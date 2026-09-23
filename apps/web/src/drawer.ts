import { FileModel, Kind } from "./model";

const KIND_NAMES = ["Container", "Field", "Warning", "Error"];
const COLOR_TYPES: Record<number, string> = {
  0: "Greyscale",
  2: "RGB",
  3: "Palette",
  4: "Greyscale + alpha",
  6: "RGBA",
};

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

const hex = (n: number) => `0x${n.toString(16).toUpperCase()}`;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function fact(grid: HTMLElement, key: string, value: string, mono = false): void {
  grid.append(el("dt", undefined, key), el("dd", mono ? "mono" : undefined, value));
}

const FACT_LABELS: Record<string, string> = {
  camera: "Camera",
  lens: "Lens",
  serial: "Serial number",
  owner: "Owner",
  taken: "Taken",
  software: "Software",
  thumbnail: "Thumbnail",
};

const degrees = (v: number, pos: string, neg: string) =>
  `${Math.abs(v).toFixed(5)}° ${v >= 0 ? pos : neg}`;

/** Details for one node on the left, facts about the whole file on the right. */
export class Drawer {
  private readonly node: HTMLElement;
  private readonly file: HTMLElement;

  constructor(
    host: HTMLElement,
    private readonly onSelect: (id: number) => void,
  ) {
    this.node = el("section", "drawer-node");
    this.file = el("section", "drawer-file");
    host.append(this.node, this.file);
  }

  showFile(m: FileModel | null): void {
    this.file.replaceChildren();
    if (!m) return;
    const f = m.file;

    const fileGroup = el("div", "group");
    fileGroup.append(el("h3", undefined, "File"));
    const grid = el("dl", "facts");
    fact(grid, "Size", `${formatBytes(m.bytes.length)} · ${m.bytes.length.toLocaleString()} bytes`);
    if (!f.ihdr && f.dimensions) fact(grid, "Image", `${f.dimensions[0]} × ${f.dimensions[1]}`);
    if (f.ihdr) {
      const [w, h, depth, color, interlace] = f.ihdr;
      fact(grid, "Image", `${w} × ${h}`);
      fact(grid, "Pixels", `${COLOR_TYPES[color] ?? `type ${color}`}, ${depth}-bit`);
      if (interlace) fact(grid, "Layout", "Interlaced (Adam7)");
    }
    fact(grid, "Parsed in", `${f.parseMs.toFixed(1)} ms`);
    fileGroup.append(grid);
    this.file.append(fileGroup);
    if (f.format === "jpeg") this.file.append(this.reveals(m));

    if (f.trace) {
      const [events, literals, matches, output] = f.trace;
      const deflate = el("div", "group");
      deflate.append(el("h3", undefined, "DEFLATE"));
      const d = el("dl", "facts");
      fact(d, "Compressed", `${formatBytes(f.idatBytes)} → ${formatBytes(output)}`);
      if (f.idatBytes > 0) fact(d, "Ratio", `${(output / f.idatBytes).toFixed(2)}×`);
      fact(d, "Steps", events.toLocaleString());
      deflate.append(d);
      this.file.append(deflate);

      // Literal vs back-reference split: the one number that says how much
      // LZ77 actually found to reuse.
      const coded = literals + matches;
      if (coded > 0) {
        const share = (matches / coded) * 100;
        const bar = el("div", "split");
        const lit = el("span", "split-lit");
        const mat = el("span", "split-match");
        lit.style.flexGrow = String(literals);
        mat.style.flexGrow = String(matches);
        bar.append(lit, mat);
        const legend = el(
          "p",
          "split-legend",
          `${literals.toLocaleString()} literals · ${matches.toLocaleString()} back-references (${share.toFixed(0)}%)`,
        );
        deflate.append(bar, legend);
      }
    }
  }

  /**
   * What the photo's metadata gives away. Each fact is a link to the bytes
   * that hold it; the map link sends the coordinates nowhere unless clicked.
   */
  private reveals(m: FileModel): HTMLElement {
    const f = m.file;
    const group = el("div", "group reveals");
    group.append(el("h3", undefined, "What this photo reveals"));
    if (!f.location && f.facts.length === 0) {
      group.append(el("p", "hint", "No EXIF metadata: nothing about the camera, the time or the place."));
      return group;
    }

    const list = el("dl", "reveal-list");
    const row = (label: string, text: string, node: number, strong = false) => {
      const dd = el("dd");
      const link = el("button", strong ? "reveal-link is-strong" : "reveal-link", text);
      link.title = "Show where in the file this is";
      link.addEventListener("click", () => this.onSelect(node));
      dd.append(link);
      list.append(el("dt", strong ? "is-strong" : undefined, label), dd);
      return dd;
    };

    if (f.location) {
      const { latitude, longitude, altitude, node } = f.location;
      const where = `${degrees(latitude, "N", "S")}, ${degrees(longitude, "E", "W")}${
        altitude !== null ? ` · ${Math.round(altitude)} m` : ""
      }`;
      const dd = row("Location", where, node, true);
      const map = el("a", "map-link", "Open in OpenStreetMap ↗");
      map.href = `https://www.openstreetmap.org/?mlat=${latitude.toFixed(6)}&mlon=${longitude.toFixed(6)}#map=17/${latitude.toFixed(6)}/${longitude.toFixed(6)}`;
      map.target = "_blank";
      map.rel = "noopener noreferrer";
      map.title = "Opens openstreetmap.org in a new tab. The coordinates leave this page only if you click.";
      dd.append(map);
    }
    for (const fact of f.facts) row(FACT_LABELS[fact.kind] ?? fact.kind, fact.text, fact.node);
    group.append(list);
    return group;
  }

  showNode(m: FileModel | null, id: number, pinned: boolean): void {
    this.node.replaceChildren();
    if (!m) return;
    if (id < 0) {
      this.node.append(
        el("p", "hint", "Hover any byte or tree row to see what it is. Click to pin it here."),
      );
      return;
    }

    const crumbs = el("nav", "crumbs");
    const path = m.path(id);
    path.forEach((p, i) => {
      if (i > 0) crumbs.append(el("span", "sep", "›"));
      crumbs.append(el("span", i === path.length - 1 ? "crumb is-current" : "crumb", m.label(p)));
    });
    if (pinned) crumbs.append(el("span", "pin", "pinned"));
    this.node.append(crumbs);

    const kind = m.kind(id);
    if (kind >= Kind.Warning) {
      const note = el("p", kind === Kind.Error ? "problem is-error" : "problem is-warning");
      note.textContent = kind === Kind.Error ? "Damage — this region could not be read." : "Suspicious — readable, but not what the format requires.";
      this.node.append(note);
    }

    const grid = el("dl", "facts");
    const start = m.start(id);
    const len = m.len(id);
    if (len > 0) {
      fact(grid, "Offset", `${hex(start)} · ${start.toLocaleString()}`, true);
      fact(grid, "Length", len === 1 ? "1 byte" : `${len.toLocaleString()} bytes`, true);
    } else {
      fact(grid, "Offset", "— no bytes to point at");
    }
    fact(grid, "Kind", KIND_NAMES[kind]);
    if (m.value(id)) fact(grid, "Value", m.value(id), true);
    this.node.append(grid);
  }
}
