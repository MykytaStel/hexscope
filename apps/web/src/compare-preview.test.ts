import { expect, it } from "vitest";
import type { FileModel } from "./model";
import { canCompareVisual } from "./compare";

function image(format: string, bytes: number, width: number, height: number): FileModel {
  return { bytes: new Uint8Array(bytes), file: { format, dimensions: [width, height] } } as unknown as FileModel;
}

it("allows an in-budget pair of browser-decodable images", () => {
  expect(canCompareVisual(image("jpeg", 1024, 4000, 3000), image("png", 2048, 3000, 4000))).toBe(true);
});

it("keeps the wipe out for oversized, non-image and unknown-dimension inputs", () => {
  const a = image("jpeg", 1024, 4000, 3000);
  expect(canCompareVisual(a, image("heif", 16 * 1024 * 1024 + 1, 4000, 3000))).toBe(false);
  expect(canCompareVisual(a, image("jpeg", 1024, 4001, 3000))).toBe(false);
  expect(canCompareVisual(a, image("pdf", 1024, 400, 300))).toBe(false);
  expect(canCompareVisual(a, image("gif", 1024, 0, 300))).toBe(false);
});
