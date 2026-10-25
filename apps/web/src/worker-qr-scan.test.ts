import { describe, expect, it, vi } from "vitest";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { scanCodes } from "./worker-qr-scan";

describe("worker QR scan cancellation", () => {
  it("does not load parser resources for a canceled scan", async () => {
    const controller = new AbortController();
    const reason = new DOMException("Reading cancelled", "AbortError");
    controller.abort(reason);
    const loadPdf = vi.fn();

    await expect(scanCodes(
      { id: 1, type: "codes", picture: new Blob(["image"]) },
      controller.signal,
      null as unknown as Parsed,
      loadPdf,
    )).rejects.toBe(reason);

    expect(loadPdf).not.toHaveBeenCalled();
  });
});
