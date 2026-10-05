import { filmRecipe, type FilmSettings, type RGB } from "./film-lab";
let loading: Promise<typeof import("./wasm-raster/hexscope_raster")> | null = null;
export function rasterModule() {
  return loading ??= import("./wasm-raster/hexscope_raster").then(async (m) => { await m.default(); return m; }).catch((e) => { loading = null; throw e; });
}
/** The production pixel path shares Rust with the native CLI. */
export async function renderCorePixels(source: Uint8ClampedArray, width: number, height: number, settings: FilmSettings) {
  const m = await rasterModule();
  const r = m.renderRgba8(new Uint8Array(source.buffer, source.byteOffset, source.byteLength), width, height, filmRecipe(settings));
  try {
    const info = JSON.parse(r.info) as { baseColor: RGB; clipping: { shadows: number; highlights: number } };
    return { pixels: new Uint8ClampedArray(r.pixels), ...info };
  } finally { r.free(); }
}
