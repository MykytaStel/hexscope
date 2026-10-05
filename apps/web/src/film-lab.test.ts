import { describe, expect, it } from "vitest";
import { boundedLabSize, cropGeometry, DEFAULT_FILM_SETTINGS, filmRecipe, readFilmRecipe, renderFilmPixels, sampleFilmBase } from "./film-lab";
import { LatestFilmQueue } from "./film-queue";

const rgba = (values: number[][]) => new Uint8ClampedArray(values.flatMap((pixel) => [...pixel, 255]));

it("serializes film work globally and discards an obsolete pending request", async () => {
  let release!: () => void;
  const started: number[] = [], discarded: number[] = [];
  const queue = new LatestFilmQueue<number>(async (id) => {
    started.push(id);
    if (id === 1) await new Promise<void>((resolve) => { release = resolve; });
  }, (id) => discarded.push(id));
  queue.push(1); await Promise.resolve();
  queue.push(2); queue.push(3);
  expect(started).toEqual([1]); expect(discarded).toEqual([2]);
  release();
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(started).toEqual([1, 3]);
});

describe("film lab geometry and recipes", () => {
  it("crops the displayed orientation with independent margins and keeps a nonempty image", () => {
    expect(cropGeometry(400, 200, [0.1, 0.2, 0.15, 0.05])).toEqual({ x: 40, y: 40, width: 300, height: 150 });
    expect(cropGeometry(2, 2, [0.45, 0.45, 0.45, 0.45]).width).toBeGreaterThan(0);
    expect(() => cropGeometry(400, 200, [NaN, 0, 0, 0])).toThrow();
    expect(() => cropGeometry(400, 200, [-0.1, 0, 0, 0])).toThrow();
  });

  it("bounds working rasters and rejects unreasonable source dimensions before decoding", () => {
    const size = boundedLabSize(12000, 8000, "export");
    expect(size.width * size.height).toBeLessThanOrEqual(12_000_000);
    expect(Math.max(size.width, size.height)).toBeLessThanOrEqual(4096);
    expect(boundedLabSize(12000, 8000, "preview").width).toBe(1200);
    expect(() => boundedLabSize(50000, 50000, "preview")).toThrow();
    expect(() => boundedLabSize(0, 100, "export")).toThrow();
  });

  it("round-trips calibrated settings without storing a filename or image bytes", () => {
    const settings = { ...DEFAULT_FILM_SETTINGS, mode: "color-negative" as const, baseColor: [220, 160, 90] as [number, number, number] };
    const json = filmRecipe(settings);
    expect(readFilmRecipe(json)).toEqual(settings);
    expect(json).not.toContain("filename");
    expect(() => readFilmRecipe(json.replace('"version": 1', '"version": 2'))).toThrow();
    expect(() => readFilmRecipe(json.replace('"exposure": 0', '"exposure": 999'))).toThrow();
    expect(() => readFilmRecipe("[]")).toThrow();
  });
});

describe("film lab rendering", () => {
  it("preserves a positive at neutral controls and never changes its input", () => {
    const source = rgba([[10, 80, 200], [220, 160, 40]]);
    const before = source.slice();
    const result = renderFilmPixels(source, 2, 1, DEFAULT_FILM_SETTINGS);
    expect(result.pixels).toEqual(before);
    expect(source).toEqual(before);
    expect(result.pixels).not.toBe(source);
  });

  it("inverts grayscale density and keeps a monochrome negative achromatic", () => {
    const source = rgba(Array.from({ length: 64 }, (_, i) => [30 + i * 3, 30 + i * 3, 30 + i * 3]));
    const result = renderFilmPixels(source, 64, 1, { ...DEFAULT_FILM_SETTINGS, mode: "mono-negative" });
    expect(result.pixels[4 * 10]).toBeGreaterThan(result.pixels[4 * 50]);
    for (let i = 0; i < 64; i++) {
      expect(result.pixels[i * 4]).toBe(result.pixels[i * 4 + 1]);
      expect(result.pixels[i * 4 + 1]).toBe(result.pixels[i * 4 + 2]);
    }
  });

  it("removes a sampled orange base from equal channel transmissions", () => {
    const base: [number, number, number] = [230, 180, 120];
    const linear = (v: number) => v / 255 <= 0.04045 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4;
    const srgb = (v: number) => Math.round(255 * (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055));
    const source = rgba(Array.from({ length: 100 }, (_, i) => base.map((channel) => srgb(linear(channel) * (0.1 + i * 0.008)))));
    const result = renderFilmPixels(source, 100, 1, { ...DEFAULT_FILM_SETTINGS, mode: "color-negative", baseColor: base });
    const middle = Array.from(result.pixels.slice(200, 203));
    expect(Math.max(...middle) - Math.min(...middle)).toBeLessThanOrEqual(4);
    const uncalibrated = renderFilmPixels(source, 100, 1, { ...DEFAULT_FILM_SETTINGS, mode: "color-negative", baseColor: [255, 255, 255] });
    expect(Array.from(uncalibrated.pixels.slice(200, 203))).not.toEqual(middle);
  });

  it("reports source clipping and limits invalid buffers and controls", () => {
    const source = rgba([[0, 50, 60], [80, 90, 255], [100, 100, 100], [120, 120, 120]]);
    const result = renderFilmPixels(source, 4, 1, DEFAULT_FILM_SETTINGS);
    expect(result.clipping).toEqual({ shadows: 25, highlights: 25 });
    expect(() => renderFilmPixels(source, 400, 1, DEFAULT_FILM_SETTINGS)).toThrow();
    expect(() => renderFilmPixels(source, 4, 1, { ...DEFAULT_FILM_SETTINGS, contrast: Infinity })).toThrow();
  });

  it("samples a robust patch at the displayed point and rejects points outside the photo", () => {
    const source = rgba(Array.from({ length: 100 }, () => [220, 150, 90]));
    source.set([255, 255, 255, 255], 44 * 4);
    expect(sampleFilmBase(source, 10, 10, [0.5, 0.5])).toEqual([220, 150, 90]);
    expect(() => sampleFilmBase(source, 10, 10, [2, 0])).toThrow();
  });
});
