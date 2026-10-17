// Which of the parser's builds reads a file, told by its bytes before any
// build is loaded. The small build for pictures and movies also takes SVG
// drawings and every file no build reads, so a drawing, a text file or an
// unknown binary never costs the documents' build; that build is asked only
// for what it may read — a ZIP, a compound file, RTF, a WebAssembly module,
// or text that starts the way an email does.
import { isPdf } from "./paint";

export type Family = "media" | "pdf" | "docs";

/** Whether the small build reads these bytes as a picture or movie: PNG, JPEG, WebP, GIF, TIFF, and the ISO boxes of HEIF and MP4. */
export function isMedia(b: Uint8Array): boolean {
  const png = b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47;
  const jpeg = b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff;
  const iso = b[4] === 0x66 && b[5] === 0x74 && b[6] === 0x79 && b[7] === 0x70;
  const webp = b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45;
  const gif = b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x38;
  const tiff = (b[0] === 0x49 && b[1] === 0x49 && b[2] === 0x2a && b[3] === 0x00) || (b[0] === 0x4d && b[1] === 0x4d && b[2] === 0x00 && b[3] === 0x2a);
  return png || jpeg || iso || webp || gif || tiff;
}

const starts = (b: Uint8Array, sig: number[]) => sig.every((v, i) => b[i] === v);

/**
 * A ZIP: from its first bytes, or by the record that ends its directory —
 * within the last 64 KiB, as the core looks for it — when something comes
 * before the archive, as in a self-extracting one.
 */
function isZip(b: Uint8Array): boolean {
  if (starts(b, [0x50, 0x4b, 0x03, 0x04]) || starts(b, [0x50, 0x4b, 0x05, 0x06])) return true;
  for (let p = b.length - 22; p >= Math.max(0, b.length - 22 - 0xffff); p--) {
    if (b[p] === 0x50 && b[p + 1] === 0x4b && b[p + 2] === 0x05 && b[p + 3] === 0x06) return true;
  }
  return false;
}

/** Whether the first line is a header field or a mailbox's "From " line, as an email's is. */
function mayBeMail(b: Uint8Array): boolean {
  let line = "";
  for (let i = 0; i < Math.min(b.length, 200) && b[i] !== 0x0a && b[i] !== 0x0d; i++) line += String.fromCharCode(b[i]);
  return line.startsWith("From ") || /^[A-Za-z0-9_-]{1,79}:/.test(line);
}

/** Whether the documents' build may read these bytes. */
function isDocument(b: Uint8Array): boolean {
  const wasm = starts(b, [0x00, 0x61, 0x73, 0x6d]);
  const cfb = starts(b, [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
  const rtf = starts(b, [0x7b, 0x5c, 0x72, 0x74, 0x66]);
  return wasm || cfb || rtf || isZip(b) || mayBeMail(b);
}

/** The build that reads these bytes, asked in the order the core asks: pictures and movies, a WebAssembly module, a PDF, the documents' formats, then everything else. */
export function familyOf(b: Uint8Array): Family {
  if (isMedia(b)) return "media";
  const wasm = starts(b, [0x00, 0x61, 0x73, 0x6d]);
  if (!wasm && isPdf(b)) return "pdf";
  if (isDocument(b)) return "docs";
  return "media";
}
