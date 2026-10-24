import { describe, expect, it } from "vitest";
import { imageDimensions } from "./image-size";

function png(width: number, height: number): Uint8Array {
  return Uint8Array.of(
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52,
    width >>> 24, width >>> 16, width >>> 8, width,
    height >>> 24, height >>> 16, height >>> 8, height,
  );
}

function jpeg(width: number, height: number): Uint8Array {
  return Uint8Array.of(
    0xff, 0xd8,
    0xff, 0xe0, 0, 4, 0, 0,
    0xff, 0xc0, 0, 11, 8, height >>> 8, height, width >>> 8, width, 1, 1, 0x11, 0,
  );
}

function webp(kind: "VP8 " | "VP8L" | "VP8X", payload: number[]): Uint8Array {
  const chunkLength = payload.length;
  const riffSize = 4 + 8 + chunkLength + (chunkLength & 1);
  return Uint8Array.of(
    0x52, 0x49, 0x46, 0x46,
    riffSize, riffSize >>> 8, riffSize >>> 16, riffSize >>> 24,
    0x57, 0x45, 0x42, 0x50,
    ...Array.from(kind, (c) => c.charCodeAt(0)),
    chunkLength, chunkLength >>> 8, chunkLength >>> 16, chunkLength >>> 24,
    ...payload,
    ...(chunkLength & 1 ? [0] : []),
  );
}

describe("imageDimensions", () => {
  it("reads PNG dimensions from the IHDR header", () => {
    expect(imageDimensions(png(4032, 3024))).toEqual([4032, 3024]);
  });

  it("finds JPEG frame dimensions after unrelated header segments", () => {
    expect(imageDimensions(jpeg(4032, 3024))).toEqual([4032, 3024]);
  });

  it("reads both GIF logical-screen versions", () => {
    expect(imageDimensions(Uint8Array.of(...Array.from("GIF87a", (c) => c.charCodeAt(0)), 0x80, 0x07, 0xe0, 0x05))).toEqual([1920, 1504]);
    expect(imageDimensions(Uint8Array.of(...Array.from("GIF89a", (c) => c.charCodeAt(0)), 0x80, 0x07, 0xe0, 0x05))).toEqual([1920, 1504]);
  });

  it("reads VP8X, VP8 and VP8L WebP dimensions", () => {
    expect(imageDimensions(webp("VP8X", [0, 0, 0, 0, 0x7f, 0x02, 0, 0xdf, 0x01, 0]))).toEqual([640, 480]);
    expect(imageDimensions(webp("VP8 ", [0, 0, 0, 0x9d, 0x01, 0x2a, 0x80, 0x02, 0xe0, 0x01]))).toEqual([640, 480]);
    expect(imageDimensions(webp("VP8L", [0x2f, 0x7f, 0xc2, 0x77, 0]))).toEqual([640, 480]);
  });

  it("reads bottom-up and top-down BMP dimensions", () => {
    const bytes = new Uint8Array(54);
    const view = new DataView(bytes.buffer);
    bytes.set([0x42, 0x4d]);
    view.setUint32(2, bytes.length, true);
    view.setUint32(14, 40, true);
    view.setInt32(18, 640, true);
    view.setInt32(22, -480, true);
    expect(imageDimensions(bytes)).toEqual([640, 480]);
  });

  it("returns no dimensions for malformed, truncated, zero-sized or unsupported images", () => {
    expect(imageDimensions(png(0, 10))).toBeUndefined();
    expect(imageDimensions(png(10, 10).subarray(0, 20))).toBeUndefined();
    expect(imageDimensions(Uint8Array.of(0xff, 0xd8, 0xff, 0xc0, 0, 2))).toBeUndefined();
    expect(imageDimensions(webp("VP8X", [0, 0, 0]))).toBeUndefined();
    expect(imageDimensions(new Uint8Array([1, 2, 3]))).toBeUndefined();
  });
});
