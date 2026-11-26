import { describe, expect, it } from "vitest";
import { allowDocumentPrefetch, assertInputBudget, MAX_INPUT_BYTES, retentionBudget } from "./resource-budget";

describe("whole-input admission", () => {
  it("rejects oversized or invalid inputs before allocating their buffers", () => {
    for (const size of [MAX_INPUT_BYTES + 1, Infinity, NaN, -1]) {
      expect(() => assertInputBudget(size)).toThrow();
    }
    expect(() => assertInputBudget(MAX_INPUT_BYTES)).not.toThrow();
  });
});

describe("speculative parser downloads", () => {
  it("requires a reported fast connection and sufficient device memory", () => {
    expect(allowDocumentPrefetch({ effectiveType: "4g" }, 8)).toBe(true);
    expect(allowDocumentPrefetch({ effectiveType: "4g", saveData: true }, 8)).toBe(false);
    expect(allowDocumentPrefetch({ effectiveType: "3g" }, 8)).toBe(false);
    expect(allowDocumentPrefetch({ effectiveType: "4g" }, 4)).toBe(false);
    expect(allowDocumentPrefetch({ effectiveType: "4g" })).toBe(false);
    expect(allowDocumentPrefetch()).toBe(false);
  });
});

describe("adaptive retention budgets", () => {
  it("uses bounded low, unknown and roomy profiles", () => {
    expect(retentionBudget(2)).toEqual({ pageCount: 16, pendingPages: 2, batchFiles: 2, batchBytes: 8 * 1024 * 1024 });
    expect(retentionBudget(8)).toEqual({ pageCount: 64, pendingPages: 4, batchFiles: 8, batchBytes: 32 * 1024 * 1024 });
    for (const hint of [undefined, NaN, Infinity, -1, 0, 4]) {
      expect(retentionBudget(hint)).toEqual({ pageCount: 32, pendingPages: 3, batchFiles: 4, batchBytes: 16 * 1024 * 1024 });
    }
  });
});
