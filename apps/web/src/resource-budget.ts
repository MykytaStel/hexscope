/** Admission limits apply before allocating a whole non-streaming input. */
export const MAX_INPUT_BYTES = 128 * 1024 * 1024;
export const PARSE_DEADLINE_MS = 60_000;

/** Batch-only heuristic, not a measurement of live WASM memory. */
export const BATCH_READER_RECYCLE_FILES = 8;
export const BATCH_READER_RECYCLE_BYTES = 32 * 1024 * 1024;

/** Optional retention only; this never reduces the supported input limit
 * or omits findings. deviceMemory is a coarse browser hint, not free RAM.
 */
export interface RetentionBudget {
  pageCount: number;
  pendingPages: number;
  batchFiles: number;
  batchBytes: number;
}

export function retentionBudget(deviceMemory?: number): RetentionBudget {
  if (typeof deviceMemory === "number" && Number.isFinite(deviceMemory) && deviceMemory > 0) {
    if (deviceMemory <= 2) return { pageCount: 16, pendingPages: 2, batchFiles: 2, batchBytes: 8 * 1024 * 1024 };
    if (deviceMemory > 4) return { pageCount: 64, pendingPages: 4, batchFiles: BATCH_READER_RECYCLE_FILES, batchBytes: BATCH_READER_RECYCLE_BYTES };
  }
  // Includes Safari/Firefox or other browsers without the device-memory API.
  return { pageCount: 32, pendingPages: 3, batchFiles: 4, batchBytes: 16 * 1024 * 1024 };
}

export function browserRetentionBudget(): RetentionBudget {
  const memory = typeof navigator === "undefined" ? undefined : (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
  return retentionBudget(memory);
}

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
