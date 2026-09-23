// Parsing runs here so a large file never freezes the page. The parsed
// document stays alive in the worker so the DEFLATE player can ask for steps
// on demand instead of receiving millions of them up front.
import init, { parse, type Parsed } from "./wasm/hexscope_wasm.js";
import type { ParsedFile } from "./model";

export type WorkerRequest =
  | { id: number; type: "parse"; file: File }
  | { id: number; type: "steps"; from: number; count: number }
  | { id: number; type: "inflated" };

export type WorkerResponse =
  | { id: number; type: "parsed"; result: ParsedFile }
  | { id: number; type: "steps"; steps: Float64Array }
  | { id: number; type: "inflated"; bytes: Uint8Array }
  | { id: number; type: "error"; message: string };

const SEPARATOR = "\u001f";
const ready = init();
let current: Parsed | null = null;

const post = (msg: WorkerResponse, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: unknown, t: Transferable[]): void }).postMessage(
    msg,
    transfer,
  );

async function handle(req: WorkerRequest): Promise<void> {
  await ready;

  if (req.type === "parse") {
    const bytes = new Uint8Array(await req.file.arrayBuffer());
    // Timed from the call into WASM through reading every array back out, so
    // the number shown to the user is the whole cost, not the flattering part.
    const t0 = performance.now();
    const parsed = parse(bytes);
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
      format: parsed.format as ParsedFile["format"],
      dimensions: null,
      facts: [],
      location: null,
      parseMs: 0,
    };
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
    const ihdr = Array.from(parsed.ihdr);
    const trace = Array.from(parsed.trace);
    result.ihdr = ihdr.length ? ihdr : null;
    result.trace = trace.length ? trace : null;
    result.parseMs = performance.now() - t0;

    current?.free();
    current = parsed;
    post({ id: req.id, type: "parsed", result }, [
      result.starts.buffer,
      result.lens.buffer,
      result.parents.buffer,
      result.kinds.buffer,
      result.segments.buffer,
    ]);
    return;
  }

  if (!current) throw new Error("No file is open");
  if (req.type === "steps") {
    const steps = current.steps(req.from, req.count);
    post({ id: req.id, type: "steps", steps }, [steps.buffer]);
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
