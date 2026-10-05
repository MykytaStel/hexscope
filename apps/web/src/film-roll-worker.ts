import { rasterModule } from "./film-core";
import { filmRecipe, MAX_FILM_BYTES, validateFilmSettings, type FilmSettings } from "./film-lab";
import { renderFilm } from "./film-render";

export interface RollOutput {
  copy: Blob; width: number; height: number; sourceDepth: number; outputDepth: number;
  profilePresent: boolean; limited: boolean; baseColor: [number, number, number];
  sha256: string; bytesReadback: "matched"; metadataVerification: "not_checked";
}
/** One file at a time; caller owns cancellation between jobs. */
export async function renderRoll(source: Blob, settings: FilmSettings, format: "jpeg" | "tiff16", dimensions?: [number, number], orientation = 1): Promise<RollOutput> {
  if (source.size > MAX_FILM_BYTES) throw new Error("This scan exceeds the 50 MiB file limit.");
  const recipe = filmRecipe(validateFilmSettings(settings));
  const head = new Uint8Array(await source.slice(0, 4).arrayBuffer());
  const tiff = (head[0] === 73 && head[1] === 73 && (head[2] === 42 || head[2] === 43)) || (head[0] === 77 && head[1] === 77 && (head[3] === 42 || head[3] === 43));
  const m = await rasterModule();
  let pixels: Uint8Array, width: number, height: number, sourceDepth = 8, profilePresent = false, limited = false;
  let baseColor: [number, number, number], copy: Blob;
  if (tiff) {
    const r = m.processTiff(new Uint8Array(await source.arrayBuffer()), recipe, format === "jpeg" ? 4096 : 30000);
    try {
      const info = JSON.parse(r.info);
      ({ width, height, sourceDepth, profilePresent, limited, baseColor } = info);
      pixels = r.pixels;
      copy = new Blob([r.bytes as BlobPart], { type: "image/tiff" });
    } finally { r.free(); }
  } else {
    if (!dimensions) throw new Error("The source dimensions could not be inspected.");
    const result = await renderFilm(source, dimensions, orientation, settings, "export", undefined, "png");
    [width, height] = result.dimensions; ({ limited, baseColor } = result);
    const bitmap = await createImageBitmap(result.copy);
    try {
      const g = new OffscreenCanvas(width, height).getContext("2d", { willReadFrequently: true });
      if (!g) throw new Error("This browser cannot process scan pixels.");
      g.drawImage(bitmap, 0, 0);
      pixels = new Uint8Array(g.getImageData(0, 0, width, height).data);
      g.canvas.width = g.canvas.height = 1;
    } finally { bitmap.close(); }
    copy = new Blob([m.encodeTiff8(pixels, width, height) as BlobPart], { type: "image/tiff" });
  }
  if (format === "jpeg") {
    const canvas = new OffscreenCanvas(width!, height!);
    const g = canvas.getContext("2d");
    if (!g) throw new Error("This browser cannot encode a copy.");
    g.putImageData(new ImageData(new Uint8ClampedArray(pixels!), width!, height!), 0, 0);
    copy = await canvas.convertToBlob({ type: "image/jpeg", quality: 0.94 });
    canvas.width = canvas.height = 1;
    if (copy.type !== "image/jpeg") throw new Error("JPEG encoding is unavailable.");
    const actual = await createImageBitmap(copy);
    try { if (actual.width !== width! || actual.height !== height!) throw new Error("Output dimensions differ."); } finally { actual.close(); }
  }
  const bytes = await copy!.arrayBuffer();
  const sha256 = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), (b) => b.toString(16).padStart(2, "0")).join("");
  return { copy: copy!, width: width!, height: height!, sourceDepth, outputDepth: format === "tiff16" ? 16 : 8, profilePresent, limited, baseColor: baseColor!, sha256, bytesReadback: "matched", metadataVerification: "not_checked" };
}
