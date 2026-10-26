import type { Parsed } from "./wasm-pdf/hexscope_pdf.js";
import { withWorkerResource } from "./worker-resource";
import type { OperationRequest, WorkerResponse } from "./worker";
import type { FilmScanReport } from "./filmscan";
import { readEntry } from "./worker-entry-source";

type CodeRequest = Extract<OperationRequest, { type: "codes" }>;
type CodeResult = Pick<Extract<WorkerResponse, { type: "codes" }>, "codes" | "filmScan">;
type PdfModule = Pick<typeof import("./wasm-pdf/hexscope_pdf.js"), "pdfPictures" | "pdfShapes">;

/** A picture's longer side, at most, when looking for QR codes in it. */
const CODE_SCAN = 2400;
/** Pages of a PDF whose pictures are looked through for codes. */
const PDF_PAGES = 30;

/** Adds one decoded result with normalized bounds, deduplicating by payload in constant time. */
export function rememberCode(
  found: CodeResult["codes"],
  seen: Set<string>,
  code: { text: string; corners: { x: number; y: number }[] },
  where: string,
  width: number,
  height: number,
): boolean {
  if (seen.has(code.text)) return false;
  seen.add(code.text);
  const xs = code.corners.map((q) => q.x / width);
  const ys = code.corners.map((q) => q.y / height);
  found.push({ text: code.text, where, box: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] });
  return true;
}

/** A picture's brightness, one byte a pixel, scaled down if it is large. */
async function brightness(
  image: Blob | ImageData,
  resize: ImageBitmapOptions | undefined,
  luminance: (rgba: Uint8ClampedArray | Uint8Array, width: number, height: number) => Uint8Array,
  signal: AbortSignal,
): Promise<{ lum: Uint8Array; width: number; height: number } | null> {
  signal.throwIfAborted();
  let bitmap: ImageBitmap | null = null;
  try {
    if (resize) {
      try {
        bitmap = await createImageBitmap(image, resize);
      } catch {
        // Older browsers may decode this image but not support resize options.
      }
      signal.throwIfAborted();
    }
    bitmap ??= await createImageBitmap(image).catch(() => null);
    signal.throwIfAborted();
    if (!bitmap) return null;
    const scale = Math.min(1, CODE_SCAN / Math.max(bitmap.width, bitmap.height));
    const width = Math.max(1, Math.round(bitmap.width * scale));
    const height = Math.max(1, Math.round(bitmap.height * scale));
    const g = new OffscreenCanvas(width, height).getContext("2d", { willReadFrequently: true });
    if (!g) return null;
    g.drawImage(bitmap, 0, 0, width, height);
    return { lum: luminance(g.getImageData(0, 0, width, height).data, width, height), width, height };
  } finally { bitmap?.close(); }
}

/** Scans a picture, PDF, or a document's embedded pictures; honors the owning worker request's cancellation. */
export async function scanCodes(
  req: CodeRequest,
  signal: AbortSignal,
  current: Parsed | null,
  loadPdf: () => Promise<PdfModule>,
  source: Blob | null = null,
  isCurrent: () => boolean = () => true,
): Promise<CodeResult> {
  signal.throwIfAborted();
  const { findCodes, luminance, bitmapResizeOptions, imageDimensions } = await import("./qr/index");
  signal.throwIfAborted();
  const found: CodeResult["codes"] = [];
  const seen = new Set<string>();
  let filmScan: FilmScanReport | undefined;
  const look = async (image: Blob | ImageData, where: string, dimensions?: readonly [number, number]) => {
    signal.throwIfAborted();
    const p = await brightness(image, bitmapResizeOptions(dimensions, CODE_SCAN), luminance, signal);
    signal.throwIfAborted();
    if (req.inspectFilmScan) {
      try {
        const { inspectFilmScan } = await import("./filmscan");
        signal.throwIfAborted();
        filmScan = p
          ? inspectFilmScan(p.lum, p.width, p.height)
          : inspectFilmScan(new Uint8Array(0), 0, 0);
      } catch {
        if (signal.aborted) throw signal.reason;
        filmScan = {
          availability: "unavailable",
          perforationEdges: [],
          perforationRepeats: 0,
          frameEdges: [],
          evidenceStrength: "none",
        };
      }
    }
    if (!p) return;
    for (const c of findCodes(p.lum, p.width, p.height)) {
      signal.throwIfAborted();
      rememberCode(found, seen, c, where, p.width, p.height);
    }
  };
  if (req.picture) await look(req.picture, "", req.pictureDimensions);
  if (req.pdf) {
    const pdf = await loadPdf();
    signal.throwIfAborted();
    await withWorkerResource(pdf.pdfPictures(req.pdf, PDF_PAGES), async (pictures) => {
      for (let i = 0; i < pictures.count; i++) {
        signal.throwIfAborted();
        const where = `the picture on page ${pictures.page(i)}`;
        const [width, height] = pictures.size(i);
        const at = pictures.jpeg(i);
        if (at.length === 2) {
          await look(new Blob([req.pdf!.subarray(at[0], at[0] + at[1]) as BlobPart], { type: "image/jpeg" }), where, [width, height]);
        } else if (width >= 21 && height >= 21) {
          await look(new ImageData(new Uint8ClampedArray(pictures.rgba(i).buffer as ArrayBuffer), width, height), where, [width, height]);
        }
      }
    });
    signal.throwIfAborted();
    // A code drawn in boxes on a page, as label and invoice makers do.
    await withWorkerResource(pdf.pdfShapes(req.pdf, PDF_PAGES), async (pages) => {
      for (let i = 0; i < pages.count; i++) {
        signal.throwIfAborted();
        const [left, bottom, right, top] = pages.media(i);
        const scale = Math.min(4, CODE_SCAN / Math.max(right - left, top - bottom));
        const width = Math.ceil((right - left) * scale);
        const height = Math.ceil((top - bottom) * scale);
        const g = new OffscreenCanvas(width, height).getContext("2d", { willReadFrequently: true });
        if (!g) continue;
        g.fillStyle = "#fff";
        g.fillRect(0, 0, width, height);
        g.fillStyle = "#000";
        const boxes = pages.boxes(i);
        for (let k = 0; k + 3 < boxes.length; k += 4) {
          const x = (boxes[k] - left) * scale;
          const y = (top - boxes[k + 3]) * scale;
          g.fillRect(x, y, (boxes[k + 2] - boxes[k]) * scale, (boxes[k + 3] - boxes[k + 1]) * scale);
        }
        await look(g.getImageData(0, 0, width, height), `a code drawn on page ${pages.page(i)}`);
      }
    });
  }
  for (const e of req.entries ?? []) {
    signal.throwIfAborted();
    const bytes = current ? await readEntry(current, source, e.index, isCurrent) : undefined;
    if (bytes && bytes.length > 0) await look(new Blob([bytes as BlobPart]), e.name, imageDimensions(bytes));
  }
  return { codes: found, ...(filmScan ? { filmScan } : {}) };
}
