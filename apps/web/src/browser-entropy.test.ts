import { describe, expect, it, vi } from "vitest";
import { browserEntropy, entropySliceMs } from "./browser-entropy";

function reference(bytes: Uint8Array, window: number, count: number, sample = Infinity): Float32Array {
  const result = new Float32Array(count);
  for (let i = 0; i < count; i++) {
    const part = bytes.subarray(i * window, Math.min(bytes.length, i * window + window, i * window + sample));
    const histogram = new Uint32Array(256);
    for (const b of part) histogram[b]++;
    let h = 0;
    for (const n of histogram) if (n) {
      const p = n / part.length;
      h -= p * Math.log2(p);
    }
    result[i] = h;
  }
  return result;
}

describe("cooperative entropy", () => {
  it("preserves exact windows and optional sample semantics", async () => {
    const bytes = Uint8Array.from({ length: 200_003 }, (_, i) => (i * 73 + (i >>> 9)) & 255);
    for (const sample of [undefined, 511]) {
      const result = await browserEntropy(bytes, 3, {
        signal: new AbortController().signal,
        onProgress: vi.fn(),
        maxSampleBytes: sample,
      });
      expect(result.values).toEqual(reference(bytes, result.window, result.values.length, sample));
    }
  });

  it("can abort in the middle of one large exact window", async () => {
    const controller = new AbortController();
    // Advance the slice clock per observation, independent of host speed.
    let now = 0;
    vi.stubGlobal("performance", { now: () => now += 10 });
    try {
      const progress = vi.fn((loaded: number) => {
        if (progress.mock.calls.length === 2) {
          expect(loaded).toBe(0);
          controller.abort();
        }
      });
      await expect(browserEntropy(new Uint8Array(1024 * 1024), 1, {
        signal: controller.signal, onProgress: progress,
      })).rejects.toMatchObject({ name: "AbortError" });
      expect(progress).toHaveBeenCalledTimes(2);
    } finally { vi.unstubAllGlobals(); }
  });

  it("uses a conservative slice for unknown or low concurrency", () => {
    for (const cores of [undefined, NaN, Infinity, 0, 2, 4]) expect(entropySliceMs(cores)).toBe(3);
    expect(entropySliceMs(8)).toBe(6);
  });
});
