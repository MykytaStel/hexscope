import { describe, expect, it } from "vitest";
import { bitmapResizeOptions } from "./bitmap-size";

describe("QR bitmap decode size", () => {
  it("caps a landscape image by its longer axis without distorting its ratio", () => {
    expect(bitmapResizeOptions([6000, 4000])).toEqual({ resizeWidth: 2400, resizeQuality: "high" });
  });

  it("caps a portrait image by its longer axis", () => {
    expect(bitmapResizeOptions([3000, 6000])).toEqual({ resizeHeight: 2400, resizeQuality: "high" });
  });

  it("does not resize a small image or trust invalid dimensions", () => {
    expect(bitmapResizeOptions([1200, 800])).toBeUndefined();
    expect(bitmapResizeOptions([0, 6000])).toBeUndefined();
    expect(bitmapResizeOptions([NaN, 6000])).toBeUndefined();
    expect(bitmapResizeOptions(undefined)).toBeUndefined();
  });
});
