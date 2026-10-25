import { describe, expect, it, vi } from "vitest";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { rememberCode, scanCodes } from "./worker-qr-scan";

describe("unique QR result collection", () => {
  it("deduplicates codes with constant-time text lookup", () => {
    const found: { text: string; where: string; box: [number, number, number, number] }[] = [];
    const seen = new Set<string>();
    const code = { text: "same payload", corners: [{ x: 2, y: 4 }, { x: 8, y: 4 }, { x: 8, y: 10 }, { x: 2, y: 10 }] };

    expect(rememberCode(found, seen, code, "first image", 20, 20)).toBe(true);
    expect(rememberCode(found, seen, code, "second image", 20, 20)).toBe(false);
    expect(found).toEqual([{ text: "same payload", where: "first image", box: [0.1, 0.2, 0.4, 0.5] }]);
  });
});

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
