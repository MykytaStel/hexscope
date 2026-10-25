import type { PagePicture } from "./worker";

/** A page's picture ready to draw: its address and the matrix that places its unit square. */
export interface Shown {
  href: string;
  matrix: number[];
}

// Addresses made for PDF page pictures; the current file owns their lifetime.
let addresses: string[] = [];

/** A page's pictures as images to draw: a JPEG as it is, pixels as a PNG. */
export async function toShown(pictures: PagePicture[]): Promise<Shown[]> {
  const out: Shown[] = [];
  for (const p of pictures) {
    let blob: Blob | null = null;
    if (p.jpeg) blob = new Blob([p.jpeg as BlobPart], { type: "image/jpeg" });
    else if (p.rgba && p.width && p.height) {
      const c = document.createElement("canvas");
      c.width = p.width;
      c.height = p.height;
      c.getContext("2d")?.putImageData(new ImageData(p.rgba as Uint8ClampedArray<ArrayBuffer>, p.width, p.height), 0, 0);
      blob = await new Promise<Blob | null>((resolve) => c.toBlob(resolve, "image/png"));
    }
    if (!blob) continue;
    const href = URL.createObjectURL(blob);
    addresses.push(href);
    out.push({ href, matrix: p.matrix });
  }
  return out;
}

/** Lets go of every picture's address when the next file is opened. */
export function forgetPictures(): void {
  for (const address of addresses) URL.revokeObjectURL(address);
  addresses = [];
}
