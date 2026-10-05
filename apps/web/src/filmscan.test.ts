import { describe, expect, it } from "vitest";
import { inspectFilmScan } from "./filmscan";

function solid(width: number, height: number, value: number): Uint8Array {
  return new Uint8Array(width * height).fill(value);
}

function rect(
  pixels: Uint8Array,
  width: number,
  left: number,
  top: number,
  right: number,
  bottom: number,
  value: number,
): void {
  for (let y = top; y < bottom; y++) {
    for (let x = left; x < right; x++) pixels[y * width + x] = value;
  }
}

function perforatedFrame(): Uint8Array {
  const width = 320;
  const height = 200;
  const pixels = solid(width, height, 70);
  for (let x = 16; x < width - 16; x += 28) {
    rect(pixels, width, x, 4, x + 10, 14, 245);
    rect(pixels, width, x, height - 14, x + 10, height - 4, 245);
  }
  rect(pixels, width, 22, 22, width - 22, 25, 180);
  rect(pixels, width, 22, height - 25, width - 22, height - 22, 180);
  rect(pixels, width, 22, 22, 25, height - 22, 180);
  rect(pixels, width, width - 25, 22, width - 22, height - 22, 180);
  return pixels;
}

function randomGrain(width: number, height: number): Uint8Array {
  const pixels = new Uint8Array(width * height);
  let state = 0x12345678;
  for (let i = 0; i < pixels.length; i++) {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    pixels[i] = 96 + ((state >>> 24) % 65);
  }
  return pixels;
}

describe("film-scan visual evidence", () => {
  it("finds regular openings and a separate frame boundary in a film-border scan", () => {
    const report = inspectFilmScan(perforatedFrame(), 320, 200);

    expect(report.availability).toBe("inspected");
    expect(report.perforationEdges).toEqual(["top", "bottom"]);
    expect(report.perforationRepeats).toBeGreaterThanOrEqual(8);
    expect(report.frameEdges).toEqual(["top", "right", "bottom", "left"]);
    expect(report.evidenceStrength).toBe("corroborated");
  });

  it("does not treat random film-like grain as a repeated edge pattern", () => {
    const report = inspectFilmScan(randomGrain(320, 200), 320, 200);

    expect(report.perforationEdges).toEqual([]);
    expect(report.frameEdges).toEqual([]);
    expect(report.evidenceStrength).toBe("none");
  });

  it("keeps a cropped frame boundary separate from perforation evidence", () => {
    const width = 320;
    const height = 200;
    const pixels = solid(width, height, 35);
    rect(pixels, width, 16, 16, width - 16, height - 16, 180);
    const report = inspectFilmScan(pixels, width, height);

    expect(report.perforationEdges).toEqual([]);
    expect(report.frameEdges).toEqual(["top", "right", "bottom", "left"]);
    expect(report.evidenceStrength).toBe("limited");
  });

  it("does not inspect malformed or too-small pixel buffers", () => {
    expect(inspectFilmScan(new Uint8Array(12), 4, 4).availability).toBe("unavailable");
    expect(inspectFilmScan(new Uint8Array(64 * 64 - 1), 64, 64).availability).toBe("unavailable");
  });
});
