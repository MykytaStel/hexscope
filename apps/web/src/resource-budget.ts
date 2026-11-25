/** Admission limits apply before allocating a whole non-streaming input. */
export const MAX_INPUT_BYTES = 128 * 1024 * 1024;
export const PARSE_DEADLINE_MS = 60_000;

/** Batch-only heuristic, not a measurement of live WASM memory. */
export const BATCH_READER_RECYCLE_FILES = 8;
export const BATCH_READER_RECYCLE_BYTES = 32 * 1024 * 1024;

/** Do not mistake ordinary resource admission errors for a poisoned WASM realm. */
export function isFatalReaderError(error: unknown): boolean {
  if (error instanceof WebAssembly.RuntimeError) return true;
  const message = error instanceof Error ? error.message : String(error);
  return /out of memory|memory allocation failed|allocation failed|array buffer allocation failed|unable to allocate|memory\.grow.*fail|invalid typed array length/i.test(message);
}

export function assertInputBudget(size: number): void {
  if (!Number.isSafeInteger(size) || size < 0 || size > MAX_INPUT_BYTES) {
    throw new Error("This file exceeds the 128 MiB browser reading limit. Use the command line tool for larger files. Large supported movies are read in slices.");
  }
}

/** Speculative downloads are optional; unknown capabilities are not fast ones. */
export function allowDocumentPrefetch(
  connection?: { saveData?: boolean; effectiveType?: string },
  deviceMemory?: number,
): boolean {
  return connection?.saveData !== true && connection?.effectiveType === "4g" &&
    typeof deviceMemory === "number" && Number.isFinite(deviceMemory) && deviceMemory > 4;
}
