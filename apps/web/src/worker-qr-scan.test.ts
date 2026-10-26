import { afterEach, describe, expect, it, vi } from "vitest";
import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { rememberCode, scanCodes } from "./worker-qr-scan";

const qr = vi.hoisted(() => ({
  findCodes: vi.fn(() => []),
  luminance: vi.fn(() => new Uint8Array()),
  bitmapResizeOptions: vi.fn(() => undefined),
  imageDimensions: vi.fn(() => undefined),
}));
vi.mock("./qr/index", () => qr);

afterEach(() => vi.unstubAllGlobals());

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

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

  it("reads ZIP image entries from only their external compressed range", async () => {
    const output = new Uint8Array([11, 12]);
    const parsed = {
      entryDataRange: () => new Float64Array([2, 3]),
      extractEntry: vi.fn(() => new Uint8Array()),
      extractEntryFrom: vi.fn(() => output),
    } as unknown as Parsed;
    const bitmap = { width: 4, height: 4, close: vi.fn() } as unknown as ImageBitmap;
    vi.stubGlobal("createImageBitmap", vi.fn(async () => bitmap));
    vi.stubGlobal("OffscreenCanvas", class {
      getContext() {
        return {
          drawImage: vi.fn(),
          getImageData: () => ({ data: new Uint8ClampedArray(4 * 4 * 4) }),
        };
      }
    });
    qr.imageDimensions.mockClear();

    await scanCodes(
      { id: 3, type: "codes", entries: [{ index: 0, name: "photo.png" }] },
      new AbortController().signal,
      parsed,
      vi.fn() as unknown as Parameters<typeof scanCodes>[3],
      new Blob(["0123456789"]),
    );

    expect(parsed.extractEntryFrom).toHaveBeenCalledWith(0, new Uint8Array([50, 51, 52]));
    expect(parsed.extractEntry).not.toHaveBeenCalled();
    expect(qr.imageDimensions).toHaveBeenCalledWith(output);
    expect(bitmap.close).toHaveBeenCalledOnce();
  });

  it("frees PDF picture memory when cancellation interrupts image decoding", async () => {
    const controller = new AbortController();
    const reason = new DOMException("Reading cancelled", "AbortError");
    const decoding = deferred<ImageBitmap>();
    const bitmap = { width: 10, height: 10, close: vi.fn() } as unknown as ImageBitmap;
    const createBitmap = vi.fn(() => decoding.promise);
    vi.stubGlobal("createImageBitmap", createBitmap);

    const free = vi.fn();
    const pdfPictures = vi.fn(() => ({
      count: 1,
      page: () => 1,
      size: () => new Uint32Array([10, 10]),
      jpeg: () => new Float64Array([0, 1]),
      rgba: vi.fn(),
      free,
    }));
    const pdfShapes = vi.fn();
    const loadPdf = vi.fn(async () => ({ pdfPictures, pdfShapes }));
    const read = scanCodes(
      { id: 2, type: "codes", pdf: new Uint8Array([1]) },
      controller.signal,
      null as unknown as Parsed,
      loadPdf as unknown as Parameters<typeof scanCodes>[3],
    );
    await vi.waitFor(() => expect(createBitmap).toHaveBeenCalledOnce());

    controller.abort(reason);
    decoding.resolve(bitmap);

    await expect(read).rejects.toBe(reason);
    expect(bitmap.close).toHaveBeenCalledOnce();
    expect(free).toHaveBeenCalledOnce();
    expect(pdfShapes).not.toHaveBeenCalled();
  });
});
