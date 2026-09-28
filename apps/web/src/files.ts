// Files in and out of the page: what a copy is called, what kind of file
// this is in a word, how large a file a tab can take, and handing bytes to
// the browser as a download or to a phone's share sheet.
import type { FileModel } from "./model";
import { misfit } from "./verdict";
import { formatBytes } from "./dom";

/** The largest file read in a tab: past it, browsers cannot hold it whole. */
export const MAX_FILE = 2 * 1024 ** 3 - 1;

/** Why a file is too large to read here, and what reads it instead. */
export function tooLarge(file: { name: string; size: number }): string {
  return `${file.name} is ${formatBytes(file.size)}: hexscope reads files up to 2 GB in a browser tab. The command line tool reads any size: hexscope check "${file.name}".`;
}

/** "photo.jpg" → "photo-clean.jpg"; a HEIC named .jpg → "photo-clean.heic". */
export function cleanName(m: FileModel): string {
  const base = m.name.split("/").pop() ?? m.name;
  const dot = base.lastIndexOf(".");
  const ext = misfit(m)?.fits ?? (dot > 0 ? base.slice(dot) : "");
  return `${dot > 0 ? base.slice(0, dot) : base}-clean${ext}`;
}

/** "photo.jpg" → "photo-repaired.jpg", with the extension that fits. */
export function repairedName(m: FileModel): string {
  return cleanName(m).replace(/-clean(\.[^.]*)?$/, "-repaired$1");
}

/** "report.pdf" → "report-redacted.pdf". */
export function redactedName(m: FileModel): string {
  return cleanName(m).replace(/-clean(\.[^.]*)?$/, "-redacted$1");
}

/** A short name for the kind of file, beside its name in a list. */
export function kindOf(m: FileModel): string {
  const first = m.value(0).split(" · ")[0];
  switch (m.file.format) {
    case "jpeg":
      return "JPEG";
    case "png":
      return "PNG";
    case "heif":
    case "webp":
    case "gif":
    case "video":
    case "pdf":
      return first;
    case "zip": {
      // An Office document says which by its main part.
      const labels = m.file.labels;
      if (labels.includes("word/document.xml")) return "Word document";
      if (labels.includes("xl/workbook.xml")) return "Excel workbook";
      if (labels.includes("ppt/presentation.xml")) return "PowerPoint deck";
      if (labels.includes("content.xml") && labels.includes("mimetype")) return "OpenDocument";
      if (labels.includes("META-INF/container.xml")) return "EPUB book";
      if (labels.includes("AndroidManifest.xml")) return "Android app";
      return "ZIP";
    }
    case "wasm":
      return "WebAssembly";
    case "eml":
      return "Email";
    default:
      return "";
  }
}

/** Media types by extension, for sharing a copy: a share sheet goes by type. */
const TYPES: Record<string, string> = {
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  png: "image/png",
  webp: "image/webp",
  gif: "image/gif",
  heic: "image/heic",
  heif: "image/heif",
  avif: "image/avif",
  mp4: "video/mp4",
  m4v: "video/mp4",
  mov: "video/quicktime",
  m4a: "audio/mp4",
  pdf: "application/pdf",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  zip: "application/zip",
  wasm: "application/wasm",
};

/** The media type a file's name says, or "" when unknown. */
export function typeOf(name: string): string {
  return TYPES[name.slice(name.lastIndexOf(".") + 1).toLowerCase()] ?? "";
}

/**
 * A phone that can share a file of this name: its copy goes to the share
 * sheet rather than a download, which there is a dialog in the way of
 * sending it on.
 */
export function phoneCanShare(name = "photo.jpg"): boolean {
  return (
    matchMedia("(hover: none) and (pointer: coarse)").matches &&
    (navigator.canShare?.({ files: [new File([], name, { type: typeOf(name) })] }) ?? false)
  );
}

/** Hands bytes, or a finished archive, to the browser as a download. */
export function saveAs(name: string, data: Uint8Array | Blob): void {
  const blob = data instanceof Blob ? data : new Blob([data.slice() as BlobPart]);
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  // Long enough for a large archive to be written out.
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}
