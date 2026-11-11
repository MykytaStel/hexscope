import { checkedProfiles } from "./preflight-model";
import { done } from "./announce";
import { beforeAfter, cleanLimits, copyPictureButton, copyVerification, isAudio, renderBeforeAfter, shareButton, shareFallbackHint, type CleanActions } from "./cleancard";
import { confirmedRemovedKinds } from "./clean-summary";
import { el, formatBytes, hasCoarsePointer } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { FileModel } from "./model";
import { KEPT_NOTE } from "./knowledge";
import { dismissTour } from "./tour";
import { thanksLine } from "./support";

import { CLEAN_NOTES, cleanCopyNote } from "./clean-copy-plan";
export { CLEAN_NOTES, cleanCopyNote } from "./clean-copy-plan";

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
  box.append(
    button,
    el("p", "hint clean-copy-note", note),
    el("p", "hint clean-copy-limit", translateText(
      "Clean copies over 10 MiB may not be read back in this tab; findings that were not checked stay Not checked.",
      currentLocale(),
    )),
  );
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
    const profile = checkedProfiles.get(m.file);
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
        li.append(el("span", undefined, translateText(what, currentLocale())), el("span", "clean-size", formatBytes(item.bytes)));
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
    if (profile) {
      const status = el("div", "copy-policy");
      status.setAttribute("role", "status");
      box.append(status);
      void import("./copy-policy")
        .then(({ showCopyPolicy }) => showCopyPolicy(status, r.copy, profile))
        .catch(() => { status.textContent = currentLocale() === "uk" ? "Профіль для копії не перевірено" : "Copy profile not checked"; });
    }
    const thanks = thanksLine();
    if (thanks) box.append(thanks);
  });
  return box;
}
