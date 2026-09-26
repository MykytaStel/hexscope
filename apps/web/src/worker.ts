// Parsing runs here so a large file never freezes the page. The parsed
// document stays alive in the worker so the DEFLATE player can ask for steps
// on demand instead of receiving millions of them up front.
import init, { cleanCopy, entropy, parse, repairCopy, type Parsed } from "./wasm/hexscope_wasm.js";
import type { Blackout, PageArea, ParsedFile } from "./model";

export type WorkerRequest =
  | { id: number; type: "parse"; file: File }
  | { id: number; type: "steps"; from: number; count: number }
  | { id: number; type: "inflated" }
  | { id: number; type: "explain"; index: number; knownBlock: number }
  | { id: number; type: "selectEntry"; index: number }
  | { id: number; type: "open"; index: number }
  | { id: number; type: "back"; depth: number }
  | { id: number; type: "clean"; bytes: Uint8Array }
  | { id: number; type: "repair"; bytes: Uint8Array }
  | { id: number; type: "locate"; by: 0 | 1; pos: number }
  | { id: number; type: "blocks" };

export type WorkerResponse =
  | { id: number; type: "parsed"; result: ParsedFile }
  | { id: number; type: "opened"; result: ParsedFile; bytes: Uint8Array }
  | { id: number; type: "back" }
  | { id: number; type: "cleaned"; bytes: Uint8Array; removed: { what: string; bytes: number }[]; orientation: number; error: string }
  | { id: number; type: "repaired"; bytes: Uint8Array; fixed: string[]; error: string }
  | { id: number; type: "steps"; steps: Float64Array }
  | { id: number; type: "located"; step: Float64Array }
  | { id: number; type: "blocks"; map: Float64Array; note: string }
  | { id: number; type: "inflated"; bytes: Uint8Array }
  | { id: number; type: "explain"; parts: Float64Array; tables: Float64Array | null }
  | { id: number; type: "stream"; playable: boolean; trace: number[] | null; segments: Float64Array; idatBytes: number }
  | { id: number; type: "error"; message: string };

const SEPARATOR = "\u001f";
const ready = init();
/** The file, then each entry opened inside it; the last one is on screen. */
let stack: Parsed[] = [];

const post = (msg: WorkerResponse, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: unknown, t: Transferable[]): void }).postMessage(
    msg,
    transfer,
  );

/** Reads everything the page needs out of a parsed document. */
function describe(parsed: Parsed): ParsedFile {
  const result: ParsedFile = {
    starts: parsed.starts,
    lens: parsed.lens,
    parents: parsed.parents,
    kinds: parsed.kinds,
    labels: parsed.labels.split(SEPARATOR),
    values: parsed.values.split(SEPARATOR),
    ihdr: null,
    trace: null,
    idatBytes: parsed.idatBytes,
    segments: parsed.segments,
    streamHeader: parsed.streamHeader,
    entries: parsed.entries,
    docIds: parsed.docIds,
    docTexts: parsed.docTexts.split(SEPARATOR),
    docCites: parsed.docCites.split(SEPARATOR),
    docUrls: parsed.docUrls.split(SEPARATOR),
    docConcerns: parsed.docConcerns,
    composition: parsed.composition,
    entropy: new Float32Array(0),
    entropyWindow: 0,
    format: parsed.format as ParsedFile["format"],
    dimensions: null,
    orientation: parsed.orientation,
    facts: [],
    location: null,
    blackouts: [],
    parseMs: 0,
    preview: null,
    rowFilters: parsed.rowFilters,
  };
  const size = Array.from(parsed.previewSize);
  if (size.length === 2) {
    result.preview = { width: size[0], height: size[1], pixels: parsed.previewPixels };
  }
  const dims = Array.from(parsed.dimensions);
  result.dimensions = dims.length === 2 ? [dims[0], dims[1]] : null;
  const facts = parsed.facts ? parsed.facts.split(SEPARATOR) : [];
  for (let i = 0; i + 2 < facts.length; i += 3) {
    result.facts.push({ kind: facts[i], text: facts[i + 1], node: Number(facts[i + 2]) });
  }
  const loc = Array.from(parsed.location);
  if (loc.length === 4) {
    result.location = {
      latitude: loc[0],
      longitude: loc[1],
      altitude: Number.isNaN(loc[2]) ? null : loc[2],
      node: loc[3],
    };
  }
  result.blackouts = blackouts(parsed.blackouts, parsed.blackoutTexts.split(SEPARATOR));
  const ihdr = Array.from(parsed.ihdr);
  const trace = Array.from(parsed.trace);
  result.ihdr = ihdr.length ? ihdr : null;
  result.trace = trace.length ? trace : null;
  return result;
}

/** The layout `Parsed.blackouts` documents, as objects. */
function blackouts(n: Float64Array, texts: string[]): Blackout[] {
  const out: Blackout[] = [];
  const area = (i: number): PageArea => [n[i], n[i + 1], n[i + 2], n[i + 3]];
  let i = 0;
  let t = 0;
  while (i + 8 <= n.length) {
    const b: Blackout = { page: n[i], media: area(i + 1), boxes: [], texts: [], context: [] };
    const [boxes, pieces, beside] = [n[i + 5], n[i + 6], n[i + 7]];
    i += 8;
    for (let k = 0; k < boxes && i + 4 <= n.length; k++, i += 4) b.boxes.push(area(i));
    for (let k = 0; k < pieces && i + 4 <= n.length; k++, i += 4) b.texts.push({ area: area(i), text: texts[t++] ?? "" });
    for (let k = 0; k < beside && i + 4 <= n.length; k++, i += 4) b.context.push({ area: area(i), text: texts[t++] ?? "" });
    out.push(b);
  }
  return out;
}

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
  ...(r.preview ? [r.preview.pixels.buffer] : []),
];

/** Bins the entropy minimap is drawn from; the page draws at most one per pixel row. */
const ENTROPY_BINS = 1024;

/** Entropy for the minimap, outside the timed parse. The window matches the core's: never under 256 bytes. */
function addEntropy(result: ParsedFile, bytes: Uint8Array): void {
  result.entropy = entropy(bytes, ENTROPY_BINS);
  const bins = Math.max(1, Math.min(ENTROPY_BINS, Math.floor(bytes.length / 256)));
  result.entropyWindow = Math.ceil(bytes.length / bins);
}

/** Nested documents opened from archives are kept for the way back. */
const MAX_DEPTH = 4;

async function handle(req: WorkerRequest): Promise<void> {
  await ready;

  if (req.type === "parse") {
    const bytes = new Uint8Array(await req.file.arrayBuffer());
    // Timed from the call into WASM through reading every array back out, so
    // the number shown to the user is the whole cost, not the flattering part.
    const t0 = performance.now();
    const parsed = parse(bytes);
    const result = describe(parsed);
    result.parseMs = performance.now() - t0;
    addEntropy(result, bytes);
    for (const p of stack) p.free();
    stack = [parsed];
    post({ id: req.id, type: "parsed", result }, transfers(result));
    return;
  }

  if (req.type === "clean") {
    const c = cleanCopy(req.bytes);
    const parts = c.removed ? c.removed.split(SEPARATOR) : [];
    const removed = [];
    for (let i = 0; i + 1 < parts.length; i += 2) removed.push({ what: parts[i], bytes: Number(parts[i + 1]) });
    const bytes = c.bytes;
    post({ id: req.id, type: "cleaned", bytes, removed, orientation: c.orientationKept, error: c.error }, [bytes.buffer]);
    c.free();
    return;
  }

  if (req.type === "repair") {
    const r = repairCopy(req.bytes);
    const bytes = r.bytes;
    post({ id: req.id, type: "repaired", bytes, fixed: r.fixed ? r.fixed.split(SEPARATOR) : [], error: r.error }, [bytes.buffer]);
    r.free();
    return;
  }

  if (stack.length === 0) throw new Error("No file is open");
  const current = stack[stack.length - 1];
  if (req.type === "open") {
    if (stack.length > MAX_DEPTH) throw new Error(`files nest at most ${MAX_DEPTH} deep here`);
    const bytes = current.extractEntry(req.index);
    if (bytes.length === 0) throw new Error(`This entry cannot be opened: ${current.extractError}.`);
    const t0 = performance.now();
    const parsed = parse(bytes);
    const result = describe(parsed);
    result.parseMs = performance.now() - t0;
    addEntropy(result, bytes);
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
  handle(event.data).catch((err: unknown) =>
    post({
      id: event.data.id,
      type: "error",
      message: err instanceof Error ? err.message : String(err),
    }),
  );
};
