// What the colours in the bytes mean, for this file: each colour it uses,
// named for its format, and a way to the first part in it. Folded to one
// button until asked for, so the bytes keep the room.
import type { FileModel, Tint } from "./model";

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
};

const ALWAYS: Partial<Record<Tint, string>> = {
  gps: "Location",
  warning: "Breaks a rule of the format",
  error: "Damaged",
};

/** The legend for `m`, placed in `pane`; replaces any earlier one. */
export function showLegend(pane: HTMLElement, m: FileModel, select: (id: number) => void): void {
  pane.querySelector(".hex-legend")?.remove();
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
    b.addEventListener("click", () => select(node));
    li.append(b);
    list.append(li);
  }
  box.append(summary, list);
  pane.append(box);
}
