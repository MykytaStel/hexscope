// Blocks ↔ bytes, for a JPEG. Its picture is cut into blocks of 8×8 samples,
// grouped in minimum coded units — MCUs, usually 16×16 pixels — each written
// as a run of Huffman-coded bits in a scan. A progressive JPEG has several
// scans: first every block's average, then bands of detail, so each part of
// the picture is written a little at a time. The core finds where each unit
// of each scan begins; this view shows it both ways — point at the picture
// to see the bytes that draw it, point at a byte to see its block — lights
// the picture by what each part costs, and, for several scans, shows the
// picture as it stands after each one.
import type { FileModel } from "./model";

export interface BlockHooks {
  /** The core's block map (see `Parsed::block_map`) and why it is empty or short. */
  blocks(): Promise<{ map: Float64Array; note: string }>;
  /** Marks file bytes `[start, end)`; `-1, -1` clears. */
  onBytes(start: number, end: number): void;
  /** Opens the bytes `[start, end)` where they are not on screen: a phone's Bytes view. */
  showBytes(start: number, end: number): void;
}

/** The tallest the picture is shown, in CSS pixels. */
const MAX_HEIGHT = 420;
/** The picture's longer side on its canvas: sharp enough, and a fixed cost for a 50 MP photo. */
const MAX_SIDE = 1600;
/** The smallest a marked block is drawn, in screen pixels, so it can be found in a large photo. */
const MIN_MARK = 8;
/** Between scans when the build-up plays. */
const PLAY_MS = 700;
const HINT = "Point at the picture, or tap it, to see the bytes that draw it.";
/** Numbers before each scan's starts in the core's map. */
const SCAN_HEADER = 12;

const plural = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
const offset = (n: number) => `0x${n.toString(16).toUpperCase().padStart(6, "0")}`;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** "Show in bytes", for where the bytes are a view away: only on narrow screens. */
export function bytesLink(show: () => void): HTMLButtonElement {
  const b = el("button", "link only-narrow", "Show in bytes");
  b.addEventListener("click", show);
  return b;
}

interface Scan {
  /** The frame's components it codes, a bit each. */
  components: number;
  /** The coefficients it codes, in zig-zag order, and the bits: before, and now down to. */
  ss: number;
  se: number;
  ah: number;
  al: number;
  unitWidth: number;
  unitHeight: number;
  columns: number;
  rows: number;
  perUnit: number;
  /** The byte after its data. */
  end: number;
  /** Each unit's first file bit, then the bit after the last one found. */
  starts: Float64Array;
  /** Units found: fewer than columns × rows when the scan stops short. */
  count: number;
}

interface Blocks {
  progressive: boolean;
  /** Components in the frame. */
  components: number;
  scans: Scan[];
}

/** The core's flat map, read back into scans. */
export function readBlocks(map: Float64Array): Blocks | null {
  if (map.length < 3) return null;
  const scans: Scan[] = [];
  let at = 3;
  for (let s = 0; s < map[2]; s++) {
    if (at + SCAN_HEADER > map.length) return null;
    const [components, ss, se, ah, al, unitWidth, unitHeight, columns, rows, perUnit, end, n] = map.subarray(
      at,
      at + SCAN_HEADER,
    );
    const starts = map.subarray(at + SCAN_HEADER, at + SCAN_HEADER + n);
    at += SCAN_HEADER + n;
    if (n < 2) break;
    scans.push({ components, ss, se, ah, al, unitWidth, unitHeight, columns, rows, perUnit, end, starts, count: n - 1 });
  }
  return scans.length ? { progressive: map[0] === 1, components: map[1], scans } : null;
}

/** The frame's components by name: Y, Cb, Cr for a colour photo. */
function names(frame: number, mask: number): string {
  const all = frame === 1 ? ["grey"] : frame === 3 ? ["Y", "Cb", "Cr"] : ["C", "M", "Y", "K"];
  const picked = all.filter((_, i) => mask & (1 << i));
  return picked.length === frame && frame > 1 ? "every colour" : picked.join(" ");
}

/** What a scan adds, in a few words. */
export function describe(s: Scan, frame: number, progressive: boolean): string {
  const of = names(frame, s.components);
  if (!progressive) return `all of ${of}`;
  if (s.ss === 0) return s.ah === 0 ? `averages of ${of}` : `averages of ${of}, one more bit`;
  const band = s.ss === s.se ? `detail ${s.ss}` : `detail ${s.ss}–${s.se}`;
  return s.ah === 0 ? `${band} of ${of}` : `${band} of ${of}, one more bit`;
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
 * here, where the blocks are turned with it. With `end`, only the scans
 * before it are kept, and the file closed there.
 */
function asStored(b: Uint8Array, end = b.length): Uint8Array {
  if (b[0] !== 0xff || b[1] !== 0xd8) return b;
  const parts: Uint8Array[] = [b.subarray(0, 2)];
  let i = 2;
  while (i + 4 <= end && b[i] === 0xff) {
    const marker = b[i + 1];
    if (marker === 0xff) {
      i++;
      continue;
    }
    // From the scan on, everything is kept.
    if (marker === 0xda || marker === 0xd9) break;
    const next = i + 2 + ((b[i + 2] << 8) | b[i + 3]);
    if (marker !== 0xe1) parts.push(b.subarray(i, next));
    i = next;
  }
  parts.push(b.subarray(Math.min(i, end), end));
  if (end < b.length) parts.push(Uint8Array.of(0xff, 0xd9));
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/** The last index `i` of `starts[0..count)` with `starts[i] <= bit`. */
function unitAt(starts: Float64Array, count: number, bit: number): number {
  let lo = 0;
  let hi = count - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid] <= bit) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

export class BlockView {
  private m: FileModel | null = null;
  private b: Blocks | null = null;
  /** Stored size, and the turn that stands it up. */
  private w = 0;
  private h = 0;
  private turned = new DOMMatrix();
  private frame: HTMLElement | null = null;
  private image: HTMLCanvasElement | null = null;
  private heat: HTMLCanvasElement | null = null;
  private mark: HTMLCanvasElement | null = null;
  private caption: HTMLElement | null = null;
  private heatOn = false;
  /** The scan whose unit is outlined and whose bytes are marked. */
  private scan = 0;
  /** The pixel last clicked or tapped, stored then shown: what stays when the pointer leaves. */
  private pinned: [number, number, number, number] | null = null;
  /** The pixel the caption is about, so it is not rebuilt under a click on its link. */
  private shown = "";
  /** Why the map stops short of the whole picture, when it does. */
  private note = "";
  /** Scans drawn, and the drawing in flight and the one waiting. */
  private drawn = 0;
  private drawingFor: FileModel | null = null;
  private wanted = 0;
  private playing = 0;

  constructor(private readonly hooks: BlockHooks) {}

  /** The panel for a JPEG, or null for anything else. */
  element(m: FileModel): HTMLElement | null {
    this.m = null;
    this.b = null;
    this.heatOn = false;
    this.note = "";
    this.pinned = null;
    this.shown = "";
    this.scan = 0;
    this.drawn = 0;
    this.wanted = 0;
    clearTimeout(this.playing);
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
    const scale = Math.min(1, MAX_SIDE / Math.max(dw, dh));
    image.width = Math.max(1, Math.round(dw * scale));
    image.height = Math.max(1, Math.round(dh * scale));
    const heat = el("canvas", "picture-overlay");
    heat.hidden = true;
    const mark = el("canvas", "picture-overlay");
    frame.append(image, heat, mark);
    this.frame = frame;
    this.image = image;
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
    const buildup = el("div", "buildup");
    buildup.hidden = true;
    this.caption = el("div", "picture-caption hint", "Finding where each block of the picture is written…");
    group.append(frame, tools, buildup, this.caption);

    void this.draw(m, 0);
    void this.hooks.blocks().then(({ map, note }) => {
      if (this.m !== m || !this.caption) return;
      this.b = readBlocks(map);
      if (!this.b) {
        const why = note || "its blocks could not be found";
        this.caption.textContent = `${why[0].toUpperCase()}${why.slice(1)}.`;
        return;
      }
      tools.hidden = false;
      this.note = note;
      if (this.b.scans.length > 1) this.buildUp(buildup, this.b);
      this.idle();
    });

    const at = (e: PointerEvent): [number, number, number, number] => {
      const r = frame.getBoundingClientRect();
      const dx = ((e.clientX - r.left) / r.width) * dw;
      const dy = ((e.clientY - r.top) / r.height) * dh;
      const p = this.turned.inverse().transformPoint(new DOMPoint(dx, dy));
      return [
        Math.min(w - 1, Math.max(0, Math.floor(p.x))),
        Math.min(h - 1, Math.max(0, Math.floor(p.y))),
        Math.floor(Math.min(dw - 1, Math.max(0, dx))),
        Math.floor(Math.min(dh - 1, Math.max(0, dy))),
      ];
    };
    // Hovering shows; a click or a tap also pins, so the caption and its
    // links stay when the pointer leaves for them.
    frame.addEventListener("pointermove", (e) => {
      if (frame.clientWidth) this.fromPixel(...at(e));
    });
    frame.addEventListener("pointerdown", (e) => {
      if (!frame.clientWidth) return;
      this.pinned = at(e);
      this.fromPixel(...this.pinned);
    });
    frame.addEventListener("pointerleave", (e) => {
      if (e.pointerType !== "mouse") return;
      if (this.pinned) {
        this.fromPixel(...this.pinned);
        return;
      }
      this.clear();
      this.hooks.onBytes(-1, -1);
    });
    new ResizeObserver(() => {
      if (this.heatOn && this.frame === frame) this.drawHeat();
    }).observe(frame);
    return group;
  }

  /** From the hex view: the block a byte of a scan belongs to. */
  fromByte(at: number): void {
    const b = this.b;
    if (!b || !this.mark?.isConnected) return;
    const bit = at * 8;
    const n = at < 0 ? -1 : b.scans.findIndex((s) => bit + 7 >= s.starts[0] && bit < s.starts[s.count]);
    if (n < 0) {
      this.clear();
      return;
    }
    const s = b.scans[n];
    const i = unitAt(s.starts, s.count, bit + 7);
    this.outline(s, i);
    this.shown = "";
    if (!this.caption) return;
    const where = b.scans.length > 1 ? ` in scan ${n + 1} of ${b.scans.length} (${describe(s, b.components, b.progressive)})` : "";
    const cost = s.starts[i + 1] - s.starts[i];
    this.caption.textContent =
      `Byte ${offset(at)} is part of block ${i % s.columns}, ${Math.floor(i / s.columns)}${where} — ` +
      `${s.unitWidth}×${s.unitHeight} pixels, written in ${plural(cost, "bit")}.`;
  }

  /** The slider that shows the picture after each scan, and a button that plays it. */
  private buildUp(host: HTMLElement, b: Blocks): void {
    const m = this.m;
    if (!m) return;
    const n = b.scans.length;
    const label = el("label", "buildup-label");
    const range = el("input");
    range.type = "range";
    range.min = "1";
    range.max = String(n);
    range.value = String(n);
    range.setAttribute("aria-label", "Scans shown");
    const says = el("span", "buildup-says");
    const play = el("button", "btn btn-small", "Play");
    const set = (k: number) => {
      range.value = String(k);
      const bytes = k === n ? m.bytes.length : b.scans[k - 1].end;
      const share = Math.round((bytes / m.bytes.length) * 100);
      says.textContent =
        k === n
          ? `All ${n} scans: the whole picture.`
          : `After scan ${k} of ${n} (${describe(b.scans[k - 1], b.components, b.progressive)}): ${share}% of the file.`;
      void this.draw(m, k === n ? 0 : bytes);
    };
    range.addEventListener("input", () => {
      clearTimeout(this.playing);
      this.playing = 0;
      play.textContent = "Play";
      set(Number(range.value));
    });
    play.addEventListener("click", () => {
      if (this.playing) {
        clearTimeout(this.playing);
        this.playing = 0;
        play.textContent = "Play";
        return;
      }
      play.textContent = "Stop";
      let k = 1;
      const step = () => {
        if (this.m !== m) return;
        set(k);
        if (k === n) {
          this.playing = 0;
          play.textContent = "Play";
          return;
        }
        k++;
        this.playing = window.setTimeout(step, PLAY_MS);
      };
      step();
    });
    label.append(range);
    host.append(
      el(
        "p",
        "hint",
        b.progressive
          ? "This JPEG is progressive: it arrives blurry, then sharpens, scan by scan. See it build up:"
          : "This JPEG comes in one scan per colour. See it build up:",
      ),
      label,
      play,
      says,
    );
    set(n);
    host.hidden = false;
  }

  /**
   * Draws the picture as stored, stood up by its orientation; with `end`,
   * only the scans before that byte. The newest request waits for the one
   * being drawn.
   */
  private async draw(m: FileModel, end: number): Promise<void> {
    this.wanted = end;
    if (this.drawingFor === m) return;
    this.drawingFor = m;
    try {
      while (this.m === m && this.drawn !== this.wanted + 1) {
        const upto = this.wanted;
        const bytes = upto ? asStored(m.bytes, upto) : asStored(m.bytes);
        let bitmap: ImageBitmap;
        try {
          bitmap = await createImageBitmap(new Blob([bytes as BlobPart], { type: "image/jpeg" }));
        } catch {
          if (this.m === m && !upto) {
            this.caption?.before(el("p", "hint", "This browser could not draw the picture; its blocks are still shown."));
          }
          this.drawn = upto + 1;
          continue;
        }
        const image = this.image;
        const ctx = image?.getContext("2d");
        if (image && ctx && this.m === m) {
          const [dw, dh] = m.file.orientation >= 5 ? [this.h, this.w] : [this.w, this.h];
          ctx.resetTransform();
          ctx.clearRect(0, 0, image.width, image.height);
          ctx.imageSmoothingQuality = "high";
          ctx.setTransform(new DOMMatrix().scale(image.width / dw, image.height / dh).multiply(this.turned));
          ctx.drawImage(bitmap, 0, 0, this.w, this.h);
        }
        bitmap.close();
        this.drawn = upto + 1;
      }
    } finally {
      if (this.drawingFor === m) this.drawingFor = null;
    }
  }

  private fromPixel(x: number, y: number, dx: number, dy: number): void {
    const b = this.b;
    if (!b || !this.m || !this.caption) return;
    const key = `${dx},${dy},${this.scan}`;
    if (key === this.shown) return;
    this.shown = key;
    // Each scan's unit under the pixel, when the scan reaches it.
    const units = b.scans.map((s) => {
      const i = Math.floor(y / s.unitHeight) * s.columns + Math.floor(x / s.unitWidth);
      return i < s.count ? i : -1;
    });
    const s = b.scans[this.scan];
    const i = units[this.scan];
    if (i < 0) {
      this.clear();
      this.shown = key;
      this.caption.textContent = `Pixel ${dx}, ${dy}: its block comes after where the scan stops.`;
      this.hooks.onBytes(-1, -1);
      return;
    }
    const start = Math.floor(s.starts[i] / 8);
    const end = Math.max(start + 1, Math.ceil(s.starts[i + 1] / 8));
    this.outline(s, i);
    this.hooks.onBytes(start, end);
    const link = bytesLink(() => this.hooks.showBytes(start, end));
    if (b.scans.length === 1) {
      const bits = s.starts[i + 1] - s.starts[i];
      const ratio = bits / ((s.starts[s.count] - s.starts[0]) / s.count);
      const compare =
        ratio > 1.5 ? "more detail than most, so more bits" : ratio < 0.5 ? "flatter than most, so fewer bits" : "about as much as most";
      this.caption.replaceChildren(
        `Pixel ${dx}, ${dy} is in block ${i % s.columns}, ${Math.floor(i / s.columns)}: ${s.unitWidth}×${s.unitHeight} pixels, ` +
          `${plural(s.perUnit, "block")} of 8×8 samples, written in ${plural(bits, "bit")} — ` +
          `${plural(end - start, "byte")} from ${offset(start)}. ` +
          `An average block takes ${plural(Math.round((s.starts[s.count] - s.starts[0]) / s.count), "bit")}; this one has ${compare}. `,
        link,
      );
      return;
    }
    // Several scans: each writes a little of this pixel.
    const total = units.reduce((n, u, k) => (u < 0 ? n : n + b.scans[k].starts[u + 1] - b.scans[k].starts[u]), 0);
    const list = el("ol", "scan-list");
    b.scans.forEach((sc, k) => {
      const u = units[k];
      const row = el("button", "scan-row");
      row.setAttribute("aria-pressed", String(k === this.scan));
      const bits = u < 0 ? null : sc.starts[u + 1] - sc.starts[u];
      row.append(
        el("span", "scan-n", String(k + 1)),
        el("span", "scan-what", describe(sc, b.components, b.progressive)),
        el("span", "scan-bits", bits === null ? "—" : bits === 0 ? "0 bits*" : plural(bits, "bit")),
      );
      row.addEventListener("click", () => {
        this.scan = k;
        this.fromPixel(x, y, dx, dy);
      });
      const li = el("li");
      li.append(row);
      list.append(li);
    });
    const zero = units.some((u, k) => u >= 0 && b.scans[k].starts[u + 1] === b.scans[k].starts[u]);
    this.caption.replaceChildren(
      `Pixel ${dx}, ${dy} is written a little in each of ${b.scans.length} scans — ${plural(total, "bit")} in all. ` +
        `Scan ${this.scan + 1}'s part: block ${i % s.columns}, ${Math.floor(i / s.columns)}, ` +
        `${s.unitWidth}×${s.unitHeight} pixels, ${plural(end - start, "byte")} from ${offset(start)}. `,
      link,
      list,
    );
    if (zero) {
      this.caption.append(
        el("span", "hint scan-zero", "* No bits of its own: an earlier block's end-of-band code covered it too."),
      );
    }
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

  /** Outlines unit `i` of a scan. */
  private outline(s: Scan, i: number): void {
    const c = this.mark;
    if (!c) return;
    const ctx = this.fit(c);
    if (!ctx) return;
    const x0 = (i % s.columns) * s.unitWidth;
    const y0 = Math.floor(i / s.columns) * s.unitHeight;
    const x1 = Math.min(this.w, x0 + s.unitWidth);
    const y1 = Math.min(this.h, y0 + s.unitHeight);
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
   * Lights the picture by its bits: each cell dimmed by how few it takes,
   * against the costliest few percent. Every scan's units add their bits to
   * the cells they cover, cells as small as the smallest unit. One pixel per
   * cell on a small canvas, scaled up without smoothing, so neighbours meet
   * without seams.
   */
  private drawHeat(): void {
    const b = this.b;
    const c = this.heat;
    if (!b || !c) return;
    const ctx = this.fit(c);
    if (!ctx) return;
    const cw = Math.min(...b.scans.map((s) => s.unitWidth));
    const ch = Math.min(...b.scans.map((s) => s.unitHeight));
    const cols = Math.ceil(this.w / cw);
    const rows = Math.ceil(this.h / ch);
    const costs = new Float64Array(cols * rows);
    for (const s of b.scans) {
      for (let i = 0; i < s.count; i++) {
        const x0 = Math.floor(((i % s.columns) * s.unitWidth) / cw);
        const y0 = Math.floor((Math.floor(i / s.columns) * s.unitHeight) / ch);
        const x1 = Math.min(cols, x0 + Math.max(1, Math.round(s.unitWidth / cw)));
        const y1 = Math.min(rows, y0 + Math.max(1, Math.round(s.unitHeight / ch)));
        const share = (s.starts[i + 1] - s.starts[i]) / ((x1 - x0) * (y1 - y0));
        for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) costs[y * cols + x] += share;
      }
    }
    const sorted = Float64Array.from(costs).sort();
    const top = Math.max(1e-9, sorted[Math.floor(sorted.length * 0.98)] ?? 1);
    const small = new ImageData(cols, rows);
    for (let i = 0; i < cols * rows; i++) {
      small.data[i * 4 + 3] = Math.round(235 * (1 - Math.sqrt(Math.min(1, costs[i] / top))));
    }
    const tiles = document.createElement("canvas");
    tiles.width = cols;
    tiles.height = rows;
    tiles.getContext("2d")?.putImageData(small, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.setTransform(this.onto(c));
    ctx.drawImage(tiles, 0, 0, cols * cw, rows * ch);
  }

  private idle(): void {
    this.shown = "";
    if (!this.caption || !this.b) return;
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
