import { HEX, MONO, themeColors } from "./canvas";
import { FileModel, Kind, type Tint } from "./model";

export interface HexCallbacks {
  onHover(id: number, offset: number): void;
  onSelect(id: number): void;
}

const ROW_H = 22;
const FONT_PX = 13;
/** The smallest the text shrinks to, on a narrow phone. */
const MIN_FONT_PX = 10;
const PAD_X = 20;
const NARROW_PAD_X = 8;
/** How long a flash takes to fade, in milliseconds. */
const FLASH_MS = 700;
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
const PAD_Y = 14;
/** Browsers cap element height; past this the scrollbar maps proportionally. */
const MAX_SCROLL_PX = 8_000_000;
const TINTS: Tint[] = ["sig", "ihdr", "plte", "idat", "iend", "text", "anc", "gps", "warning", "error"];

/** Fill styles per tint at each emphasis level, precomputed once per theme. */
interface Palette {
  bg: string;
  text: string;
  zero: string;
  offset: string;
  head: string;
  headFill: string;
  fill: Record<Tint, { base: string; selected: string; hover: string; solid: string }>;
}

function readPalette(): Palette {
  const { v, rgba } = themeColors();
  const alpha = (name: string) => parseFloat(v(name));

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
  const accent = v("--accent");
  return {
    bg: v("--hex-bg"),
    text: v("--hex-text"),
    zero: v("--hex-zero"),
    offset: v("--hex-offset"),
    head: accent,
    headFill: rgba(accent, 0.22),
    fill,
  };
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
  /** Bytes flashing to show where something just led, and when it began. */
  private flashing: { start: number; end: number; t0: number } | null = null;
  /** Bytes the DEFLATE player is reading right now, as [start, end). */
  private head: [number, number] = [-1, -1];
  private frame = 0;
  private viewW = 0;
  private viewH = 0;
  private ch = 8;
  /** Font size and side padding: smaller on a screen too narrow for 8 bytes a row. */
  private fontPx = FONT_PX;
  private padX = PAD_X;
  /** 16 bytes per row, or 8 when 16 would not fit the width. */
  private perRow = 16;
  /** Hex digits in the offset column: enough for the file, at least six. */
  private offsetDigits = 8;
  /** Told the byte range on screen after every draw, e.g. by the minimap. */
  onView: ((start: number, end: number) => void) | null = null;

  constructor(
    host: HTMLElement,
    private readonly cb: HexCallbacks,
  ) {
    this.scroller = document.createElement("div");
    this.scroller.className = "hex-scroller";
    this.canvas = document.createElement("canvas");
    this.canvas.className = "hex-canvas";
    // The bytes as drawn; the tree beside them says the same in words.
    this.canvas.setAttribute("role", "img");
    this.canvas.setAttribute("aria-label", "The file's bytes, in hex and as text");
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
    this.head = [-1, -1];
    this.offsetDigits = Math.max(6, (model?.bytes.length ?? 0).toString(16).length);
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

  /** Marks the bytes the DEFLATE player is reading; `-1` clears it. */
  setHead(start: number, end: number): void {
    if (start === this.head[0] && end === this.head[1]) return;
    this.head = [start, end];
    this.schedule();
  }

  /**
   * Briefly lights up a node's bytes, so the eye finds what another view
   * pointed at. Nothing when motion is to be kept down: the outline stays.
   */
  flash(id: number): void {
    const m = this.model;
    if (!m || id < 0 || m.len(id) === 0 || reducedMotion.matches) return;
    this.flashing = { start: m.start(id), end: m.end(id), t0: performance.now() };
    this.schedule();
  }

  /** Brings a node into view unless it is already visible. */
  reveal(id: number): void {
    const m = this.model;
    if (!m || m.len(id) === 0) return;
    this.revealOffset(m.start(id), true);
  }

  /** Brings a byte into view unless it is already visible. */
  revealOffset(offset: number, smooth: boolean): void {
    if (!this.model || offset < 0) return;
    const row = Math.floor(offset / this.perRow);
    const top = this.contentTop();
    const rowY = PAD_Y + row * ROW_H;
    if (rowY >= top + ROW_H && rowY + ROW_H * 2 <= top + this.viewH) return;

    const target = Math.max(0, rowY - this.viewH / 3);
    const far = Math.abs(rowY - top) > this.viewH * 3;
    this.scroller.scrollTo({
      top: this.toScrollTop(target),
      behavior: smooth && !far ? "smooth" : "auto",
    });
  }

  /** Bytes on screen, as [start, end). */
  visibleRange(): [number, number] {
    const m = this.model;
    if (!m) return [0, 0];
    const top = this.contentTop();
    const first = Math.max(0, Math.floor((top - PAD_Y) / ROW_H));
    const last = Math.max(first, Math.ceil((top + this.viewH - PAD_Y) / ROW_H));
    return [Math.min(first * this.perRow, m.bytes.length), Math.min(last * this.perRow, m.bytes.length)];
  }

  /** Scrolls so that byte `offset` sits a third of the way down the view. */
  scrollToOffset(offset: number): void {
    if (!this.model) return;
    const rowY = PAD_Y + Math.floor(Math.max(0, offset) / this.perRow) * ROW_H;
    this.scroller.scrollTop = this.toScrollTop(Math.max(0, rowY - this.viewH / 3));
  }

  // --- geometry ---------------------------------------------------------

  private get rows(): number {
    return this.model ? Math.ceil(this.model.bytes.length / this.perRow) : 0;
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
    const base = this.padX + this.ch * (this.offsetDigits + 3);
    return base + i * this.ch * 3 + (i >= 8 ? this.ch : 0);
  }

  private asciiX(i: number): number {
    return this.hexX(this.perRow) + this.ch * 2 + i * this.ch;
  }

  /** Width the grid needs at a given number of bytes per row. */
  private widthFor(perRow: number): number {
    return this.padX * 2 + this.ch * this.columns(perRow);
  }

  /** Character columns a row takes: offset, hex, gap, text. */
  private columns(perRow: number): number {
    return this.offsetDigits + 3 + perRow * 3 + (perRow > 8 ? 1 : 0) + 2 + perRow;
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
    this.fontPx = FONT_PX;
    this.padX = PAD_X;
    this.ctx.font = `${this.fontPx}px ${MONO}`;
    this.ch = this.ctx.measureText("0").width;
    // Too narrow for 8 bytes a row: tighter margins, then smaller text, so
    // the text column is never cut off.
    if (this.viewW < this.widthFor(8)) {
      this.padX = NARROW_PAD_X;
      const fit = (this.viewW - this.padX * 2) / this.columns(8);
      this.fontPx = Math.max(MIN_FONT_PX, Math.floor((FONT_PX * fit) / this.ch * 10) / 10);
      this.ctx.font = `${this.fontPx}px ${MONO}`;
      this.ch = this.ctx.measureText("0").width;
    }
    const perRow = this.viewW >= this.widthFor(16) ? 16 : 8;
    // Keep the same byte at the top of the view if the row width changes.
    const keepByte =
      perRow !== this.perRow
        ? Math.max(0, Math.floor((this.contentTop() - PAD_Y) / ROW_H)) * this.perRow
        : -1;
    this.perRow = perRow;

    const virtualH = this.scaled ? MAX_SCROLL_PX : this.contentH;
    this.spacer.style.height = `${Math.max(0, virtualH - this.viewH)}px`;
    // Only now that the scroll height matches the new rows can the position
    // be restored without the browser clamping it.
    if (keepByte >= 0) {
      this.scroller.scrollTop = this.toScrollTop(PAD_Y + Math.floor(keepByte / perRow) * ROW_H);
    }
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
    for (let i = 0; i < this.perRow; i++) {
      const hx = this.hexX(i);
      const ax = this.asciiX(i);
      if ((x >= hx - this.ch * 0.5 && x < hx + this.ch * 2.5) || (x >= ax && x < ax + this.ch)) {
        col = i;
        break;
      }
    }

    const offset = col < 0 || row < 0 ? -1 : row * this.perRow + col;
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
    this.onView?.(...this.visibleRange());

    ctx.font = `${this.fontPx}px ${MONO}`;
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

    const ids = new Int32Array(this.perRow);
    const levels = new Uint8Array(this.perRow);

    for (let r = 0; r < visible; r++) {
      const row = firstRow + r;
      if (row >= this.rows) break;
      const y = yBase + r * ROW_H;
      const cy = y + ROW_H / 2;
      const base = row * this.perRow;
      const count = Math.min(this.perRow, bytes.length - base);

      ctx.fillStyle = p.offset;
      ctx.fillText(base.toString(16).padStart(this.offsetDigits, "0").toUpperCase(), this.padX, cy);

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

      // Pass 4b: a flash over bytes another view just pointed at, fading out.
      const fl = this.flashing;
      if (fl && fl.end > base && fl.start < base + count) {
        const t = (performance.now() - fl.t0) / FLASH_MS;
        const a = Math.max(fl.start, base) - base;
        const b = Math.min(fl.end, base + count) - base - 1;
        ctx.globalAlpha = Math.max(0, 1 - t) ** 2 * 0.4;
        ctx.fillStyle = p.head;
        const x0 = this.hexX(a) - ch * 0.5;
        const x1 = this.hexX(b) + ch * 2.5;
        ctx.fillRect(x0, y + 1, x1 - x0, ROW_H - 2);
        ctx.fillRect(this.asciiX(a), y + 1, this.asciiX(b) + ch - this.asciiX(a), ROW_H - 2);
        ctx.globalAlpha = 1;
      }

      // Pass 5: the player's reading head, drawn last so it sits on top.
      const [hs0, he0] = this.head;
      if (he0 > base && hs0 < base + count) {
        const a = Math.max(hs0, base) - base;
        const b = Math.min(he0, base + count) - base - 1;
        const x0 = this.hexX(a) - ch * 0.5;
        const x1 = this.hexX(b) + ch * 2.5;
        ctx.fillStyle = p.headFill;
        ctx.fillRect(x0, y + 1, x1 - x0, ROW_H - 2);
        ctx.strokeStyle = p.head;
        ctx.lineWidth = 2;
        ctx.strokeRect(x0 + 1, y + 2, x1 - x0 - 2, ROW_H - 4);
        for (let i = a; i <= b; i++) {
          const byte = bytes[base + i];
          ctx.fillStyle = p.text;
          ctx.fillText(HEX[byte], this.hexX(i), cy);
        }
      }
    }
    if (this.flashing) {
      if (performance.now() - this.flashing.t0 < FLASH_MS) this.schedule();
      else this.flashing = null;
    }
  }
}
