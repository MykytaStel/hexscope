// The landing page's demonstration: a pointer moving over a real PNG, and
// for each pixel, the bytes of the file that wrote it — what hexscope does,
// shown before anyone has to pick a file. It runs on the same worker as the
// app, so it only runs while no file is open, and stops when one is.
import { FileModel } from "./model";
import { geometry, pixelOffset, runs, type Geometry } from "./pixels";
import { call } from "./rpc";

/** Pixels the pointer visits in the sample, in order: rows that were copied, and ones spelled out. */
const PATH: [number, number][] = [
  [0, 0],
  [4, 1],
  [16, 19],
  [26, 9],
  [9, 27],
  [22, 4],
  [13, 14],
];
const DWELL_MS = 2400;
const SAMPLE = "samples/sample.png";

const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};
const hex = (n: number) => n.toString(16).toUpperCase().padStart(2, "0");

const idle = () => document.body.dataset.state === "empty";

export async function startDemo(host: HTMLElement): Promise<void> {
  if (!idle()) return;
  const bytes = new Uint8Array(await (await fetch(SAMPLE)).arrayBuffer());
  if (!idle()) return;
  const r = await call({ type: "parse", file: new File([bytes], "sample.png") });
  if (r.type !== "parsed" || !r.result.preview || !r.result.ihdr || !idle()) return;
  const m = new FileModel(r.result, bytes, "sample.png");
  const g = geometry(r.result.ihdr);
  const preview = r.result.preview;

  const frame = el("div", "demo-frame");
  const image = el("canvas", "demo-image");
  image.width = preview.width;
  image.height = preview.height;
  image
    .getContext("2d")
    ?.putImageData(new ImageData(new Uint8ClampedArray(preview.pixels), preview.width, preview.height), 0, 0);
  const overlay = el("canvas", "demo-overlay");
  const pointer = el("div", "demo-pointer");
  frame.append(image, overlay, pointer);
  const bytesRow = el("div", "demo-bytes");
  const caption = el("p", "demo-caption");
  const side = el("div", "demo-side");
  side.append(el("p", "demo-kicker", "Point at a pixel, see the bytes that made it"), bytesRow, caption);
  host.replaceChildren(frame, side);
  host.hidden = false;

  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  for (let i = 0; idle(); i = (i + 1) % PATH.length) {
    const [x, y] = PATH[i];
    await show(m, g, x, y, { frame, overlay, pointer, bytesRow, caption });
    // Without motion, one frame says it: stay on the first pixel.
    if (reduce) return;
    await new Promise((resolve) => setTimeout(resolve, DWELL_MS));
  }
  host.hidden = true;
}

interface Parts {
  frame: HTMLElement;
  overlay: HTMLCanvasElement;
  pointer: HTMLElement;
  bytesRow: HTMLElement;
  caption: HTMLElement;
}

async function show(m: FileModel, g: Geometry, x: number, y: number, p: Parts): Promise<void> {
  const r = await call({ type: "locate", by: 0, pos: pixelOffset(g, x, y) });
  if (r.type !== "located" || r.step.length !== 7 || !idle()) return;
  const [, kind, a, b, bitStart, bitEnd, outStart] = r.step;
  const len = kind === 1 ? 1 : b;

  // The pointer glides to the pixel; the pixels the step wrote light up.
  p.pointer.style.transform = `translate(${((x + 0.5) / g.width) * 100}cqw, ${((y + 0.5) / g.height) * 100}cqh)`;
  const dpr = window.devicePixelRatio || 1;
  p.overlay.width = Math.round(p.overlay.clientWidth * dpr);
  p.overlay.height = Math.round(p.overlay.clientHeight * dpr);
  const ctx = p.overlay.getContext("2d");
  if (ctx) {
    const sx = p.overlay.width / g.width;
    const sy = p.overlay.height / g.height;
    const fill = (start: number, end: number, colour: string) => {
      ctx.fillStyle = colour;
      for (const [rx, ry, rw] of runs(g, start, end)) ctx.fillRect(rx * sx, ry * sy, rw * sx, sy);
    };
    if (kind === 2) fill(outStart - a, outStart - a + len, "rgba(59, 130, 246, 0.5)");
    fill(outStart, outStart + len, "rgba(232, 82, 122, 0.65)");
  }

  // The file's bytes around the ones that hold the step.
  const [start, end] = m.bitsToFile(bitStart, bitEnd);
  const from = Math.max(0, start - 3);
  const to = Math.min(m.bytes.length, Math.max(end, start + 1) + 3);
  const cells: Node[] = [el("span", "demo-offset", start.toString(16).toUpperCase().padStart(6, "0"))];
  for (let i = from; i < to; i++) {
    cells.push(el("span", i >= start && i < end ? "demo-byte is-hit" : "demo-byte", hex(m.bytes[i])));
  }
  p.bytesRow.replaceChildren(...cells);

  const row = g.passes[0].row;
  const rows = kind === 2 && a % row === 0 ? `exactly ${a / row === 1 ? "one row" : `${a / row} rows`} up` : `${a} bytes back`;
  p.caption.textContent =
    kind === 1
      ? `Pixel ${x}, ${y}: a literal byte, 0x${hex(a)}, spelled out in ${bitEnd - bitStart} bits.`
      : `Pixel ${x}, ${y}: part of a copy of ${len} bytes from ${rows} — ${bitEnd - bitStart} bits say so.`;
}
