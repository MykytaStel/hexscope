import { done } from "./announce";
import { beforeAfter, cleanLimits, copyPictureButton, copyVerification, isAudio, renderBeforeAfter, shareButton, type CleanActions } from "./cleancard";
import { confirmedRemovedKinds } from "./clean-summary";
import { el, formatBytes } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { FileModel } from "./model";
import { KEPT_NOTE } from "./knowledge";
import { dismissTour } from "./tour";

/** What each kind of clean copy leaves out, said before it is made. */
export const CLEAN_NOTES: Record<string, string> = {
  zip: "Removes the document's properties, the camera data and location of every photo in it, and the properties of a workbook or deck kept inside it. In a Word document, tracked changes are accepted — what was deleted goes, with its text — and comments are deleted, with their authors.",
  archive:
    "Leaves out what the Mac noted about each file in __MACOSX — where it was downloaded from, with which app, its tags. The files inside are copied byte for byte.",
  webp: "Leaves out the EXIF, XMP and Content Credentials chunks: camera, place, dates, editing history, who or what made it. The picture is copied byte for byte.",
  gif: "Leaves out the comments and the XMP. Every frame is copied byte for byte, with its timing.",
  heif: "Blanks the camera data, location, serial numbers and XMP where they lie, so the file keeps its size. The picture and its thumbnail are copied unchanged.",
  png: "Removes the text notes — an image generator's prompt among them — EXIF, XMP, Content Credentials and the time it was last changed. The pixels are copied byte for byte.",
  video:
    "Blanks the location, the camera, the software and the dates where they lie, so the file keeps its size. The picture and sound are copied byte for byte.",
  audio:
    "Blanks the location, the device, the software and the dates where they lie, so the file keeps its size. The sound is copied byte for byte.",
  wasm: "Leaves out the custom sections that say who built it and how: function names, tools, source map and debug info links, DWARF. The code and data are copied byte for byte; paths inside the data are part of the program, and stay.",
  office97:
    "Blanks the document's properties where they lie — title, author, last editor, company, template, dates, editing time — and the sectors no part uses, where deleted text can remain. Nothing else in the file moves.",
  pdf: "Writes the document anew with only what its pages use: no author, programs or dates, no XMP, no earlier versions. Text under black boxes or hidden from view is taken out, and marks for redaction applied, with every other letter left where it was; a scanned page loses its pixels under a box; photos lose their camera data.",
  picture:
    "Removes the camera data, location, serial numbers, the maker's notes, thumbnail, comments and Content Credentials. The picture itself is copied unchanged.",
};

/** Builds the clean-copy panel and keeps its verified result beside its facts. */
export function createCleaner(m: FileModel, cleaning: CleanActions, revealGroup: HTMLElement): HTMLElement {
  const format = m.file.format;
  const box = el("div", "cleaner");
  const button = el("button", "btn btn-clean", "Remove it — save a clean copy");
  // The visible action is in the verdict; this control runs the existing clean-copy flow.
  button.hidden = true;
  button.title = "Makes the copy in this tab: nothing is uploaded";
  const office = m.file.labels.includes("[Content_Types].xml");
  const note = isAudio(m)
    ? CLEAN_NOTES.audio
    : (CLEAN_NOTES[format === "zip" && !office ? "archive" : format] ?? CLEAN_NOTES.picture);
  const limits = cleanLimits(m);
  box.append(button, el("p", "hint", note));
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
    const removed = el("details", "clean-removed");
    removed.append(el("summary", undefined, `What was removed · ${formatBytes(r.removed.reduce((n, x) => n + x.bytes, 0))}`));
    const ul = el("ul", "clean-list");
    for (const item of r.removed) {
      const li = el("li");
      // A sentence starts with a capital; a path such as "word/media/…" stays as named.
      const path = /^[^\s:]+\//.test(item.what);
      const what = path ? item.what : item.what.charAt(0).toUpperCase() + item.what.slice(1);
      li.append(el("span", undefined, what), el("span", "clean-size", formatBytes(item.bytes)));
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
    const sharer = shareButton(r.name, r.copy);
    if (sharer) actions.append(sharer);
    if (!r.saved) {
      const save = el("button", "btn", "Save it");
      save.addEventListener("click", () => cleaning.save(r.name, r.copy));
      actions.append(save);
    }
    const diff = el("button", "btn", translateText("Compare with the original", currentLocale()));
    diff.title = translateText("What the copy took out, part by part", currentLocale());
    diff.addEventListener("click", () => cleaning.compare(new File([r.copy], "the clean copy")));
    actions.append(open, diff);
    const copy = copyPictureButton(m);
    if (copy) actions.prepend(copy);
    box.append(limits);
    const verification = copyVerification(r, (report) => {
      renderBeforeAfter(comparison, report);
      const revealList = revealGroup.querySelector<HTMLElement>(".reveal-list");
      if (!revealList) return;
      const removedKinds = new Set(confirmedRemovedKinds(report));
      for (const fact of revealList.querySelectorAll<HTMLElement>("dt[data-kind], dd[data-kind]")) {
        fact.classList.toggle("is-removed", report !== undefined && removedKinds.has(fact.dataset.kind ?? ""));
      }
    });
    if (verification) box.append(verification);
  });
  return box;
}
