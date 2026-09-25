// Pixels ↔ bytes. A PNG's pixels are its decompressed scanlines, each a
// filter byte and then the pixels' bytes; every one of those bytes was written
// by one DEFLATE step, which read a few bits of the file. This view follows
// that chain both ways: point at a pixel to see the step and the file bytes
// behind it, point at a byte of IDAT to see which pixels it became.
import { BlockView, bytesLink } from "./blocks";
import { buildUp } from "./buildup";
import type { FileModel } from "./model";

/** Numbers per located step: index, kind, a, b, bitStart, bitEnd, outStart. */
type Located = Float64Array;

export interface PictureHooks {
  locate(by: 0 | 1, pos: number): Promise<Located>;
  /** A JPEG's block map; see `BlockView`. */
  blocks(): Promise<{ map: Float64Array; note: string }>;
  /** Marks file bytes `[start, end)`; `-1, -1` clears. */
  onBytes(start: number, end: number): void;
  /** Opens the DEFLATE player at a step. */
  onStep(index: number): void;
  /** Opens the bytes `[start, end)` where they are not on screen: a phone's Bytes view. */
  showBytes(start: number, end: number): void;
}

const FILTERS = ["None", "Sub", "Up", "Average", "Paeth"];
const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };
/** The tallest the picture is shown, in CSS pixels. */
const MAX_HEIGHT = 420;
/** Rows drawn for one range at most: a huge copy is outlined, not tiled. */
const MAX_RECTS = 4096;
/** Single pixels drawn for one range of an interlaced picture at most. */
const MAX_DOTS = 100_000;
/** The picture's grain after each Adam7 pass: every pixel stands for a cell this size. */
const CELL: [number, number][] = [
  [8, 8],
  [4, 8],
  [4, 4],
  [2, 4],
  [2, 2],
  [1, 2],
  [1, 1],
];

const hex = (n: number) => `0x${n.toString(16).toUpperCase().padStart(2, "0")}`;
const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
/** A share as a percentage; under 1%, with a decimal. */
const pct = (f: number) => (f < 0.01 ? `${(f * 100).toFixed(1)}%` : `${Math.round(f * 100)}%`);

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/**
 * One stored picture: the whole image, or one of an interlaced image's seven
 * Adam7 passes, which holds every `dx`-th pixel of every `dy`-th row from
 * (`x0`, `y0`) and is filtered as a picture of its own (PNG §8.2).
 */
export interface Pass {
  /** 1 to 7 for an Adam7 pass, 0 for a picture that is not interlaced. */
  number: number;
  x0: number;
  y0: number;
  dx: number;
  dy: number;
  width: number;
  height: number;
  /** Bytes per scanline, filter byte included. */
  row: number;
  /** Where its first filter byte is in the decompressed data. */
  start: number;
  /** How many rows are stored before its first: an index into the row filters. */
  first: number;
}

/** Image geometry, from IHDR. */
export interface Geometry {
  width: number;
  height: number;
  /** Bits per pixel. */
  bpp: number;
  /** The passes with pixels, in the order they are stored; one when not interlaced. */
  passes: Pass[];
  interlaced: boolean;
}

/** Where each Adam7 pass starts on the 8×8 grid, and its steps, across and down. */
const ADAM7: [number, number, number, number][] = [
  [0, 0, 8, 8],
  [4, 0, 8, 8],
  [0, 4, 4, 8],
  [2, 0, 4, 4],
  [0, 2, 2, 4],
  [1, 0, 2, 2],
  [0, 1, 1, 2],
];

/** A PNG's geometry from its IHDR numbers [width, height, depth, colour type, interlace]. */
export function geometry(ihdr: number[]): Geometry {
  const [width, height, depth, color, interlace] = ihdr;
  const bpp = (CHANNELS[color] ?? 1) * depth;
  const rowOf = (w: number) => Math.ceil((w * bpp) / 8) + 1;
  const interlaced = interlace === 1;
  const passes: Pass[] = [];
  if (!interlaced) {
    passes.push({ number: 0, x0: 0, y0: 0, dx: 1, dy: 1, width, height, row: rowOf(width), start: 0, first: 0 });
  } else {
    let start = 0;
    let first = 0;
    ADAM7.forEach(([x0, y0, dx, dy], i) => {
      const w = Math.max(0, Math.ceil((width - x0) / dx));
      const h = Math.max(0, Math.ceil((height - y0) / dy));
      // A pass with no pixels has no bytes, not even filter bytes.
      if (!w || !h) return;
      passes.push({ number: i + 1, x0, y0, dx, dy, width: w, height: h, row: rowOf(w), start, first });
      start += rowOf(w) * h;
      first += h;
    });
  }
  return { width, height, bpp, passes, interlaced };
}

/** The pass pixel (x, y) is stored in, and its column and row there. */
export function passOf(g: Geometry, x: number, y: number): [Pass, number, number] {
  for (const p of g.passes) {
    if (x >= p.x0 && y >= p.y0 && (x - p.x0) % p.dx === 0 && (y - p.y0) % p.dy === 0) {
      return [p, (x - p.x0) / p.dx, (y - p.y0) / p.dy];
    }
  }
  return [g.passes[0], x, y];
}

/** The pass decompressed byte `offset` is in, and its row there. */
function passAt(g: Geometry, offset: number): [Pass, number] {
  let p = g.passes[0];
  for (const q of g.passes) if (q.start <= offset) p = q;
  return [p, Math.min(p.height - 1, Math.floor((offset - p.start) / p.row))];
}

/** The decompressed byte where pixel (x, y) begins. */
export function pixelOffset(g: Geometry, x: number, y: number): number {
  const [p, c, r] = passOf(g, x, y);
  return p.start + r * p.row + 1 + Math.floor((c * g.bpp) / 8);
}

/**
 * The pixels decompressed bytes `[start, end)` fall on, as [x, y, count,
 * step] runs, one per stored row: `count` pixels from (x, y), `step` apart.
 */
export function runs(g: Geometry, start: number, end: number): [number, number, number, number][] {
  const out: [number, number, number, number][] = [];
  for (const p of g.passes) {
    const pEnd = p.start + p.row * p.height;
    const from = Math.max(start, p.start) - p.start;
    const to = Math.min(end, pEnd) - p.start;
    if (to <= from) continue;
    const first = Math.floor(from / p.row);
    const last = Math.min(p.height - 1, Math.floor((to - 1) / p.row));
    for (let r = first; r <= last && out.length < MAX_RECTS; r++) {
      const a = r === first ? from % p.row : 0;
      const b = r === last ? (to - 1) % p.row : p.row - 1;
      // Column 0 is the filter byte: it belongs to the row, not a pixel.
      if (b < 1) continue;
      const c0 = Math.floor(((Math.max(a, 1) - 1) * 8) / g.bpp);
      const c1 = Math.min(p.width - 1, Math.floor(((b - 1) * 8 + 7) / g.bpp));
      out.push([p.x0 + c0 * p.dx, p.y0 + r * p.dy, c1 - c0 + 1, p.dx]);
    }
  }
  return out;
}

export class PictureView {
  private m: FileModel | null = null;
  private g: Geometry | null = null;
  private overlay: HTMLCanvasElement | null = null;
  private caption: HTMLElement | null = null;
  /** One lookup at a time; the newest waiting one replaces older ones. */
  private busy = false;
  private next: (() => Promise<void>) | null = null;
  /** The pixel last clicked or tapped: what stays when the pointer leaves. */
  private pinned: [number, number] | null = null;
  /** The pixel the caption is about, so it is not rebuilt under a click on its link. */
  private shown = "";

  /** A JPEG's picture has blocks, not scanlines: it has a view of its own. */
  private readonly jpeg: BlockView;

  constructor(private readonly hooks: PictureHooks) {
    this.jpeg = new BlockView(hooks);
  }

  /** The panel for this file, or null when it has no picture to show. */
  element(m: FileModel): HTMLElement | null {
    this.m = null;
    this.overlay = null;
    this.pinned = null;
    this.shown = "";
    const f = m.file;
    if (f.format === "jpeg") return this.jpeg.element(m);
    if (f.format !== "png" || !f.ihdr) return null;
    const group = el("div", "group picture");
    group.append(el("h3", undefined, "Picture"));
    if (!f.preview) {
      group.append(el("p", "hint", "No pixels to show: the image data could not be decoded."));
      return group;
    }
    this.g = geometry(f.ihdr);
    const { width, height } = this.g;
    this.m = m;

    const frame = el("div", "picture-frame");
    const canvas = el("canvas", "picture-image");
    canvas.width = f.preview.width;
    canvas.height = f.preview.height;
    const ctx = canvas.getContext("2d");
    ctx?.putImageData(new ImageData(new Uint8ClampedArray(f.preview.pixels), f.preview.width, f.preview.height), 0, 0);
    // Keep the picture's shape: as wide as the panel, unless that would make
    // a tall picture taller than MAX_HEIGHT.
    frame.style.aspectRatio = `${width} / ${height}`;
    frame.style.width = `min(100%, ${Math.round((MAX_HEIGHT * width) / height)}px)`;
    const overlay = el("canvas", "picture-overlay");
    frame.append(canvas, overlay);
    this.overlay = overlay;

    this.caption = el("p", "picture-caption hint", "Point at a pixel, or tap it, to see the bytes it came from.");
    // Pointer events, so a tap on a phone picks a pixel as hovering does.
    const at = (e: PointerEvent): [number, number] => {
      const r = frame.getBoundingClientRect();
      const x = Math.floor(((e.clientX - r.left) / r.width) * width);
      const y = Math.floor(((e.clientY - r.top) / r.height) * height);
      return [Math.min(width - 1, Math.max(0, x)), Math.min(height - 1, Math.max(0, y))];
    };
    // Hovering shows; a click or a tap also pins, so the caption and its
    // links stay when the pointer leaves for them.
    frame.addEventListener("pointermove", (e) => {
      if (!frame.clientWidth) return;
      const [x, y] = at(e);
      this.queue(() => this.fromPixel(x, y));
    });
    frame.addEventListener("pointerdown", (e) => {
      if (!frame.clientWidth) return;
      const [x, y] = (this.pinned = at(e));
      this.queue(() => this.fromPixel(x, y));
    });
    frame.addEventListener("pointerleave", (e) => {
      if (e.pointerType !== "mouse") return;
      const pinned = this.pinned;
      if (pinned) {
        this.queue(() => this.fromPixel(...pinned));
        return;
      }
      this.next = null;
      this.clear();
      this.hooks.onBytes(-1, -1);
    });
    group.append(frame);
    if (this.g.interlaced) {
      const host = el("div", "buildup");
      group.append(host);
      this.buildUp(host, m, canvas);
    }
    group.append(this.caption);
    return group;
  }

  /** An interlaced picture after each pass: coarse first, then filled in. */
  private buildUp(host: HTMLElement, m: FileModel, canvas: HTMLCanvasElement): void {
    const g = this.g;
    const preview = m.file.preview;
    const ctx = canvas.getContext("2d");
    if (!g || !preview || !ctx) return;
    const { width: pw, height: ph, pixels } = preview;
    const n = g.passes.length;
    const total = g.width * g.height;
    buildUp(host, {
      intro: "This PNG is interlaced (Adam7): its pixels are stored in seven passes, a coarse picture first, then filled in. See it build up:",
      count: n,
      show: (k) => {
        // Each pixel shows the one in the passes so far that stands for it:
        // the top-left of its cell. The preview may be scaled, so its pixels
        // are looked up by where they sit in the picture.
        const [cw, ch] = CELL[g.passes[k - 1].number - 1];
        const out = new Uint8ClampedArray(pw * ph * 4);
        for (let py = 0; py < ph; py++) {
          const y = Math.floor((py * g.height) / ph);
          const qy = Math.min(ph - 1, Math.floor(((y - (y % ch)) * ph) / g.height));
          for (let px = 0; px < pw; px++) {
            const x = Math.floor((px * g.width) / pw);
            const qx = Math.min(pw - 1, Math.floor(((x - (x % cw)) * pw) / g.width));
            const from = (qy * pw + qx) * 4;
            out.set(pixels.subarray(from, from + 4), (py * pw + px) * 4);
          }
        }
        ctx.putImageData(new ImageData(out, pw, ph), 0, 0);
      },
      says: async (k) => {
        const p = g.passes[k - 1];
        const pixelsSoFar = g.passes.slice(0, k).reduce((sum, q) => sum + q.width * q.height, 0);
        const what = k === n ? `All ${n} passes: the whole picture` : `After pass ${p.number} of 7: ${pct(pixelsSoFar / total)} of the pixels`;
        if (k === n) return `${what}.`;
        // How much of the file those pixels took: the step that wrote the
        // pass's last byte, and where its bits end.
        const s = await this.hooks.locate(0, p.start + p.row * p.height - 1);
        if (s.length !== 7) return `${what}.`;
        const [, end] = m.bitsToFile(s[4], s[5]);
        return `${what}, ${pct(end / m.bytes.length)} of the file.`;
      },
    });
  }

  /** From the hex view: the pixels a byte of the file became. */
  fromByte(offset: number): void {
    this.jpeg.fromByte(offset);
    const m = this.m;
    if (!m || !this.overlay?.isConnected) return;
    if (offset < 0) {
      this.queue(async () => this.clear());
      return;
    }
    const n = m.fileToStream(offset);
    const bit = (n - m.file.streamHeader) * 8;
    if (n < 0 || bit < 0) {
      this.queue(async () => this.clear());
      return;
    }
    this.queue(async () => {
      const s = await this.hooks.locate(1, bit);
      if (this.m !== m) return;
      this.show(s, null);
    });
  }

  /** Runs `job` now, or when the lookup in flight is done — only the newest waits. */
  private queue(job: () => Promise<void>): void {
    if (this.busy) {
      this.next = job;
      return;
    }
    this.busy = true;
    void job().finally(() => {
      this.busy = false;
      const next = this.next;
      this.next = null;
      if (next) this.queue(next);
    });
  }

  private async fromPixel(x: number, y: number): Promise<void> {
    const m = this.m;
    const g = this.g;
    if (!m || !g || this.shown === `${x},${y}`) return;
    const s = await this.hooks.locate(0, pixelOffset(g, x, y));
    if (this.m !== m) return;
    this.show(s, [x, y]);
    this.shown = `${x},${y}`;
    if (s.length === 7) {
      const [start, end] = m.bitsToFile(s[4], s[5]);
      this.hooks.onBytes(start, end);
    }
  }

  /** Draws a step's output (and a copy's source) on the picture, and says what it is. */
  private show(s: Located, pixel: [number, number] | null): void {
    const m = this.m;
    const g = this.g;
    if (!m || !g || !this.caption) return;
    this.clear();
    if (s.length !== 7) {
      if (pixel) this.caption.textContent = `Pixel ${pixel[0]}, ${pixel[1]}: not decoded.`;
      return;
    }
    const [index, kind, a, b, , , outStart] = s;
    const len = kind === 1 ? 1 : kind === 2 ? b : 0;
    if (len === 0) {
      this.caption.textContent =
        kind === 0
          ? "These bits begin a block of the compressed stream, with its Huffman codes: they write no pixels, but every pixel in the block is read with them."
          : "These bits end a block of the compressed stream: they write no pixels.";
      return;
    }
    const ctx = this.context();
    if (ctx) {
      if (kind === 2) this.paint(ctx, outStart - a, outStart - a + len, "rgba(59, 130, 246, 0.45)");
      this.paint(ctx, outStart, outStart + len, "rgba(232, 82, 122, 0.6)");
    }

    const lines: (string | Node)[] = [];
    const [pass, passRow] = passAt(g, outStart);
    if (pixel) {
      const [p, , r] = passOf(g, pixel[0], pixel[1]);
      const filter = m.file.rowFilters[p.first + r];
      const where = g.interlaced ? `pass ${p.number} of 7, its row ${r}` : `row ${r}`;
      lines.push(`Pixel ${pixel[0]}, ${pixel[1]} · ${where} filtered with ${FILTERS[filter] ?? `type ${filter}`}. `);
    } else {
      const where = g.interlaced ? `pass ${pass.number}, row ${passRow}` : `row ${passRow}`;
      lines.push(`These bits wrote ${plural(len, "byte")} of ${where}${len > pass.row ? " and on" : ""}. `);
    }
    if (kind === 1) {
      lines.push(`Its byte, ${hex(a)}, is a literal: spelled out in the file's bits.`);
    } else {
      // Where the copy comes from, said only when it is in the same pass.
      const from = outStart - a;
      const up = g.interlaced ? " in its pass" : "";
      const rows =
        from < pass.start
          ? ""
          : a % pass.row === 0
            ? ` — exactly ${plural(a / pass.row, "row")} up${up}`
            : Math.floor((from - pass.start) / pass.row) === passRow
              ? " on the same row"
              : "";
      lines.push(`It was copied: ${plural(len, "byte")} from ${a.toLocaleString()} back${rows}, shown in blue.`);
    }
    // The player counts steps from 1.
    const step = el("button", "link", `Step ${(index + 1).toLocaleString()} in the player`);
    step.addEventListener("click", () => this.hooks.onStep(index));
    this.caption.replaceChildren(...lines, " ", step);
    if (pixel) {
      const [start, end] = m.bitsToFile(s[4], s[5]);
      this.caption.append(" · ", bytesLink(() => this.hooks.showBytes(start, end)));
    }
  }

  private context(): CanvasRenderingContext2D | null {
    const o = this.overlay;
    if (!o) return null;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(o.clientWidth * dpr);
    const h = Math.round(o.clientHeight * dpr);
    if (o.width !== w || o.height !== h) {
      o.width = w;
      o.height = h;
    }
    return o.getContext("2d");
  }

  private clear(): void {
    this.shown = "";
    const o = this.overlay;
    o?.getContext("2d")?.clearRect(0, 0, o.width, o.height);
    if (this.caption && this.m) this.caption.textContent = "Point at a pixel, or tap it, to see the bytes it came from.";
  }

  /** Fills the pixels output bytes `[start, end)` fall on. */
  private paint(ctx: CanvasRenderingContext2D, start: number, end: number, colour: string): void {
    const g = this.g;
    const o = this.overlay;
    if (!g || !o || end <= start) return;
    const sx = o.width / g.width;
    const sy = o.height / g.height;
    ctx.fillStyle = colour;
    let dots = 0;
    for (const [x, y, count, step] of runs(g, start, end)) {
      // At least one screen pixel, so a copy inside a large picture still shows.
      if (step === 1) {
        ctx.fillRect(x * sx, y * sy, Math.max(1, count * sx), Math.max(1, sy));
        continue;
      }
      // A pass's pixels are spread out: each is drawn where it is.
      for (let i = 0; i < count && dots < MAX_DOTS; i++, dots++) {
        ctx.fillRect((x + i * step) * sx, y * sy, Math.max(1, sx), Math.max(1, sy));
      }
    }
  }
}
