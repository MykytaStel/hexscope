/** Admission limits apply before allocating a whole non-streaming input. */
export const MAX_INPUT_BYTES = 128 * 1024 * 1024;
export const PARSE_DEADLINE_MS = 60_000;

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
