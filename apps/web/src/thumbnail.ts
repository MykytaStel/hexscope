// Does the thumbnail inside a photo show the photo? A camera writes a small
// copy of the picture into EXIF; an editor that crops or retouches the
// picture does not always write a new one, so the thumbnail can still show
// what was cut out. Both are decoded here, the black bars cameras pad
// thumbnails with trimmed off, and compared by shape and by a small grey
// copy of each.
import { asStored, turn } from "./blocks";
import type { FileModel } from "./model";

/** Side of the grey copies compared, in pixels. */
const GRID = 24;
/** Shapes this far apart, as a ratio, are different pictures. */
const SHAPE = 0.04;
/** Mean difference of the grey copies, 0 to 1, past which they differ. */
const CONTENT = 0.1;
/** Brightest a padding bar can be, 0 to 255. */
const BAR = 18;

export interface ThumbnailCheck {
  thumbnail: ImageBitmap;
  picture: ImageBitmap;
  /** Why they differ, or null when the thumbnail is the picture. */
  differs: "shape" | "content" | null;
  /** The thumbnail's node. */
  node: number;
}

function canvas(w: number, h: number): CanvasRenderingContext2D | null {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return c.getContext("2d", { willReadFrequently: true });
}

/** The part of the thumbnail inside any near-black bars along its edges. */
function content(b: ImageBitmap): [number, number, number, number] {
  const ctx = canvas(b.width, b.height);
  if (!ctx) return [0, 0, b.width, b.height];
  ctx.drawImage(b, 0, 0);
  const px = ctx.getImageData(0, 0, b.width, b.height).data;
  const dark = (x0: number, y0: number, x1: number, y1: number) => {
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const i = (y * b.width + x) * 4;
        if (Math.max(px[i], px[i + 1], px[i + 2]) > BAR) return false;
      }
    }
    return true;
  };
  let [top, bottom, left, right] = [0, b.height, 0, b.width];
  while (top < bottom - 1 && dark(0, top, b.width, top + 1)) top++;
  while (bottom > top + 1 && dark(0, bottom - 1, b.width, bottom)) bottom--;
  while (left < right - 1 && dark(left, top, left + 1, bottom)) left++;
  while (right > left + 1 && dark(right - 1, top, right, bottom)) right--;
  return [left, top, right - left, bottom - top];
}

/** A small grey copy of part of a picture, values 0 to 1. */
function grey(b: ImageBitmap, [x, y, w, h]: [number, number, number, number]): Float32Array {
  const out = new Float32Array(GRID * GRID);
  const ctx = canvas(GRID, GRID);
  if (!ctx) return out;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(b, x, y, w, h, 0, 0, GRID, GRID);
  const px = ctx.getImageData(0, 0, GRID, GRID).data;
  for (let i = 0; i < out.length; i++) {
    out[i] = (0.299 * px[i * 4] + 0.587 * px[i * 4 + 1] + 0.114 * px[i * 4 + 2]) / 255;
  }
  return out;
}

/** Decodes and compares; null when there is no thumbnail, or either will not decode. */
export async function checkThumbnail(m: FileModel): Promise<ThumbnailCheck | null> {
  const f = m.file;
  const fact = f.facts.find((x) => x.kind === "thumbnail");
  if (!fact || (f.format !== "jpeg" && f.format !== "png")) return null;
  const start = m.start(fact.node);
  const bytes = m.bytes.subarray(start, start + m.len(fact.node));
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;
  let thumbnail: ImageBitmap;
  let picture: ImageBitmap;
  try {
    thumbnail = await createImageBitmap(new Blob([bytes as BlobPart], { type: "image/jpeg" }));
    if (f.format === "jpeg") {
      picture = await createImageBitmap(new Blob([asStored(m.bytes) as BlobPart], { type: "image/jpeg" }));
    } else if (f.preview) {
      const { width, height, pixels } = f.preview;
      picture = await createImageBitmap(new ImageData(new Uint8ClampedArray(pixels), width, height));
    } else {
      return null;
    }
  } catch {
    return null;
  }
  const inner = content(thumbnail);
  const shape = inner[2] / inner[3] / (picture.width / picture.height);
  let differs: ThumbnailCheck["differs"] = null;
  if (Math.abs(shape - 1) > SHAPE) {
    differs = "shape";
  } else {
    const a = grey(thumbnail, inner);
    const b = grey(picture, [0, 0, picture.width, picture.height]);
    let sum = 0;
    for (let i = 0; i < a.length; i++) sum += Math.abs(a[i] - b[i]);
    if (sum / a.length > CONTENT) differs = "content";
  }
  return { thumbnail, picture, differs, node: fact.node };
}

/** Draws a picture as stored, stood up by the photo's orientation, at most `side` pixels. */
export function drawn(b: ImageBitmap, orientation: number, side: number): HTMLCanvasElement {
  const [dw, dh] = orientation >= 5 ? [b.height, b.width] : [b.width, b.height];
  const scale = Math.min(1, side / Math.max(dw, dh));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(dw * scale));
  c.height = Math.max(1, Math.round(dh * scale));
  const ctx = c.getContext("2d");
  if (ctx) {
    ctx.imageSmoothingQuality = "high";
    ctx.setTransform(new DOMMatrix().scale(c.width / dw, c.height / dh).multiply(turn(orientation, b.width, b.height)));
    ctx.drawImage(b, 0, 0);
  }
  return c;
}
