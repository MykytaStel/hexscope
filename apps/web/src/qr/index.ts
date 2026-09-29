// QR codes in a picture: where each is, and what it says. Written for
// hexscope, following ISO/IEC 18004, with no library.

import { readGrid, transposed } from "./grid";
import { binarize, inverted, locate, type Bitmap, type Point } from "./locate";

export interface QrCode {
  text: string;
  /** Its corners in the picture: top left, top right, bottom right, bottom left. */
  corners: Point[];
}

/** Codes found at most, per picture. */
const MAX_CODES = 8;

/** Tries each set of finders in turn; a set that reads claims its finders from the rest. */
function scan(b: Bitmap, out: QrCode[]): void {
  const used = new Set<object>();
  for (const c of locate(b)) {
    if (out.length >= MAX_CODES) return;
    if (c.finders.some((f) => used.has(f))) continue;
    for (const grid of c.grids()) {
      const read = readGrid(grid) ?? readGrid(transposed(grid));
      if (read) {
        for (const f of c.finders) used.add(f);
        if (!out.some((o) => o.text === read.text)) out.push({ text: read.text, corners: c.corners(grid.size) });
        break;
      }
    }
  }
}

/** The QR codes in a picture's brightness, one byte a pixel. */
export function findCodes(lum: Uint8Array, width: number, height: number): QrCode[] {
  const b = binarize(lum, width, height);
  const out: QrCode[] = [];
  scan(b, out);
  if (out.length === 0) scan(inverted(b), out);
  return out;
}

/** Brightness from RGBA pixels, as the eye weighs the channels. */
export function luminance(rgba: Uint8ClampedArray | Uint8Array, width: number, height: number): Uint8Array {
  const out = new Uint8Array(width * height);
  for (let i = 0, j = 0; i < out.length; i++, j += 4) {
    // A transparent pixel reads as white, as a viewer shows it on a page.
    const a = rgba[j + 3] / 255;
    const y = (rgba[j] * 77 + rgba[j + 1] * 150 + rgba[j + 2] * 29) >> 8;
    out[i] = Math.round(y * a + 255 * (1 - a));
  }
  return out;
}
