import { boundedLabSize, cropGeometry, MAX_FILM_BYTES, sampleFilmBase, validateFilmSettings, type FilmSettings, type RGB } from "./film-lab";
import { renderCorePixels } from "./film-core";
import type { FilmScanReport } from "./filmscan";

export interface FilmOutputCheck {
  status: "checked" | "unavailable";
  metadata: string[];
  qrCount: number | null;
  scan?: FilmScanReport;
  sha256?: string;
}
export interface FilmRenderResult {
  copy: Blob;
  sourcePreview?: Blob;
  baseColor: RGB;
  dimensions: [number, number];
  sourceDimensions: [number, number];
  limited: boolean;
  clipping: { shadows: number; highlights: number };
  check?: FilmOutputCheck;
}

/** No worker-global source state: an old request can never edit a newly opened file. */
export async function renderFilm(source: Blob, dimensions: [number, number], orientation: number, given: FilmSettings, purpose: "preview" | "export", point?: [number, number], encoding: "jpeg" | "png" = "jpeg"): Promise<FilmRenderResult> {
  if (source.size > MAX_FILM_BYTES) throw new Error("This scan exceeds the film lab's 50 MiB file limit.");
  const settings = validateFilmSettings(given);
  const [sw, sh] = orientation >= 5 && orientation <= 8 ? [dimensions[1], dimensions[0]] : dimensions;
  const size = boundedLabSize(sw, sh, purpose);
  const bitmap = await createImageBitmap(source, { imageOrientation: "from-image", colorSpaceConversion: "default", resizeWidth: size.width, resizeHeight: size.height });
  let original: OffscreenCanvas | undefined;
  let output: OffscreenCanvas | undefined;
  try {
    if (bitmap.width !== size.width || bitmap.height !== size.height) throw new Error("This browser cannot resize the scan safely.");
    original = new OffscreenCanvas(size.width, size.height);
    const g = original.getContext("2d", { willReadFrequently: true });
    if (!g) throw new Error("This browser cannot process scan pixels.");
    g.fillStyle = "#fff";
    g.fillRect(0, 0, size.width, size.height);
    g.drawImage(bitmap, 0, 0);
    bitmap.close();
    let baseColor = settings.baseColor;
    if (point) {
      if (point.length !== 2 || point.some((v) => !Number.isFinite(v) || v < 0 || v > 1)) throw new Error("Invalid film sample.");
      const x = Math.max(0, Math.min(size.width - 1, Math.floor(point[0] * size.width)) - 4);
      const y = Math.max(0, Math.min(size.height - 1, Math.floor(point[1] * size.height)) - 4);
      const patch = g.getImageData(x, y, Math.min(9, size.width - x), Math.min(9, size.height - y));
      baseColor = sampleFilmBase(patch.data, patch.width, patch.height, [0.5, 0.5]);
    } else if (!baseColor) {
      const small = new OffscreenCanvas(Math.min(256, size.width), Math.min(256, size.height));
      const sg = small.getContext("2d", { willReadFrequently: true });
      if (!sg) throw new Error("This browser cannot process scan pixels.");
      sg.drawImage(original, 0, 0, small.width, small.height);
      baseColor = sampleFilmBase(sg.getImageData(0, 0, small.width, small.height).data, small.width, small.height);
      small.width = small.height = 1;
    }
    const crop = cropGeometry(size.width, size.height, settings.crop);
    output = new OffscreenCanvas(crop.width, crop.height);
    const og = output.getContext("2d", { willReadFrequently: true });
    if (!og) throw new Error("This browser cannot process scan pixels.");
    og.drawImage(original, crop.x, crop.y, crop.width, crop.height, 0, 0, crop.width, crop.height);
    const sourcePreview = purpose === "preview" ? await original.convertToBlob({ type: "image/jpeg", quality: 0.92 }) : undefined;
    original.width = original.height = 1;
    const rendered = await renderCorePixels(og.getImageData(0, 0, crop.width, crop.height).data, crop.width, crop.height, { ...settings, baseColor });
    og.putImageData(new ImageData(rendered.pixels as Uint8ClampedArray<ArrayBuffer>, crop.width, crop.height), 0, 0);
    const copy = await output.convertToBlob({ type: purpose === "export" ? `image/${encoding}` : "image/png", quality: 0.94 });
    if (purpose === "export" && copy.type !== `image/${encoding}`) throw new Error("This browser cannot encode a JPEG copy.");
    return { copy, sourcePreview, baseColor: rendered.baseColor, dimensions: [crop.width, crop.height], sourceDimensions: [sw, sh], limited: size.width < sw || size.height < sh, clipping: rendered.clipping };
  } finally {
    bitmap.close();
    if (original) original.width = original.height = 1;
    if (output) output.width = output.height = 1;
  }
}
