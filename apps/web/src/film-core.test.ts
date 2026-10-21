import { readFileSync } from "node:fs";
import { beforeAll } from "vitest";
import { expect, it } from "vitest";
import { renderCorePixels } from "./film-core";
import { DEFAULT_FILM_SETTINGS } from "./film-lab";
import * as rasterBuild from "./wasm-raster/hexscope_raster";

beforeAll(() => rasterBuild.initSync({ module: readFileSync(new URL("./wasm-raster/hexscope_raster_bg.wasm", import.meta.url)) }));

it("renders an ordinary photo above the WASM sampling cap", async () => {
  const width = 512;
  const height = 512;
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let offset = 0; offset < pixels.length; offset += 4) {
    pixels[offset] = 64;
    pixels[offset + 1] = 96;
    pixels[offset + 2] = 128;
    pixels[offset + 3] = 255;
  }

  const rendered = await renderCorePixels(pixels, width, height, DEFAULT_FILM_SETTINGS);

  expect(rendered.pixels).toHaveLength(width * height * 4);
  expect(rendered.pixels[3]).toBe(255);
  expect(rendered.pixels.at(-1)).toBe(255);
});
