// Parsing runs here so a large file never freezes the page. The parsed
// document stays alive in the worker so the DEFLATE player can ask for steps
// on demand instead of receiving millions of them up front.
import type { CleanCopy, Parsed } from "./wasm/hexscope_wasm.js";
import type { PageArea, ParsedFile } from "./model";
import { describe, SEPARATOR } from "./describe";
import { assemble, LARGE_MOVIE, movieEntropy, readMovie, type Movie } from "./movie";

export type WorkerRequest =
  | { id: number; type: "parse"; file: File }
  | { id: number; type: "steps"; from: number; count: number }
  | { id: number; type: "inflated" }
  | { id: number; type: "explain"; index: number; knownBlock: number }
  | { id: number; type: "selectEntry"; index: number }
  | { id: number; type: "open"; index: number }
  | { id: number; type: "openBytes"; bytes: Uint8Array }
  | { id: number; type: "back"; depth: number }
  | { id: number; type: "clean"; source: Blob; notes?: boolean }
  | { id: number; type: "repair"; bytes: Uint8Array }
  | { id: number; type: "pageTexts"; bytes: Uint8Array }
  | { id: number; type: "pagePictures"; bytes: Uint8Array; page: number }
  | { id: number; type: "redact"; bytes: Uint8Array; areas: Float64Array }
  | { id: number; type: "locate"; by: 0 | 1; pos: number }
  | { id: number; type: "blocks" };

export type WorkerResponse =
  | { id: number; type: "parsed"; result: ParsedFile; bytes: Uint8Array }
  | { id: number; type: "opened"; result: ParsedFile; bytes: Uint8Array }
  | { id: number; type: "back" }
  | { id: number; type: "cleaned"; copy: Blob; removed: { what: string; bytes: number }[]; orientation: number; error: string }
  | { id: number; type: "repaired"; bytes: Uint8Array; fixed: string[]; error: string }
  | { id: number; type: "pageTexts"; pages: PageGlyphs[] }
  | { id: number; type: "pagePictures"; pictures: PagePicture[] }
  | { id: number; type: "steps"; steps: Float64Array }
  | { id: number; type: "located"; step: Float64Array }
  | { id: number; type: "blocks"; map: Float64Array; note: string }
  | { id: number; type: "inflated"; bytes: Uint8Array }
  | { id: number; type: "explain"; parts: Float64Array; tables: Float64Array | null }
  | { id: number; type: "stream"; playable: boolean; trace: number[] | null; segments: Float64Array; idatBytes: number }
  | { id: number; type: "error"; message: string };

/** One page's visible glyphs: where each lands, four numbers each, and its text. */
export interface PageGlyphs {
  media: PageArea;
  areas: Float64Array;
  texts: string[];
  /** The page's own dark boxes and marks, four numbers each. */
  boxes: Float64Array;
}

/** A picture a page draws: where its unit square goes, and a JPEG's bytes or its pixels. */
export interface PagePicture {
  matrix: number[];
  jpeg?: Uint8Array;
  width?: number;
  height?: number;
  rgba?: Uint8ClampedArray;
}

/** The parser's two builds: all of it, and pictures and movies alone — half the size, and what most files need. */
type Module = typeof import("./wasm/hexscope_wasm.js");
let full: Promise<Module> | null = null;
let media: Promise<Module> | null = null;
/**
 * Why a parser could not be had, in words. A failed load is not kept: the
 * next file tries again, so a dropped connection is not the end of the tab.
 * A tab open across a new release asks for files the site no longer has.
 */
function unavailable(): Error {
  return new Error(
    navigator.onLine
      ? "hexscope was updated while this page was open. Reload the page to read it"
      : "you are offline, and this part of hexscope has not been saved for offline use yet",
  );
}
const loadFull = () =>
  (full ??= import("./wasm/hexscope_wasm.js")
    .then(async (m) => {
      await m.default();
      return m;
    })
    .catch(() => {
      full = null;
      throw unavailable();
    }));
const loadMedia = () =>
  (media ??= import("./wasm-media/hexscope_wasm.js")
    .then(async (m) => {
      await m.default();
      // The same interface, less what documents need.
      return m as unknown as Module;
    })
    .catch(() => {
      media = null;
      throw unavailable();
    }));

/** Whether the small build reads these bytes: PNG, JPEG, WebP, GIF, and the ISO boxes of HEIF and MP4. */
function isMedia(b: Uint8Array): boolean {
  const png = b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47;
  const jpeg = b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff;
  const iso = b[4] === 0x66 && b[5] === 0x74 && b[6] === 0x79 && b[7] === 0x70;
  const webp = b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45;
  const gif = b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x38;
  return png || jpeg || iso || webp || gif;
}
const moduleFor = (b: Uint8Array) => (isMedia(b) ? loadMedia() : loadFull());

/**
 * Once a picture has been read, the whole parser is fetched quietly — not
 * started — so the service worker has it for opening a document offline.
 * Not on a connection that asks to save data.
 */
let prefetched = false;
function prefetchFull(): void {
  if (prefetched || full) return;
  prefetched = true;
  const saveData = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection?.saveData;
  if (saveData || !navigator.onLine) return;
  setTimeout(() => {
    if (full) return;
    void import("./wasm/hexscope_wasm.js").catch(() => {});
    void fetch(new URL("./wasm/hexscope_wasm_bg.wasm", import.meta.url)).catch(() => {});
  }, 10_000);
}
/** The file, then each entry opened inside it; the last one is on screen. */
let stack: Parsed[] = [];

const post = (msg: WorkerResponse, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: unknown, t: Transferable[]): void }).postMessage(
    msg,
    transfer,
  );

const transfers = (r: ParsedFile): Transferable[] => [
  r.starts.buffer,
  r.lens.buffer,
  r.parents.buffer,
  r.kinds.buffer,
  r.segments.buffer,
  r.entries.buffer,
  r.docIds.buffer,
  r.docConcerns.buffer,
  r.composition.buffer,
  r.entropy.buffer,
  r.rowFilters.buffer,
  r.missing.buffer,
  ...(r.preview ? [r.preview.pixels.buffer] : []),
];

/** Bins the entropy minimap is drawn from; the page draws at most one per pixel row. */
const ENTROPY_BINS = 1024;

/** Entropy for the minimap, outside the timed parse. The window matches the core's: never under 256 bytes. */
function addEntropy(result: ParsedFile, bytes: Uint8Array, wasm: Module): void {
  result.entropy = wasm.entropy(bytes, ENTROPY_BINS);
  const bins = Math.max(1, Math.min(ENTROPY_BINS, Math.floor(bytes.length / 256)));
  result.entropyWindow = Math.ceil(bytes.length / bins);
}

/** A movie read in parts, when `file` is one large enough to need it. */
async function largeMovie(file: Blob): Promise<Movie | null> {
  if (file.size < LARGE_MOVIE) return null;
  const wasm = await loadMedia();
  return readMovie(file, (head, len) => wasm.isMovie(head, len));
}

/** Answers with a copy the parser made, as a file made by `wrap`. */
function postCopy(id: number, c: CleanCopy, wrap: (bytes: Uint8Array) => Blob): void {
  const parts = c.removed ? c.removed.split(SEPARATOR) : [];
  const removed = [];
  for (let i = 0; i + 1 < parts.length; i += 2) removed.push({ what: parts[i], bytes: Number(parts[i + 1]) });
  const copy = c.error ? new Blob([]) : wrap(c.bytes);
  post({ id, type: "cleaned", copy, removed, orientation: c.orientationKept, error: c.error });
  c.free();
}

/** Nested documents opened from archives are kept for the way back. */
const MAX_DEPTH = 4;


/**
 * The JPEG pictures under the areas to black out, each painted black there
 * and encoded again: a PDF's scanned page keeps its pixels under a box, so
 * the box goes into the picture. The core says which and where; the
 * browser has the JPEG codec. One that will not decode is left out, and
 * the core then makes no copy rather than one that keeps it.
 */
/** `%PDF-` within the first kilobyte, as the core looks for it. */
function isPdf(bytes: Uint8Array): boolean {
  const head = bytes.subarray(0, 1024);
  for (let i = 0; i + 5 <= head.length; i++) {
    if (head[i] === 0x25 && head[i + 1] === 0x50 && head[i + 2] === 0x44 && head[i + 3] === 0x46 && head[i + 4] === 0x2d) return true;
  }
  return false;
}

async function paintJpegs(pdf: Uint8Array, found: Float64Array): Promise<{ nums: Uint32Array; lens: Uint32Array; bytes: Uint8Array }> {
  const nums: number[] = [];
  const parts: Uint8Array[] = [];
  for (let i = 0; i < found.length; ) {
    const [num, width, height, start, len, count] = found.subarray(i, i + 6);
    const rects = found.subarray(i + 6, i + 6 + 4 * count);
    i += 6 + 4 * count;
    try {
      const bitmap = await createImageBitmap(new Blob([pdf.subarray(start, start + len) as BlobPart], { type: "image/jpeg" }), { imageOrientation: "none" });
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const g = canvas.getContext("2d");
      if (!g) continue;
      g.drawImage(bitmap, 0, 0);
      bitmap.close();
      // The core counts in the picture's own size; the decoder may differ.
      const sx = canvas.width / width;
      const sy = canvas.height / height;
      g.fillStyle = "#000";
      for (let r = 0; r < rects.length; r += 4) {
        const x0 = Math.floor(rects[r] * sx);
        const y0 = Math.floor(rects[r + 1] * sy);
        g.fillRect(x0, y0, Math.ceil(rects[r + 2] * sx) - x0, Math.ceil(rects[r + 3] * sy) - y0);
      }
      const blob = await canvas.convertToBlob({ type: "image/jpeg", quality: 0.92 });
      nums.push(num);
      parts.push(new Uint8Array(await blob.arrayBuffer()));
    } catch {
      // Not decoded: not painted.
    }
  }
  const bytes = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    bytes.set(p, at);
    at += p.length;
  }
  return { nums: Uint32Array.from(nums), lens: Uint32Array.from(parts, (p) => p.length), bytes };
}

async function handle(req: WorkerRequest): Promise<void> {
  if (req.type === "parse") {
    const movie = await largeMovie(req.file);
    if (movie) {
      const wasm = await loadMedia();
      const t0 = performance.now();
      const parsed = wasm.parseMovie(movie.given, movie.gaps);
      const result = describe(parsed);
      result.parseMs = performance.now() - t0;
      const e = await movieEntropy(req.file, movie, ENTROPY_BINS, (b) => wasm.entropy(b, 1)[0]);
      result.entropy = e.values;
      result.entropyWindow = e.window;
      result.missing = movie.missing;
      for (const p of stack) p.free();
      stack = [parsed];
      post({ id: req.id, type: "parsed", result, bytes: movie.whole }, [...transfers(result), movie.whole.buffer]);
      return;
    }
    const bytes = new Uint8Array(await req.file.arrayBuffer());
    const wasm = await moduleFor(bytes);
    // Timed from the call into WASM through reading every array back out, so
    // the number shown to the user is the whole cost, not the flattering part.
    const t0 = performance.now();
    const parsed = wasm.parse(bytes);
    const result = describe(parsed);
    result.parseMs = performance.now() - t0;
    addEntropy(result, bytes, wasm);
    for (const p of stack) p.free();
    stack = [parsed];
    if (isMedia(bytes)) prefetchFull();
    // The parser keeps its own copy: these bytes go to the page, moved, not
    // copied, so a large file is held twice at most rather than three times.
    post({ id: req.id, type: "parsed", result, bytes }, [...transfers(result), bytes.buffer]);
    return;
  }

  if (req.type === "clean") {
    const movie = await largeMovie(req.source);
    if (movie) {
      const source = req.source;
      postCopy(req.id, (await loadMedia()).cleanMovie(movie.given, movie.gaps), (b) => assemble(b, movie, source));
      return;
    }
    const bytes = new Uint8Array(await req.source.arrayBuffer());
    const wasm = await moduleFor(bytes);
    // A PDF with a black box over a scanned JPEG: the picture is redrawn
    // with the box in it, as when blacking out by hand.
    let c = null;
    if (isPdf(bytes)) {
      const none = new Float64Array(0);
      const found = wasm.jpegsUnder(bytes, none);
      if (found.length > 0) {
        const painted = await paintJpegs(bytes, found);
        c = wasm.redactCopy(bytes, none, painted.nums, painted.lens, painted.bytes);
      }
    }
    c ??= wasm.cleanCopy(bytes, req.notes ?? false);
    postCopy(req.id, c, (b) => new Blob([b as BlobPart]));
    return;
  }

  if (req.type === "pageTexts") {
    const t = (await loadFull()).pageTexts(req.bytes);
    const pages: PageGlyphs[] = [];
    for (let i = 0; i < t.count; i++) {
      const texts = t.texts(i);
      pages.push({ media: Array.from(t.media(i)) as PageArea, areas: t.areas(i), texts: texts ? texts.split(SEPARATOR) : [], boxes: t.boxes(i) });
    }
    t.free();
    post({ id: req.id, type: "pageTexts", pages });
    return;
  }

  if (req.type === "pagePictures") {
    const t = (await loadFull()).pagePictures(req.bytes, req.page);
    const pictures: PagePicture[] = [];
    const moved: ArrayBuffer[] = [];
    for (let i = 0; i < t.count; i++) {
      const matrix = Array.from(t.matrix(i));
      const at = t.jpeg(i);
      if (at.length === 2) {
        const jpeg = req.bytes.slice(at[0], at[0] + at[1]);
        moved.push(jpeg.buffer);
        pictures.push({ matrix, jpeg });
        continue;
      }
      const [width, height] = t.size(i);
      const rgba = new Uint8ClampedArray(t.rgba(i).buffer);
      moved.push(rgba.buffer as ArrayBuffer);
      pictures.push({ matrix, width, height, rgba });
    }
    t.free();
    post({ id: req.id, type: "pagePictures", pictures }, moved);
    return;
  }

  if (req.type === "redact") {
    const wasm = await loadFull();
    const painted = await paintJpegs(req.bytes, wasm.jpegsUnder(req.bytes, req.areas));
    const c = wasm.redactCopy(req.bytes, req.areas, painted.nums, painted.lens, painted.bytes);
    postCopy(req.id, c, (b) => new Blob([b as BlobPart]));
    return;
  }

  if (req.type === "repair") {
    const r = (await moduleFor(req.bytes)).repairCopy(req.bytes);
    const bytes = r.bytes;
    post({ id: req.id, type: "repaired", bytes, fixed: r.fixed ? r.fixed.split(SEPARATOR) : [], error: r.error }, [bytes.buffer]);
    r.free();
    return;
  }

  if (stack.length === 0) throw new Error("No file is open");
  const current = stack[stack.length - 1];
  if (req.type === "open" || req.type === "openBytes") {
    if (stack.length > MAX_DEPTH) throw new Error(`files nest at most ${MAX_DEPTH} deep here`);
    // An archive's entry, or a file found inside the bytes.
    const bytes = req.type === "open" ? current.extractEntry(req.index) : req.bytes;
    if (bytes.length === 0) throw new Error(`This entry cannot be opened: ${current.extractError}.`);
    // Its own build: a ZIP behind a photo needs the one that reads archives.
    const wasm = await moduleFor(bytes);
    const t0 = performance.now();
    const parsed = wasm.parse(bytes);
    const result = describe(parsed);
    result.parseMs = performance.now() - t0;
    addEntropy(result, bytes, wasm);
    stack.push(parsed);
    post({ id: req.id, type: "opened", result, bytes }, [...transfers(result), bytes.buffer]);
    return;
  }
  if (req.type === "back") {
    // Never below the file itself.
    while (stack.length > Math.max(1, req.depth + 1)) stack.pop()!.free();
    post({ id: req.id, type: "back" });
    return;
  }
  if (req.type === "blocks") {
    const map = current.blockMap();
    post({ id: req.id, type: "blocks", map, note: current.blocksNote }, [map.buffer]);
  } else if (req.type === "locate") {
    const step = current.locate(req.by, req.pos);
    post({ id: req.id, type: "located", step }, [step.buffer]);
  } else if (req.type === "steps") {
    const steps = current.steps(req.from, req.count);
    post({ id: req.id, type: "steps", steps }, [steps.buffer]);
  } else if (req.type === "selectEntry") {
    const playable = current.selectEntry(req.index);
    const trace = Array.from(current.trace);
    const segments = current.segments;
    post({ id: req.id, type: "stream", playable, trace: trace.length ? trace : null, segments, idatBytes: current.idatBytes }, [
      segments.buffer,
    ]);
  } else if (req.type === "explain") {
    const parts = current.explain(req.index);
    // Tables change once per block: send them only when the page's are stale.
    const tables = parts.length > 0 && parts[0] !== req.knownBlock ? current.tables(req.index) : null;
    post({ id: req.id, type: "explain", parts, tables }, tables ? [parts.buffer, tables.buffer] : [parts.buffer]);
  } else {
    const bytes = current.inflated;
    post({ id: req.id, type: "inflated", bytes }, [bytes.buffer]);
  }
}

self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  handle(event.data).catch((err: unknown) => {
    // Out of memory, the WebAssembly stops for good: start the next file
    // on a fresh one, and say what happened in words.
    const text = err instanceof Error ? err.message : String(err);
    const crashed = err instanceof WebAssembly.RuntimeError || /memory|allocation|array buffer/i.test(text);
    if (crashed) {
      full = null;
      media = null;
      stack = [];
    }
    post({
      id: event.data.id,
      type: "error",
      message: crashed ? "it needs more memory than this browser tab has. Close other tabs and try again, or use the command line tool" : text,
    });
  });
};
