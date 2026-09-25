// Blocks ↔ bytes, for a JPEG. Its picture is cut into minimum coded units —
// MCUs, usually 16×16 pixels — each written as a run of Huffman-coded bits
// in the scan. The core finds where each run begins; this view shows it both
// ways: point at the picture to see its block's bytes, point at a byte of the
// scan to see its block, and light the picture by what each block costs.
import type { FileModel } from "./model";

export interface BlockHooks {
  /** The core's block map (see `Parsed::block_map`) and why it is empty or short. */
  blocks(): Promise<{ map: Float64Array; note: string }>;
  /** Marks file bytes `[start, end)`; `-1, -1` clears. */
  onBytes(start: number, end: number): void;
}

/** The tallest the picture is shown, in CSS pixels. */
const MAX_HEIGHT = 420;
/** The picture's longer side on its canvas: sharp enough, and a fixed cost for a 50 MP photo. */
const MAX_SIDE = 1600;
/** The smallest a marked block is drawn, in screen pixels, so it can be found in a large photo. */
const MIN_MARK = 8;
const HINT = "Point at the picture, or tap it, to see the bytes that draw it.";

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
const offset = (n: number) => `0x${n.toString(16).toUpperCase().padStart(6, "0")}`;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

interface Grid {
  mcuWidth: number;
  mcuHeight: number;
  columns: number;
  rows: number;
  perMcu: number;
  /** Each MCU's first file bit, then the bit after the last one found. */
  starts: Float64Array;
  /** MCUs found: fewer than columns × rows when the scan stops short. */
  count: number;
  /** Bits in an average MCU. */
  mean: number;
}

function grid(map: Float64Array): Grid | null {
  if (map.length < 7) return null;
  const [mcuWidth, mcuHeight, columns, rows, perMcu] = map;
  const starts = map.subarray(5);
  const count = starts.length - 1;
  return { mcuWidth, mcuHeight, columns, rows, perMcu, starts, count, mean: (starts[count] - starts[0]) / count };
}

/**
 * The matrix that turns the stored picture, `w` × `h`, to stand the way its
 * EXIF orientation says (TIFF 6.0, tag 274). Orientations 5 to 8 swap sides.
 */
export function turn(o: number, w: number, h: number): DOMMatrix {
  const m: Record<number, number[]> = {
    2: [-1, 0, 0, 1, w, 0],
    3: [-1, 0, 0, -1, w, h],
    4: [1, 0, 0, -1, 0, h],
    5: [0, 1, 1, 0, 0, 0],
    6: [0, 1, -1, 0, h, 0],
    7: [0, -1, -1, 0, h, w],
    8: [0, -1, 1, 0, 0, w],
  };
  return new DOMMatrix(m[o] ?? [1, 0, 0, 1, 0, 0]);
}

/**
 * The file without its APP1 segments, so the browser draws the picture as
 * stored instead of turning it by its EXIF orientation: the turning is done
 * here, where the blocks are turned with it.
 */
function asStored(b: Uint8Array): Uint8Array {
  if (b[0] !== 0xff || b[1] !== 0xd8) return b;
  const parts: Uint8Array[] = [b.subarray(0, 2)];
  let i = 2;
  while (i + 4 <= b.length && b[i] === 0xff) {
    const marker = b[i + 1];
    if (marker === 0xff) {
      i++;
      continue;
    }
    // From the scan on, everything is kept.
    if (marker === 0xda || marker === 0xd9) break;
    const end = i + 2 + ((b[i + 2] << 8) | b[i + 3]);
    if (marker !== 0xe1) parts.push(b.subarray(i, end));
    i = end;
  }
  parts.push(b.subarray(Math.min(i, b.length)));
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

export class BlockView {
  private m: FileModel | null = null;
  private g: Grid | null = null;
  /** Stored size, and the turn that stands it up. */
  private w = 0;
  private h = 0;
  private turned = new DOMMatrix();
  private frame: HTMLElement | null = null;
  private heat: HTMLCanvasElement | null = null;
  private mark: HTMLCanvasElement | null = null;
  private caption: HTMLElement | null = null;
  private heatOn = false;
  /** Why the map stops short of the whole picture, when it does. */
  private note = "";

  constructor(private readonly hooks: BlockHooks) {}

  /** The panel for a JPEG, or null for anything else. */
  element(m: FileModel): HTMLElement | null {
    this.m = null;
    this.g = null;
    this.heatOn = false;
    this.note = "";
    const f = m.file;
    if (f.format !== "jpeg" || !f.dimensions) return null;
    const [w, h] = f.dimensions;
    if (w === 0 || h === 0) return null;
    this.m = m;
    this.w = w;
    this.h = h;
    this.turned = turn(f.orientation, w, h);
    const [dw, dh] = f.orientation >= 5 ? [h, w] : [w, h];

    const group = el("div", "group picture");
    group.append(el("h3", undefined, "Picture"));
    const frame = el("div", "picture-frame");
    frame.style.aspectRatio = `${dw} / ${dh}`;
    frame.style.width = `min(100%, ${Math.round((MAX_HEIGHT * dw) / dh)}px)`;
    const image = el("canvas", "picture-image");
    image.style.imageRendering = "auto";
    const heat = el("canvas", "picture-overlay");
    heat.hidden = true;
    const mark = el("canvas", "picture-overlay");
    frame.append(image, heat, mark);
    this.frame = frame;
    this.heat = heat;
    this.mark = mark;

    const tools = el("div", "picture-tools");
    const heatBtn = el("button", "btn btn-small", "Where the bits go");
    heatBtn.setAttribute("aria-pressed", "false");
    // Shown once there are blocks to light.
    tools.hidden = true;
    heatBtn.addEventListener("click", () => {
      this.heatOn = !this.heatOn;
      heatBtn.setAttribute("aria-pressed", String(this.heatOn));
      heat.hidden = !this.heatOn;
      if (this.heatOn) this.drawHeat();
      this.idle();
    });
    tools.append(heatBtn);
    this.caption = el("p", "picture-caption hint", "Finding where each block of the picture is written…");
    group.append(frame, tools, this.caption);

    void this.draw(m, image, dw, dh);
    void this.hooks.blocks().then(({ map, note }) => {
      if (this.m !== m || !this.caption) return;
      this.g = grid(map);
      if (!this.g) {
        const why = note || "its blocks could not be found";
        this.caption.textContent = `${why[0].toUpperCase()}${why.slice(1)}.`;
        return;
      }
      tools.hidden = false;
      this.note = note;
      this.idle();
    });

    const pick = (e: PointerEvent) => {
      const r = frame.getBoundingClientRect();
      const dx = ((e.clientX - r.left) / r.width) * dw;
      const dy = ((e.clientY - r.top) / r.height) * dh;
      const p = this.turned.inverse().transformPoint(new DOMPoint(dx, dy));
      const x = Math.min(w - 1, Math.max(0, Math.floor(p.x)));
      const y = Math.min(h - 1, Math.max(0, Math.floor(p.y)));
      this.fromPixel(x, y, Math.floor(Math.min(dw - 1, Math.max(0, dx))), Math.floor(Math.min(dh - 1, Math.max(0, dy))));
    };
    frame.addEventListener("pointermove", pick);
    frame.addEventListener("pointerdown", pick);
    // A finger lifting also "leaves": keep what it picked.
    frame.addEventListener("pointerleave", (e) => {
      if (e.pointerType !== "mouse") return;
      this.clear();
      this.hooks.onBytes(-1, -1);
    });
    new ResizeObserver(() => {
      if (this.heatOn && this.frame === frame) this.drawHeat();
    }).observe(frame);
    return group;
  }

  /** From the hex view: the block a byte of the scan belongs to. */
  fromByte(at: number): void {
    const g = this.g;
    if (!g || !this.mark?.isConnected) return;
    const bit = at * 8;
    if (at < 0 || bit < g.starts[0] - 7 || bit >= g.starts[g.count]) {
      this.clear();
      return;
    }
    // The last MCU starting at or before the byte's last bit.
    let lo = 0;
    let hi = g.count - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (g.starts[mid] <= bit + 7) lo = mid;
      else hi = mid - 1;
    }
    const col = lo % g.columns;
    const row = Math.floor(lo / g.columns);
    this.show(lo);
    if (this.caption) {
      this.caption.textContent =
        `Byte ${offset(at)} is part of block ${col}, ${row} of the picture — ` +
        `${g.mcuWidth}×${g.mcuHeight} pixels, written in ${plural(this.bits(lo), "bit")}.`;
    }
  }

  /** Draws the picture as stored, stood up by its orientation. */
  private async draw(m: FileModel, image: HTMLCanvasElement, dw: number, dh: number): Promise<void> {
    const scale = Math.min(1, MAX_SIDE / Math.max(dw, dh));
    image.width = Math.max(1, Math.round(dw * scale));
    image.height = Math.max(1, Math.round(dh * scale));
    let bitmap: ImageBitmap;
    try {
      bitmap = await createImageBitmap(new Blob([asStored(m.bytes) as BlobPart], { type: "image/jpeg" }));
    } catch {
      if (this.m === m) this.caption?.before(el("p", "hint", "This browser could not draw the picture; its blocks are still shown."));
      return;
    }
    const ctx = image.getContext("2d");
    if (ctx && this.m === m) {
      ctx.imageSmoothingQuality = "high";
      ctx.setTransform(new DOMMatrix().scale(image.width / dw, image.height / dh).multiply(this.turned));
      ctx.drawImage(bitmap, 0, 0, this.w, this.h);
    }
    bitmap.close();
  }

  private fromPixel(x: number, y: number, dx: number, dy: number): void {
    const g = this.g;
    const m = this.m;
    if (!g || !m || !this.caption) return;
    const col = Math.floor(x / g.mcuWidth);
    const row = Math.floor(y / g.mcuHeight);
    const i = row * g.columns + col;
    if (i >= g.count) {
      this.clear();
      this.caption.textContent = `Pixel ${dx}, ${dy}: its block comes after where the scan stops.`;
      this.hooks.onBytes(-1, -1);
      return;
    }
    const bits = this.bits(i);
    const start = Math.floor(g.starts[i] / 8);
    const end = Math.ceil(g.starts[i + 1] / 8);
    this.show(i);
    this.hooks.onBytes(start, end);
    const ratio = bits / g.mean;
    const compare =
      ratio > 1.5
        ? "more detail than most, so more bits"
        : ratio < 0.5
          ? "flatter than most, so fewer bits"
          : "about as much as most";
    this.caption.replaceChildren(
      `Pixel ${dx}, ${dy} is in block ${col}, ${row}: ${g.mcuWidth}×${g.mcuHeight} pixels, ` +
        `${plural(g.perMcu, "block")} of 8×8 samples, written in ${plural(bits, "bit")} — ` +
        `${plural(end - start, "byte")} from ${offset(start)}. ` +
        `An average block takes ${plural(Math.round(g.mean), "bit")}; this one has ${compare}.`,
    );
  }

  private bits(i: number): number {
    const g = this.g;
    return g ? g.starts[i + 1] - g.starts[i] : 0;
  }

  /** A canvas sized to its box on screen, and its context. */
  private fit(c: HTMLCanvasElement): CanvasRenderingContext2D | null {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(c.clientWidth * dpr);
    const h = Math.round(c.clientHeight * dpr);
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
    }
    const ctx = c.getContext("2d");
    ctx?.resetTransform();
    ctx?.clearRect(0, 0, w, h);
    return ctx;
  }

  /** The stored-picture matrix scaled to a canvas. */
  private onto(c: HTMLCanvasElement): DOMMatrix {
    const [dw, dh] = this.m && this.m.file.orientation >= 5 ? [this.h, this.w] : [this.w, this.h];
    return new DOMMatrix().scale(c.width / dw, c.height / dh).multiply(this.turned);
  }

  /** Outlines MCU `i`. */
  private show(i: number): void {
    const g = this.g;
    const c = this.mark;
    if (!g || !c) return;
    const ctx = this.fit(c);
    if (!ctx) return;
    const col = i % g.columns;
    const row = Math.floor(i / g.columns);
    const x0 = col * g.mcuWidth;
    const y0 = row * g.mcuHeight;
    const x1 = Math.min(this.w, x0 + g.mcuWidth);
    const y1 = Math.min(this.h, y0 + g.mcuHeight);
    const to = this.onto(c);
    const a = to.transformPoint(new DOMPoint(x0, y0));
    const b = to.transformPoint(new DOMPoint(x1, y1));
    const dpr = window.devicePixelRatio || 1;
    const min = MIN_MARK * dpr;
    let left = Math.min(a.x, b.x);
    let top = Math.min(a.y, b.y);
    let width = Math.abs(b.x - a.x);
    let height = Math.abs(b.y - a.y);
    if (width < min) {
      left -= (min - width) / 2;
      width = min;
    }
    if (height < min) {
      top -= (min - height) / 2;
      height = min;
    }
    ctx.fillStyle = "rgba(232, 82, 122, 0.35)";
    ctx.fillRect(left, top, width, height);
    ctx.strokeStyle = "rgb(232, 82, 122)";
    ctx.lineWidth = 2 * dpr;
    ctx.strokeRect(left, top, width, height);
  }

  /**
   * Lights the picture by its bits: each block dimmed by how few it takes,
   * against the costliest few percent. One pixel per block on a small canvas,
   * scaled up without smoothing, so neighbours meet without seams.
   */
  private drawHeat(): void {
    const g = this.g;
    const c = this.heat;
    if (!g || !c) return;
    const ctx = this.fit(c);
    if (!ctx) return;
    const costs = new Float64Array(g.count);
    for (let i = 0; i < g.count; i++) costs[i] = this.bits(i);
    const sorted = Float64Array.from(costs).sort();
    const top = Math.max(1, sorted[Math.floor(sorted.length * 0.98)] ?? 1);
    const small = new ImageData(g.columns, g.rows);
    for (let i = 0; i < g.columns * g.rows; i++) {
      // Blocks the scan never reached stay dark.
      const t = i < g.count ? Math.sqrt(Math.min(1, costs[i] / top)) : 0;
      small.data[i * 4 + 3] = Math.round(235 * (1 - t));
    }
    const tiles = document.createElement("canvas");
    tiles.width = g.columns;
    tiles.height = g.rows;
    tiles.getContext("2d")?.putImageData(small, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.setTransform(this.onto(c));
    ctx.drawImage(tiles, 0, 0, g.columns * g.mcuWidth, g.rows * g.mcuHeight);
  }

  private idle(): void {
    if (!this.caption || !this.g) return;
    this.caption.textContent = this.heatOn
      ? "The brighter a block, the more of the file it takes: detail and noise cost bits, a clear sky almost none. Point at a block to see its bytes."
      : HINT;
    if (this.note) this.caption.append(` The blocks stop short: ${this.note}.`);
  }

  private clear(): void {
    if (this.mark) this.fit(this.mark);
    this.idle();
  }
}
