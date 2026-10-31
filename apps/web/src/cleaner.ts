import { done } from "./announce";
import { beforeAfter, cleanLimits, copyPictureButton, copyVerification, isAudio, renderBeforeAfter, shareButton, shareFallbackHint, type CleanActions } from "./cleancard";
import { confirmedRemovedKinds } from "./clean-summary";
import { el, formatBytes, hasCoarsePointer } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { FileModel, ParsedFile } from "./model";
import { KEPT_NOTE } from "./knowledge";
import { dismissTour } from "./tour";
import { thanksLine } from "./support";

/** What each kind of clean copy leaves out, said before it is made. */
export const CLEAN_NOTES: Record<string, string> = {
  zip: "Removes the document's properties, the camera data and location of every photo in it, and the properties of a workbook or deck kept inside it. In a Word document, tracked changes are accepted — what was deleted goes, with its text — and comments are deleted, with their authors.",
  archive:
    "Leaves out what the Mac noted about each file in __MACOSX — where it was downloaded from, with which app, its tags. The files inside are copied byte for byte.",
  webp: "Leaves out the EXIF, XMP and Content Credentials chunks: camera, place, dates, editing history, who or what made it. The picture is copied byte for byte.",
  gif: "Leaves out the comments and the XMP. Every frame is copied byte for byte, with its timing.",
  heif: "Blanks the camera data, location, serial numbers and XMP where they lie, so the file keeps its size. The picture and its thumbnail are copied unchanged.",
  tiff: "Empties the camera, place, time and serial number tags where they lie, and the EXIF and GPS tables, so the file keeps its size. The picture is copied byte for byte.",
  png: "Removes the text notes — an image generator's prompt among them — EXIF, XMP, Content Credentials and the time it was last changed. The pixels are copied byte for byte.",
  video:
    "Blanks the location, the camera, the software and the dates where they lie, so the file keeps its size. The picture and sound are copied byte for byte.",
  audio:
    "Blanks the location, the device, the software and the dates where they lie, so the file keeps its size. The sound is copied byte for byte.",
  mp3: "Removes the ID3v2, APE, ID3v1 Plus and ID3v1 tag blocks from the start or end of the file. The compressed audio frames are copied byte for byte; they are not decoded or re-encoded.",
  flac: "Removes the FLAC Vorbis comment block. Keeps the other recognized metadata blocks and compressed audio frames byte for byte; the audio is not decoded or re-encoded. If a metadata block cannot be inspected, Hexscope does not make a copy.",
  wasm: "Leaves out the custom sections that say who built it and how: function names, tools, source map and debug info links, DWARF. The code and data are copied byte for byte; paths inside the data are part of the program, and stay.",
  svg: "Leaves out the metadata, Inkscape's names, paths and settings, a comment naming the program and Illustrator's own copy of the drawing; a linked picture keeps only its file name, and a photo inside is cleaned as a photo of its own. The shapes, text and scripts are copied as they are.",
  rtf: "Leaves out the document's information — title, author, who saved it last, company, dates, editing time — the program and template that made it, its custom properties and the numbers that tie its editing sessions together. Tracked changes are accepted and comments deleted, with their authors; the rest of the text is copied as it is.",
  office97:
    "Blanks the document's properties where they lie — title, author, last editor, company, template, dates, editing time — and the sectors no part uses, where deleted text can remain. Nothing else in the file moves.",
  pdf: "Writes the document anew with only what its pages use: no author, programs or dates, no XMP, no earlier versions. Text under black boxes or hidden from view is taken out, and marks for redaction applied, with every other letter left where it was; a scanned page loses its pixels under a box; photos lose their camera data. Notes go, every other mark loses who made it, and attached files go — what was typed or signed on the page stays.",
  picture:
    "Removes the camera data, location, serial numbers, the maker's notes, thumbnail, comments and Content Credentials. The picture itself is copied unchanged.",
  jpegHdr:
    "Removes personal metadata from the main picture and its HDR gain map. Keeps both pictures and the technical HDR settings needed to display them; the photo may look less bright on HDR screens if the gain map is missing.",
  jpegMpfUnsupported:
    "This JPEG has an additional indexed picture that Hexscope cannot safely clean yet, so it will not make a clean copy.",
};

export function cleanCopyNote(file: Pick<ParsedFile, "format" | "labels" | "values">): string {
  if (file.format === "flac") return CLEAN_NOTES.flac;
  if (file.format === "jpeg") {
    const indexedPictures = file.labels
      .map((label, index) => (label === "picture listed in the MPF index" ? file.values[index] : undefined))
      .filter((value): value is string => value !== undefined);
    if (indexedPictures.length === 1 && indexedPictures[0] === "HDR gain map") return CLEAN_NOTES.jpegHdr;
    if (indexedPictures.length > 0) return CLEAN_NOTES.jpegMpfUnsupported;
  }
  return CLEAN_NOTES[file.format] ?? CLEAN_NOTES.picture;
}

/** Builds the clean-copy panel and keeps its verified result beside its facts. */
export function createCleaner(m: FileModel, cleaning: CleanActions, revealGroup: HTMLElement): HTMLElement {
  const format = m.file.format;
  const box = el("div", "cleaner");
  const button = el("button", "btn btn-clean", "Remove it — save a clean copy");
  // The visible action is in the verdict; this control runs the existing clean-copy flow.
  button.hidden = true;
  button.title = "Makes the copy in this tab: nothing is uploaded";
  const office = m.file.labels.includes("[Content_Types].xml");
  const note = format === "flac"
    ? CLEAN_NOTES.flac
    : isAudio(m)
    ? CLEAN_NOTES.audio
    : format === "zip" && !office
      ? CLEAN_NOTES.archive
      : cleanCopyNote(m.file);
  const limits = cleanLimits(m);
  box.append(button, el("p", "hint clean-copy-note", note));
  // A workbook's or a deck's comments and notes are its content: gone only when asked.
  const word = m.file.labels.includes("word/document.xml");
  const extras = format === "zip" && !word && m.file.facts.some((x) => x.kind === "notes" || x.kind === "comments");
  const also = el("input");
  if (extras) {
    also.type = "checkbox";
    const label = el("label", "clean-also");
    label.append(also, " Also empty the comments and the speaker's notes, and who wrote them");
    box.append(label);
  }
  box.append(limits);
  button.addEventListener("click", async () => {
    // Let the produced-copy result take the place of the onboarding note.
    dismissTour();
    button.disabled = true;
    button.textContent = "Making the copy…";
    const notes = extras && also.checked;
    // What goes with them is struck out with the rest.
    if (notes) {
      for (const e of revealGroup.querySelectorAll<HTMLElement>("[data-kept]")) {
        const kind = e.dataset.kind ?? e.nextElementSibling?.getAttribute("data-kind");
        if (kind === "notes" || kind === "comments") delete e.dataset.kept;
      }
    }
    const r = await cleaning.clean(notes);
    box.replaceChildren();
    if (r.error) {
      const message = r.error.startsWith("no copy was made because ") ? `${r.error}.` : `No copy was made: ${r.error}.`;
      box.append(el("p", "problem is-warning", message));
      return;
    }
    // Both the summary and crossed-out facts wait for the readback shown below.
    const comparison = beforeAfter(m, r);
    box.append(comparison);
    const saved = r.saved ? `Saved as “${r.name}” — look for it in your downloads.` : "Made a clean copy.";
    box.append(done(translateText(saved, currentLocale())));
    // Keep the HDR preservation note beside the result; the indexed gain map
    // stays in the copy, so it must not appear under "What was removed".
    if (format === "jpeg" && note === CLEAN_NOTES.jpegHdr) {
      box.append(el("p", "hint clean-copy-note", note));
    }
    const removed = el("details", "clean-removed");
    removed.append(el("summary", undefined, `What was removed · ${formatBytes(r.removed.reduce((n, x) => n + x.bytes, 0))}`));
    const ul = el("ul", "clean-list");
    for (const item of r.removed) {
      const li = el("li");
      // A part of a document — "meta.xml: who made it…" — is named as the file
      // names it, and what it held is said in words, each translated on its own.
      const part = /^([^\s:]+\.[A-Za-z0-9]+): (.+)$/.exec(item.what);
      if (part) {
        const said = el("span");
        said.append(el("code", "clean-part", part[1]), ": ", ...part[2].split("; ").flatMap((x, i) => [...(i ? ["; "] : []), el("span", undefined, x)]));
        li.append(said, el("span", "clean-size", formatBytes(item.bytes)));
      } else {
        // A sentence starts with a capital; a path such as "word/media/…" stays as named.
        const path = /^[^\s:]+\//.test(item.what);
        const what = path ? item.what : item.what.charAt(0).toUpperCase() + item.what.slice(1);
        li.append(el("span", undefined, what), el("span", "clean-size", formatBytes(item.bytes)));
      }
      ul.append(li);
    }
    removed.append(ul);
    const actions = el("div", "clean-actions");
    box.append(actions, removed);
    if (revealGroup.querySelector(".reveal-list [data-kept]")) {
      const kept = notes
        ? "Kept: what is part of a workbook or a deck itself — hidden sheets, rows and slides, links to other files. Delete them in Excel or PowerPoint, then save."
        : KEPT_NOTE[format];
      box.append(el("p", "hint", kept ?? "What is not struck out is part of the file's content, and stays."));
    }
    if (r.orientation > 1) {
      box.append(el("p", "hint", "Kept only the orientation, so the picture stays the right way up."));
    }
    const open = el("button", "btn", translateText("Open the clean copy", currentLocale()));
    open.title = translateText("Check it yourself: the card should now be empty", currentLocale());
    open.addEventListener("click", () => cleaning.open(r.copy));
    // On a phone, straight on to the app it was going to: no hunting for it in Downloads.
    const phone = hasCoarsePointer();
    const sharer = shareButton(r.name, r.copy, r.saved);
    const shareFallback = sharer?.classList.contains("share-fallback") ?? false;
    if (sharer) {
      actions.append(sharer);
      if (shareFallback) actions.append(shareFallbackHint());
    } else if (phone && r.saved) {
      actions.append(shareFallbackHint(true));
    }
    if (!r.saved && !shareFallback) {
      const save = el("button", "btn", "Save it");
      save.dataset.shareSaveFallback = "true";
      save.addEventListener("click", () => cleaning.save(r.name, r.copy));
      actions.append(save);
    }
    const diff = el("button", "btn", translateText("Compare with the original", currentLocale()));
    diff.title = translateText("What the copy took out, part by part", currentLocale());
    diff.addEventListener("click", () => cleaning.compare(new File([r.copy], "the clean copy")));
    actions.append(open, diff);
    // After sharing and saving, which most people come for.
    const copy = copyPictureButton(m);
    if (copy) actions.insertBefore(copy, open);
    box.append(limits);
    const verification = copyVerification(r, (report) => {
      renderBeforeAfter(comparison, report);
      const revealList = revealGroup.querySelector<HTMLElement>(".reveal-list");
      if (!revealList) return;
      const removedKinds = new Set(confirmedRemovedKinds(report));
      for (const fact of revealList.querySelectorAll<HTMLElement>("dt[data-kind], dd[data-kind]")) {
        fact.classList.toggle("is-removed", report !== undefined && removedKinds.has(fact.dataset.kind ?? ""));
      }
    }, format);
    if (verification) box.append(verification);
    const thanks = thanksLine();
    if (thanks) box.append(thanks);
  });
  return box;
}
