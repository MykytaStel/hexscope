import { areasOf, find, findPreset, pageFigure, PRESETS, searchable, textIn, forgetPictures, toShown, type Match, type Searchable, type Shown } from "./redactor";
import { canBlackOut, openBlackPicture } from "./blackpicture";
import { findEmbedded, type Embedded } from "./embedded";
import type { PageGlyphs, PagePicture } from "./worker";
import { blackoutFigure } from "./blackout";
import { advice } from "./advice";
import { checkThumbnail, decodePicture, drawn } from "./thumbnail";
import { openReport } from "./report";
import { saveStructure } from "./export";
import { categories, share } from "./share";
import { Concern, FileModel, Kind, Role, type PageArea, type ZipEntryInfo } from "./model";
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
const KEPT: Record<string, string[]> = { wasm: ["paths", "imports"], zip: ["comments", "hiddensheets", "hiddencells", "links", "notes", "hiddenslides"], pdf: ["comments", "form", "attachments"] };
const KEPT_NOTE: Record<string, string> = {
  wasm: "Kept: what it imports, which is the program itself, and any paths in its data, which its error messages print. Only a new build, with the paths remapped, can take those out.",
  pdf: "Kept: comments, form answers and attached files, which are part of the document. Delete them in a PDF editor — or flatten the form by printing to PDF — if they should not travel with it.",
  zip: "Kept: what is part of a workbook or a deck itself — its comments, hidden sheets, rows and slides, speaker notes, links to other files. Remove them in Excel or PowerPoint, then save.",
};

/** Facts shown first in colour: what someone would least want to send. */
const STRONG = ["covered", "hiddentext", "deleted", "earlier", "photoplace", "replyto", "authfail"];

/** A link that opens a place on OpenStreetMap, only when clicked. */
function mapLink(latitude: number, longitude: number): HTMLAnchorElement {
  const map = el("a", "map-link", "Open in OpenStreetMap ↗");
  const [lat, lon] = [latitude.toFixed(6), longitude.toFixed(6)];
  map.href = `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=17/${lat}/${lon}`;
  map.target = "_blank";
  map.rel = "noopener noreferrer";
  map.title = "Opens openstreetmap.org in a new tab. The coordinates leave this page only if you click.";
  return map;
}

/** What a clean copy of each kind of file cannot take out, because it is what the file shows or says. */
const LIMITS: Record<string, string[]> = {
  photo: [
    "What the picture shows: faces, street signs, house numbers, a screen or a letter in view, reflections. Black those out with “Black out part of the picture”.",
    "Marks some apps and AI tools hide in the pixels themselves.",
  ],
  video: ["What the video shows and what its sound says: voices, place names, anything in view."],
  audio: ["What the recording says: voices, names, places, anything heard in the background."],
  pdf: [
    "Anything still visible on the pages: a name or number you did not black out. “Black out text yourself” takes it out.",
    "Text that is part of a picture, such as a scanned page or a screenshot, under a black box: the copy cannot take letters out of a picture. Check those pages by eye, or black them out before scanning.",
    "Words and numbers in the text itself that say who wrote it or for whom.",
  ],
  zip: [
    "What the text says: names, addresses and details written in the document itself.",
    "What pasted pictures show: a screenshot can show a desktop, a name, an open tab.",
  ],
};

/** Media types by extension, for sharing a copy: a share sheet goes by type. */
const TYPES: Record<string, string> = {
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
  heic: "image/heic",
  heif: "image/heif",
  avif: "image/avif",
  mp4: "video/mp4",
  m4v: "video/mp4",
  mov: "video/quicktime",
  m4a: "audio/mp4",
  pdf: "application/pdf",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  zip: "application/zip",
  wasm: "application/wasm",
};

/**
 * A button that hands the copy to the device's share sheet — Messages,
 * Telegram, mail — or null where the browser cannot share such a file.
 * Its own button, because sharing must follow a tap of its own.
 */
function shareButton(name: string, bytes: Uint8Array): HTMLButtonElement | null {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  const file = new File([bytes as BlobPart], name, { type: TYPES[ext] ?? "" });
  if (!navigator.canShare?.({ files: [file] })) return null;
  const b = el("button", "btn btn-primary", "Share the clean copy");
  b.title = "Send the copy on: to a chat, to mail, to another app";
  b.addEventListener("click", async () => {
    try {
      await navigator.share({ files: [file] });
    } catch {
      // Cancelled, or refused: the copy is saved all the same.
    }
  });
  return b;
}

/** A movie file with sound and no picture: a voice memo, a song. */
function isAudio(m: FileModel): boolean {
  return m.file.format === "video" && / audio\b/.test(m.value(0));
}

/** A name with a date in it, the way phones and messengers name files: `IMG_20260614_183207`, `Screenshot 2026-06-14 at 18.32`. */
const DATED = /(?:19|20)\d{2}[-_.]?(?:0[1-9]|1[0-2])[-_.]?(?:0[1-9]|[12]\d|3[01])/;

/** What no clean copy can remove from this file, folded away until asked for; empty for what has none worth saying. */
function cleanLimits(m: FileModel): HTMLElement {
  const format = m.file.format;
  const kind = ["jpeg", "heif", "png", "webp", "gif"].includes(format) ? "photo" : isAudio(m) ? "audio" : format;
  const items = [...(LIMITS[kind] ?? [])];
  const base = m.name.split("/").pop() ?? m.name;
  if (DATED.test(base)) {
    items.push(`Its name, “${base}”, says when it was made; the copy keeps the name, with “-clean” added. Rename it if that matters.`);
  }
  const box = el("details", "clean-limits");
  if (items.length === 0) {
    box.hidden = true;
    return box;
  }
  box.append(el("summary", undefined, "What no clean copy can remove"));
  const ul = el("ul");
  ul.append(...items.map((t) => el("li", undefined, t)));
  box.append(ul);
  return box;
}

const FACT_LABELS: Record<string, string> = {
  camera: "Camera",
  lens: "Lens",
  serial: "Serial number",
  owner: "Owner",
  taken: "Taken",
  software: "Software",
  thumbnail: "Thumbnail",
  place: "Place named",
  caption: "Caption",
  original: "Made from",
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
  screenshot: "Screenshot",
  sentfrom: "Sent from",
  computer: "Computer's name",
  mailer: "Mail app",
  timezone: "Time zone",
  replyto: "Replies go to",
  returnpath: "Bounces",
  auth: "Sender checks",
  authfail: "Sender checks",
  language: "Language",
  toolchain: "Built with",
  sdk: "SDK",
  sourcemap: "Source map",
  debuginfo: "Debug info at",
  debug: "Debug info",
  paths: "Built by user",
  imports: "Asks its host to",
  covered: "Hidden, not removed",
  hiddentext: "Hidden text",
  earlier: "Taken off the page",
  comments: "Comments",
  form: "Form answers",
  attachments: "Attached files",
  hiddensheets: "Hidden sheets",
  hiddencells: "Hidden rows and columns",
  links: "Links to files",
  notes: "Speaker notes",
  hiddenslides: "Hidden slides",
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
          : format === "eml"
            ? "Message and attachments"
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
  /** What the copy is called when saved. */
  name: string;
  /** Whether it was saved already; if not, it waits for Share or Save. */
  saved: boolean;
  removed: { what: string; bytes: number }[];
  orientation: number;
  error: string;
}

export interface RepairResult {
  bytes: Uint8Array;
  fixed: string[];
  error: string;
}

export interface CleanActions {
  /** Makes the copy and saves it; resolves with what was done. `notes` also empties a workbook's or a deck's comments and notes. */
  clean(notes?: boolean): Promise<CleanResult>;
  /** Opens the copy in hexscope, to check it. */
  open(bytes: Uint8Array): void;
  /** Opens a file found inside this one. */
  openInside(bytes: Uint8Array, name: string): void;
  /** Every page's visible glyphs, for searching. */
  pages(): Promise<PageGlyphs[]>;
  /** The pictures a page draws, counted from 1: a scanned page, to see where to draw. */
  pictures(page: number): Promise<PagePicture[]>;
  /** A clean copy with these areas blacked out (see `areasOf`), saved or waiting to be. */
  redact(areas: Float64Array): Promise<CleanResult>;
  /** Saves bytes as a download. */
  save(name: string, bytes: Uint8Array): void;
  /** Makes a repaired copy and saves it; resolves with what was done. */
  repair(): Promise<RepairResult>;
  /** Opens the repaired copy in hexscope. */
  openRepaired(bytes: Uint8Array): void;
  /** Compares the file on screen with another. */
  compare(other: File): void;
}

/** Formats a damaged file of which hexscope can save what survived. */
const REPAIRABLE = ["png", "jpeg", "zip"];

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

  // Each page's pictures, fetched once per file, when a page is first drawn.
  private pictured: FileModel | null = null;
  private pictureCache = new Map<number, Promise<Shown[]>>();

  private picturesOf(m: FileModel, page: number): Promise<Shown[]> {
    if (m !== this.pictured) {
      forgetPictures();
      this.pictured = m;
      this.pictureCache = new Map();
    }
    let p = this.pictureCache.get(page);
    if (!p) {
      p = this.cleaning.pictures(page).then(toShown).catch(() => []);
      this.pictureCache.set(page, p);
    }
    return p;
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
    // A PDF: black out what you choose, not only what the file already hides.
    if (f.format === "pdf" && !f.facts.some((x) => x.kind === "encryption")) this.file.append(this.redactor(m));
    // On a phone, how the file is made waits under one line, so what to do
    // about it is not scrolled past.
    const more = matchMedia("(max-width: 900px)").matches ? el("details", "more-details") : null;
    const later = (e: HTMLElement) => (more ?? this.file).append(e);
    const picture = this.picture(m);
    if (picture) later(picture);
    later(this.makeup(m));

    const fileGroup = el("div", "group");
    fileGroup.append(el("h2", undefined, "File"));
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
    const save = el("button", "link report-link", "Save the structure as JSON");
    save.title = "Every part, its offset, length, value and explanation, nested as in the tree";
    save.addEventListener("click", () => saveStructure(m));
    const inside = el("button", "link report-link", "Find files inside");
    inside.title = "Looks through every byte for the start of another file: pictures, archives, documents";
    inside.addEventListener("click", () => {
      const found = findEmbedded(m.bytes, 0, m.bytes.length, m.file.format === "zip" ? ["zip"] : []);
      const list = found.length > 0 ? this.insideList(m, found, "Files inside") : el("p", "hint", "No other file starts anywhere in it.");
      inside.replaceWith(list);
    });
    fileGroup.append(report, save, inside);
    // Another file beside this one: what one gives away that the other does not.
    const pick = el("button", "link compare-pick", "Compare with another file…");
    const input = el("input");
    input.type = "file";
    input.hidden = true;
    input.addEventListener("change", () => {
      const f = input.files?.[0];
      input.value = "";
      if (f) this.cleaning.compare(f);
    });
    pick.addEventListener("click", () => input.click());
    fileGroup.append(input);
    fileGroup.append(pick);
    later(fileGroup);
    if (more) {
      more.prepend(el("summary", undefined, "More about the file: what it is made of, its picture, its structure"));
      this.file.append(more);
    }

    // For a ZIP, the stream is whichever entry was last played: not the file's.
    if (f.trace && f.format === "png") {
      const [events, literals, matches, output] = f.trace;
      const deflate = el("div", "group");
      deflate.append(el("h2", undefined, "DEFLATE"));
      const d = el("dl", "facts");
      fact(d, "Compressed", `${formatBytes(f.idatBytes)} → ${formatBytes(output)}`);
      if (f.idatBytes > 0) fact(d, "Ratio", `${(output / f.idatBytes).toFixed(2)}×`);
      fact(d, "Steps", events.toLocaleString());
      deflate.append(d);
      later(deflate);

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
    group.append(el("h2", undefined, "What hexscope found"));
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
    group.append(list);
    // What the structure leaves over — data after the end, a gap — may be a
    // whole file of its own: say which, and open it.
    const hidden = m
      .slices()
      .filter((sl) => sl.role === Role.Hidden)
      .flatMap((sl) => findEmbedded(m.bytes, sl.start, sl.start + sl.len));
    if (hidden.length > 0) group.append(this.insideList(m, hidden, "Hidden in it"));
    group.append(el("p", "hint", "Found by reading the file's structure. It is not a virus scan."));
    if (REPAIRABLE.includes(m.file.format) && verdict(m).some((l) => l.kind === "damage")) group.append(this.repairer());
    return group;
  }

  /** Files found inside this one, each to open or save. */
  private insideList(m: FileModel, found: Embedded[], title: string): HTMLElement {
    const box = el("div", "inside");
    box.append(el("p", "inside-title", `${title}: ${found.length === 1 ? "1 file" : `${found.length} files`}`));
    const ul = el("ul", "inside-list");
    const stem = (m.name.split("/").pop() ?? m.name).replace(/\.[^.]*$/, "");
    for (const e of found) {
      const li = el("li");
      const name = `${stem}-at-${e.start.toString(16).toUpperCase()}.${e.ext}`;
      const bytes = () => m.bytes.subarray(e.start, e.end);
      const at = el("button", "link inside-at", `0x${e.start.toString(16).toUpperCase()}`);
      at.title = "Show where it starts";
      at.addEventListener("click", () => this.onSelect(m.nodeAt(e.start)));
      const open = el("button", "btn", "Open");
      open.addEventListener("click", () => this.cleaning.openInside(bytes(), name));
      const save = el("button", "btn", "Save");
      save.addEventListener("click", () => this.cleaning.save(name, bytes().slice()));
      li.append(el("span", "inside-what", `${e.what.charAt(0).toUpperCase()}${e.what.slice(1)}`), " at ", at, el("span", "clean-size", formatBytes(e.end - e.start)), open, save);
      ul.append(li);
    }
    box.append(ul);
    return box;
  }

  /** One button that saves a repaired copy, then says what it did. */
  private repairer(): HTMLElement {
    const box = el("div", "repairer");
    const button = el("button", "btn btn-repair", "Try to repair — save a fixed copy");
    button.title = "Makes the copy in this tab: nothing is uploaded";
    box.append(
      button,
      el("p", "hint", "Keeps what survived and puts it back in order: checksums made right, a file cut short given its end, an archive rebuilt from its whole files. Nothing is guessed."),
    );
    button.addEventListener("click", async () => {
      button.disabled = true;
      button.textContent = "Repairing…";
      const r = await this.cleaning.repair();
      button.remove();
      if (r.error) {
        box.append(el("p", "hint", `No repaired copy: ${r.error}.`));
        return;
      }
      box.append(el("p", "clean-done", "Saved a repaired copy. What was done:"));
      const ul = el("ul", "clean-list");
      for (const f of r.fixed) ul.append(el("li", undefined, f.charAt(0).toUpperCase() + f.slice(1)));
      const open = el("button", "btn", "Open the repaired copy");
      open.title = "Check it yourself: the damage should be gone";
      open.addEventListener("click", () => this.cleaning.openRepaired(r.bytes));
      box.append(ul, open);
    });
    return box;
  }

  /** What the file is made of: a bar in file order and a legend by size. */
  private makeup(m: FileModel): HTMLElement {
    const group = el("div", "group makeup");
    group.append(el("h2", undefined, "What it's made of"));
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
      webp: "What this image reveals",
      gif: "What this image reveals",
      png: "What this image reveals",
      video: "What this video reveals",
      wasm: "What this module reveals",
      eml: "What this email reveals",
    };
    const title = el("h2", undefined, isAudio(m) ? "What this recording reveals" : (heading[f.format] ?? "What this document reveals"));
    group.append(title);
    // The picture it is about, small, beside the heading: which photo this is.
    if (["jpeg", "png", "heif", "webp", "gif"].includes(f.format)) {
      void decodePicture(m).then((b) => {
        if (!b) return;
        const c = drawn(b, f.format === "jpeg" ? f.orientation : 1, 128);
        c.className = "reveal-picture";
        c.setAttribute("role", "img");
        c.setAttribute("aria-label", "The picture");
        const head = el("div", "reveal-head");
        title.replaceWith(head);
        head.append(title, c);
      });
    }
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
              ? isAudio(m)
                ? "No location, device or software in its metadata."
                : "No location, camera or software in its metadata."
              : f.format === "wasm"
                ? "No names, tools, paths or debug info: nothing about how, or by whom, it was built."
              : "No EXIF metadata: nothing about the camera, the time or the place.";
      group.append(el("p", "hint", none));
      if (removable) group.append(this.cleaner(m));
      if (canBlackOut(m)) group.append(this.blackOutButton(m));
      // A screenshot with no metadata still shows what was on the screen.
      const tips = this.tips(m);
      if (tips) group.append(tips);
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
        altitude !== null ? ` · ${Math.round(altitude)}\u00a0m` : ""
      }`;
      const dd = row("Location", where, node, true);
      dd.append(mapLink(latitude, longitude));
    }
    let thumbRow: HTMLElement | null = null;
    for (const fact of f.facts) {
      const covered = fact.kind === "covered";
      const dd = row(FACT_LABELS[fact.kind] ?? fact.kind, fact.text, fact.node, STRONG.includes(fact.kind));
      // A sentence reads better in the body font; only coordinates and codes are set in mono.
      if (!covered) dd.querySelector(".reveal-link")?.classList.remove("is-strong");
      // Hidden text is shown as if selected: that is how someone finds it.
      const ghost = fact.kind === "hiddentext" ? /^(.*?: )“(.*)”$/.exec(fact.text) : null;
      if (ghost) dd.querySelector(".reveal-link")?.replaceChildren(ghost[1], el("mark", "ghost-text", ghost[2]));
      // Deleted text looks the way Word marks it: struck through.
      const deleted = fact.kind === "deleted" || fact.kind === "earlier" ? /^“(.*)”$/.exec(fact.text) : null;
      if (deleted) {
        const parts = deleted[1].split(" … ").flatMap((t, i) => [...(i ? [" … "] : []), el("del", "deleted-text", t)]);
        dd.querySelector(".reveal-link")?.replaceChildren(...parts);
      }
      // Each attached file opens as a file of its own, with the way back.
      if (fact.kind === "attachments" && f.attachments.length > 0) {
        const open = el("span", "attachment-open");
        f.attachments.forEach((name, i) => {
          const b = el("button", "link", `Open ${name}`);
          b.title = "Opens the attached file here, with the way back to this one";
          b.addEventListener("click", () => this.onOpen(i));
          open.append(b);
        });
        dd.append(open);
      }
      // A photo inside a document that says where: the same map link as a photo's own.
      const place = fact.kind === "photoplace" ? /taken at ([\d.]+)° ([NS]), ([\d.]+)° ([EW])/.exec(fact.text) : null;
      if (place) {
        const lat = Number(place[1]) * (place[2] === "S" ? -1 : 1);
        const lon = Number(place[3]) * (place[4] === "W" ? -1 : 1);
        dd.append(mapLink(lat, lon));
      }
      if (fact.kind === "thumbnail") thumbRow = dd;
      // Text found under a black box is shown the way it was meant to look:
      // blacked out, with what is still there showing through.
      const quoted = covered ? /^(.*?: )“(.*)”$/.exec(fact.text) : null;
      if (quoted) {
        const link = dd.querySelector(".reveal-link");
        const bars = quoted[2].split(" · ").map((t) => el("span", "redacted", t));
        link?.replaceChildren(quoted[1], ...bars);
      }
      // What the clean copy cannot take out stays unstruck.
      // A Word document's comments go with the clean copy; a workbook's or a deck's stay.
      const word = f.format === "zip" && f.labels.includes("word/document.xml");
      if (KEPT[f.format]?.includes(fact.kind) && !(word && fact.kind === "comments")) {
        dd.dataset.kept = "";
        dd.dataset.kind = fact.kind;
        (dd.previousElementSibling as HTMLElement | null)?.setAttribute("data-kept", "");
      }
    }
    group.append(list);
    // The first pages with black boxes over text or pictures, drawn; with
    // their pictures when a box lies on one.
    const overPictures = f.labels.includes("a picture under a black box");
    for (const b of f.blackouts.slice(0, 2)) group.append(blackoutFigure(b, overPictures ? this.picturesOf(m, b.page) : undefined));
    if (thumbRow) group.append(this.thumbnailCheck(m, thumbRow));
    // The one thing to do first, then when it matters and how to stop it
    // next time, then telling others.
    // An encrypted PDF is not rewritten: the copy would drop its protection.
    if (f.facts.some((x) => x.kind === "encryption")) {
      group.append(el("p", "hint", "It is encrypted, so hexscope does not make a clean copy of it."));
    } else if (f.format === "eml") {
      // A message is read, not sent on as it is: what it says is what the servers wrote.
      group.append(el("p", "hint", "The servers it passed through wrote these lines; an email is not cleaned, but its attached files can be — open one, then save a clean copy of it."));
    } else {
      group.append(this.cleaner(m));
    }
    if (canBlackOut(m)) group.append(this.blackOutButton(m));
    const tips = this.tips(m);
    if (tips) group.append(tips);
    if (categories(m).length > 0) group.append(this.sharer(m));
    return group;
  }

  /** What to do about what was found, or null when there is nothing to say. */
  private tips(m: FileModel): HTMLElement | null {
    const tips = advice(m);
    if (tips.length === 0) return null;
    const box = el("div", "advice");
    box.append(el("p", "advice-title", "What you can do"));
    const ul = el("ul");
    for (const t of tips) ul.append(el("li", undefined, t));
    box.append(ul);
    return box;
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

  /** Opens the picture to black out what it shows: faces, plates, an address. */
  private blackOutButton(m: FileModel): HTMLElement {
    const b = el("button", "link blackout-open", "Black out part of the picture…");
    b.title = "Draw boxes over what the picture itself shows, and save a new picture with them in it";
    b.addEventListener("click", () =>
      void openBlackPicture(m, {
        deliver: (name, bytes, into) => {
          const phone = matchMedia("(hover: none) and (pointer: coarse)").matches;
          const sharer = shareButton(name, bytes);
          if (sharer) into.append(sharer);
          if (phone && sharer) {
            const save = el("button", "btn", "Save it");
            save.addEventListener("click", () => this.cleaning.save(name, bytes));
            into.append(save);
          } else {
            this.cleaning.save(name, bytes);
          }
          const open = el("button", "btn", "Open the copy");
          open.addEventListener("click", () => {
            into.closest("dialog")?.close();
            this.cleaning.open(bytes);
          });
          into.append(open);
        },
      }),
    );
    return b;
  }

  /** Black out text yourself: search or pick a kind, tick, draw boxes, see the pages, save. */
  private redactor(m: FileModel): HTMLElement {
    const group = el("div", "group redactor");
    group.append(el("h2", undefined, "Black out text yourself"));
    group.append(
      el(
        "p",
        "hint",
        "Type a name, a number or an address, or pick a kind below: every place it appears is found. The copy takes it out of the pages — not only covers it — and draws a black box where it was.",
      ),
    );
    const form = el("form", "redact-form");
    const input = el("input");
    input.type = "search";
    input.placeholder = "A name, a number…";
    input.setAttribute("aria-label", "Text to black out");
    input.spellcheck = false;
    const findBtn = el("button", "btn", "Find");
    findBtn.type = "submit";
    form.append(input, findBtn);
    const presets = el("div", "redact-presets");
    const list = el("div", "redact-list");
    const status = el("p", "hint redact-status");
    const preview = el("div", "redact-preview");
    const save = el("button", "btn btn-primary", "Save a blacked-out copy");
    save.hidden = true;
    const result = el("div", "cleaner");
    group.append(form, presets, status, list, preview, save, result);

    let pages: Searchable[] | null = null;
    const load = async () => (pages ??= searchable(await this.cleaning.pages()));
    const picturesOf = (n: number) => this.picturesOf(m, n);
    // Every search kept, each place with its tick; boxes drawn are one more.
    const chosen: { term: string; matches: Match[]; ticks: HTMLInputElement[]; block: HTMLElement }[] = [];
    // Pages shown to draw on, beyond those with something ticked.
    const shownPages = new Set<number>();
    const picked = () => chosen.flatMap((c) => c.matches.filter((_, i) => c.ticks[i].checked));

    const drawPreview = () => {
      preview.replaceChildren();
      if (!pages) return;
      const marks = picked();
      const withMarks = new Set(marks.map((m) => m.page));
      const show = [...new Set([...withMarks, ...shownPages])].sort((a, b) => a - b);
      if (show.length > 0) preview.append(el("p", "redact-preview-title", "As the copy will show it"));
      for (const n of show) {
        const p = pages[n - 1];
        if (!p) continue;
        const areas = marks.filter((m) => m.page === n).flatMap((m) => m.areas);
        preview.append(pageFigure(p, areas, (area) => addBox(p, area), picturesOf(n)));
      }
      // Any other page, to draw a box on.
      if (pages.length > 0) {
        const pick = el("select", "redact-page-pick");
        pick.setAttribute("aria-label", "Show a page to draw a box on");
        pick.append(el("option", undefined, show.length ? "Draw on another page…" : "Draw a box on a page…"));
        for (const p of pages) if (!show.includes(p.page)) pick.append(Object.assign(el("option", undefined, `Page ${p.page}`), { value: String(p.page) }));
        pick.addEventListener("change", () => {
          shownPages.add(Number(pick.value));
          drawPreview();
        });
        if (pick.options.length > 1) preview.append(pick);
      }
    };
    const refresh = () => {
      const n = picked().length;
      save.hidden = chosen.length === 0;
      save.disabled = n === 0;
      save.textContent = n === 1 ? "Save a copy with 1 place blacked out" : `Save a copy with ${n} places blacked out`;
      drawPreview();
    };

    /** Lists what a search found, each place with a tick; `extend` adds to a list already there. */
    const addBlock = (term: string, matches: Match[]) => {
      const existing = chosen.find((c) => c.term === term);
      const ul = existing ? existing.block.querySelector("ul")! : el("ul");
      const ticks = matches.map((mt) => {
        const li = el("li");
        const label = el("label");
        const tick = el("input");
        tick.type = "checkbox";
        tick.checked = true;
        tick.addEventListener("change", refresh);
        label.append(tick, el("span", "redact-page", `Page ${mt.page}`), " ", mt.before, el("mark", "redact-hit", mt.text || "a box you drew"), mt.after);
        li.append(label);
        ul.append(li);
        return tick;
      });
      if (existing) {
        existing.matches.push(...matches);
        existing.ticks.push(...ticks);
        existing.block.querySelector(".redact-count")!.textContent = ` — ${existing.matches.length === 1 ? "1 place" : `${existing.matches.length} places`} `;
        refresh();
        return;
      }
      const block = el("div", "redact-term");
      const head = el("div", "redact-head");
      const remove = el("button", "link", "Remove");
      head.append(el("strong", undefined, term), el("span", "redact-count", ` — ${matches.length === 1 ? "1 place" : `${matches.length} places`} `), remove);
      const entry = { term, matches: [...matches], ticks, block };
      chosen.push(entry);
      remove.addEventListener("click", () => {
        chosen.splice(chosen.indexOf(entry), 1);
        block.remove();
        refresh();
      });
      block.append(head, ul);
      list.append(block);
      refresh();
    };
    const addBox = (p: Searchable, area: PageArea) => {
      const text = textIn(p, area);
      addBlock("Boxes you drew", [{ page: p.page, areas: [area], before: "", text, after: "" }]);
    };

    form.addEventListener("submit", async (e) => {
      e.preventDefault();
      const term = input.value.trim();
      if (!term || chosen.some((c) => c.term.toLowerCase() === `“${term}”`.toLowerCase())) return;
      findBtn.disabled = true;
      status.textContent = pages ? "" : "Reading the pages…";
      const ps = await load();
      findBtn.disabled = false;
      const matches = find(ps, term);
      if (matches.length === 0) {
        status.textContent = `“${term}” is not on the pages as text. If it is in a picture — a scan, a screenshot — draw a box over it on the page instead.`;
        drawPreview();
        return;
      }
      status.textContent = "";
      input.value = "";
      addBlock(`“${term}”`, matches);
    });
    for (const preset of PRESETS) {
      const b = el("button", "btn redact-preset", preset.label);
      b.type = "button";
      b.addEventListener("click", async () => {
        if (chosen.some((c) => c.term === preset.name)) return;
        b.disabled = true;
        const matches = findPreset(await load(), preset);
        b.disabled = false;
        if (matches.length === 0) {
          status.textContent = `No ${preset.name.toLowerCase()} on the pages.`;
          drawPreview();
          return;
        }
        status.textContent = "";
        addBlock(preset.name, matches);
      });
      presets.append(b);
    }
    // The pages to draw on, once asked for.
    const drawOn = el("button", "link", "Draw a box on a page instead");
    drawOn.type = "button";
    drawOn.addEventListener("click", async () => {
      await load();
      drawOn.remove();
      if (pages && pages.length > 0) shownPages.add(1);
      drawPreview();
    });
    presets.append(drawOn);

    save.addEventListener("click", async () => {
      const marks = picked();
      save.disabled = true;
      save.textContent = "Making the copy…";
      const r = await this.cleaning.redact(areasOf(marks));
      refresh();
      result.replaceChildren();
      if (r.error) {
        result.append(el("p", "problem is-warning", `No copy was made: ${r.error}.`));
        return;
      }
      result.append(el("p", "clean-done", `${r.saved ? "Saved" : "Made"} a copy with ${marks.length === 1 ? "1 place" : `${marks.length} places`} blacked out, and nothing about who made the file. Removed:`));
      const ul = el("ul", "clean-list");
      for (const item of r.removed) {
        const li = el("li");
        li.append(el("span", undefined, item.what.charAt(0).toUpperCase() + item.what.slice(1)), el("span", "clean-size", formatBytes(item.bytes)));
        ul.append(li);
      }
      result.append(ul);
      const sharer = shareButton(r.name, r.bytes);
      if (sharer) result.append(sharer);
      if (!r.saved) {
        const again = el("button", "btn", "Save it");
        again.addEventListener("click", () => this.cleaning.save(r.name, r.bytes));
        result.append(again);
      }
      const open = el("button", "btn", "Open the copy");
      open.title = "Check it yourself: search it for what you blacked out";
      open.addEventListener("click", () => this.cleaning.open(r.bytes));
      result.append(open);
    });
    return group;
  }

  /** One button that saves a copy without all of the above, then says what went. */
  private cleaner(m: FileModel): HTMLElement {
    const format = m.file.format;
    const box = el("div", "cleaner");
    const button = el("button", "btn btn-clean", "Remove it — save a clean copy");
    button.title = "Makes the copy in this tab: nothing is uploaded";
    const notes: Record<string, string> = {
      zip: "Removes the document's properties, and the camera data and location of every photo in it. In a Word document, tracked changes are accepted — what was deleted goes, with its text — and comments are deleted, with their authors.",
      webp: "Leaves out the EXIF and XMP chunks: camera, place, dates, editing history. The picture is copied byte for byte.",
      gif: "Leaves out the comments and the XMP. Every frame is copied byte for byte, with its timing.",
      heif: "Blanks the camera data, location, serial numbers and XMP where they lie, so the file keeps its size. The picture and its thumbnail are copied unchanged.",
      png: "Removes the text notes, EXIF, XMP and the time it was last changed. The pixels are copied byte for byte.",
      video:
        "Blanks the location, the camera, the software and the dates where they lie, so the file keeps its size. The picture and sound are copied byte for byte.",
      wasm: "Leaves out the custom sections that say who built it and how: function names, tools, source map and debug info links, DWARF. The code and data are copied byte for byte; paths inside the data are part of the program, and stay.",
      pdf: "Writes the document anew with only what its pages use: no author, programs or dates, no XMP, no earlier versions. Text under black boxes or hidden from view is taken out, and marks for redaction applied, with every other letter left where it was; a scanned page loses its pixels under a box; photos lose their camera data.",
    };
    const note =
      (isAudio(m) ? "Blanks the location, the device, the software and the dates where they lie, so the file keeps its size. The sound is copied byte for byte." : undefined) ??
      notes[format] ??
      "Removes the camera data, location, serial numbers, the maker's notes, thumbnail and comments. The picture itself is copied unchanged.";
    const limits = cleanLimits(m);
    box.append(button, el("p", "hint", note));
    // A workbook's or a deck's comments and notes are its content: gone only when asked.
    const word = m.file.labels.includes("word/document.xml");
    const extras = format === "zip" && !word && m.file.facts.some((x) => x.kind === "notes" || x.kind === "comments");
    const also = el("input");
    if (extras) {
      also.type = "checkbox";
      const label = el("label", "clean-also");
      label.append(also, " Also empty the comments and the speaker's notes, and who wrote them");
      box.append(label);
    }
    box.append(limits);
    button.addEventListener("click", async () => {
      button.disabled = true;
      button.textContent = "Making the copy…";
      const notes = extras && also.checked;
      // What goes with them is struck out with the rest.
      if (notes) {
        for (const e of box.closest(".reveals")?.querySelectorAll<HTMLElement>("[data-kept]") ?? []) {
          const kind = e.dataset.kind ?? e.nextElementSibling?.getAttribute("data-kind");
          if (kind === "notes" || kind === "comments") delete e.dataset.kept;
        }
      }
      const r = await this.cleaning.clean(notes);
      box.replaceChildren();
      if (r.error) {
        box.append(el("p", "problem is-warning", `No copy was made: ${r.error}.`));
        return;
      }
      box.append(el("p", "clean-done", r.saved ? "Saved a clean copy. Removed:" : "Made a clean copy. Removed:"));
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
        const kept = notes
          ? "Kept: what is part of a workbook or a deck itself — hidden sheets, rows and slides, links to other files. Delete them in Excel or PowerPoint, then save."
          : KEPT_NOTE[format];
        box.append(el("p", "hint", kept ?? "What is not struck out is part of the file's content, and stays."));
      }
      if (r.orientation > 1) {
        box.append(el("p", "hint", "Kept only the orientation, so the picture stays the right way up."));
      }
      const open = el("button", "btn", "Open the clean copy");
      open.title = "Check it yourself: the card should now be empty";
      open.addEventListener("click", () => this.cleaning.open(r.bytes));
      // On a phone, straight on to the app it was going to: no hunting for it in Downloads.
      const sharer = shareButton(r.name, r.bytes);
      if (sharer) box.append(sharer);
      if (!r.saved) {
        const save = el("button", "btn", "Save it");
        save.addEventListener("click", () => this.cleaning.save(r.name, r.bytes));
        box.append(save);
      }
      const diff = el("button", "btn", "Compare with the original");
      diff.title = "What the copy took out, part by part";
      diff.addEventListener("click", () => this.cleaning.compare(new File([r.bytes as BlobPart], "the clean copy")));
      box.append(open, diff, limits);
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
    group.append(el("h2", undefined, "Entry"));
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
