import { describe, expect, it, vi } from "vitest";
import { withWorkerResource } from "./worker-resource";

describe("worker-owned WebAssembly resources", () => {
  it("frees a result when QR work is aborted", async () => {
    const free = vi.fn();
    const abort = new DOMException("Reading cancelled", "AbortError");

    await expect(withWorkerResource({ free }, async () => { throw abort; })).rejects.toBe(abort);

    expect(free).toHaveBeenCalledOnce();
  });
});
