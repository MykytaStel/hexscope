// The page's line to the parsing worker. The worker keeps the parsed document,
// so requests after the first (steps, explanations, entries) are cheap.
import type { WorkerRequest, WorkerResponse } from "./worker";

// One outstanding promise per request id; responses can arrive in any order.
let nextId = 0;
const waiting = new Map<number, (r: WorkerResponse) => void>();

/**
 * Starts the worker. If it dies — the browser ends it for memory, or its
 * script cannot be had — every request still waiting is answered with why,
 * rather than left to wait for ever, and the next request starts a new one.
 */
function start(): Worker {
  const w = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  w.addEventListener("message", (e: MessageEvent<WorkerResponse>) => {
    waiting.get(e.data.id)?.(e.data);
    waiting.delete(e.data.id);
  });
  const died = (e: Event) => {
    e.preventDefault();
    const message = navigator.onLine
      ? "the reader stopped, most likely for lack of memory. Close other tabs and try again, or use the command line tool"
      : "the reader could not start: you are offline, and it has not been saved for offline use yet";
    for (const [id, answer] of waiting) answer({ id, type: "error", message });
    waiting.clear();
    w.terminate();
    if (worker === w) worker = null;
  };
  w.addEventListener("error", died);
  w.addEventListener("messageerror", died);
  return w;
}

let worker: Worker | null = start();

export type Req = WorkerRequest extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;

export function call(req: Req): Promise<WorkerResponse> {
  const id = ++nextId;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    worker ??= start();
    worker.postMessage({ ...req, id } as WorkerRequest);
  });
}

/** The player's view of the worker's current stream. */
export const playerSource = {
  steps: async (from: number, count: number) => {
    const r = await call({ type: "steps", from, count });
    return r.type === "steps" ? r.steps : new Float64Array(0);
  },
  inflated: async () => {
    const r = await call({ type: "inflated" });
    return r.type === "inflated" ? r.bytes : new Uint8Array(0);
  },
  explain: async (index: number, knownBlock: number) => {
    const r = await call({ type: "explain", index, knownBlock });
    return r.type === "explain" ? { parts: r.parts, tables: r.tables } : { parts: new Float64Array(0), tables: null };
  },
};
