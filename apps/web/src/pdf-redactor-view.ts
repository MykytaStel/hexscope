import { announce, done } from "./announce";
import { copyVerification, shareButton, shareFallbackHint, type CleanActions } from "./cleancard";
import { el, formatBytes, hasCoarsePointer } from "./dom";
import type { FileModel, PageArea } from "./model";
import { areasOf, find, findPreset, pageFigure, PRESETS, searchable, textIn, type Match, type Searchable } from "./redactor";
import type { Shown } from "./pdf-pictures";

export function pdfRedactorView(
  m: FileModel,
  cleaning: Pick<CleanActions, "pages" | "pictures" | "redact" | "save" | "open">,
  picturesOf: (page: number) => Promise<Shown[]>,
): HTMLElement {
  const group = el("div", "group redactor");
  group.append(el("h2", undefined, "Black out text yourself"));
  group.append(
    el(
      "p",
      "hint",
      "Type a name, a number or an address, or pick a kind below: every place it appears is found. The copy takes it out of the pages — not only covers it — and draws a black box where it was.",
    ),
  );
  const incomplete = el(
    "p",
    "problem is-warning redact-incomplete",
    "Some PDF page or form content could not be fully checked. Search may miss text.",
  );
  incomplete.hidden = true;
  incomplete.setAttribute("role", "alert");
  const form = el("form", "redact-form");
  const input = el("input");
  input.type = "search";
  input.placeholder = "A name, a number…";
  input.setAttribute("aria-label", "Text to black out");
  input.spellcheck = false;
  const findBtn = el("button", "btn", "Find");
  findBtn.type = "submit";
  form.append(input, findBtn);
  const presets = el("div", "redact-presets");
  const list = el("div", "redact-list");
  const status = el("p", "hint redact-status");
  // "not on the pages", "Reading the pages…": heard as they change.
  status.setAttribute("role", "status");
  const preview = el("div", "redact-preview");
  const save = el("button", "btn btn-primary", "Save a blacked-out copy");
  save.hidden = true;
  const result = el("div", "cleaner");
  group.append(incomplete, form, presets, status, list, preview, save, result);

  let pages: Searchable[] | null = null;
  const load = async () => {
    pages ??= searchable(await cleaning.pages());
    incomplete.hidden = pages.every((page) => page.complete);
    return pages;
  };
  // Every search kept, each place with its tick; boxes drawn are one more.
  const chosen: { term: string; matches: Match[]; ticks: HTMLInputElement[]; block: HTMLElement }[] = [];
  // Pages shown to draw on, beyond those with something ticked.
  const shownPages = new Set<number>();
  const picked = () => chosen.flatMap((c) => c.matches.filter((_, i) => c.ticks[i].checked));

  const drawPreview = () => {
    // A page drawn on with the keyboard keeps the focus when drawn again.
    const focused = preview.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.page : undefined;
    preview.replaceChildren();
    if (!pages) return;
    const marks = picked();
    const withMarks = new Set(marks.map((m) => m.page));
    const show = [...new Set([...withMarks, ...shownPages])].sort((a, b) => a - b);
    if (show.length > 0) preview.append(el("p", "redact-preview-title", "As the copy will show it"));
    for (const n of show) {
      const p = pages[n - 1];
      if (!p) continue;
      const areas = marks.filter((m) => m.page === n).flatMap((m) => m.areas);
      preview.append(pageFigure(p, areas, (area) => addBox(p, area), picturesOf(n)));
    }
    // Any other page, to draw a box on.
    if (pages.length > 0) {
      const pick = el("select", "redact-page-pick");
      pick.setAttribute("aria-label", "Show a page to draw a box on");
      pick.append(el("option", undefined, show.length ? "Draw on another page…" : "Draw a box on a page…"));
      for (const p of pages) if (!show.includes(p.page)) pick.append(new Option(`Page ${p.page}`, String(p.page)));
      pick.addEventListener("change", () => {
        shownPages.add(Number(pick.value));
        drawPreview();
      });
      if (pick.options.length > 1) preview.append(pick);
    }
    if (focused) preview.querySelector<SVGSVGElement>(`svg[data-page="${focused}"]`)?.focus();
  };
  const refresh = () => {
    const n = picked().length;
    save.hidden = chosen.length === 0;
    save.disabled = n === 0;
    save.textContent = n === 1 ? "Save a copy with 1 place blacked out" : `Save a copy with ${n} places blacked out`;
    drawPreview();
  };

  /** Lists what a search found, each place with a tick; `extend` adds to a list already there. */
  const addBlock = (term: string, matches: Match[]) => {
    const existing = chosen.find((c) => c.term === term);
    const ul = existing ? existing.block.querySelector("ul")! : el("ul");
    const ticks = matches.map((mt) => {
      const li = el("li");
      const label = el("label");
      const tick = el("input");
      tick.type = "checkbox";
      tick.checked = true;
      tick.addEventListener("change", refresh);
      label.append(tick, el("span", "redact-page", `Page ${mt.page}`), " ", mt.before, el("mark", "redact-hit", mt.text || "a box you drew"), mt.after);
      li.append(label);
      ul.append(li);
      return tick;
    });
    if (existing) {
      existing.matches.push(...matches);
      existing.ticks.push(...ticks);
      existing.block.querySelector(".redact-count")!.textContent = ` — ${existing.matches.length === 1 ? "1 place" : `${existing.matches.length} places`} `;
      refresh();
      return;
    }
    const block = el("div", "redact-term");
    const head = el("div", "redact-head");
    const remove = el("button", "link", "Remove");
    head.append(el("strong", undefined, term), el("span", "redact-count", ` — ${matches.length === 1 ? "1 place" : `${matches.length} places`} `), remove);
    const entry = { term, matches: [...matches], ticks, block };
    chosen.push(entry);
    remove.addEventListener("click", () => {
      chosen.splice(chosen.indexOf(entry), 1);
      block.remove();
      refresh();
    });
    block.append(head, ul);
    list.append(block);
    refresh();
  };
  const addBox = (p: Searchable, area: PageArea) => {
    const text = textIn(p, area);
    addBlock("Boxes you drew", [{ page: p.page, areas: [area], before: "", text, after: "" }]);
    announce(`A box drawn on page ${p.page}${text ? `, over “${text}”` : ""}.`);
  };

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const term = input.value.trim();
    if (!term || chosen.some((c) => c.term.toLowerCase() === `“${term}”`.toLowerCase())) return;
    findBtn.disabled = true;
    status.textContent = pages ? "" : "Reading the pages…";
    const ps = await load();
    findBtn.disabled = false;
    const matches = find(ps, term);
    if (matches.length === 0) {
      status.textContent = `“${term}” is not on the pages as text. If it is in a picture — a scan, a screenshot — draw a box over it on the page instead.`;
      drawPreview();
      return;
    }
    status.textContent = "";
    input.value = "";
    addBlock(`“${term}”`, matches);
    announce(`“${term}”: ${matches.length === 1 ? "1 place" : `${matches.length} places`} found, each ticked to black out.`);
  });
  for (const preset of PRESETS) {
    const b = el("button", "btn redact-preset", preset.label);
    b.type = "button";
    b.addEventListener("click", async () => {
      if (chosen.some((c) => c.term === preset.name)) return;
      b.disabled = true;
      const matches = findPreset(await load(), preset);
      b.disabled = false;
      if (matches.length === 0) {
        status.textContent = `No ${preset.name.toLowerCase()} on the pages.`;
        drawPreview();
        return;
      }
      status.textContent = "";
      addBlock(preset.name, matches);
    announce(`${preset.name}: ${matches.length === 1 ? "1 place" : `${matches.length} places`} found, each ticked to black out.`);
    });
    presets.append(b);
  }
  // The pages to draw on, once asked for.
  const drawOn = el("button", "link", "Draw a box on a page instead");
  drawOn.type = "button";
  drawOn.addEventListener("click", async () => {
    await load();
    drawOn.remove();
    if (pages && pages.length > 0) shownPages.add(1);
    drawPreview();
  });
  presets.append(drawOn);

  save.addEventListener("click", async () => {
    const marks = picked();
    const selections = marks.map((mark) => ({
      page: mark.page,
      text: mark.text,
      sourceComplete: pages?.find((page) => page.page === mark.page)?.complete ?? false,
      areaOnly: mark.text.trim().length === 0,
    }));
    save.disabled = true;
    save.textContent = "Making the copy…";
    const r = await cleaning.redact(areasOf(marks), selections);
    refresh();
    result.replaceChildren();
    if (r.error) {
      const message = r.error.startsWith("no copy was made because ") ? `${r.error}.` : `No copy was made: ${r.error}.`;
      result.append(el("p", "problem is-warning", message));
      return;
    }
    result.append(done(`${r.saved ? `Saved “${r.name}”, a copy` : "Made a copy"} with ${marks.length === 1 ? "1 place" : `${marks.length} places`} blacked out and nothing about who made the file${r.saved ? " — look for it in your downloads" : ""}. Removed:`));
    const ul = el("ul", "clean-list");
    for (const item of r.removed) {
      const li = el("li");
      li.append(el("span", undefined, item.what.charAt(0).toUpperCase() + item.what.slice(1)), el("span", "clean-size", formatBytes(item.bytes)));
      ul.append(li);
    }
    result.append(ul);
    const phone = hasCoarsePointer();
    const sharer = shareButton(r.name, r.copy, r.saved);
    const shareFallback = sharer?.classList.contains("share-fallback") ?? false;
    if (sharer) {
      result.append(sharer);
      if (shareFallback) result.append(shareFallbackHint());
    } else if (phone && r.saved) {
      result.append(shareFallbackHint(true));
    }
    if (!r.saved && !shareFallback) {
      const again = el("button", "btn", "Save it");
      again.dataset.shareSaveFallback = "true";
      again.addEventListener("click", () => cleaning.save(r.name, r.copy));
      result.append(again);
    }
    const open = el("button", "btn", "Open the copy");
    open.title = "Check it yourself: search it for what you blacked out";
    open.addEventListener("click", () => cleaning.open(r.copy));
    result.append(open);
    const verification = copyVerification(r, undefined, m.file.format);
    if (verification) result.append(verification);
  });
  return group;
}
