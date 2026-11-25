import { themeColors } from "./canvas";
import { onThemeChange } from "./theme";
import { FileModel } from "./model";
import { currentLocale, translateText } from "./i18n";

export interface MinimapCallbacks {
  /** Scroll the hex view to this byte. */
  onJump(offset: number): void;
  /** Resume a byte map whose background calculation was cancelled or failed. */
  onBuild?(): void;
}

export type MinimapState = "building" | "ready" | "cancelled" | "failed";

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
 * Clicking, dragging or keyboard scrolling moves the hex view there.
 */
export class Minimap {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly build: HTMLButtonElement;
  private model: FileModel | null = null;
  private state: MinimapState = "ready";
  private requestedBuild = false;
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
    this.canvas.setAttribute("aria-label", "Whole-file map by entropy. Use arrows, Page Up/Down, Home or End to jump.");
    this.canvas.setAttribute("aria-orientation", "vertical");
    this.canvas.setAttribute("aria-valuemin", "0");
    this.canvas.setAttribute("aria-valuemax", "100");
    this.canvas.setAttribute("aria-valuenow", "0");
    this.canvas.tabIndex = -1;
    host.append(this.canvas);
    this.build = document.createElement("button");
    this.build.type = "button";
    this.build.className = "btn minimap-action";
    this.build.hidden = true;
    this.build.addEventListener("click", () => {
      this.requestedBuild = true;
      this.cb.onBuild?.();
    });
    this.build.addEventListener("blur", () => {
      if (this.state === "ready" && this.build.disabled) this.build.hidden = true;
    });
    host.append(this.build);
    const ctx = this.canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas 2D is not available");
    this.ctx = ctx;
    this.readTheme();
    window.addEventListener("hexscope:locale", () => {
      this.updateBuildLabel();
      this.updateAccessibleLabel();
      this.updateAccessibleOffset(this.view[0]);
    });

    new ResizeObserver(() => this.schedule()).observe(this.canvas);
    onThemeChange(() => {
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
    this.canvas.addEventListener("keydown", (e) => this.onKeyDown(e));
    const stop = () => (this.dragging = false);
    this.canvas.addEventListener("pointerup", stop);
    this.canvas.addEventListener("pointercancel", stop);
  }

  setModel(m: FileModel | null): void {
    this.model = m;
    this.requestedBuild = false;
    this.selected = -1;
    this.view = [0, 0];
    this.canvas.hidden = !m || m.size === 0;
    this.canvas.tabIndex = m && m.size > 0 ? 0 : -1;
    this.canvas.setAttribute("aria-disabled", String(!m || m.size === 0));
    this.updateAccessibleLabel();
    this.updateAccessibleOffset(0);
    this.setState(!m || m.size === 0 || m.file.entropy.length > 0 ? "ready" : "cancelled");
    this.schedule();
  }

  setState(state: MinimapState): void {
    const keepFocus = this.requestedBuild && this.build === document.activeElement;
    const restoreFocus = (state === "cancelled" && this.requestedBuild)
      || (state === "failed" && this.build === document.activeElement);
    this.state = state;
    const canBuild = state === "cancelled" || state === "failed";
    const hasModel = Boolean(this.model && this.model.size > 0 && this.cb.onBuild);
    this.build.hidden = !hasModel || (!canBuild && !keepFocus);
    this.build.disabled = state === "building" || (state === "ready" && keepFocus);
    this.updateBuildLabel();
    if (state !== "building") {
      this.requestedBuild = false;
      if (restoreFocus) this.build.focus();
    }
  }

  private updateBuildLabel(): void {
    const label = this.state === "building"
      ? "Building byte map…"
      : this.state === "ready"
        ? "Byte map ready"
        : this.state === "failed"
          ? "Try again"
          : "Build byte map";
    this.build.textContent = translateText(label, currentLocale());
  }

  private updateAccessibleLabel(): void {
    this.canvas.setAttribute(
      "aria-label",
      translateText("Whole-file map by entropy. Use arrows, Page Up/Down, Home or End to jump.", currentLocale()),
    );
  }

  private updateAccessibleOffset(offset: number): void {
    const len = this.model?.size ?? 0;
    const at = len > 0 ? Math.min(len - 1, Math.max(0, offset)) : 0;
    this.canvas.setAttribute("aria-valuenow", String(len > 0 ? Math.round((at / len) * 100) : 0));
    const address = `0x${at.toString(16).toUpperCase()}`;
    const locale = currentLocale();
    const parts = [`${translateText("Offset", locale)} ${address}`];
    const model = this.model;
    if (model && model.file.entropyWindow > 0) {
      const index = Math.floor(at / model.file.entropyWindow);
      const value = model.file.entropy[index];
      if (value !== undefined) {
        parts.push(`${value.toFixed(2)} ${translateText("bits per byte", locale)}: ${translateText(entropyMeaning(value), locale)}`);
      }
    }
    this.canvas.setAttribute("aria-valuetext", parts.join("; "));
  }

  private onKeyDown(event: KeyboardEvent): void {
    const m = this.model;
    if (!m || m.size === 0) return;
    const viewport = Math.max(1, this.view[1] - this.view[0]);
    const stride = Math.max(1, Math.ceil(viewport / 10));
    const current = Math.min(m.size - 1, Math.max(0, this.view[0]));
    const moves: Record<string, number> = {
      ArrowUp: -stride,
      ArrowDown: stride,
      PageUp: -viewport,
      PageDown: viewport,
      Home: -current,
      End: m.size - 1 - current,
    };
    if (!Object.prototype.hasOwnProperty.call(moves, event.key)) return;
    event.preventDefault();
    const target = Math.min(m.size - 1, Math.max(0, current + moves[event.key]));
    this.updateAccessibleOffset(target);
    this.cb.onJump(target);
  }

  refresh(): void {
    this.schedule();
  }

  setView(start: number, end: number): void {
    if (start === this.view[0] && end === this.view[1]) return;
    this.view = [start, end];
    this.updateAccessibleOffset(start);
    this.schedule();
  }

  setSelected(id: number): void {
    this.selected = id;
    this.schedule();
  }

  private readTheme(): void {
    const { color } = themeColors();
    this.ramp = STOPS.map(([at, name]) => ({ at, color: rgb(color(name)) }));
    this.marks = { problem: color("--tint-error"), selection: color("--accent"), outline: color("--text") };
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
    return Math.min(m.size - 1, Math.max(0, Math.floor((y / h) * m.size)));
  }

  private jump(e: PointerEvent): void {
    const offset = this.offsetAt(e.offsetY);
    this.updateAccessibleOffset(offset);
    this.cb.onJump(offset);
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

    const len = m.size;
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
