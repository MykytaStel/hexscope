// The clean copy's pieces: what it produced, sharing it, the file and the
// copy side by side, and what no clean copy can take out.
import type { FileModel } from "./model";
import type { PageGlyphs, PagePicture } from "./worker";
import type { VerificationReport, VerificationSelection } from "./verification";
import { categories } from "./share";
import { noun } from "./headline";
import { el, formatBytes } from "./dom";
import { LIMITS } from "./knowledge";
import { typeOf } from "./files";
import { decodePicture, drawn } from "./thumbnail";
import { announce } from "./announce";

/** What cleaning produced, as the page needs it. */
export interface CleanResult {
  copy: Blob;
  /** What the copy is called when saved. */
  name: string;
  /** Whether it was saved already; if not, it waits for Share or Save. */
  saved: boolean;
  removed: { what: string; bytes: number }[];
  orientation: number;
  error: string;
  /** Local reparse of the actual output; copy actions do not wait for it. */
  verification?: Promise<VerificationReport>;
}

export interface RepairResult {
  bytes: Uint8Array;
  /** What the copy is saved as. */
  name: string;
  fixed: string[];
  error: string;
}

export interface CleanActions {
  /** Makes the copy and saves it; resolves with what was done. `notes` also empties a workbook's or a deck's comments and notes. */
  clean(notes?: boolean): Promise<CleanResult>;
  /** Opens the copy in hexscope, to check it. */
  open(copy: Blob): void;
  /** Opens a file found inside this one. */
  openInside(bytes: Uint8Array, name: string): void;
  /** Every page's visible glyphs, for searching. */
  pages(): Promise<PageGlyphs[]>;
  /** The pictures a page draws, counted from 1: a scanned page, to see where to draw. */
  pictures(page: number): Promise<PagePicture[]>;
  /** A clean copy with these areas blacked out (see `areasOf`), saved or waiting to be. */
  redact(areas: Float64Array, selections: VerificationSelection[]): Promise<CleanResult>;
  /** Saves a copy as a download. */
  save(name: string, data: Uint8Array | Blob): void;
  /** Makes a repaired copy and saves it; resolves with what was done. */
  repair(): Promise<RepairResult>;
  /** Opens the repaired copy in hexscope. */
  openRepaired(bytes: Uint8Array): void;
  /** Compares the file on screen with another. */
  compare(other: File): void;
}

/** A compact report about the output Blob; the finding's value never reaches this view. */
export function copyVerification(result: CleanResult): HTMLElement | null {
  if (!result.verification) return null;

  const region = el("section", "copy-verification");
  region.setAttribute("role", "status");
  region.setAttribute("aria-live", "polite");
  region.setAttribute("aria-atomic", "false");
  region.append(el("p", "copy-verification-pending", "Checking the copy in this tab…"));

  const render = (report: VerificationReport) => {
    const groups: [string, VerificationReport["removed"]][] = [
      ["Removed", report.removed],
      ["Still present", report.present],
      ["Not checked", report.unchecked],
    ];
    region.replaceChildren(
      ...groups.map(([heading, items]) => {
        const group = el("section", "copy-verification-group");
        group.append(el("h3", undefined, heading));
        const list = el("ul", "copy-verification-list");
        for (const item of items) {
          const row = el("li", "copy-verification-item");
          row.append(el("span", "copy-verification-label", item.label));
          const reason = item.reason ?? (item.unexpected ? "This finding appeared in the copy but was not found in the source file." : undefined);
          if (reason) row.append(el("p", "copy-verification-reason", reason));
          list.append(row);
        }
        group.append(list);
        return group;
      }),
    );
  };

  void result.verification.then(render, () =>
    render({
      removed: [],
      present: [],
      unchecked: [{ kind: "verification", label: "Copy verification", reason: "Hexscope could not check this copy because verification failed or timed out." }],
    }),
  );
  return region;
}

/**
 * A button that hands the copy to the device's share sheet — Messages,
 * Telegram, mail — or null where the browser cannot share such a file.
 * Its own button, because sharing must follow a tap of its own.
 */
export function shareButton(name: string, data: Uint8Array | Blob): HTMLButtonElement | null {
  const file = new File([data as BlobPart], name, { type: typeOf(name) });
  if (!navigator.canShare?.({ files: [file] })) return null;
  const b = el("button", "btn btn-primary", "Share the clean copy");
  b.title = "Send the copy on: to a chat, to mail, to another app";
  b.addEventListener("click", async () => {
    try {
      await navigator.share({ files: [file] });
    } catch {
      // Cancelled, or refused: the copy is saved all the same.
    }
  });
  return b;
}

/** A movie file with sound and no picture: a voice memo, a song. */
export function isAudio(m: FileModel): boolean {
  return m.file.format === "video" && / audio\b/.test(m.value(0));
}

/** A name with a date in it, the way phones and messengers name files: `IMG_20260614_183207`, `Screenshot 2026-06-14 at 18.32`. */
const DATED = /(?:19|20)\d{2}[-_.]?(?:0[1-9]|1[0-2])[-_.]?(?:0[1-9]|[12]\d|3[01])/;

/** The file and its clean copy side by side: what it gave away, what is left, and their sizes. */
export function beforeAfter(m: FileModel, r: CleanResult, left: number): HTMLElement {
  const before = categories(m).length;
  const card = el("div", "before-after");
  const side = (tone: string, label: string, count: number, what: string, name: string, size: number) => {
    const s = el("div", `ba-side ${tone}`);
    const n = el("span", "ba-count", String(count));
    s.append(el("span", "ba-label", label), n, el("span", "ba-what", what), el("span", "ba-file", `${name} · ${formatBytes(size)}`));
    return [s, n] as const;
  };
  const base = m.name.split("/").pop() ?? m.name;
  const [was] = side("is-before", "Before", before, before === 1 ? "thing it gave away" : "things it gave away", base, m.bytes.length);
  const [now, count] = side(
    left === 0 ? "is-after is-clear" : "is-after",
    "Clean copy",
    left,
    left === 0 ? "left" : `left — part of the ${noun(m)} itself`,
    r.name,
    r.copy.size,
  );
  card.append(was, el("span", "ba-arrow", "→"), now);
  // The number counts down from what it was, unless motion is unwelcome.
  if (before > left && !matchMedia("(prefers-reduced-motion: reduce)").matches) {
    let shown = before;
    count.textContent = String(shown);
    const step = () => {
      shown -= 1;
      count.textContent = String(shown);
      if (shown > left) setTimeout(step, Math.max(40, 420 / (before - left)));
    };
    setTimeout(step, 250);
  }
  return card;
}

/** What no clean copy can remove from this file, folded away until asked for; empty for what has none worth saying. */
export function cleanLimits(m: FileModel): HTMLElement {
  const format = m.file.format;
  const kind = ["jpeg", "heif", "png", "webp", "gif"].includes(format) ? "photo" : isAudio(m) ? "audio" : format;
  const items = [...(LIMITS[kind] ?? [])];
  const base = m.name.split("/").pop() ?? m.name;
  if (DATED.test(base)) {
    items.push(`Its name, “${base}”, says when it was made; the copy keeps the name, with “-clean” added. Rename it if that matters.`);
  }
  const box = el("details", "clean-limits");
  if (items.length === 0) {
    box.hidden = true;
    return box;
  }
  box.append(el("summary", undefined, "What no clean copy can remove"));
  const ul = el("ul");
  ul.append(...items.map((t) => el("li", undefined, t)));
  box.append(ul);
  return box;
}

/** Picture formats a browser can draw, and so copy as a clean picture. */
const PICTURES = ["jpeg", "png", "webp", "gif", "heif"];

/**
 * "Copy a clean picture": the picture drawn again from its pixels and put
 * on the clipboard as a PNG, to paste straight into a chat or a document.
 * Drawn pixels carry nothing else — no place, no camera, no names — so the
 * copy is clean by how it is made. Null where the clipboard cannot take a
 * picture, or the file is not one.
 */
export function copyPictureButton(m: FileModel): HTMLButtonElement | null {
  if (!PICTURES.includes(m.file.format) || typeof ClipboardItem === "undefined" || !navigator.clipboard?.write) return null;
  const b = el("button", "btn", "Copy a clean picture");
  b.title = "Puts the picture on the clipboard without anything else, to paste into a chat";
  b.addEventListener("click", async () => {
    const said = b.textContent;
    // The item is handed over at once, with the picture to come: Safari
    // lets a page write to the clipboard only while the tap is fresh.
    const png = (async () => {
      const bitmap = await decodePicture(m);
      if (!bitmap) throw new Error("undrawable");
      const canvas = drawn(bitmap, m.file.format === "jpeg" ? m.file.orientation : 1, Math.max(bitmap.width, bitmap.height));
      return await new Promise<Blob>((ok, no) => canvas.toBlob((blob) => (blob ? ok(blob) : no(new Error("no blob"))), "image/png"));
    })();
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
      b.textContent = "Copied — paste it anywhere";
      announce("Copied a clean picture. Paste it into a chat or a document.");
    } catch {
      b.textContent = "This browser would not copy it";
    }
    setTimeout(() => (b.textContent = said), 2500);
  });
  return b;
}
