// Black out text yourself, in a PDF: type a name or a number — or pick
// "every email address", "every phone number" — see every place it appears
// on the pages, tick the ones to go, draw boxes of your own on a page, and
// see each page as it will be before the copy is made. The copy takes the
// ticked places out of the page — not only covers them — with a black box
// drawn where each was. The search runs here, over each page's glyphs and
// where they land; the core rewrites the pages.
import type { PageArea } from "./model";
import type { PageGlyphs, PagePicture } from "./worker";

/** One place to black out: which page, the boxes to draw, and the text around it. */
export interface Match {
  page: number;
  areas: PageArea[];
  before: string;
  text: string;
  after: string;
}

/** A page as one searchable string, with the glyph each character came from (-1 for a space put between words). */
export interface Searchable {
  page: number;
  media: PageArea;
  text: string;
  lower: string;
  glyph: Int32Array;
  areas: Float64Array;
  texts: string[];
  /** The page's own dark boxes and marks, four numbers each: black in the copy too. */
  boxes: Float64Array;
}

/** Characters of context either side of a match. */
const CONTEXT = 28;
/** Places listed per search, at most. */
const MAX_MATCHES = 200;
/** Points added around each box, so no edge of a letter shows. */
const PAD = 0.6;

export function searchable(pages: PageGlyphs[]): Searchable[] {
  return pages.map((p, n) => {
    let text = "";
    const owner: number[] = [];
    const a = p.areas;
    for (let i = 0; i < p.texts.length; i++) {
      const t = p.texts[i];
      if (i > 0 && t) {
        // A gap, or a new line, is a space between words when the file has none.
        const h = Math.max(a[i * 4 + 3] - a[i * 4 + 1], a[i * 4 - 1] - a[i * 4 - 3], 0.01);
        const sameLine = Math.abs(a[i * 4 + 1] - a[i * 4 - 3]) < 0.3 * h;
        const gap = a[i * 4] - a[i * 4 - 2];
        if ((!sameLine || gap > 0.2 * h) && !/\s$/.test(text) && !/^\s/.test(t)) {
          text += " ";
          owner.push(-1);
        }
      }
      text += t;
      for (let k = 0; k < t.length; k++) owner.push(i);
    }
    // Lower case letter for letter, so positions still match the original.
    let lower = "";
    for (const ch of text) {
      const l = ch.toLowerCase();
      lower += l.length === ch.length ? l : ch;
    }
    return { page: n + 1, media: p.media, text, lower: lower.replace(/\s/g, " "), glyph: Int32Array.from(owner), areas: p.areas, texts: p.texts, boxes: p.boxes };
  });
}

/** The match for characters `[s, e)` of a page, or null when no glyph is in them. */
function toMatch(p: Searchable, s: number, e: number): Match | null {
  const glyphs: number[] = [];
  for (let i = s; i < e; i++) if (p.glyph[i] >= 0 && glyphs[glyphs.length - 1] !== p.glyph[i]) glyphs.push(p.glyph[i]);
  if (glyphs.length === 0) return null;
  return {
    page: p.page,
    areas: boxes(p.areas, glyphs),
    before: (s > CONTEXT ? "…" : "") + p.text.slice(Math.max(0, s - CONTEXT), s),
    text: p.text.slice(s, e),
    after: p.text.slice(e, e + CONTEXT) + (e + CONTEXT < p.text.length ? "…" : ""),
  };
}

/** Every place `query` appears, ignoring case and how words are spaced. */
export function find(pages: Searchable[], query: string): Match[] {
  const q = query.trim().replace(/\s+/g, " ").toLowerCase();
  const out: Match[] = [];
  if (!q) return out;
  for (const p of pages) {
    let from = 0;
    while (out.length < MAX_MATCHES) {
      const at = indexLoose(p.lower, q, from);
      if (!at) break;
      from = at[1];
      const m = toMatch(p, at[0], at[1]);
      if (m) out.push(m);
    }
  }
  return out;
}

/** A kind of thing to find everywhere at once: a pattern, and a check that what it matched is one. */
export interface Preset {
  label: string;
  /** What a found one is called in the list. */
  name: string;
  pattern: RegExp;
  valid?: (s: string) => boolean;
  /** Another kind whose finds this one never overlaps: no phone number inside an account number. */
  not?: () => Preset;
}

const digits = (s: string) => s.replace(/\D/g, "");

/** Luhn's check: what a card number's last digit is for. */
function luhn(s: string): boolean {
  const d = digits(s);
  if (d.length < 13 || d.length > 19) return false;
  let sum = 0;
  for (let i = 0; i < d.length; i++) {
    let n = d.charCodeAt(d.length - 1 - i) - 48;
    if (i % 2 === 1) n = n * 2 > 9 ? n * 2 - 9 : n * 2;
    sum += n;
  }
  return sum % 10 === 0;
}

/** ISO 13616's check on an IBAN: moved and read as a number, it leaves 1 over 97. */
function iban(s: string): boolean {
  const v = s.replace(/\s/g, "").toUpperCase();
  if (v.length < 15 || v.length > 34) return false;
  const moved = v.slice(4) + v.slice(0, 4);
  let rest = 0;
  for (const ch of moved) {
    const n = ch >= "A" ? ch.charCodeAt(0) - 55 : ch.charCodeAt(0) - 48;
    rest = n > 9 ? (rest * 100 + n) % 97 : (rest * 10 + n) % 97;
  }
  return rest === 1;
}

const MONTHS = "jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec|січ|лют|бер|кві|тра|чер|лип|сер|вер|жов|лис|гру";

export const PRESETS: Preset[] = [
  { label: "Email addresses", name: "Email addresses", pattern: /[\p{L}\p{N}._%+-]+@[\p{L}\p{N}-]+(?:\.[\p{L}\p{N}-]+)*\.\p{L}{2,}/gu },
  {
    label: "Phone numbers",
    name: "Phone numbers",
    pattern: /(?<![\d.,])\+?\(?\d[\d ()\-.]{7,17}\d(?![\d,])/g,
    valid: (s) => {
      const n = digits(s).length;
      // A number with a plus, or written in groups, not a figure in a table.
      return n >= 9 && n <= 15 && (s.startsWith("+") || /[ ()\-.]/.test(s)) && !/^\d{1,3}(\.\d{3})+$/.test(s);
    },
    not: () => PRESETS[2],
  },
  {
    label: "Card and bank numbers",
    name: "Card and bank account numbers",
    pattern: /\b(?:\d[ -]?){12,18}\d\b|\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]){11,30}\b/g,
    valid: (s) => (/^[A-Z]/.test(s) ? iban(s) : luhn(s)),
  },
  {
    label: "Dates",
    name: "Dates",
    pattern: new RegExp(
      `\\b\\d{1,2}[./]\\d{1,2}[./]\\d{2,4}\\b|\\b\\d{4}-\\d{2}-\\d{2}\\b|\\b\\d{1,2} (?:${MONTHS})\\p{L}*\\.? \\d{4}\\b|\\b(?:${MONTHS})\\p{L}*\\.? \\d{1,2},? \\d{4}\\b`,
      "giu",
    ),
  },
];

/** Every place a preset's pattern finds one. */
export function findPreset(pages: Searchable[], preset: Preset): Match[] {
  const out: Match[] = [];
  const other = preset.not?.();
  for (const p of pages) {
    const taken = other ? spans(p.text, other) : [];
    for (const hit of p.text.matchAll(preset.pattern)) {
      if (out.length >= MAX_MATCHES) return out;
      if (preset.valid && !preset.valid(hit[0])) continue;
      const [s, e] = [hit.index, hit.index + hit[0].length];
      if (taken.some(([a, b]) => s < b && a < e)) continue;
      const m = toMatch(p, hit.index, hit.index + hit[0].length);
      if (m) out.push(m);
    }
  }
  return out;
}

/** Where a kind finds its things in a text, as `[start, end)`. */
function spans(text: string, preset: Preset): [number, number][] {
  return [...text.matchAll(preset.pattern)].filter((h) => !preset.valid || preset.valid(h[0])).map((h) => [h.index, h.index + h[0].length]);
}

/** Finds `q` in `hay` from `from`, where a space in `q` matches one or more spaces; `[start, end)` or null. */
function indexLoose(hay: string, q: string, from: number): [number, number] | null {
  const words = q.split(" ");
  let at = hay.indexOf(words[0], from);
  while (at >= 0) {
    let end = at + words[0].length;
    let ok = true;
    for (let w = 1; w < words.length && ok; w++) {
      let k = end;
      while (hay[k] === " ") k++;
      if (k === end || !hay.startsWith(words[w], k)) ok = false;
      else end = k + words[w].length;
    }
    if (ok) return [at, end];
    at = hay.indexOf(words[0], at + 1);
  }
  return null;
}

/** The glyphs' boxes, one per line they run along, a little larger than the letters. */
function boxes(a: Float64Array, glyphs: number[]): PageArea[] {
  const out: PageArea[] = [];
  for (const g of glyphs) {
    const [l, b, r, t] = [a[g * 4], a[g * 4 + 1], a[g * 4 + 2], a[g * 4 + 3]];
    const last = out[out.length - 1];
    const h = t - b;
    if (last && Math.abs(last[1] - b) < 0.3 * Math.max(h, 0.01) && l - last[2] < 1.5 * h) {
      last[0] = Math.min(last[0], l - PAD);
      last[1] = Math.min(last[1], b - PAD);
      last[2] = Math.max(last[2], r + PAD);
      last[3] = Math.max(last[3], t + PAD);
    } else {
      out.push([l - PAD, b - PAD, r + PAD, t + PAD]);
    }
  }
  return out;
}

/** The text of the glyphs mostly inside an area, as a drawn box would take it out. */
export function textIn(p: Searchable, area: PageArea): string {
  const [L, B, R, T] = area;
  let out = "";
  let last = -1;
  for (let i = 0; i < p.text.length; i++) {
    const g = p.glyph[i];
    if (g < 0) {
      if (last >= 0 && !out.endsWith(" ")) out += " ";
      continue;
    }
    const [l, b, r, t] = [p.areas[g * 4], p.areas[g * 4 + 1], p.areas[g * 4 + 2], p.areas[g * 4 + 3]];
    const w = Math.max(0, Math.min(r, R) - Math.max(l, L));
    const h = Math.max(0, Math.min(t, T) - Math.max(b, B));
    if (w * h >= 0.5 * (r - l) * (t - b) && r > l) {
      out += p.text[i];
      last = g;
    }
  }
  return out.trim();
}

/** The areas to send to the core: five numbers each, the page then the box. */
export function areasOf(matches: Match[]): Float64Array {
  const out: number[] = [];
  for (const m of matches) for (const a of m.areas) out.push(m.page, ...a);
  return Float64Array.from(out);
}

// --- the page, as the copy will show it ------------------------------------------

const SVG = "http://www.w3.org/2000/svg";

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/**
 * A page drawn from its glyphs — each line's text where it lands — with
 * black boxes over what will go. Dragging across it draws a box of one's
 * own, handed to `onBox` in the page's points.
 */
/** A page's picture ready to draw: an image's address, and the matrix that places its unit square. */
export interface Shown {
  href: string;
  matrix: number[];
}

// Addresses made for pictures, let go when the next file's are made.
let addresses: string[] = [];

/** A page's pictures as images to draw: a JPEG as it is, pixels as a PNG. */
export async function toShown(pictures: PagePicture[]): Promise<Shown[]> {
  const out: Shown[] = [];
  for (const p of pictures) {
    let blob: Blob | null = null;
    if (p.jpeg) blob = new Blob([p.jpeg as BlobPart], { type: "image/jpeg" });
    else if (p.rgba && p.width && p.height) {
      const c = document.createElement("canvas");
      c.width = p.width;
      c.height = p.height;
      c.getContext("2d")?.putImageData(new ImageData(p.rgba as Uint8ClampedArray<ArrayBuffer>, p.width, p.height), 0, 0);
      blob = await new Promise<Blob | null>((resolve) => c.toBlob(resolve, "image/png"));
    }
    if (!blob) continue;
    const href = URL.createObjectURL(blob);
    addresses.push(href);
    out.push({ href, matrix: p.matrix });
  }
  return out;
}

/** Lets go of every picture's address: a new file is open. */
export function forgetPictures(): void {
  for (const a of addresses) URL.revokeObjectURL(a);
  addresses = [];
}

export function pageFigure(p: Searchable, boxesToDraw: PageArea[], onBox: (area: PageArea) => void, pictures?: Promise<Shown[]>): HTMLElement {
  const figure = document.createElement("figure");
  figure.className = "redact-figure";
  const [ml, mb, mr, mt] = p.media;
  const y = (v: number) => mt - v;
  // The part of the page with text on it, and a margin: a page is mostly
  // white, and its words should be big enough to read.
  let [el, eb, er, et] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let g = 0; g < p.texts.length; g++) {
    el = Math.min(el, p.areas[g * 4]);
    eb = Math.min(eb, p.areas[g * 4 + 1]);
    er = Math.max(er, p.areas[g * 4 + 2]);
    et = Math.max(et, p.areas[g * 4 + 3]);
  }
  const own: PageArea[] = [];
  for (let i = 0; i + 4 <= p.boxes.length; i += 4) own.push([p.boxes[i], p.boxes[i + 1], p.boxes[i + 2], p.boxes[i + 3]]);
  for (const [l, b, r, t] of [...own, ...boxesToDraw]) [el, eb, er, et] = [Math.min(el, l), Math.min(eb, b), Math.max(er, r), Math.max(et, t)];
  const view =
    el < er
      ? [Math.max(0, el - ml - 24), Math.max(0, mt - et - 24), Math.min(mr - ml, er - ml + 24), Math.min(mt - mb, mt - eb + 24)]
      : [0, 0, mr - ml, mt - mb];
  const pic = svg("svg", {
    viewBox: `${view[0]} ${view[1]} ${view[2] - view[0]} ${view[3] - view[1]}`,
    role: "img",
    "aria-label": `Page ${p.page} as the copy will show it; drag across it to black out more`,
  });
  pic.append(svg("rect", { class: "blackout-page", x: 0, y: 0, width: mr - ml, height: mt - mb }));
  // The page's pictures, under its text: a scan is all picture. Each unit
  // square placed by its matrix, turned the right way up for the screen.
  const layer = svg("g", {});
  pic.append(layer);
  const runs: { node: SVGElement; box: PageArea }[] = [];
  // The text in runs along a line, each stretched to the width it takes.
  const a = p.areas;
  let run = "";
  let box: PageArea | null = null;
  const flush = () => {
    if (!box || !run.trim()) return;
    const [l, b, r, t] = box;
    const h = t - b;
    const text = svg("text", {
      class: "blackout-context",
      x: l - ml,
      y: y(b + 0.2 * h),
      "font-size": h,
      textLength: Math.max(1, r - l),
      lengthAdjust: "spacingAndGlyphs",
    });
    text.textContent = run;
    pic.append(text);
    runs.push({ node: text, box });
  };
  for (let g = 0; g < p.texts.length; g++) {
    const area: PageArea = [a[g * 4], a[g * 4 + 1], a[g * 4 + 2], a[g * 4 + 3]];
    const h = area[3] - area[1];
    if (box && Math.abs(box[1] - area[1]) < 0.3 * Math.max(h, 0.01) && area[0] - box[2] < 1.5 * h && area[0] >= box[0]) {
      if (area[0] - box[2] > 0.2 * h && !run.endsWith(" ")) run += " ";
      run += p.texts[g];
      box[2] = Math.max(box[2], area[2]);
      box[3] = Math.max(box[3], area[3]);
      continue;
    }
    flush();
    run = p.texts[g];
    box = [...area];
  }
  flush();
  // The page's own boxes, then the ones chosen: all black in the copy.
  for (const [l, b, r, t] of [...own, ...boxesToDraw]) {
    pic.append(svg("rect", { class: "blackout-box", x: l - ml, y: y(t), width: r - l, height: t - b }));
  }
  void pictures?.then((list) => {
    for (const { href, matrix: [a, b, c, d, e, f] } of list) {
      layer.append(
        svg("image", {
          href,
          x: 0,
          y: 0,
          width: 1,
          height: 1,
          preserveAspectRatio: "none",
          transform: `matrix(${a} ${-b} ${-c} ${d} ${c + e - ml} ${mt - d - f})`,
        }),
      );
      // Text under a picture is a scan's recognised words: the picture
      // shows them already.
      const xs = [e, a + e, c + e, a + c + e];
      const ys = [f, b + f, d + f, b + d + f];
      const [pl, pb, pr, pt] = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
      for (const r of runs) {
        const [l, bb, rr, t] = r.box;
        if (l >= pl - 1 && rr <= pr + 1 && bb >= pb - 1 && t <= pt + 1) r.node.classList.add("is-under-picture");
      }
    }
  });

  // A box of one's own: from where the pointer goes down to where it comes up.
  const drawn = svg("rect", { class: "redact-drawing", x: 0, y: 0, width: 0, height: 0 });
  drawn.style.display = "none";
  pic.append(drawn);
  const at = (e: PointerEvent): [number, number] => {
    const m = pic.getScreenCTM();
    if (!m) return [0, 0];
    const pt = new DOMPoint(e.clientX, e.clientY).matrixTransform(m.inverse());
    return [Math.min(mr - ml, Math.max(0, pt.x)), Math.min(mt - mb, Math.max(0, pt.y))];
  };
  let start: [number, number] | null = null;
  pic.addEventListener("pointerdown", (e) => {
    start = at(e);
    pic.setPointerCapture(e.pointerId);
  });
  pic.addEventListener("pointermove", (e) => {
    if (!start) return;
    const [x, yy] = at(e);
    drawn.style.display = "";
    drawn.setAttribute("x", String(Math.min(start[0], x)));
    drawn.setAttribute("y", String(Math.min(start[1], yy)));
    drawn.setAttribute("width", String(Math.abs(x - start[0])));
    drawn.setAttribute("height", String(Math.abs(yy - start[1])));
  });
  const end = (e: PointerEvent) => {
    if (!start) return;
    const [x, yy] = at(e);
    const [x0, y0] = start;
    start = null;
    drawn.style.display = "none";
    if (Math.abs(x - x0) < 2 || Math.abs(yy - y0) < 2) return;
    // Back to the page's points, which count up from its bottom left.
    onBox([ml + Math.min(x, x0), mt - Math.max(yy, y0), ml + Math.max(x, x0), mt - Math.min(yy, y0)]);
  };
  pic.addEventListener("pointerup", end);
  pic.addEventListener("pointercancel", () => {
    start = null;
    drawn.style.display = "none";
  });

  // The same with the keyboard: a box to move with the arrows, size with
  // Shift and the arrows, and black out with Enter. It stays where it was
  // left when the page is drawn again.
  const [vx0, vy0, vx1, vy1] = view;
  const kb = keyed.get(p.page) ?? (() => {
    const w = Math.min(120, (vx1 - vx0) / 3);
    return [vx0 + (vx1 - vx0 - w) / 2, vy0 + (vy1 - vy0) / 2 - 8, w, 16];
  })();
  const place = () => {
    const [x, yy, w, h] = kb;
    drawn.style.display = "";
    drawn.setAttribute("x", String(x));
    drawn.setAttribute("y", String(yy));
    drawn.setAttribute("width", String(w));
    drawn.setAttribute("height", String(h));
  };
  pic.tabIndex = 0;
  pic.dataset.page = String(p.page);
  pic.addEventListener("focus", () => {
    place();
    caption.textContent = `Page ${p.page} — arrows move the box, Shift and arrows size it, Alt for small steps, Enter blacks it out`;
  });
  pic.addEventListener("blur", () => {
    if (!start) drawn.style.display = "none";
    caption.textContent = hint;
  });
  pic.addEventListener("keydown", (e) => {
    const step = e.altKey ? 1 : 6;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const move = moves[e.key];
    if (move) {
      e.preventDefault();
      if (e.shiftKey) {
        kb[2] = Math.max(2, Math.min(mr - ml - kb[0], kb[2] + move[0]));
        kb[3] = Math.max(2, Math.min(mt - mb - kb[1], kb[3] + move[1]));
      } else {
        kb[0] = Math.max(0, Math.min(mr - ml - kb[2], kb[0] + move[0]));
        kb[1] = Math.max(0, Math.min(mt - mb - kb[3], kb[1] + move[1]));
      }
      keyed.set(p.page, kb);
      place();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      keyed.set(p.page, kb);
      const [x, yy, w, h] = kb;
      onBox([ml + x, mt - (yy + h), ml + x + w, mt - yy]);
    }
  });
  const hint = `Page ${p.page} — drag across it to black out more, or use the keyboard`;
  const caption = document.createElement("figcaption");
  caption.textContent = hint;
  figure.append(pic, caption);
  return figure;
}

/** Where the keyboard's box was left on each page, in the figure's units. */
const keyed = new Map<number, number[]>();
