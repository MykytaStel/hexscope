export interface EntropyOptions {
  signal: AbortSignal;
  onProgress(loaded: number, total: number): void;
  /** Keep a minimum bytes-per-window requirement unless a view needs smaller bins. */
  minimumWindow?: number;
  /** Bound work per bin for an approximate overview. Omit to inspect each full window. */
  maxSampleBytes?: number;
}

export interface EntropyResult {
  values: Float32Array;
  window: number;
}

const MIN_WINDOW = 256;
const WORK_SLICE_MS = 6;
export const DEFAULT_ENTROPY_BINS = 1024;

function entropyOf(bytes: Uint8Array, counts: Uint32Array): number {
  counts.fill(0);
  for (const byte of bytes) counts[byte]++;
  const total = bytes.length;
  let entropy = 0;
  for (const count of counts) {
    if (count === 0) continue;
    const p = count / total;
    entropy -= p * Math.log2(p);
  }
  return entropy;
}

function yieldToBrowser(signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    let idle: number | undefined;
    let timer: number | undefined;
    const setTimer = window.setTimeout.bind(window);
    const cleanup = () => {
      signal.removeEventListener("abort", aborted);
      if (idle !== undefined && "cancelIdleCallback" in window) window.cancelIdleCallback(idle);
      if (timer !== undefined) window.clearTimeout(timer);
    };
    const done = () => {
      cleanup();
      resolve();
    };
    const aborted = () => {
      cleanup();
      reject(signal.reason ?? new DOMException("The byte map was cancelled.", "AbortError"));
    };
    signal.addEventListener("abort", aborted, { once: true });
    if ("requestIdleCallback" in window) idle = window.requestIdleCallback(done, { timeout: 50 });
    else timer = setTimer(done, 0);
    if (signal.aborted) aborted();
  });
}

/** Computes Core-compatible Shannon windows by default, yielding between work slices. */
export async function browserEntropy(bytes: Uint8Array, bins: number, options: EntropyOptions): Promise<EntropyResult> {
  if (bytes.length === 0 || bins <= 0) {
    options.onProgress(0, Math.max(1, bytes.length));
    return { values: new Float32Array(0), window: 0 };
  }

  const minimumWindow = Math.max(1, Math.floor(options.minimumWindow ?? MIN_WINDOW));
  const count = Math.max(1, Math.min(Math.floor(bins), Math.floor(bytes.length / minimumWindow)));
  const windowSize = Math.ceil(bytes.length / count);
  const values = new Float32Array(count);
  const counts = new Uint32Array(256);
  const sampleBytes = options.maxSampleBytes === undefined
    ? Number.POSITIVE_INFINITY
    : Math.max(1, Math.floor(options.maxSampleBytes));
  let index = 0;

  options.onProgress(0, bytes.length);
  options.signal.throwIfAborted();
  await yieldToBrowser(options.signal);

  while (index < count) {
    options.signal.throwIfAborted();
    const started = performance.now();
    while (index < count && performance.now() - started < WORK_SLICE_MS) {
      const start = index * windowSize;
      const end = Math.min(bytes.length, start + windowSize, start + sampleBytes);
      values[index] = end > start ? entropyOf(bytes.subarray(start, end), counts) : 0;
      index++;
    }
    options.onProgress(Math.min(bytes.length, index * windowSize), bytes.length);
    if (index < count) await yieldToBrowser(options.signal);
  }

  return { values, window: windowSize };
}
