import { themeColors } from "./canvas";
import { FileModel } from "./model";

export interface MinimapCallbacks {
  /** Scroll the hex view to this byte. */
  onJump(offset: number): void;
}

/** Colour stops over 0–8 bits per byte: padding, text and structure, then compressed or encrypted. */
const STOPS: [number, string][] = [
  [0, "--border"],
  [3, "--tint-ihdr"],
  [6, "--tint-text"],
  [7.3, "--tint-idat"],
  [8, "--tint-error"],
];

/** What an entropy value says about the bytes, in words. */
export function entropyMeaning(h: number): string {
  if (h < 1) return "zeros or repetition";
  if (h < 5) return "text or structure";
  if (h < 7.3) return "mixed data";
  return "compressed or encrypted";
}

function mix(a: [number, number, number], b: [number, number, number], t: number): string {
  const c = a.map((x, i) => Math.round(x + (b[i] - x) * t));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", "").slice(0, 6), 16) || 0;
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/**
 * The whole file in one column beside the hex view: each pixel row coloured
 * by how random its bytes are, problems marked, the part on screen outlined.
 * Clicking or dragging moves the hex view there.
 */
export class Minimap {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private model: FileModel | null = null;
  private view: [number, number] = [0, 0];
  private selected = -1;
  private ramp: { at: number; color: [number, number, number] }[] = [];
  private marks = { problem: "#e5383b", selection: "#2563eb", outline: "#1c1c1e" };
  private frame = 0;
  private dragging = false;

  constructor(
    host: HTMLElement,
    private readonly cb: MinimapCallbacks,
  ) {
    this.canvas = document.createElement("canvas");
    this.canvas.className = "minimap";
    this.canvas.setAttribute("role", "scrollbar");
    this.canvas.setAttribute("aria-label", "Whole-file map by entropy: click to jump");
    host.append(this.canvas);
    const ctx = this.canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas 2D is not available");
    this.ctx = ctx;
    this.readTheme();

    new ResizeObserver(() => this.schedule()).observe(this.canvas);
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      this.readTheme();
      this.schedule();
    });
    this.canvas.addEventListener("pointerdown", (e) => {
      this.dragging = true;
      this.canvas.setPointerCapture(e.pointerId);
      this.jump(e);
    });
    this.canvas.addEventListener("pointermove", (e) => {
      if (this.dragging) this.jump(e);
      this.canvas.title = this.describe(this.offsetAt(e.offsetY));
    });
    const stop = () => (this.dragging = false);
    this.canvas.addEventListener("pointerup", stop);
    this.canvas.addEventListener("pointercancel", stop);
  }

  setModel(m: FileModel | null): void {
    this.model = m;
    this.selected = -1;
    this.canvas.hidden = !m || m.bytes.length === 0;
    this.schedule();
  }

  setView(start: number, end: number): void {
    if (start === this.view[0] && end === this.view[1]) return;
    this.view = [start, end];
    this.schedule();
  }

  setSelected(id: number): void {
    this.selected = id;
    this.schedule();
  }

  private readTheme(): void {
    const { v } = themeColors();
    this.ramp = STOPS.map(([at, name]) => ({ at, color: rgb(v(name)) }));
    this.marks = { problem: v("--tint-error"), selection: v("--accent"), outline: v("--text") };
  }

  private colorFor(h: number): string {
    const r = this.ramp;
    for (let i = 1; i < r.length; i++) {
      if (h <= r[i].at) return mix(r[i - 1].color, r[i].color, (h - r[i - 1].at) / (r[i].at - r[i - 1].at));
    }
    return mix(r[r.length - 1].color, r[r.length - 1].color, 0);
  }

  private offsetAt(y: number): number {
    const m = this.model;
    if (!m) return 0;
    const h = this.canvas.clientHeight || 1;
    return Math.min(m.bytes.length - 1, Math.max(0, Math.floor((y / h) * m.bytes.length)));
  }

  private jump(e: PointerEvent): void {
    this.cb.onJump(this.offsetAt(e.offsetY));
  }

  /** The tooltip: where, how random, and what is there. */
  private describe(offset: number): string {
    const m = this.model;
    if (!m) return "";
    const f = m.file;
    const bin = f.entropyWindow > 0 ? Math.floor(offset / f.entropyWindow) : -1;
    const h = f.entropy[bin];
    const node = m.nodeAt(offset);
    const where = m
      .path(node)
      .slice(1)
      .map((id) => m.label(id))
      .join(" › ");
    const lines = [`0x${offset.toString(16).toUpperCase()}`];
    if (h !== undefined) lines.push(`${h.toFixed(2)} bits/byte: ${entropyMeaning(h)}`);
    if (where) lines.push(where);
    return lines.join("\n");
  }

  private schedule(): void {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.draw();
    });
  }

  private draw(): void {
    const m = this.model;
    const dpr = window.devicePixelRatio || 1;
    const w = this.canvas.clientWidth;
    const h = this.canvas.clientHeight;
    if (!m || w === 0 || h === 0) return;
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(h * dpr);
    const ctx = this.ctx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const len = m.bytes.length;
    const bins = m.file.entropy;
    const win = m.file.entropyWindow;
    const y = (offset: number) => (offset / len) * h;

    // One band per pixel row, from the mean of the bins it covers.
    if (bins.length > 0 && win > 0) {
      for (let row = 0; row < h; row++) {
        const a = Math.floor(((row / h) * len) / win);
        const b = Math.max(a + 1, Math.ceil((((row + 1) / h) * len) / win));
        let sum = 0;
        let n = 0;
        for (let i = a; i < Math.min(b, bins.length); i++) {
          sum += bins[i];
          n++;
        }
        if (n === 0) continue;
        ctx.fillStyle = this.colorFor(sum / n);
        ctx.fillRect(0, row, w, 1);
      }
    }

    // Problems, then the selection, as ticks on the right edge.
    ctx.fillStyle = this.marks.problem;
    for (const id of m.problems) ctx.fillRect(w - 5, Math.min(h - 2, y(m.start(id))), 5, 2);
    if (this.selected >= 0) {
      ctx.fillStyle = this.marks.selection;
      ctx.fillRect(0, Math.min(h - 2, y(m.start(this.selected))), w, 2);
    }

    // What the hex view shows right now.
    const top = y(this.view[0]);
    const height = Math.max(3, y(this.view[1]) - top);
    ctx.strokeStyle = this.marks.outline;
    ctx.globalAlpha = 0.85;
    ctx.lineWidth = 1.5;
    ctx.strokeRect(0.75, top + 0.75, w - 1.5, Math.min(height, h - top) - 1.5);
    ctx.globalAlpha = 1;
  }
}
