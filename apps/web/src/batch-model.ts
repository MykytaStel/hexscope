import type { Headline } from "./headline";
import type { PhotoSignal } from "./photo-signal";
import type { CoverageStatus } from "./model";
import type { VerdictLine } from "./verdict";

export interface BatchItem {
  file: File;
  state: "waiting" | "reading" | "done" | "failed";
  /** Shown beside the name: "JPEG", "PDF 1.7". */
  kind: string;
  lines: VerdictLine[];
  /** The answer a single file opens on; null until it is read. */
  headline: Headline | null;
  /** What it gives away, as categories: "where it was taken". */
  reveals: string[];
  /** The clean copy's name, for the ZIP. */
  cleanName: string;
  /** Machine format code from the parser, used by portable receipts. */
  format?: string;
  /** Canonical parser read status, classified by the Rust core. */
  coverage?: CoverageStatus;
  /** The file has a supported metadata block that can be removed even if it has no recognized facts. */
  hasRemovableMetadata?: boolean;
  note: string;
  /** Left out of the clean copies, by choice. */
  skip: boolean;
  copyStatus?: string;
  photoSignal?: PhotoSignal;
}

export interface BatchFormatCoverage {
  format: string;
  files: number;
  complete: number;
  partial: number;
  unsupported: number;
}

/** Whether a file has something a clean copy takes out. */
export function cleanable(i: BatchItem): boolean {
  // Audio copies are offered only when a supported metadata block passed the
  // same boundary checks used by its cleaner.
  if (i.format === "mp3" || i.format === "flac" || i.format === "ogg" || i.format === "wav") {
    return i.state === "done" && i.hasRemovableMetadata === true;
  }
  // These formats can expose findings, but the core does not make a clean
  // copy of the file itself. Attachments can be opened and cleaned separately.
  if (i.format && ["unknown", "eml", "msg"].includes(i.format)) return false;
  return i.state === "done" && (
    i.reveals.length > 0
    || i.lines.some((l) => l.kind === "hidden")
  );
}
