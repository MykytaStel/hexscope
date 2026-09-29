// What the colours in the bytes mean, for this file: each colour it uses,
// named for its format, and a way to the first part in it.
import type { FileModel, Tint } from "./model";

/** A compound file's colours: Office 97–2003 and Outlook's messages. */
const COMPOUND: Partial<Record<Tint, string>> = {
  sig: "Header",
  ihdr: "Tables: which sectors hold what, and the directory",
  plte: "The mini stream, where small streams are kept",
  idat: "Streams: the document, a message's text and files",
  text: "Properties: who, when; a message's headers",
  anc: "Free and unused sectors",
};

/** What each colour is, per format; `*` for pictures and anything else. */
const NAMES: Record<string, Partial<Record<Tint, string>>> = {
  "*": {
    sig: "Signature: what kind of file it is",
    ihdr: "Header: size and layout",
    plte: "Tables: colours, quantisation, Huffman codes",
    idat: "The picture's data",
    iend: "End of the file",
    text: "Metadata and text",
    anc: "Other parts",
  },
  pdf: {
    sig: "Header",
    ihdr: "Cross-reference: where each object is",
    plte: "Objects: pages, fonts, links",
    idat: "Streams: page content, images, files",
    iend: "End of a version",
    text: "Document information and XMP",
    anc: "Other parts",
  },
  zip: {
    text: "Entries: the files inside",
    ihdr: "Central directory: the list of entries",
    iend: "End record",
    anc: "Other parts",
  },
  heif: { sig: "File type", ihdr: "Metadata boxes", idat: "Media data: the pictures", text: "EXIF and XMP", anc: "Other boxes" },
  video: { sig: "File type", ihdr: "Movie boxes: tracks, timing", idat: "Media data: picture and sound", text: "Metadata: place, device, software", anc: "Other boxes" },
  wasm: { sig: "Magic and version", ihdr: "Sections: types, imports, exports", idat: "Code and data", text: "Custom sections: names, tools, debug info", anc: "Other parts" },
  eml: { anc: "Headers and parts" },
  msg: COMPOUND,
  office97: COMPOUND,
  cfb: COMPOUND,
};

const ALWAYS: Partial<Record<Tint, string>> = {
  gps: "Location",
  warning: "Breaks a rule of the format",
  error: "Damaged",
};

let closing: AbortController | null = null;

/** The legend for `m`, placed in `host`; replaces any earlier one. */
export function showLegend(host: HTMLElement, m: FileModel, select: (id: number) => void): void {
  host.replaceChildren();
  closing?.abort();
  // The first node of each colour, in file order.
  const first = new Map<Tint, number>();
  for (let i = 1; i < m.count; i++) {
    const t = m.tint(i);
    if (!first.has(t) && m.len(i) > 0) first.set(t, i);
  }
  const names = { ...NAMES["*"], ...(NAMES[m.file.format] ?? {}), ...ALWAYS };
  const shown = [...first].filter(([t]) => names[t]);
  if (shown.length < 2) return;

  const box = document.createElement("details");
  box.className = "hex-legend";
  const summary = document.createElement("summary");
  summary.textContent = "Colours";
  const list = document.createElement("ul");
  for (const [tint, node] of shown) {
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button";
    b.title = "Go to the first part in this colour";
    const swatch = document.createElement("span");
    swatch.className = "hex-legend-swatch";
    swatch.style.background = `var(--tint-${tint})`;
    b.append(swatch, names[tint] ?? tint);
    b.addEventListener("click", () => {
      box.open = false;
      select(node);
    });
    li.append(b);
    list.append(li);
  }
  box.append(summary, list);
  host.append(box);

  closing = new AbortController();
  const { signal } = closing;
  document.addEventListener("pointerdown", (e) => {
    if (box.open && !box.contains(e.target as Node)) box.open = false;
  }, { signal });
  box.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && box.open) {
      e.stopPropagation();
      box.open = false;
      summary.focus();
    }
  }, { signal });
}
