// A scanned PDF page's JPEG pictures, painted black under the boxes drawn
// over them.

/** `%PDF-` within the first kilobyte, as the core looks for it. */
export function isPdf(bytes: Uint8Array): boolean {
  const head = bytes.subarray(0, 1024);
  for (let i = 0; i + 5 <= head.length; i++) {
    if (head[i] === 0x25 && head[i + 1] === 0x50 && head[i + 2] === 0x44 && head[i + 3] === 0x46 && head[i + 4] === 0x2d) return true;
  }
  return false;
}

/**
 * The JPEG pictures under the areas to black out, each painted black there
 * and encoded again: a PDF's scanned page keeps its pixels under a box, so
 * the box goes into the picture. The core says which and where; the
 * browser has the JPEG codec. One that will not decode is left out, and
 * the core then makes no copy rather than one that keeps it.
 */
export async function paintJpegs(pdf: Uint8Array, found: Float64Array): Promise<{ nums: Uint32Array; lens: Uint32Array; bytes: Uint8Array }> {
  const nums: number[] = [];
  const parts: Uint8Array[] = [];
  for (let i = 0; i < found.length; ) {
    const [num, width, height, start, len, count] = found.subarray(i, i + 6);
    const rects = found.subarray(i + 6, i + 6 + 4 * count);
    i += 6 + 4 * count;
    try {
      const bitmap = await createImageBitmap(new Blob([pdf.subarray(start, start + len) as BlobPart], { type: "image/jpeg" }), { imageOrientation: "none" });
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
      const g = canvas.getContext("2d");
      if (!g) continue;
      g.drawImage(bitmap, 0, 0);
      bitmap.close();
      // The core counts in the picture's own size; the decoder may differ.
      const sx = canvas.width / width;
      const sy = canvas.height / height;
      g.fillStyle = "#000";
      for (let r = 0; r < rects.length; r += 4) {
        const x0 = Math.floor(rects[r] * sx);
        const y0 = Math.floor(rects[r + 1] * sy);
        g.fillRect(x0, y0, Math.ceil(rects[r + 2] * sx) - x0, Math.ceil(rects[r + 3] * sy) - y0);
      }
      const blob = await canvas.convertToBlob({ type: "image/jpeg", quality: 0.92 });
      nums.push(num);
      parts.push(new Uint8Array(await blob.arrayBuffer()));
    } catch {
      // Not decoded: not painted.
    }
  }
  const bytes = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    bytes.set(p, at);
    at += p.length;
  }
  return { nums: Uint32Array.from(nums), lens: Uint32Array.from(parts, (p) => p.length), bytes };
}
