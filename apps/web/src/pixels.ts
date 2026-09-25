// Pixels ↔ bytes. A PNG's pixels are its decompressed scanlines, each a
// filter byte and then the pixels' bytes; every one of those bytes was written
// by one DEFLATE step, which read a few bits of the file. This view follows
// that chain both ways: point at a pixel to see the step and the file bytes
// behind it, point at a byte of IDAT to see which pixels it became.
import type { FileModel } from "./model";

/** Numbers per located step: index, kind, a, b, bitStart, bitEnd, outStart. */
type Located = Float64Array;

export interface PictureHooks {
  locate(by: 0 | 1, pos: number): Promise<Located>;
  /** Marks file bytes `[start, end)`; `-1, -1` clears. */
  onBytes(start: number, end: number): void;
  /** Opens the DEFLATE player at a step. */
  onStep(index: number): void;
}

const FILTERS = ["None", "Sub", "Up", "Average", "Paeth"];
const CHANNELS: Record<number, number> = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };
/** The tallest the picture is shown, in CSS pixels. */
const MAX_HEIGHT = 420;
/** Rectangles drawn for one range at most: a huge copy is outlined, not tiled. */
const MAX_RECTS = 4096;

const hex = (n: number) => `0x${n.toString(16).toUpperCase().padStart(2, "0")}`;
const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Image geometry, from IHDR. */
interface Geometry {
  width: number;
  height: number;
  /** Bits per pixel. */
  bpp: number;
  /** Bytes per scanline, filter byte included. */
  row: number;
}

export class PictureView {
  private m: FileModel | null = null;
  private g: Geometry | null = null;
  private overlay: HTMLCanvasElement | null = null;
  private caption: HTMLElement | null = null;
  /** One lookup at a time; the newest waiting one replaces older ones. */
  private busy = false;
  private next: (() => Promise<void>) | null = null;

  constructor(private readonly hooks: PictureHooks) {}

  /** The panel for this file, or null when it has no picture to show. */
  element(m: FileModel): HTMLElement | null {
    this.m = null;
    this.overlay = null;
    const f = m.file;
    if (f.format !== "png" || !f.ihdr) return null;
    const group = el("div", "group picture");
    group.append(el("h3", undefined, "Picture"));
    if (!f.preview) {
      const why = f.ihdr[4]
        ? "Interlaced: its pixels are stored in seven passes, which this view does not follow yet."
        : "No pixels to show: the image data could not be decoded.";
      group.append(el("p", "hint", why));
      return group;
    }
    const [width, height, depth, color] = f.ihdr;
    const bpp = (CHANNELS[color] ?? 1) * depth;
    this.g = { width, height, bpp, row: Math.ceil((width * bpp) / 8) + 1 };
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
    const pick = (e: PointerEvent) => {
      const r = frame.getBoundingClientRect();
      const x = Math.floor(((e.clientX - r.left) / r.width) * width);
      const y = Math.floor(((e.clientY - r.top) / r.height) * height);
      this.queue(() => this.fromPixel(Math.min(width - 1, Math.max(0, x)), Math.min(height - 1, Math.max(0, y))));
    };
    frame.addEventListener("pointermove", pick);
    frame.addEventListener("pointerdown", pick);
    // A finger lifting also "leaves": keep what it picked.
    frame.addEventListener("pointerleave", (e) => {
      if (e.pointerType !== "mouse") return;
      this.next = null;
      this.clear();
      this.hooks.onBytes(-1, -1);
    });
    group.append(frame, this.caption);
    return group;
  }

  /** From the hex view: the pixels a byte of the file became. */
  fromByte(offset: number): void {
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
    if (!m || !g) return;
    const out = y * g.row + 1 + Math.floor((x * g.bpp) / 8);
    const s = await this.hooks.locate(0, out);
    if (this.m !== m) return;
    this.show(s, [x, y]);
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
    if (pixel) {
      const y = pixel[1];
      const filter = m.file.rowFilters[y];
      lines.push(`Pixel ${pixel[0]}, ${y} · row ${y} filtered with ${FILTERS[filter] ?? `type ${filter}`}. `);
    } else {
      const first = Math.floor(outStart / g.row);
      lines.push(`These bits wrote ${plural(len, "byte")} of row ${first}${len > g.row ? " and on" : ""}. `);
    }
    if (kind === 1) {
      lines.push(`Its byte, ${hex(a)}, is a literal: spelled out in the file's bits.`);
    } else {
      const rows = a % g.row === 0 ? ` — exactly ${plural(a / g.row, "row")} up` : a < g.row ? " on the same row" : "";
      lines.push(`It was copied: ${plural(len, "byte")} from ${a.toLocaleString()} back${rows}, shown in blue.`);
    }
    // The player counts steps from 1.
    const step = el("button", "link", `Step ${(index + 1).toLocaleString()} in the player`);
    step.addEventListener("click", () => this.hooks.onStep(index));
    this.caption.replaceChildren(...lines, " ", step);
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
    const first = Math.floor(start / g.row);
    const last = Math.min(g.height - 1, Math.floor((end - 1) / g.row));
    for (let y = first, n = 0; y <= last && n < MAX_RECTS; y++, n++) {
      const from = y === first ? start % g.row : 0;
      const to = y === last ? (end - 1) % g.row : g.row - 1;
      // Column 0 is the filter byte: it belongs to the row, not a pixel.
      if (to < 1) continue;
      const x0 = Math.floor(((Math.max(from, 1) - 1) * 8) / g.bpp);
      const x1 = Math.min(g.width - 1, Math.floor(((to - 1) * 8 + 7) / g.bpp));
      // At least one screen pixel, so a copy inside a large picture still shows.
      ctx.fillRect(x0 * sx, y * sy, Math.max(1, (x1 - x0 + 1) * sx), Math.max(1, sy));
    }
  }
}
