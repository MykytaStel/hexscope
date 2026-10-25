// The page's line to the parsing worker. The worker keeps the parsed document,
// so requests after the first (steps, explanations, entries) are cheap.
import type { OperationRequest, WorkerRequest, WorkerResponse } from "./worker";

// One outstanding promise per request id; responses can arrive in any order.
let nextId = 0;
interface PendingRequest {
  finish(response: WorkerResponse): void;
  onProgress?: (progress: Extract<WorkerResponse, { type: "progress" }>["progress"]) => void;
}
const waiting = new Map<number, PendingRequest>();
const workerResetListeners = new Set<() => void>();

export interface CallOptions {
  signal?: AbortSignal;
  onProgress?: (progress: Extract<WorkerResponse, { type: "progress" }>["progress"]) => void;
}

/** Runs after a timed-out verification stops the parser worker. */
export function onWorkerReset(listener: () => void): () => void {
  workerResetListeners.add(listener);
  return () => workerResetListeners.delete(listener);
}

function notifyWorkerReset(): void {
  for (const listener of workerResetListeners) listener();
}

function failWaiting(message: string): void {
  for (const [id, pending] of waiting) pending.finish({ id, type: "error", message });
  waiting.clear();
}

/**
 * Starts the worker. If it dies — the browser ends it for memory, or its
 * script cannot be had — every request still waiting is answered with why,
 * rather than left to wait for ever, and the next request starts a new one.
 */
function start(): Worker {
  const w = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  w.addEventListener("message", (e: MessageEvent<WorkerResponse>) => {
    const pending = waiting.get(e.data.id);
    if (!pending) return;
    if (e.data.type === "progress") {
      pending.onProgress?.(e.data.progress);
      return;
    }
    if (e.data.type === "parsed") {
      try {
        // The worker keeps its previous document until the page accepts this
        // parse. Posting the commit before resolving preserves message order
        // for the next operation against the newly visible file.
        w.postMessage({ id: e.data.id, type: "commitParse", target: e.data.id } satisfies WorkerRequest);
      } catch {
        pending.finish({ id: e.data.id, type: "error", message: "the reader could not keep this file open; reopen it to continue" });
        return;
      }
    }
    pending.finish(e.data);
  });
  const died = (e: Event) => {
    // A terminated worker may still deliver a queued error after its
    // replacement has already started. It must not fail the new worker's RPCs.
    if (worker !== w) return;
    e.preventDefault();
    const message = navigator.onLine
      ? "the reader stopped, most likely for lack of memory. Close other tabs and try again, or use the command line tool"
      : "the reader could not start: you are offline, and it has not been saved for offline use yet";
    failWaiting(message);
    w.terminate();
    if (worker === w) worker = null;
  };
  w.addEventListener("error", died);
  w.addEventListener("messageerror", died);
  return w;
}

let worker: Worker | null = start();

export type Req = OperationRequest extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;

export function call(req: Req): Promise<WorkerResponse>;
export function call(req: Extract<Req, { type: "verifyCopy" }>, timeoutMs: number): Promise<WorkerResponse>;
export function call(req: Req, timeoutMs: number | undefined, options: CallOptions): Promise<WorkerResponse>;
export function call(req: Req, timeoutMs?: number, options?: CallOptions): Promise<WorkerResponse> {
  const id = ++nextId;
  return new Promise((resolve) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let abort: (() => void) | undefined;
    const finish = (response: WorkerResponse) => {
      if (!waiting.has(id)) return;
      if (timer !== undefined) clearTimeout(timer);
      if (abort) options?.signal?.removeEventListener("abort", abort);
      waiting.delete(id);
      resolve(response);
    };
    waiting.set(id, { finish, onProgress: options?.onProgress });
    let target: Worker;
    try {
      target = worker ??= start();
    } catch {
      finish({ id, type: "error", message: "the worker could not start this request" });
      return;
    }
    if (timeoutMs !== undefined) {
      timer = setTimeout(
        () => {
          finish({ id, type: "error", message: "copy verification did not finish in time" });
          if (worker !== target) return;
          // WebAssembly work is synchronous, so a cancel message could sit
          // behind the timed-out request. Terminating the worker is the only
          // way to stop it before it blocks the reader any longer.
          worker = null;
          target.terminate();
          failWaiting("the reader stopped after copy verification timed out; reopen the file to continue");
          notifyWorkerReset();
        },
        timeoutMs,
      );
    }
    if (options?.signal?.aborted) {
      finish({ id, type: "error", message: "Reading cancelled" });
      return;
    }
    if (options?.signal) {
      abort = () => {
        try {
          target.postMessage({ id, type: "cancel", target: id } as WorkerRequest);
        } catch {
          // The request still resolves as canceled if the worker has died.
        }
        finish({ id, type: "error", message: "Reading cancelled" });
      };
      options.signal.addEventListener("abort", abort, { once: true });
    }
    try {
      target.postMessage({ ...req, id } as WorkerRequest);
    } catch {
      finish({ id, type: "error", message: "the worker could not start this request" });
    }
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
