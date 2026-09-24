import { Concern, FileModel, Kind, type ZipEntryInfo } from "./model";
import { verdict } from "./verdict";

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
  title: "Title",
  author: "Author",
  editor: "Last saved by",
  created: "Created",
  modified: "Modified",
  revisions: "Revisions",
  editing: "Editing time",
  company: "Company",
  application: "Application",
  template: "Template",
};

/** Why an entry cannot be played, or null when it can. */
function playReason(e: ZipEntryInfo): string | null {
  if (e.playable) return null;
  if (e.flags & 1) return "Encrypted: its bytes cannot be decompressed without the password.";
  if (e.method === 0) return "Stored: its bytes are the file itself, with nothing to decompress.";
  if (e.method !== 8) return "Compressed with a method this tool does not decompress.";
  if (e.compressed === 0) return "Empty: there is nothing to decompress.";
  return "Its data runs past the end of the file.";
}

/** How deep archives may nest on screen, matching the worker's limit. */
const MAX_NESTING = 4;

/** Why an entry cannot be opened as a file of its own, or null when it can. */
function openReason(e: ZipEntryInfo, nested: number): string | null {
  if (nested >= MAX_NESTING) return `Files nest at most ${MAX_NESTING} deep here.`;
  if (e.openable) return null;
  if (e.flags & 1) return "Encrypted: its bytes cannot be read without the password.";
  if (e.method !== 0 && e.method !== 8) return "Compressed with a method this tool does not decompress.";
  if (e.uncompressed === 0) return "Empty: there is nothing to open.";
  return "Its data runs past the end of the file.";
}

const degrees = (v: number, pos: string, neg: string) =>
  `${Math.abs(v).toFixed(5)}° ${v >= 0 ? pos : neg}`;

/** Details for one node on the left, facts about the whole file on the right. */
export class Drawer {
  private readonly node: HTMLElement;
  private readonly file: HTMLElement;
  /** How many archives the document on screen sits inside. */
  nested = 0;

  constructor(
    host: HTMLElement,
    private readonly onSelect: (id: number) => void,
    private readonly onPlay: (entry: number) => void,
    private readonly onOpen: (entry: number) => void,
  ) {
    this.node = el("section", "drawer-node");
    this.file = el("section", "drawer-file");
    host.append(this.node, this.file);
  }

  showFile(m: FileModel | null): void {
    this.file.replaceChildren();
    if (!m) return;
    const f = m.file;

    this.file.append(this.verdict(m));

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
    // A photo always gets the card, if only to say it gives nothing away; an
    // archive only when it is a document with properties to show.
    if (f.format === "jpeg" || f.facts.length > 0) this.file.append(this.reveals(m));

    // For a ZIP, the stream is whichever entry was last played: not the file's.
    if (f.trace && f.format === "png") {
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

  /** The answer to the question people arrive with: is it all right, and what does it say? */
  private verdict(m: FileModel): HTMLElement {
    const group = el("div", "group verdict");
    group.append(el("h3", undefined, "What hexscope found"));
    const list = el("ul", "verdict-lines");
    for (const line of verdict(m)) {
      const li = el("li", `verdict-line is-${line.kind}`);
      li.append(el("span", "verdict-text", line.text));
      if (line.node >= 0) {
        const show = el("button", "verdict-show", "Show me");
        show.addEventListener("click", () => this.onSelect(line.node));
        li.append(show);
      }
      list.append(li);
    }
    group.append(list, el("p", "hint", "Found by reading the file's structure. It is not a virus scan."));
    return group;
  }

  /**
   * What the file's metadata gives away. Each fact is a link to the bytes
   * that hold it; the map link sends the coordinates nowhere unless clicked.
   */
  private reveals(m: FileModel): HTMLElement {
    const f = m.file;
    const group = el("div", "group reveals");
    group.append(el("h3", undefined, f.format === "zip" ? "What this document reveals" : "What this photo reveals"));
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

  /** A message in place of node details, such as why something did not open. */
  showNote(text: string): void {
    this.node.replaceChildren(el("p", "problem is-error", text));
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

    // First what it is, in plain words; the technical detail follows below.
    const kind = m.kind(id);
    const doc = m.doc(id);
    if (kind >= Kind.Warning) {
      const tone =
        doc?.concern === Concern.Hidden ? "is-hidden" : doc?.concern === Concern.Oddity ? "is-warning" : "is-error";
      this.node.append(el("p", `problem ${tone}`, doc?.text ?? "These bytes break a rule of the format."));
    } else if (doc) {
      this.node.append(el("p", "explain", doc.text));
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
    if (doc?.cite) {
      const dd = el("dd");
      if (doc.url) {
        const link = el("a", "spec-link", `${doc.cite} ↗`);
        link.href = doc.url;
        link.target = "_blank";
        link.rel = "noopener noreferrer";
        link.title = "The specification that defines this, in a new tab";
        dd.append(link);
      } else {
        dd.textContent = doc.cite;
      }
      grid.append(el("dt", undefined, "Spec"), dd);
    }
    this.node.append(grid);

    const entry = m.entryOf(id);
    if (entry >= 0) this.node.append(this.entry(m, entry));
  }

  /** The ZIP entry a node sits in: what it holds, and a way to watch it unpack. */
  private entry(m: FileModel, i: number): HTMLElement {
    const e = m.entry(i);
    const group = el("div", "group entry");
    group.append(el("h3", undefined, "Entry"));
    const grid = el("dl", "facts");
    fact(grid, "Name", m.label(e.node));
    fact(grid, "Data", m.value(e.node));
    if (e.method !== 0 && e.compressed > 0) {
      fact(grid, "Ratio", `${(e.uncompressed / e.compressed).toFixed(2)}×`);
    }
    group.append(grid);

    const actions = el("div", "entry-actions");
    const button = (text: string, reason: string | null, title: string, act: () => void) => {
      const b = el("button", "btn", text);
      b.disabled = reason !== null;
      b.title = reason ?? title;
      b.addEventListener("click", act);
      actions.append(b);
    };
    const playWhy = playReason(e);
    const openWhy = openReason(e, this.nested);
    button("Watch it decompress", playWhy, "Step through this entry's DEFLATE data (P)", () => this.onPlay(i));
    button("Open", openWhy, "Open this entry as a file of its own", () => this.onOpen(i));
    group.append(actions);
    for (const why of new Set([playWhy, openWhy])) if (why) group.append(el("p", "hint", why));
    return group;
  }
}
