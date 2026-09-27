// Black out text yourself, in a PDF: type a name or a number, see every
// place it appears on the pages, tick the ones to go, and get a copy in
// which they are taken out of the page — not only covered — with a black
// box drawn where each was. The search runs here, over each page's glyphs
// and where they land; the core rewrites the pages.
import type { PageArea } from "./model";
import type { PageGlyphs } from "./worker";

/** One place a search found: which page, the boxes to draw, and the text around it. */
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
  text: string;
  lower: string;
  glyph: Int32Array;
  areas: Float64Array;
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
    return { page: n + 1, text, lower: lower.replace(/\s/g, " "), glyph: Int32Array.from(owner), areas: p.areas };
  });
}

/** Every place `query` appears, ignoring case and how words are spaced. */
export function find(pages: Searchable[], query: string): Match[] {
  const q = query.trim().replace(/\s+/g, " ").toLowerCase();
  const out: Match[] = [];
  if (!q) return out;
  for (const p of pages) {
    // Spaces in the page run together as the query's do.
    let from = 0;
    while (out.length < MAX_MATCHES) {
      const at = indexLoose(p.lower, q, from);
      if (!at) break;
      const [s, e] = at;
      from = e;
      const glyphs: number[] = [];
      for (let i = s; i < e; i++) if (p.glyph[i] >= 0 && glyphs[glyphs.length - 1] !== p.glyph[i]) glyphs.push(p.glyph[i]);
      if (glyphs.length === 0) continue;
      out.push({
        page: p.page,
        areas: boxes(p.areas, glyphs),
        before: (s > CONTEXT ? "…" : "") + p.text.slice(Math.max(0, s - CONTEXT), s),
        text: p.text.slice(s, e),
        after: p.text.slice(e, e + CONTEXT) + (e + CONTEXT < p.text.length ? "…" : ""),
      });
    }
  }
  return out;
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

/** The areas to send to the core: five numbers each, the page then the box. */
export function areasOf(matches: Match[]): Float64Array {
  const out: number[] = [];
  for (const m of matches) for (const a of m.areas) out.push(m.page, ...a);
  return Float64Array.from(out);
}
