import { HEX, MONO, themeColors } from "./canvas";
import { CodesView, readExplained, readTables, type BlockTables } from "./codes";
import { BLOCK_NAMES, STRIDE, StepKind, type Step } from "./deflate";
import type { FileModel } from "./model";

/** Steps fetched per request. */
const BATCH = 256;
const FONT_PX = 13;


export interface PlayerSource {
  steps(from: number, count: number): Promise<Float64Array>;
  inflated(): Promise<Uint8Array>;
  /** Step `index` part by part; the block's tables unless `knownBlock` is its block. */
  explain(index: number, knownBlock: number): Promise<{ parts: Float64Array; tables: Float64Array | null }>;
}

export interface PlayerCallbacks {
  /** File bytes holding the current step's bits, `[start, end)`. */
  onHead(start: number, end: number, follow: boolean): void;
  onClose(): void;
}

const ICONS = {
  start: '<svg viewBox="0 0 16 16"><path d="M3 3h2v10H3zM13 3v10L6 8z"/></svg>',
  prev: '<svg viewBox="0 0 16 16"><path d="M10 3v10L3 8zM11 3h2v10h-2z"/></svg>',
  play: '<svg viewBox="0 0 16 16"><path d="M4 2.5 13.5 8 4 13.5z"/></svg>',
  pause: '<svg viewBox="0 0 16 16"><path d="M4 3h3v10H4zM9 3h3v10H9z"/></svg>',
  next: '<svg viewBox="0 0 16 16"><path d="M6 3v10l7-5zM3 3h2v10H3z"/></svg>',
  close: '<svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
};

const fmt = (n: number) => n.toLocaleString();
const speedFromSlider = (v: number) => Math.max(1, Math.round(10 ** ((v / 100) * Math.log10(5000))));
const sliderFromSpeed = (s: number) => (Math.log10(s) / Math.log10(5000)) * 100;

interface StripPalette {
  cell: string;
  text: string;
  faint: string;
  literal: string;
  literalFill: string;
  match: string;
  matchFill: string;
  sourceFill: string;
  error: string;
}

function readStripPalette(): StripPalette {
  const { v, rgba } = themeColors();
  const lit = v("--tint-sig");
  const mat = v("--tint-idat");
  return {
    cell: rgba(v("--tint-anc"), 0.1),
    text: v("--hex-text"),
    faint: v("--hex-offset"),
    literal: lit,
    literalFill: rgba(lit, 0.32),
    match: mat,
    matchFill: rgba(mat, 0.34),
    sourceFill: rgba(mat, 0.14),
    error: v("--tint-error"),
  };
}

/** What a step wrote, and for a back-reference where it copied from. */
interface Span {
  outEnd: number;
  isMatch: boolean;
  /** The copy overlaps itself: it repeats the last `distance` bytes. */
  overlap: boolean;
  srcStart: number;
  srcEnd: number;
}

function spanOf(step: Step): Span {
  const isMatch = step.kind === StepKind.Match;
  const produced = step.kind === StepKind.Literal ? 1 : isMatch ? step.b : 0;
  // A copy shorter-sourced than it is long overlaps itself. Only that period
  // is really "the source".
  const overlap = isMatch && step.a < step.b;
  const srcStart = isMatch ? step.outStart - step.a : -1;
  return {
    outEnd: step.outStart + produced,
    isMatch,
    overlap,
    srcStart,
    srcEnd: isMatch ? (overlap ? step.outStart : srcStart + step.b) : -1,
  };
}

/** A run of output offsets `[from, to)` drawn from x onwards. */
interface Seg {
  from: number;
  to: number;
  x: number;
}

/**
 * Which output bytes the strip shows: one window, or source | gap |
 * destination when the source is further back than the strip is wide.
 */
function stripLayout(step: Step, span: Span, capacity: number, cellW: number, padX: number) {
  const { outEnd, isMatch, overlap, srcStart, srcEnd } = span;
  const segs: Seg[] = [];
  let gap: { x: number; skipped: number } | null = null;
  const patternFrom = Math.max(0, srcStart - 2);
  if (overlap && step.outStart < patternFrom + capacity) {
    // The pattern, then as much of the repetition as fits.
    const from = patternFrom;
    segs.push({ from, to: Math.min(outEnd, from + capacity), x: padX });
  } else if (!isMatch || srcStart >= outEnd - capacity) {
    const from = Math.max(0, outEnd - capacity);
    segs.push({ from, to: outEnd, x: padX + (capacity - (outEnd - from)) * cellW });
  } else {
    const leftCells = Math.min(srcEnd - srcStart, Math.floor(capacity * 0.34));
    const gapCells = 3;
    const rightCells = capacity - leftCells - gapCells;
    // Show where the copy lands, with a little context before it.
    const rightFrom = Math.max(srcStart + leftCells, step.outStart - 2);
    segs.push({ from: srcStart, to: srcStart + leftCells, x: padX });
    gap = { x: padX + leftCells * cellW, skipped: rightFrom - (srcStart + leftCells) };
    segs.push({
      from: rightFrom,
      to: Math.min(outEnd, rightFrom + rightCells),
      x: padX + (leftCells + gapCells) * cellW,
    });
  }
  return { segs, gap };
}

/** A small filled arrowhead with its tip at (x, y), pointing down. */
function arrowDown(ctx: CanvasRenderingContext2D, x: number, y: number): void {
  ctx.beginPath();
  ctx.moveTo(x, y);
  ctx.lineTo(x - 5, y - 8);
  ctx.lineTo(x + 5, y - 8);
  ctx.closePath();
  ctx.fill();
}

/**
 * Steps through the DEFLATE stream one decoding decision at a time: which
 * bits were read, and which bytes they produced. Steps arrive from the worker
 * in batches on demand, so a file with millions of steps costs no more memory
 * than one with fifty.
 */
export class Player {
  readonly root: HTMLElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly sentence: HTMLElement;
  private readonly meta: HTMLElement;
  private readonly counter: HTMLElement;
  private readonly badge: HTMLElement;
  private readonly playBtn: HTMLButtonElement;
  private readonly speedInput: HTMLInputElement;
  private readonly speedOut: HTMLOutputElement;
  private readonly progress: HTMLElement;
  private readonly progressFill: HTMLElement;

  private model: FileModel | null = null;
  private source: PlayerSource | null = null;
  private output: Uint8Array = new Uint8Array(0);
  private total = 0;
  private index = 0;
  private batches = new Map<number, Float64Array>();
  private pending = new Set<number>();
  private generation = 0;
  private playing = false;
  private speed = 6;
  private carry = 0;
  private lastTick = 0;
  private block = -1;
  private palette = readStripPalette();
  private viewW = 0;
  private ch = 8;
  private readonly codes = new CodesView();
  /** The tables of the block last explained; the worker resends them only on a new block. */
  private tables: { block: number; tables: BlockTables | null } = { block: -1, tables: null };
  private explaining = false;
  private wanted = -1;

  constructor(
    host: HTMLElement,
    private readonly cb: PlayerCallbacks,
  ) {
    this.root = document.createElement("section");
    this.root.className = "drawer-player";
    this.root.hidden = true;
    this.root.innerHTML = `
      <div class="player-bar">
        <div class="player-buttons">
          <button data-act="start" title="Back to the start (Home)">${ICONS.start}</button>
          <button data-act="prev" title="Step back (←)">${ICONS.prev}</button>
          <button data-act="play" class="is-play" title="Play or pause (Space)">${ICONS.play}</button>
          <button data-act="next" title="Step forward (→)">${ICONS.next}</button>
        </div>
        <label class="player-speed">Speed
          <input type="range" min="0" max="100" step="1" />
          <output></output>
        </label>
        <span class="player-counter"></span>
        <span class="player-badge"></span>
        <button data-act="close" class="player-close" title="Close (Esc)">${ICONS.close}</button>
      </div>
      <p class="player-sentence"></p>
      <p class="player-meta"></p>
      <div class="player-stage"><canvas class="player-strip"></canvas></div>
      <div class="player-progress" title="Click to jump"><div></div></div>`;
    host.append(this.root);
    this.root.querySelector(".player-meta")!.after(this.codes.bits);
    this.root.querySelector(".player-stage")!.append(this.codes.panel);

    const q = <T extends Element>(sel: string) => this.root.querySelector(sel) as T;
    this.canvas = q("canvas");
    const ctx = this.canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas 2D is not available");
    this.ctx = ctx;
    this.sentence = q(".player-sentence");
    this.meta = q(".player-meta");
    this.counter = q(".player-counter");
    this.badge = q(".player-badge");
    this.playBtn = q('[data-act="play"]');
    this.speedInput = q("input");
    this.speedOut = q("output");
    this.progress = q(".player-progress");
    this.progressFill = q(".player-progress div");

    this.speedInput.value = String(sliderFromSpeed(this.speed));
    this.showSpeed();
    this.speedInput.addEventListener("input", () => {
      this.speed = speedFromSlider(Number(this.speedInput.value));
      this.showSpeed();
    });
    this.root.querySelector(".player-buttons")!.addEventListener("click", (e) => {
      const act = (e.target as Element).closest<HTMLElement>("[data-act]")?.dataset.act;
      if (act === "start") this.seek(0);
      if (act === "prev") this.stepBy(-1);
      if (act === "next") this.stepBy(1);
      if (act === "play") this.toggle();
    });
    q('[data-act="close"]').addEventListener("click", () => this.cb.onClose());
    this.progress.addEventListener("click", (e) => {
      const r = this.progress.getBoundingClientRect();
      this.seek(Math.round(((e.clientX - r.left) / r.width) * (this.total - 1)));
    });
    new ResizeObserver(() => this.resize()).observe(this.canvas);
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      this.palette = readStripPalette();
      this.draw();
    });
  }

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  async open(model: FileModel, source: PlayerSource): Promise<void> {
    this.close();
    const gen = ++this.generation;
    this.model = model;
    this.source = source;
    this.total = model.file.trace?.[0] ?? 0;
    this.index = 0;
    this.block = -1;
    this.root.hidden = false;
    this.sentence.textContent = "Loading the stream…";
    this.meta.textContent = "";

    const output = await source.inflated();
    if (gen !== this.generation) return;
    this.output = output;
    this.resize();
    this.render();
  }

  close(): void {
    this.pause();
    this.generation++;
    this.batches.clear();
    this.pending.clear();
    this.output = new Uint8Array(0);
    this.model = null;
    this.source = null;
    this.codes.clear();
    this.tables = { block: -1, tables: null };
    this.wanted = -1;
    this.root.hidden = true;
  }

  /** Keyboard control while open. Returns true when the key was handled. */
  handleKey(e: KeyboardEvent): boolean {
    if (!this.isOpen) return false;
    switch (e.key) {
      case " ":
        this.toggle();
        return true;
      case "ArrowRight":
        this.stepBy(e.shiftKey ? 10 : 1);
        return true;
      case "ArrowLeft":
        this.stepBy(e.shiftKey ? -10 : -1);
        return true;
      case "Home":
        this.seek(0);
        return true;
      case "End":
        this.seek(this.total - 1);
        return true;
    }
    return false;
  }

  // --- transport ----------------------------------------------------------

  private toggle(): void {
    if (this.playing) this.pause();
    else this.play();
  }

  private play(): void {
    if (this.total === 0) return;
    if (this.index >= this.total - 1) this.index = 0;
    this.playing = true;
    this.carry = 0;
    this.lastTick = performance.now();
    this.playBtn.innerHTML = ICONS.pause;
    requestAnimationFrame(this.tick);
  }

  private pause(): void {
    this.playing = false;
    this.playBtn.innerHTML = ICONS.play;
  }

  private stepBy(delta: number): void {
    this.pause();
    this.seek(this.index + delta);
  }

  private seek(index: number): void {
    const next = Math.max(0, Math.min(this.total - 1, index));
    if (Math.abs(next - this.index) > 1) this.block = -1;
    this.index = next;
    this.render();
  }

  private tick = (now: number): void => {
    if (!this.playing) return;
    // Cap the backlog so returning to a background tab does not jump ahead.
    this.carry = Math.min(this.carry + ((now - this.lastTick) / 1000) * this.speed, this.speed / 4 + 1);
    this.lastTick = now;

    const advance = Math.floor(this.carry);
    if (advance > 0) {
      const target = Math.min(this.index + advance, this.total - 1);
      // Only move once the destination step has arrived; otherwise wait a frame.
      if (this.stepAt(target)) {
        this.carry -= advance;
        this.index = target;
        this.render();
      }
    }
    if (this.index >= this.total - 1) this.pause();
    else requestAnimationFrame(this.tick);
  };

  // --- data -----------------------------------------------------------------

  /** The step at `i`, or null while its batch is still being fetched. */
  private stepAt(i: number): Step | null {
    const b = Math.floor(i / BATCH);
    const batch = this.batches.get(b);
    // Keep the next batch coming before playback reaches it.
    if (i % BATCH > BATCH - 64) this.fetch(b + 1);
    if (!batch) {
      this.fetch(b);
      return null;
    }
    const o = (i - b * BATCH) * STRIDE;
    if (o >= batch.length) return null;
    return {
      index: i,
      kind: batch[o],
      a: batch[o + 1],
      b: batch[o + 2],
      bitStart: batch[o + 3],
      bitEnd: batch[o + 4],
      outStart: batch[o + 5],
    };
  }

  private fetch(b: number): void {
    const source = this.source;
    if (!source || this.batches.has(b) || this.pending.has(b) || b * BATCH >= this.total + 1) return;
    this.pending.add(b);
    const gen = this.generation;
    void source.steps(b * BATCH, BATCH).then((steps) => {
      if (gen !== this.generation) return;
      this.pending.delete(b);
      this.batches.set(b, steps);
      // A damaged stream ends with a failure step the trace did not count.
      this.total = Math.max(this.total, b * BATCH + steps.length / STRIDE);
      // Keep memory bounded when scrubbing through a huge stream.
      if (this.batches.size > 64) {
        for (const key of this.batches.keys()) {
          if (Math.abs(key - Math.floor(this.index / BATCH)) > 8) this.batches.delete(key);
        }
      }
      if (Math.floor(this.index / BATCH) === b) this.render();
    });
  }

  /**
   * Asks for the breakdown of the step on screen. One request is in flight at
   * a time; when it returns and the player has moved on, the newest step is
   * asked for next, so fast playback costs one request per round trip.
   */
  private explain(step: Step): void {
    this.wanted = step.index;
    const source = this.source;
    if (this.explaining || !source) return;
    this.explaining = true;
    const gen = this.generation;
    void source
      .explain(step.index, this.tables.block)
      .then(({ parts, tables }) => {
        if (gen !== this.generation) return;
        const ex = readExplained(parts);
        if (ex && tables) this.tables = { block: ex.blockStart, tables: readTables(tables) };
        if (this.wanted === step.index) {
          const current = ex && ex.blockStart === this.tables.block ? this.tables.tables : null;
          this.codes.show(step, ex, current);
        }
      })
      .finally(() => {
        this.explaining = false;
        if (gen !== this.generation || this.wanted === step.index) return;
        const next = this.stepAt(this.wanted);
        if (next) this.explain(next);
      });
  }

  // --- rendering --------------------------------------------------------------

  private showSpeed(): void {
    this.speedOut.textContent = `${fmt(this.speed)} step${this.speed === 1 ? "" : "s"}/s`;
  }

  private render(): void {
    const m = this.model;
    if (!m) return;
    this.counter.textContent = `Step ${fmt(this.index + 1)} of ${fmt(this.total)}`;
    this.progressFill.style.width = `${this.total > 1 ? (this.index / (this.total - 1)) * 100 : 0}%`;

    const step = this.stepAt(this.index);
    if (!step) {
      this.meta.textContent = "Fetching steps…";
      return;
    }
    if (step.kind === StepKind.BlockStart) this.block = step.a;
    this.badge.textContent = this.block >= 0 ? `${BLOCK_NAMES[this.block]} block` : "";
    this.badge.hidden = this.block < 0;

    this.sentence.replaceChildren(...this.describe(step));
    const bits = step.bitEnd - step.bitStart;
    const bitsText = bits === 1 ? "1 bit" : `${fmt(bits)} bits`;
    this.meta.textContent =
      step.kind === StepKind.Failure
        ? `Stopped at compressed bit ${fmt(step.bitStart)} · output so far ${fmt(step.outStart)} bytes`
        : `Read ${bitsText} (bits ${fmt(step.bitStart)}–${fmt(Math.max(step.bitStart, step.bitEnd - 1))}) · output byte ${fmt(step.outStart)}`;

    const [start, end] = m.bitsToFile(step.bitStart, Math.max(step.bitEnd, step.bitStart + 1));
    this.cb.onHead(start, end, true);
    this.draw();
    this.explain(step);
  }

  private describe(step: Step): Node[] {
    const strong = (t: string, cls = "") => {
      const el = document.createElement("strong");
      el.textContent = t;
      if (cls) el.className = cls;
      return el;
    };
    const text = (t: string) => document.createTextNode(t);

    switch (step.kind) {
      case StepKind.BlockStart:
        return [
          strong("New block", "is-block"),
          text(
            step.a === 0
              ? " — stored: the bytes that follow are copied in as they are."
              : step.a === 1
                ? " — fixed Huffman: codes come from the table built into the format."
                : ` — dynamic Huffman: this block ships its own code tables, ${fmt(step.bitEnd - step.bitStart)} bits of them.`,
          ),
        ];
      case StepKind.Literal: {
        const printable = step.a >= 0x20 && step.a < 0x7f;
        return [
          strong("Literal", "is-literal"),
          text(` — write byte 0x${HEX[step.a]}${printable ? ` '${String.fromCharCode(step.a)}'` : ""} as it is.`),
        ];
      }
      case StepKind.Match: {
        const overlap = step.a < step.b;
        return [
          strong("Back-reference", "is-match"),
          text(` — copy ${fmt(step.b)} bytes from ${fmt(step.a)} bytes back.`),
          ...(overlap
            ? [text(" The copy overlaps what it is writing, so it repeats a pattern — this is how long runs compress to a few bits.")]
            : []),
        ];
      }
      case StepKind.BlockEnd:
        return [strong("End of block", "is-block"), text(" — an end-of-block code closes it.")];
      default:
        return [
          strong("Decoding stops here", "is-failure"),
          text(" — the compressed data is damaged. Everything before this point decoded; nothing after it can."),
        ];
    }
  }

  private resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = this.canvas.clientWidth;
    const h = this.canvas.clientHeight;
    this.viewW = w;
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(h * dpr);
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.ctx.font = `${FONT_PX}px ${MONO}`;
    this.ch = this.ctx.measureText("0").width;
    this.draw();
  }

  /**
   * The output strip: the most recent output bytes, the ones this step wrote
   * highlighted, and for a back-reference an arc from where the bytes are
   * copied from to where they land. When the source is further back than the
   * strip is wide, the strip breaks and says how far.
   */
  private draw(): void {
    const { ctx, palette: p, ch } = this;
    const h = this.canvas.clientHeight;
    ctx.clearRect(0, 0, this.viewW, h);
    const step = this.model ? this.stepAt(this.index) : null;
    if (!step) return;

    ctx.font = `${FONT_PX}px ${MONO}`;
    ctx.textBaseline = "middle";
    ctx.textAlign = "left";

    const cellW = ch * 3;
    const cellH = 26;
    const top = h - cellH - 18;
    const padX = 16;
    const capacity = Math.max(8, Math.floor((this.viewW - padX * 2) / cellW));

    const span = spanOf(step);
    const { outEnd, isMatch, overlap, srcStart, srcEnd } = span;
    const { segs, gap } = stripLayout(step, span, capacity, cellW, padX);

    const segOf = (offset: number) => segs.find((s) => offset >= s.from && offset < s.to);
    const cellX = (offset: number): number | null => {
      const seg = segOf(offset);
      return seg ? seg.x + (offset - seg.from) * cellW : null;
    };
    /** Last offset of `[a, b)` that is actually drawn, within a's segment. */
    const lastShown = (a: number, b: number): number => {
      const seg = segOf(a);
      return seg ? Math.min(b, seg.to) - 1 : a;
    };

    if (outEnd === 0) {
      ctx.fillStyle = p.faint;
      ctx.fillText("Nothing decoded yet — the output starts here.", padX, top + cellH / 2);
    }

    for (const s of segs) {
      for (let off = s.from; off < s.to; off++) {
        const x = s.x + (off - s.from) * cellW;
        const isNew = off >= step.outStart && off < outEnd;
        const isSrc = isMatch && off >= srcStart && off < srcEnd;
        ctx.fillStyle = isNew
          ? isMatch
            ? p.matchFill
            : p.literalFill
          : isSrc
            ? p.sourceFill
            : p.cell;
        ctx.fillRect(x + 1, top, cellW - 2, cellH);
        ctx.fillStyle = isNew || isSrc ? p.text : p.faint;
        ctx.fillText(HEX[this.output[off] ?? 0], x + ch * 0.5, top + cellH / 2);
      }
    }

    // A copy longer than the strip: say how much more it writes.
    const lastSeg = segs[segs.length - 1];
    if (isMatch && lastSeg && lastSeg.to < outEnd) {
      const x = lastSeg.x + (lastSeg.to - lastSeg.from) * cellW;
      ctx.fillStyle = p.match;
      ctx.fillText(`+${fmt(outEnd - lastSeg.to)}`, x + 6, top + cellH / 2);
    }

    if (gap) {
      ctx.fillStyle = p.faint;
      ctx.textAlign = "center";
      ctx.fillText("⋯", gap.x + cellW * 1.5, top + cellH / 2);
      ctx.fillText(`${fmt(gap.skipped)} bytes`, gap.x + cellW * 1.5, top + cellH + 11);
      ctx.textAlign = "left";
    }

    // Output offset under the first cell of each segment, for orientation.
    ctx.fillStyle = p.faint;
    ctx.font = `11px ${MONO}`;
    for (const s of segs) if (s.to > s.from) ctx.fillText(fmt(s.from), s.x + 2, top + cellH + 11);
    ctx.font = `${FONT_PX}px ${MONO}`;

    if (isMatch) {
      // Arc from the middle of the (visible) source to the middle of the
      // (visible) start of the copy.
      const sx0 = cellX(srcStart);
      const dx0 = cellX(step.outStart);
      if (sx0 !== null && dx0 !== null) {
        const sLast = cellX(lastShown(srcStart, srcEnd)) ?? sx0;
        const dLast = cellX(lastShown(step.outStart, Math.min(outEnd, step.outStart + Math.max(step.a, 1)))) ?? dx0;
        const sx = (sx0 + sLast + cellW) / 2;
        const dx = (dx0 + dLast + cellW) / 2;
        const lift = Math.min(top - 22, 20 + Math.abs(dx - sx) * 0.16);
        const peak = top - lift;
        ctx.strokeStyle = p.match;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(sx, top - 3);
        ctx.quadraticCurveTo((sx + dx) / 2, peak - lift * 0.35, dx, top - 3);
        ctx.stroke();
        ctx.fillStyle = p.match;
        arrowDown(ctx, dx, top - 1);
        // Outline the source so the eye finds both ends.
        ctx.lineWidth = 1.5;
        ctx.strokeRect(sx0 + 1.5, top + 0.75, sLast - sx0 + cellW - 3, cellH - 1.5);

        ctx.textAlign = "center";
        ctx.font = `600 12px ${MONO}`;
        const label = overlap
          ? `repeat ${fmt(step.a)}-byte pattern → ${fmt(step.b)} bytes`
          : `copy ${fmt(step.b)} ← ${fmt(step.a)} back`;
        ctx.fillText(label, (sx + dx) / 2, Math.max(10, peak - lift * 0.2 - 8));
        ctx.textAlign = "left";
        ctx.font = `${FONT_PX}px ${MONO}`;
      }
    } else if (step.kind === StepKind.Literal) {
      const x = cellX(step.outStart);
      if (x !== null) {
        const cx = x + cellW / 2;
        ctx.strokeStyle = p.literal;
        ctx.fillStyle = p.literal;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(cx, top - 26);
        ctx.lineTo(cx, top - 4);
        ctx.stroke();
        arrowDown(ctx, cx, top - 1);
      }
    } else if (step.kind === StepKind.Failure) {
      const x = padX + capacity * cellW;
      ctx.fillStyle = p.error;
      ctx.fillRect(Math.min(x, this.viewW - 6), top - 6, 3, cellH + 12);
    }
  }
}
