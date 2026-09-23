import { FileModel, Kind, type Tint } from "./model";

export interface HexCallbacks {
  onHover(id: number, offset: number): void;
  onSelect(id: number): void;
}

const BYTES_PER_ROW = 16;
const ROW_H = 22;
const FONT_PX = 13;
const PAD_X = 20;
const PAD_Y = 14;
/** Browsers cap element height; past this the scrollbar maps proportionally. */
const MAX_SCROLL_PX = 8_000_000;
const MONO = 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace';

const HEX = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0").toUpperCase());
const TINTS: Tint[] = ["sig", "ihdr", "plte", "idat", "iend", "text", "anc", "warning", "error"];

/** Fill styles per tint at each emphasis level, precomputed once per theme. */
interface Palette {
  bg: string;
  text: string;
  zero: string;
  offset: string;
  fill: Record<Tint, { base: string; selected: string; hover: string; solid: string }>;
}

function readPalette(): Palette {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  const alpha = (name: string) => parseFloat(v(name));
  const rgba = (hex: string, a: number) => {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  };

  const fill = {} as Palette["fill"];
  for (const t of TINTS) {
    const hex = v(`--tint-${t}`);
    fill[t] = {
      base: rgba(hex, alpha("--alpha-base")),
      selected: rgba(hex, alpha("--alpha-selected")),
      hover: rgba(hex, alpha("--alpha-hover")),
      solid: hex,
    };
  }
  return { bg: v("--hex-bg"), text: v("--hex-text"), zero: v("--hex-zero"), offset: v("--hex-offset"), fill };
}

/**
 * Canvas 2D renderer for the byte grid. Only the rows in view are drawn, so a
 * 200 MB file costs the same per frame as a 200-byte one.
 */
export class HexView {
  private readonly scroller: HTMLDivElement;
  private readonly spacer: HTMLDivElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;

  private model: FileModel | null = null;
  private palette = readPalette();
  private hover = -1;
  private selected = -1;
  private frame = 0;
  private viewW = 0;
  private viewH = 0;
  private ch = 8;

  constructor(
    host: HTMLElement,
    private readonly cb: HexCallbacks,
  ) {
    this.scroller = document.createElement("div");
    this.scroller.className = "hex-scroller";
    this.canvas = document.createElement("canvas");
    this.canvas.className = "hex-canvas";
    this.spacer = document.createElement("div");
    this.scroller.append(this.canvas, this.spacer);
    host.append(this.scroller);

    const ctx = this.canvas.getContext("2d", { alpha: false });
    if (!ctx) throw new Error("Canvas 2D is not available");
    this.ctx = ctx;

    new ResizeObserver(() => this.resize()).observe(this.scroller);
    this.scroller.addEventListener("scroll", () => this.schedule(), { passive: true });
    this.canvas.addEventListener("mousemove", (e) => this.pointer(e, false));
    this.canvas.addEventListener("mouseleave", () => this.cb.onHover(-1, -1));
    this.canvas.addEventListener("click", (e) => this.pointer(e, true));
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      this.palette = readPalette();
      this.schedule();
    });
  }

  setModel(model: FileModel | null): void {
    this.model = model;
    this.hover = -1;
    this.selected = -1;
    this.scroller.scrollTop = 0;
    this.resize();
  }

  setHover(id: number): void {
    if (id === this.hover) return;
    this.hover = id;
    this.schedule();
  }

  setSelected(id: number): void {
    this.selected = id;
    this.schedule();
  }

  /** Brings a node into view unless it is already visible. */
  reveal(id: number): void {
    const m = this.model;
    if (!m || m.len(id) === 0) return;
    const row = Math.floor(m.start(id) / BYTES_PER_ROW);
    const top = this.contentTop();
    const rowY = PAD_Y + row * ROW_H;
    if (rowY >= top && rowY + ROW_H <= top + this.viewH) return;

    const target = Math.max(0, rowY - this.viewH / 3);
    const scrollTop = this.toScrollTop(target);
    const far = Math.abs(rowY - top) > this.viewH * 3;
    this.scroller.scrollTo({ top: scrollTop, behavior: far ? "auto" : "smooth" });
  }

  // --- geometry ---------------------------------------------------------

  private get rows(): number {
    return this.model ? Math.ceil(this.model.bytes.length / BYTES_PER_ROW) : 0;
  }

  private get contentH(): number {
    return this.rows * ROW_H + PAD_Y * 2;
  }

  private get scaled(): boolean {
    return this.contentH > MAX_SCROLL_PX;
  }

  /** Where the top of the viewport sits in content coordinates. */
  private contentTop(): number {
    const st = this.scroller.scrollTop;
    if (!this.scaled) return st;
    const range = MAX_SCROLL_PX - this.viewH;
    return range > 0 ? (st / range) * (this.contentH - this.viewH) : 0;
  }

  private toScrollTop(contentY: number): number {
    if (!this.scaled) return contentY;
    return (contentY / (this.contentH - this.viewH)) * (MAX_SCROLL_PX - this.viewH);
  }

  private hexX(i: number): number {
    const base = PAD_X + this.ch * 11;
    return base + i * this.ch * 3 + (i >= 8 ? this.ch : 0);
  }

  private asciiX(i: number): number {
    return this.hexX(16) + this.ch * 2 + i * this.ch;
  }

  private resize(): void {
    const dpr = window.devicePixelRatio || 1;
    this.viewW = this.scroller.clientWidth;
    this.viewH = this.scroller.clientHeight;
    this.canvas.style.width = `${this.viewW}px`;
    this.canvas.style.height = `${this.viewH}px`;
    this.canvas.width = Math.round(this.viewW * dpr);
    this.canvas.height = Math.round(this.viewH * dpr);
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.ctx.font = `${FONT_PX}px ${MONO}`;
    this.ch = this.ctx.measureText("0").width;

    const virtualH = this.scaled ? MAX_SCROLL_PX : this.contentH;
    this.spacer.style.height = `${Math.max(0, virtualH - this.viewH)}px`;
    this.schedule();
  }

  private pointer(e: MouseEvent, click: boolean): void {
    const m = this.model;
    if (!m) return;
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    const row = Math.floor((this.contentTop() + y - PAD_Y) / ROW_H);
    let col = -1;
    for (let i = 0; i < BYTES_PER_ROW; i++) {
      const hx = this.hexX(i);
      const ax = this.asciiX(i);
      if ((x >= hx - this.ch * 0.5 && x < hx + this.ch * 2.5) || (x >= ax && x < ax + this.ch)) {
        col = i;
        break;
      }
    }

    const offset = col < 0 || row < 0 ? -1 : row * BYTES_PER_ROW + col;
    const id = offset >= 0 ? m.nodeAt(offset) : -1;
    this.canvas.style.cursor = id >= 0 ? "pointer" : "default";
    if (click) {
      if (id >= 0) this.cb.onSelect(id);
    } else {
      this.cb.onHover(id, offset < m.bytes.length ? offset : -1);
    }
  }

  // --- drawing ----------------------------------------------------------

  private schedule(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.draw();
    });
  }

  private draw(): void {
    const { ctx, palette: p, ch } = this;
    ctx.fillStyle = p.bg;
    ctx.fillRect(0, 0, this.viewW, this.viewH);
    const m = this.model;
    if (!m) return;

    ctx.font = `${FONT_PX}px ${MONO}`;
    ctx.textBaseline = "middle";

    const bytes = m.bytes;
    const top = this.contentTop();
    const firstRow = Math.max(0, Math.floor((top - PAD_Y) / ROW_H));
    const yBase = PAD_Y + firstRow * ROW_H - top;
    const visible = Math.ceil(this.viewH / ROW_H) + 1;

    const hs = this.hover >= 0 ? m.start(this.hover) : -1;
    const he = this.hover >= 0 ? m.end(this.hover) : -1;
    const ss = this.selected >= 0 ? m.start(this.selected) : -1;
    const se = this.selected >= 0 ? m.end(this.selected) : -1;

    const ids = new Int32Array(BYTES_PER_ROW);
    const levels = new Uint8Array(BYTES_PER_ROW);

    for (let r = 0; r < visible; r++) {
      const row = firstRow + r;
      if (row >= this.rows) break;
      const y = yBase + r * ROW_H;
      const cy = y + ROW_H / 2;
      const base = row * BYTES_PER_ROW;
      const count = Math.min(BYTES_PER_ROW, bytes.length - base);

      ctx.fillStyle = p.offset;
      ctx.fillText(base.toString(16).padStart(8, "0").toUpperCase(), PAD_X, cy);

      // Pass 1: owner and emphasis for each byte in the row.
      for (let i = 0; i < count; i++) {
        const off = base + i;
        ids[i] = m.nodeAt(off);
        levels[i] = off >= hs && off < he ? 2 : off >= ss && off < se ? 1 : 0;
      }

      // Pass 2: backgrounds. Cells are padded to touch their neighbours so a
      // field reads as one continuous run rather than a row of chips.
      for (let i = 0; i < count; i++) {
        const f = p.fill[m.tint(ids[i])];
        ctx.fillStyle = levels[i] === 2 ? f.hover : levels[i] === 1 ? f.selected : f.base;
        const bridge =
          i === 7 && i + 1 < count && ids[i + 1] === ids[i] && levels[i + 1] === levels[i] ? ch : 0;
        ctx.fillRect(this.hexX(i) - ch * 0.5, y + 1, ch * 3 + bridge, ROW_H - 2);
        ctx.fillRect(this.asciiX(i), y + 1, ch, ROW_H - 2);
      }

      // Pass 3: glyphs, with a mark under any byte a problem node owns.
      for (let i = 0; i < count; i++) {
        const b = bytes[base + i];
        const hx = this.hexX(i);
        ctx.fillStyle = b === 0 ? p.zero : p.text;
        ctx.fillText(HEX[b], hx, cy);
        const printable = b >= 0x20 && b < 0x7f;
        ctx.fillStyle = printable ? p.text : p.zero;
        ctx.fillText(printable ? String.fromCharCode(b) : "·", this.asciiX(i), cy);

        if (m.kind(ids[i]) >= Kind.Warning) {
          ctx.fillStyle = p.fill[m.tint(ids[i])].solid;
          ctx.fillRect(hx - ch * 0.25, y + ROW_H - 4, ch * 2.5, 2);
        }
      }

      // Pass 4: outline the selected node's bytes on this row.
      if (se > base && ss < base + count) {
        const a = Math.max(ss, base) - base;
        const b = Math.min(se, base + count) - base - 1;
        ctx.strokeStyle = p.fill[m.tint(this.selected)].solid;
        ctx.lineWidth = 1.5;
        const x0 = this.hexX(a) - ch * 0.5;
        const x1 = this.hexX(b) + ch * 2.5;
        ctx.strokeRect(x0 + 0.75, y + 1.75, x1 - x0 - 1.5, ROW_H - 3.5);
        const ax0 = this.asciiX(a);
        const ax1 = this.asciiX(b) + ch;
        ctx.strokeRect(ax0 + 0.75, y + 1.75, ax1 - ax0 - 1.5, ROW_H - 3.5);
      }
    }
  }
}
