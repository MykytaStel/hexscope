import { describe, expect, it } from "vitest";
import { assertInputBudget, MAX_INPUT_BYTES } from "./resource-budget";

describe("whole-input admission", () => {
  it("rejects oversized or invalid inputs before allocating their buffers", () => {
    for (const size of [MAX_INPUT_BYTES + 1, Infinity, NaN, -1]) {
      expect(() => assertInputBudget(size)).toThrow();
    }
    expect(() => assertInputBudget(MAX_INPUT_BYTES)).not.toThrow();
  });
});
