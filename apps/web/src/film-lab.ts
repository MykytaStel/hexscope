/** Bounded raster tools. This is a rendering recipe, never a film-origin classifier. */
export type FilmMode = "positive" | "color-negative" | "mono-negative";
export type RGB = [number, number, number];
/** Fractions removed from left, top, right and bottom, in displayed orientation. */
export type FilmCrop = [number, number, number, number];
export interface FilmSettings {
  mode: FilmMode;
  crop: FilmCrop;
  exposure: number;
  contrast: number;
  baseColor: RGB | null;
}
export const DEFAULT_FILM_SETTINGS: FilmSettings = { mode: "positive", crop: [0, 0, 0, 0], exposure: 0, contrast: 1, baseColor: null };
export const MAX_FILM_BYTES = 50 * 1024 * 1024;
const MAX_PIXELS = 12_000_000;
const MAX_SAMPLES = 131_072;
const LINEAR = Float64Array.from({ length: 256 }, (_, v) => {
  const s = v / 255;
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
});
const srgb = (v: number) => Math.round(255 * Math.max(0, Math.min(1, v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055)));

function bounded(value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < min || value > max) throw new Error("Invalid film settings.");
  return value;
}

export function validateFilmSettings(value: unknown): FilmSettings {
  if (!value || typeof value !== "object") throw new Error("Invalid film settings.");
  const s = value as Record<string, unknown>;
  if (!["positive", "color-negative", "mono-negative"].includes(s.mode as string) || !Array.isArray(s.crop) || s.crop.length !== 4) throw new Error("Invalid film settings.");
  let baseColor: RGB | null = null;
  if (s.baseColor !== null) {
    if (!Array.isArray(s.baseColor) || s.baseColor.length !== 3) throw new Error("Invalid film settings.");
    baseColor = s.baseColor.map((v) => Math.round(bounded(v, 1, 255))) as RGB;
  }
  return {
    mode: s.mode as FilmMode,
    crop: s.crop.map((v) => bounded(v, 0, 0.45)) as FilmCrop,
    exposure: bounded(s.exposure, -2, 2),
    contrast: bounded(s.contrast, 0.5, 2),
    baseColor,
  };
}

export function filmRecipe(settings: FilmSettings): string {
  return JSON.stringify({ schema: "hexscope.film-recipe", version: 1, algorithm: "srgb-density-v1", settings: validateFilmSettings(settings) }, null, 2);
}

export function readFilmRecipe(text: string): FilmSettings {
  if (text.length > 16_384) throw new Error("Invalid film recipe.");
  const r: unknown = JSON.parse(text);
  if (!r || typeof r !== "object") throw new Error("Invalid film recipe.");
  const recipe = r as Record<string, unknown>;
  if (recipe.schema !== "hexscope.film-recipe" || recipe.version !== 1 || recipe.algorithm !== "srgb-density-v1") throw new Error("Invalid film recipe.");
  return validateFilmSettings(recipe.settings);
}

export function boundedLabSize(width: number, height: number, purpose: "preview" | "export"): { width: number; height: number } {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || width > 30000 || height > 30000 || width * height > 120_000_000) throw new Error("This scan exceeds the film lab's 120 megapixel source limit.");
  const side = purpose === "preview" ? 1200 : 4096;
  const pixels = purpose === "preview" ? 1_440_000 : MAX_PIXELS;
  const scale = Math.min(1, side / Math.max(width, height), Math.sqrt(pixels / (width * height)));
  return { width: Math.max(1, Math.floor(width * scale)), height: Math.max(1, Math.floor(height * scale)) };
}

export function cropGeometry(width: number, height: number, crop: FilmCrop): { x: number; y: number; width: number; height: number } {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || !Array.isArray(crop) || crop.length !== 4) throw new Error("Invalid film crop.");
  crop.forEach((v) => bounded(v, 0, 0.45));
  const x = Math.min(width - 1, Math.round(width * crop[0]));
  const y = Math.min(height - 1, Math.round(height * crop[1]));
  return { x, y, width: Math.max(1, Math.round(width * (1 - crop[2])) - x), height: Math.max(1, Math.round(height * (1 - crop[3])) - y) };
}

function checkRaster(pixels: Uint8ClampedArray, width: number, height: number): number {
  const size = width * height;
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || size > MAX_PIXELS || pixels.length !== size * 4) throw new Error("Invalid film raster.");
  return size;
}

function percentile(histogram: Uint32Array, count: number, fraction: number): number {
  const target = Math.max(1, Math.ceil(count * fraction));
  let sum = 0;
  for (let i = 0; i < histogram.length; i++) {
    sum += histogram[i];
    if (sum >= target) return i;
  }
  return histogram.length - 1;
}

/** Median of a small patch; auto mode estimates the brightest channel transmission. */
export function sampleFilmBase(pixels: Uint8ClampedArray, width: number, height: number, point?: [number, number] | null): RGB {
  const size = checkRaster(pixels, width, height);
  const hist = [new Uint32Array(256), new Uint32Array(256), new Uint32Array(256)];
  let count = 0;
  const add = (i: number) => { for (let c = 0; c < 3; c++) hist[c][pixels[i * 4 + c]]++; count++; };
  if (point) {
    if (!Array.isArray(point) || point.length !== 2) throw new Error("Invalid film sample.");
    point.forEach((v) => bounded(v, 0, 1));
    const x = Math.min(width - 1, Math.floor(point[0] * width));
    const y = Math.min(height - 1, Math.floor(point[1] * height));
    for (let py = Math.max(0, y - 4); py <= Math.min(height - 1, y + 4); py++) {
      for (let px = Math.max(0, x - 4); px <= Math.min(width - 1, x + 4); px++) add(py * width + px);
    }
  } else {
    const step = Math.max(1, Math.ceil(size / MAX_SAMPLES));
    for (let i = 0; i < size; i += step) add(i);
  }
  return hist.map((h) => Math.max(1, percentile(h, count, point ? 0.5 : 0.95))) as RGB;
}

/**
 * Convert sRGB to linear transmission, normalize by film base, then map log
 * density through common 1%/99% tone bounds. Shared bounds preserve calibration
 * differences across channels. An 8-bit scan is already tone mapped; this is
 * an adjustable sharing rendition, not calibrated archival color recovery.
 * 256-entry lookup tables keep per-pixel work linear and bounded.
 */
export function renderFilmPixels(source: Uint8ClampedArray, width: number, height: number, given: FilmSettings): { pixels: Uint8ClampedArray; baseColor: RGB; clipping: { shadows: number; highlights: number } } {
  const size = checkRaster(source, width, height);
  const settings = validateFilmSettings(given);
  const baseColor = settings.baseColor ?? sampleFilmBase(source, width, height);
  const density = baseColor.map((base) => Float64Array.from(LINEAR, (v) => -Math.log(Math.max(0.0001, Math.min(1, v / LINEAR[base])))));
  const tones = new Uint32Array(2048);
  const step = Math.max(1, Math.ceil(size / MAX_SAMPLES));
  let count = 0, shadows = 0, highlights = 0;
  for (let i = 0; i < size; i += step) {
    const rgb = [source[i * 4], source[i * 4 + 1], source[i * 4 + 2]];
    if (rgb.some((v) => v <= 1)) shadows++;
    if (rgb.some((v) => v >= 254)) highlights++;
    const d = (density[0][rgb[0]] + density[1][rgb[1]] + density[2][rgb[2]]) / 3;
    tones[Math.min(2047, Math.floor(d / 10 * 2047))]++;
    count++;
  }
  const low = percentile(tones, count, 0.01) / 2047 * 10;
  const high = percentile(tones, count, 0.99) / 2047 * 10;
  const adjust = (v: number) => srgb(0.18 * (Math.max(0, v) / 0.18) ** settings.contrast * 2 ** settings.exposure);
  const lut = density.map((channel) => Uint8Array.from(channel, (d, v) => adjust(settings.mode === "positive" ? LINEAR[v] : high - low < 0.005 ? 0.18 : Math.max(0, (d - low) / (high - low)))));
  const pixels = new Uint8ClampedArray(source.length);
  for (let i = 0; i < pixels.length; i += 4) {
    const r = lut[0][source[i]], g = lut[1][source[i + 1]], b = lut[2][source[i + 2]];
    if (settings.mode === "mono-negative") pixels[i] = pixels[i + 1] = pixels[i + 2] = Math.round(0.2126 * r + 0.7152 * g + 0.0722 * b);
    else { pixels[i] = r; pixels[i + 1] = g; pixels[i + 2] = b; }
    pixels[i + 3] = 255;
  }
  return { pixels, baseColor, clipping: { shadows: Math.round(shadows / count * 1000) / 10, highlights: Math.round(highlights / count * 1000) / 10 } };
}
