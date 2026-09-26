// The landing page's demonstration, in two scenes: a pointer moving over a
// real PNG, and for each pixel, the bytes of the file that wrote it; then a
// progressive JPEG arriving scan by scan, blurry at first and sharp at the
// end — what hexscope does, shown before anyone has to pick a file. It runs
// on the same worker as the app, so it only runs while no file is open, and
// stops when one is.
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
/** Between scans of the JPEG. */
const SCAN_MS = 800;
const SAMPLE = "samples/sample.png";
const PROGRESSIVE = "samples/progressive.jpg";

const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};
const hex = (n: number) => n.toString(16).toUpperCase().padStart(2, "0");

const idle = () => document.body.dataset.state === "empty";
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Where each scan's data ends in a JPEG: at the first marker after it that
 * is not a restart marker or a stuffed zero. The file cut there, and closed,
 * is the picture as far as that scan.
 */
export function scanEnds(b: Uint8Array): number[] {
  const ends: number[] = [];
  let i = 2;
  while (i + 4 <= b.length && b[i] === 0xff) {
    const marker = b[i + 1];
    if (marker === 0xff) {
      i++;
      continue;
    }
    if (marker === 0xd9) break;
    i += 2 + ((b[i + 2] << 8) | b[i + 3]);
    if (marker !== 0xda) continue;
    while (i + 1 < b.length && !(b[i] === 0xff && b[i + 1] !== 0 && (b[i + 1] < 0xd0 || b[i + 1] > 0xd7))) i++;
    ends.push(i);
  }
  return ends;
}

/** The progressive sample after each scan, decoded ahead so the scene only draws. */
async function scans(): Promise<{ pictures: ImageBitmap[]; shares: number[] } | null> {
  try {
    const bytes = new Uint8Array(await (await fetch(PROGRESSIVE)).arrayBuffer());
    const ends = scanEnds(bytes);
    const pictures = await Promise.all(
      ends.map((end) => {
        const cut = new Uint8Array(end + 2);
        cut.set(bytes.subarray(0, end));
        cut.set([0xff, 0xd9], end);
        return createImageBitmap(new Blob([cut], { type: "image/jpeg" }));
      }),
    );
    return { pictures, shares: ends.map((end) => end / bytes.length) };
  } catch {
    return null;
  }
}

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
  image.setAttribute("aria-hidden", "true");
  const drawPng = () => {
    image.width = preview.width;
    image.height = preview.height;
    image.classList.remove("is-photo");
    image
      .getContext("2d")
      ?.putImageData(new ImageData(new Uint8ClampedArray(preview.pixels), preview.width, preview.height), 0, 0);
  };
  drawPng();
  const overlay = el("canvas", "demo-overlay");
  overlay.setAttribute("aria-hidden", "true");
  const pointer = el("div", "demo-pointer");
  frame.append(image, overlay, pointer);
  const bytesRow = el("div", "demo-bytes");
  const caption = el("p", "demo-caption");
  const side = el("div", "demo-side");
  const kicker = el("p", "demo-kicker");
  side.append(kicker, bytesRow, caption);
  host.replaceChildren(frame, side);
  host.hidden = false;
  const parts = { frame, overlay, pointer, bytesRow, caption };

  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const jpeg = reduce ? Promise.resolve(null) : scans();
  while (idle()) {
    kicker.textContent = "Point at a pixel, see the bytes that made it";
    pointer.hidden = false;
    drawPng();
    for (const [x, y] of PATH) {
      if (!idle()) break;
      await show(m, g, x, y, parts);
      // Without motion, one frame says it: stay on the first pixel.
      if (reduce) return;
      await sleep(DWELL_MS);
    }
    const progressive = await jpeg;
    if (progressive && idle()) await arrive(progressive, image, kicker, parts);
  }
  host.hidden = true;
}

/** The second scene: a progressive JPEG, scan after scan, and how much of the file each took. */
async function arrive(
  j: { pictures: ImageBitmap[]; shares: number[] },
  image: HTMLCanvasElement,
  kicker: HTMLElement,
  p: Parts,
): Promise<void> {
  kicker.textContent = "A progressive JPEG, scan by scan";
  p.pointer.hidden = true;
  p.overlay.getContext("2d")?.clearRect(0, 0, p.overlay.width, p.overlay.height);
  image.classList.add("is-photo");
  const n = j.pictures.length;
  const bar = el("span", "demo-bar");
  const fill = el("span", "demo-bar-fill");
  bar.append(fill);
  const share = el("span", "demo-share");
  p.bytesRow.replaceChildren(bar, share);
  for (let k = 0; k < n && idle(); k++) {
    const pic = j.pictures[k];
    // The square frame shows the middle of the photo.
    const side = Math.min(pic.width, pic.height);
    image.width = side;
    image.height = side;
    image.getContext("2d")?.drawImage(pic, (pic.width - side) / 2, (pic.height - side) / 2, side, side, 0, 0, side, side);
    const pct = Math.round(j.shares[k] * 100);
    fill.style.width = `${pct}%`;
    share.textContent = k === n - 1 ? "the whole file" : `${pct}% of the file`;
    p.caption.textContent =
      k === 0
        ? `Scan 1 of ${n}: ${pct}% of the file, and the whole photo is there already — blurry, each block just its average.`
        : k === n - 1
          ? `All ${n} scans: every detail. hexscope shows which bytes draw which block, in each scan.`
          : `Scan ${k + 1} of ${n}: ${pct}% of the file. Each scan adds finer detail.`;
    await sleep(k === n - 1 ? DWELL_MS : SCAN_MS);
  }
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
