import { describe, expect, it } from "vitest";
import { boundedLabSize, cropGeometry, DEFAULT_FILM_SETTINGS, filmRecipe, readFilmRecipe, sampleFilmBase } from "./film-lab";
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

describe("film lab sampling helpers", () => {
  it("samples a robust patch at the displayed point and rejects points outside the photo", () => {
    const source = rgba(Array.from({ length: 100 }, () => [220, 150, 90]));
    source.set([255, 255, 255, 255], 44 * 4);
    expect(sampleFilmBase(source, 10, 10, [0.5, 0.5])).toEqual([220, 150, 90]);
    expect(() => sampleFilmBase(source, 10, 10, [2, 0])).toThrow();
  });
});
