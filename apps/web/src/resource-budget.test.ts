import { describe, expect, it } from "vitest";
import { allowDocumentPrefetch, assertInputBudget, MAX_INPUT_BYTES } from "./resource-budget";

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
