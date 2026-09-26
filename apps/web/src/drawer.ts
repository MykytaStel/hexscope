import { advice } from "./advice";
import { checkThumbnail, drawn } from "./thumbnail";
import { openReport } from "./report";
import { categories, share } from "./share";
import { Concern, FileModel, Kind, Role, type ZipEntryInfo } from "./model";
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

/** Facts a clean copy keeps, because they are part of what the file does. */
const KEPT: Record<string, string[]> = { wasm: ["paths"], zip: ["comments", "tracked", "deleted"] };
const KEPT_NOTE: Record<string, string> = {
  wasm: "Kept: the paths in its data, which its error messages print. Only a new build, with the paths remapped, can take them out.",
  zip: "Kept: comments and tracked changes, which are part of the document's text. In Word, accept or reject every change and delete the comments (Review), then save.",
};

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
  subject: "Subject",
  keywords: "Keywords",
  producer: "PDF made by",
  encryption: "Encryption",
  updates: "Edited",
  history: "Editing history",
  shutter: "Shutter count",
  uptime: "Phone on for",
  linked: "Linked shots",
  names: "Function names",
  language: "Language",
  toolchain: "Built with",
  sdk: "SDK",
  sourcemap: "Source map",
  debuginfo: "Debug info at",
  debug: "Debug info",
  paths: "Built by user",
  comments: "Comments",
  tracked: "Tracked changes",
  deleted: "Deleted text",
  photoplace: "Photo inside",
  photo: "Photo inside",
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

/** Role names in plain words; "content" depends on what the file holds. */
function roleName(role: number, format: string): string {
  switch (role) {
    case Role.Content:
      return format === "zip"
        ? "Files"
        : format === "pdf"
          ? "Pages, fonts, images"
          : format === "wasm"
            ? "Code and data"
            : "Picture";
    case Role.Metadata:
      return "Metadata";
    case Role.Thumbnail:
      return "Thumbnail";
    case Role.Structure:
      return "Structure";
    case Role.Hidden:
      return "Hidden or unaccounted";
    default:
      return "Damaged";
  }
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

/** What cleaning produced, as the page needs it. */
export interface CleanResult {
  bytes: Uint8Array;
  removed: { what: string; bytes: number }[];
  orientation: number;
  error: string;
}

export interface CleanActions {
  /** Makes the copy and saves it; resolves with what was done. */
  clean(): Promise<CleanResult>;
  /** Opens the copy in hexscope, to check it. */
  open(bytes: Uint8Array): void;
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
    private readonly onHover: (id: number) => void,
    private readonly cleaning: CleanActions,
    /** The picture panel, for a file that has one. */
    private readonly picture: (m: FileModel) => HTMLElement | null,
  ) {
    this.node = el("section", "drawer-node");
    this.file = el("section", "drawer-file");
    host.append(this.node, this.file);
  }

  showFile(m: FileModel | null): void {
    this.file.replaceChildren();
    if (!m) return;
    const f = m.file;

    // What someone came to know first: the verdict, then what the file gives
    // away, then the picture; how it is made, after.
    this.file.append(this.verdict(m));
    // A photo, a video or a PDF always gets the card, if only to say it gives
    // nothing away; an archive only when it is a document with properties.
    if ((f.format !== "zip" && f.format !== "unknown") || f.facts.length > 0) {
      this.file.append(this.reveals(m));
    }
    const picture = this.picture(m);
    if (picture) this.file.append(picture);
    this.file.append(this.makeup(m));

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
    const report = el("button", "link report-link", "Report a problem with this file");
    report.title = "Shows the file's layout, without its content, to paste into a bug report";
    report.addEventListener("click", () => openReport(m));
    fileGroup.append(report);
    this.file.append(fileGroup);

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

  /** What the file is made of: a bar in file order and a legend by size. */
  private makeup(m: FileModel): HTMLElement {
    const group = el("div", "group makeup");
    group.append(el("h3", undefined, "What it's made of"));
    const slices = m.slices();
    const total = slices.reduce((n, s) => n + s.len, 0);
    if (total === 0) return group;

    const bar = el("div", "makeup-bar");
    for (const s of slices) {
      const part = el("span", `makeup-part role-${s.role}`);
      part.style.flexGrow = String(s.len);
      part.title = `${roleName(s.role, m.file.format)}${s.node >= 0 ? ` · ${m.label(s.node)}` : ""} · ${formatBytes(s.len)}`;
      if (s.node >= 0) {
        part.addEventListener("click", () => this.onSelect(s.node));
        part.addEventListener("mouseenter", () => this.onHover(s.node));
        part.addEventListener("mouseleave", () => this.onHover(-1));
      }
      bar.append(part);
    }

    // Per role: bytes, and the largest slice to jump to.
    const byRole = new Map<number, { bytes: number; biggest: (typeof slices)[number] }>();
    for (const s of slices) {
      const r = byRole.get(s.role);
      if (!r) byRole.set(s.role, { bytes: s.len, biggest: s });
      else {
        r.bytes += s.len;
        if (s.len > r.biggest.len) r.biggest = s;
      }
    }
    const legend = el("ul", "makeup-legend");
    for (const [role, r] of [...byRole].sort((a, b) => b[1].bytes - a[1].bytes)) {
      const pct = (r.bytes / total) * 100;
      const li = el("li", "makeup-row");
      li.append(
        el("span", `makeup-swatch role-${role}`),
        el("span", "makeup-name", roleName(role, m.file.format)),
        el("span", "makeup-pct", pct < 1 ? "<1%" : `${Math.round(pct)}%`),
        el("span", "makeup-size", formatBytes(r.bytes)),
      );
      if (r.biggest.node >= 0) {
        li.classList.add("is-link");
        li.title = "Show the largest part";
        li.addEventListener("click", () => this.onSelect(r.biggest.node));
      }
      legend.append(li);
    }
    group.append(bar, legend);
    return group;
  }

  /**
   * What the file's metadata gives away. Each fact is a link to the bytes
   * that hold it; the map link sends the coordinates nowhere unless clicked.
   */
  private reveals(m: FileModel): HTMLElement {
    const f = m.file;
    const group = el("div", "group reveals");
    const heading: Record<string, string> = {
      jpeg: "What this photo reveals",
      heif: "What this photo reveals",
      png: "What this image reveals",
      video: "What this video reveals",
      wasm: "What this module reveals",
    };
    group.append(el("h3", undefined, heading[f.format] ?? "What this document reveals"));
    if (!f.location && f.facts.length === 0) {
      // A PNG can hold notes that name no one, such as a comment or the
      // time it was changed: still worth a clean copy.
      const notes = ["tEXt", "zTXt", "iTXt", "eXIf", "tIME", "data after the end of the image"];
      const removable = f.format === "png" && m.children(0).some((id) => notes.includes(m.label(id)));
      const none =
        f.format === "pdf"
          ? "No document information or XMP: nothing about who wrote it, with what, or when."
          : removable
            ? "Nothing about the camera, the place or who made it, but it holds notes or data that can go."
            : f.format === "video"
              ? "No location, camera or software in its metadata."
              : f.format === "wasm"
                ? "No names, tools, paths or debug info: nothing about how, or by whom, it was built."
              : "No EXIF metadata: nothing about the camera, the time or the place.";
      group.append(el("p", "hint", none));
      if (removable) group.append(this.cleaner(f.format));
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
    let thumbRow: HTMLElement | null = null;
    for (const fact of f.facts) {
      const dd = row(FACT_LABELS[fact.kind] ?? fact.kind, fact.text, fact.node);
      if (fact.kind === "thumbnail") thumbRow = dd;
      // What the clean copy cannot take out stays unstruck.
      if (KEPT[f.format]?.includes(fact.kind)) {
        dd.dataset.kept = "";
        (dd.previousElementSibling as HTMLElement | null)?.setAttribute("data-kept", "");
      }
    }
    group.append(list);
    if (thumbRow) group.append(this.thumbnailCheck(m, thumbRow));
    // The one thing to do first, then when it matters and how to stop it
    // next time, then telling others.
    // An encrypted PDF is not rewritten: the copy would drop its protection.
    if (f.facts.some((x) => x.kind === "encryption")) {
      group.append(el("p", "hint", "It is encrypted, so hexscope does not make a clean copy of it."));
    } else {
      group.append(this.cleaner(f.format));
    }
    const tips = advice(m);
    if (tips.length > 0) {
      const box = el("div", "advice");
      box.append(el("p", "advice-title", "What you can do"));
      const ul = el("ul");
      for (const t of tips) ul.append(el("li", undefined, t));
      box.append(ul);
      group.append(box);
    }
    if (categories(m).length > 0) group.append(this.sharer(m));
    return group;
  }

  /**
   * Whether the photo's thumbnail is the photo: filled in once both are
   * decoded. A thumbnail that is not gets shown beside the picture, and a
   * line in the verdict; one that is says so quietly in its row.
   */
  private thumbnailCheck(m: FileModel, row: HTMLElement): HTMLElement {
    const box = el("div", "thumbcheck");
    box.hidden = true;
    void checkThumbnail(m).then((c) => {
      if (!c || !box.isConnected) return;
      const orientation = m.file.orientation;
      if (!c.differs) {
        row.append(el("span", "thumb-ok", " · matches the picture"));
      } else {
        const pair = el("div", "thumb-pair");
        for (const [b, caption] of [
          [c.thumbnail, "Thumbnail inside the file"],
          [c.picture, "The picture"],
        ] as const) {
          const fig = el("figure");
          fig.append(drawn(b, orientation, 160), el("figcaption", undefined, caption));
          pair.append(fig);
        }
        const what = c.differs === "shape" ? "cropped" : "edited";
        box.append(
          el("p", "thumb-title", "The thumbnail is not this picture"),
          pair,
          el(
            "p",
            "hint",
            `The photo was ${what} after the camera made its small copy, and the copy was left as it was: it still shows what was taken out, to anyone who opens it. The clean copy removes it.`,
          ),
        );
        box.hidden = false;
        this.addVerdict("hidden", `Something is hidden: the thumbnail inside the file shows the photo before it was ${what}.`, c.node);
      }
      c.thumbnail.close();
      c.picture.close();
    });
    return box;
  }

  /** A finding that arrives after the verdict is drawn, placed by how much it matters. */
  private addVerdict(kind: "hidden", text: string, node: number): void {
    const list = this.file.querySelector(".verdict-lines");
    if (!list) return;
    const li = el("li", `verdict-line is-${kind}`);
    li.append(el("span", "verdict-text", text));
    const show = el("button", "verdict-show", "Show me");
    show.addEventListener("click", () => this.onSelect(node));
    li.append(show);
    // As the verdict itself does: "looks healthy" only when nothing is hidden.
    list.querySelector(".is-healthy")?.remove();
    const after = [...list.children].find((x) => !x.matches(".is-damage, .is-hidden"));
    list.insertBefore(li, after ?? null);
  }

  /** Shares what kinds of thing the file gives away, and nothing of what they are. */
  private sharer(m: FileModel): HTMLElement {
    const box = el("div", "sharer");
    const button = el("button", "btn", "Share what it revealed");
    button.title = "A card and a sentence that name only the kinds of thing — no place, name or number";
    const status = el("span", "hint", "Only the kinds of thing, never what they are.");
    button.addEventListener("click", async () => {
      button.disabled = true;
      status.textContent = await share(m);
      button.disabled = false;
    });
    box.append(button, status);
    return box;
  }

  /** One button that saves a copy without all of the above, then says what went. */
  private cleaner(format: string): HTMLElement {
    const box = el("div", "cleaner");
    const button = el("button", "btn btn-clean", "Remove it — save a clean copy");
    button.title = "Makes the copy in this tab: nothing is uploaded";
    const notes: Record<string, string> = {
      zip: "Removes the document's properties. Comments and tracked changes inside the text keep their authors.",
      heif: "Blanks the camera data, location, serial numbers and XMP where they lie, so the file keeps its size. The picture and its thumbnail are copied unchanged.",
      png: "Removes the text notes, EXIF, XMP and the time it was last changed. The pixels are copied byte for byte.",
      video:
        "Blanks the location, the camera, the software and the dates where they lie, so the file keeps its size. The picture and sound are copied byte for byte.",
      wasm: "Leaves out the custom sections that say who built it and how: function names, tools, source map and debug info links, DWARF. The code and data are copied byte for byte; paths inside the data are part of the program, and stay.",
      pdf: "Writes the document anew with only what its pages use: no author, programs or dates, no XMP, and no earlier versions. The pages are copied byte for byte.",
    };
    const note =
      notes[format] ??
      "Removes the camera data, location, serial numbers, the maker's notes, thumbnail and comments. The picture itself is copied unchanged.";
    box.append(button, el("p", "hint", note));
    button.addEventListener("click", async () => {
      button.disabled = true;
      button.textContent = "Making the copy…";
      const r = await this.cleaning.clean();
      box.replaceChildren();
      if (r.error) {
        box.append(el("p", "problem is-warning", `No copy was made: ${r.error}.`));
        return;
      }
      box.append(el("p", "clean-done", "Saved a clean copy. Removed:"));
      // Each fact the copy no longer carries is struck out, one after another.
      const facts =
        box.closest(".reveals")?.querySelectorAll<HTMLElement>(".reveal-list dt:not([data-kept]), .reveal-list dd:not([data-kept])") ??
        [];
      facts.forEach((e, i) => {
        e.style.transitionDelay = `${Math.floor(i / 2) * 70}ms`;
        e.classList.add("is-removed");
      });
      const ul = el("ul", "clean-list");
      for (const item of r.removed) {
        const li = el("li");
        // A sentence starts with a capital; a path such as "word/media/…" stays as named.
        const path = /^[^\s:]+\//.test(item.what);
        const what = path ? item.what : item.what.charAt(0).toUpperCase() + item.what.slice(1);
        li.append(el("span", undefined, what), el("span", "clean-size", formatBytes(item.bytes)));
        ul.append(li);
      }
      box.append(ul);
      if (box.closest(".reveals")?.querySelector(".reveal-list [data-kept]")) {
        box.append(el("p", "hint", KEPT_NOTE[format] ?? "What is not struck out is part of the file's content, and stays."));
      }
      if (r.orientation > 1) {
        box.append(el("p", "hint", "Kept only the orientation, so the picture stays the right way up."));
      }
      const open = el("button", "btn", "Open the clean copy");
      open.title = "Check it yourself: the card should now be empty";
      open.addEventListener("click", () => this.cleaning.open(r.bytes));
      box.append(open);
    });
    return box;
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
        el(
          "p",
          "hint",
          matchMedia("(hover: none)").matches
            ? "Tap any byte or tree row to see what it is."
            : "Hover any byte or tree row to see what it is. Click to pin it here.",
        ),
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
