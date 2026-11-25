import { describe, expect, it } from "vitest";
import { isFatalReaderError } from "./resource-budget";

describe("reader failure classification", () => {
  it("recognizes a trapped WASM realm and allocation failures", () => {
    expect(isFatalReaderError(new WebAssembly.RuntimeError("unreachable"))).toBe(true);
    expect(isFatalReaderError(new RangeError("Array buffer allocation failed"))).toBe(true);
    expect(isFatalReaderError(new Error("out of memory"))).toBe(true);
  });
  it("keeps ordinary admission and input errors recoverable", () => {
    expect(isFatalReaderError(new Error("This file exceeds the 128 MiB browser reading limit."))).toBe(false);
    expect(isFatalReaderError(new Error("metadata memory budget exceeded"))).toBe(false);
    expect(isFatalReaderError(new DOMException("Reading cancelled", "AbortError"))).toBe(false);
  });
});
