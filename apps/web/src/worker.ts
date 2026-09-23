// Parsing runs here so a large file never freezes the page.
import init, { parse } from "./wasm/hexscope_wasm.js";
import type { ParsedFile } from "./model";

export type WorkerRequest = { id: number; file: File };
export type WorkerResponse =
  | { id: number; ok: true; result: ParsedFile }
  | { id: number; ok: false; message: string };

const SEPARATOR = "\u001f";
const ready = init();

const post = (msg: WorkerResponse, transfer: Transferable[] = []) =>
  (self as unknown as { postMessage(m: unknown, t: Transferable[]): void }).postMessage(
    msg,
    transfer,
  );

self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const { id, file } = event.data;
  try {
    await ready;
    const bytes = new Uint8Array(await file.arrayBuffer());

    // Timed from the call into WASM through reading every array back out, so
    // the number shown to the user is the whole cost, not the flattering part.
    const t0 = performance.now();
    const parsed = parse(bytes);
    const starts = parsed.starts;
    const lens = parsed.lens;
    const parents = parsed.parents;
    const kinds = parsed.kinds;
    const labels = parsed.labels.split(SEPARATOR);
    const values = parsed.values.split(SEPARATOR);
    const ihdr = Array.from(parsed.ihdr);
    const trace = Array.from(parsed.trace);
    const idatBytes = parsed.idatBytes;
    parsed.free();
    const parseMs = performance.now() - t0;

    const result: ParsedFile = {
      starts,
      lens,
      parents,
      kinds,
      labels,
      values,
      ihdr: ihdr.length ? ihdr : null,
      trace: trace.length ? trace : null,
      idatBytes,
      parseMs,
    };
    // The typed arrays move to the main thread instead of being copied.
    post({ id, ok: true, result }, [starts.buffer, lens.buffer, parents.buffer, kinds.buffer]);
  } catch (err) {
    post({ id, ok: false, message: err instanceof Error ? err.message : String(err) });
  }
};
