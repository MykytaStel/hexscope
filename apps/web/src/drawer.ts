import { canBlackOut, openBlackPicture } from "./blackpicture";
import { findEmbedded, type Embedded, type EmbeddedSearchProgress, type EmbeddedSearchResult } from "./embedded";
import { blackoutFigure } from "./blackout";
import { advice } from "./advice";
import { checkThumbnail, decodePicture, drawn } from "./thumbnail";
import { categories, share } from "./share";
import { Concern, FileModel, Kind, Role } from "./model";
import { verdict } from "./verdict";
import { done } from "./announce";
import { recentFiles } from "./recent";
import { headline, noun } from "./headline";
import { emailPath } from "./emailpath";
import { kindOf } from "./files";
import { makeupGroup } from "./makeup";
import { el, fact, formatBytes, hasCoarsePointer, hex } from "./dom";
import { COLOR_TYPES, FACT_LABELS, KEPT, KIND_NAMES, REPAIRABLE, STRONG, degrees, isOpenDocument, mapLink } from "./knowledge";
import { MAX_COPIED, copyBytes } from "./copybytes";
import { copyPictureButton, isAudio, shareButton, shareFallbackHint, type CleanActions } from "./cleancard";
import { createCleaner } from "./cleaner";
import { forgetPictures, toShown, type Shown } from "./pdf-pictures";
import { openStructureReport, pdfRedactorPlaceholder, saveStructureJson } from "./drawer-tools";
import { hasCleanableAudioTags } from "./audio-clean";
import { currentLocale, translateText } from "./i18n";

export type { CleanActions, CleanResult, RepairResult } from "./cleancard";

type MetadataGroupKey = "general" | "camera" | "location" | "dates" | "creator" | "additional";

const METADATA_GROUPS: { key: MetadataGroupKey; title: string }[] = [
  { key: "general", title: "General information" },
  { key: "camera", title: "Camera & device" },
  { key: "location", title: "Location" },
  { key: "dates", title: "Dates" },
  { key: "creator", title: "Author & software" },
  { key: "additional", title: "Additional fields" },
];

type VerdictKind = ReturnType<typeof verdict>[number]["kind"];

const AUTOMATIC_EMBEDDED_SEARCH_BYTES = 16 * 1024 * 1024;
const EMBEDDED_MEDIA_READ_CHUNK = 1024 * 1024;

/** Pulls deferred media into the existing byte view in small, cancellable reads. */
async function loadEmbeddedSearchBytes(
  m: FileModel,
  maxBytes: number,
  signal: AbortSignal,
  onProgress: (loaded: number, total: number) => void,
): Promise<void> {
  if (!m.missing || !m.source) return;
  const limit = Math.min(m.bytes.length, maxBytes);
  const total = m.missing.wanted(0, limit).reduce((n, [from, to]) => n + to - from, 0);
  onProgress(0, total);

  for (let from = 0; from < limit; from += EMBEDDED_MEDIA_READ_CHUNK) {
    const to = Math.min(limit, from + EMBEDDED_MEDIA_READ_CHUNK);
    for (let attempt = 0; ; attempt++) {
      if (signal.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
      const wanted = m.missing.wanted(from, to);
      const bytes = wanted.reduce((n, [a, b]) => n + b - a, 0);
      if (bytes === 0) break;
      if (await m.missing.fill(m.bytes, m.source, from, to)) {
        break;
      }
      // The byte view may be loading the same small range after a tap. Give
      // that read time to finish before deciding that the source has failed.
      if (attempt >= 9) throw new Error("Could not read the file's media bytes.");
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
    const remaining = m.missing.wanted(0, limit).reduce((n, [a, b]) => n + b - a, 0);
    onProgress(Math.max(0, total - remaining), total);
  }
}

/** Keeps signature searches away from media bytes the parser has not loaded. */
function readableRanges(m: FileModel, ranges: readonly [number, number][]): [number, number][] {
  const available = m.missing ? m.missing.available(m.bytes.length) : [[0, m.bytes.length] as [number, number]];
  const out: [number, number][] = [];
  for (const [from, to] of ranges) {
    for (const [readFrom, readTo] of available) {
      const start = Math.max(from, readFrom);
      const end = Math.min(to, readTo);
      if (start < end) out.push([start, end]);
    }
  }
  return out;
}

/** Whether this search actually skipped chunks that are still unloaded. */
function hasUnreadBytes(m: FileModel, ranges: readonly [number, number][]): boolean {
  return Boolean(m.missing && ranges.some(([from, to]) => m.missing!.wanted(from, to).length > 0));
}

const VERDICT_KIND_LABELS: Record<VerdictKind, string> = {
  damage: "Damage",
  misnamed: "Name mismatch",
  hidden: "Hidden content",
  warning: "Partial check",
  reveals: "Personal information",
  oddity: "Format note",
  healthy: "File health",
  unknown: "File type",
};

const METADATA_FACT_GROUPS: Record<Exclude<MetadataGroupKey, "general" | "additional">, Set<string>> = {
  camera: new Set(["camera", "lens", "serial", "owner", "shutter", "uptime"]),
  location: new Set(["place", "photoplace"]),
  dates: new Set(["taken", "created", "modified", "updates", "timezone"]),
  creator: new Set(["software", "author", "editor", "manager", "producer", "application", "company", "label", "template", "downloaded", "folder", "mailer", "sentfrom", "computer", "printer", "printed", "account", "emails"]),
};

function metadataGroupFor(kind: string): MetadataGroupKey {
  for (const key of ["camera", "location", "dates", "creator"] as const) {
    if (METADATA_FACT_GROUPS[key].has(kind)) return key;
  }
  return "additional";
}

/** Details for one node on the left, facts about the whole file on the right. */
export class Drawer {
  private shown: [FileModel | null, number, boolean] | null = null;
  private note: string | null = null;
  private readonly node: HTMLElement;
  private readonly file: HTMLElement;
  private readonly embeddedSearches = new Set<AbortController>();
  /** How many archives the document on screen sits inside. */
  nested = 0;

  constructor(
    host: HTMLElement,
    private readonly onSelect: (id: number) => void,
    private readonly onSelectRange: (start: number, len: number) => void,
    private readonly onPlay: (entry: number) => void,
    private readonly onOpen: (entry: number) => void,
    private readonly onHover: (id: number) => void,
    private readonly cleaning: CleanActions,
    /** The picture panel, for a file that has one. */
    private readonly picture: (m: FileModel) => HTMLElement | null,
  ) {
    this.node = el("section", "drawer-node");
    this.node.setAttribute("aria-label", "The part selected");
    this.file = el("section", "drawer-file");
    this.file.setAttribute("aria-label", "What was found");
    host.append(this.node, this.file);
  }

  // Each page's pictures, fetched once per file, when a page is first drawn.
  private pictured: FileModel | null = null;
  private pictureCache = new Map<number, Promise<Shown[]>>();
  private pictureReadStatus = new Map<number, "loading" | "loaded" | "failed">();

  private setPictureOwner(m: FileModel | null): void {
    if (m === this.pictured) return;
    forgetPictures();
    this.pictured = m;
    this.pictureCache = new Map();
    this.pictureReadStatus.clear();
  }

  private picturesOf(m: FileModel, page: number): Promise<Shown[]> {
    this.setPictureOwner(m);
    let p = this.pictureCache.get(page);
    if (!p) {
      this.pictureReadStatus.set(page, "loading");
      p = this.cleaning.pictures(page)
        .then(toShown)
        .then((pictures) => {
          if (m !== this.pictured) {
            forgetPictures(pictures);
            return [];
          }
          this.pictureReadStatus.set(page, "loaded");
          return pictures;
        })
        .catch(() => {
          if (m === this.pictured) this.pictureReadStatus.set(page, "failed");
          return [];
        });
      this.pictureCache.set(page, p);
    }
    return p;
  }

  private pictureState(m: FileModel, page: number): "loading" | "loaded" | "failed" {
    return m === this.pictured ? (this.pictureReadStatus.get(page) ?? "loading") : "loading";
  }

  /** Moves the keyboard to what was found, without scrolling the page away. */
  focusVerdict(): void {
    this.file.querySelector<HTMLElement>(".verdict-title")?.focus({ preventScroll: true });
  }

  showFile(m: FileModel | null): void {
    this.cancelEmbeddedSearches();
    this.setPictureOwner(m);
    this.file.replaceChildren();
    if (!m) return;
    const f = m.file;
    const answerArea = el("div", "overview-answer");
    const flow = el("div", "overview-flow");
    this.file.append(answerArea, flow);

    // What someone came to know first: the verdict, then what the file gives
    // away, then the picture; how it is made, after.
    const answer = this.verdict(m);
    answerArea.append(answer);
    // A photo, a video or a PDF always gets the card, if only to say it gives
    // nothing away; an archive only when it is a document with properties.
    if (!["zip", "cfb", "unknown"].includes(f.format) || f.facts.length > 0) {
      flow.append(this.reveals(m));
    }
    // What to do about it, right under the answer: it presses the clean
    // copy's own button, further down, where the result is said.
    const clean = this.file.querySelector<HTMLButtonElement>(".reveals .btn-clean");
    if (clean) {
      const cta = el("button", "btn btn-primary verdict-cta", "Remove it — save a clean copy");
      cta.title = clean.title;
      clean.classList.add("is-echo");
      cta.addEventListener("click", () => {
        clean.click();
        cta.hidden = true;
        clean.closest(".cleaner")?.scrollIntoView({ behavior: "smooth", block: "start" });
      });
      // A picture can also go straight to a chat: copied clean, no file at all.
      const copy = copyPictureButton(m);
      const row = el("div", "verdict-actions");
      row.append(cta);
      if (copy) {
        const otherWays = el("details", "other-ways");
        otherWays.append(el("summary", undefined, "Other ways"), copy);
        row.append(otherWays);
      }
      answer.querySelector(".verdict-lines")?.after(row);
    }
    // A PDF: black out what you choose, not only what the file already hides.
    if (f.format === "pdf" && !f.facts.some((x) => x.kind === "encryption")) {
      flow.append(
        pdfRedactorPlaceholder(
          m,
          this.cleaning,
          (page) => this.picturesOf(m, page),
          (page) => this.pictureState(m, page),
        ),
      );
    }
    // Technical detail is available in one place, after the useful answer.
    const more = el("details", "more-details");
    more.append(el("summary", undefined, "For the curious"));
    const later = (e: HTMLElement) => more.append(e);
    more.append(makeupGroup(m, (id) => this.onSelect(id), (id) => this.onHover(id)));
    if (f.format === "heif") {
      void import("./heif-map")
        .then(({ heifMapGroup }) => {
          if (!more.isConnected) return;
          const heifMap = heifMapGroup(m, (id) => this.onSelect(id));
          if (heifMap) more.append(heifMap);
          if (document.body.dataset.appView === "map") more.open = true;
        })
        .catch(() => {
          if (more.isConnected) more.append(el("p", "hint", "The HEIF map could not be loaded. The file structure is still available."));
        });
    }
    const picture = this.picture(m);
    if (picture) later(picture);

    const fileGroup = el("div", "group");
    fileGroup.append(el("h2", undefined, "File"));
    const coverageText = {
      complete: "The parser read the structures this format supports.",
      partial: "The parser could not read all supported structures; there may be more findings.",
      unsupported: "No format-aware parser could read this file; only byte-level details and signature clues are shown.",
    }[f.coverageStatus];
    const coverage = el("p", f.coverageStatus === "complete" ? "hint coverage-status" : "problem is-warning coverage-status", coverageText);
    coverage.dataset.status = f.coverageStatus;
    coverage.setAttribute("role", "status");
    fileGroup.append(coverage);
    const grid = el("dl", "facts");
    fact(grid, "Size", `${formatBytes(m.bytes.length)} · ${m.bytes.length.toLocaleString("en")} bytes`);
    if (!f.ihdr && f.dimensions) fact(grid, "Image", `${f.dimensions[0]} × ${f.dimensions[1]}`);
    if (f.ihdr) {
      const [w, h, depth, color, interlace] = f.ihdr;
      fact(grid, "Image", `${w} × ${h}`);
      fact(grid, "Pixels", `${COLOR_TYPES[color] ?? `type ${color}`}, ${depth}-bit`);
      if (interlace) fact(grid, "Layout", "Interlaced (Adam7)");
    }
    fact(grid, "Parsed in", `${f.parseMs.toFixed(1)} ms`);
    if (m.missing) fact(grid, "Read", `All but the picture and sound, ${formatBytes(m.missing.total)}: those are read where the bytes view shows them`);
    fileGroup.append(grid);
    const report = el("button", "link report-link", "Report a problem with this file");
    report.title = "Shows the file's layout, without its content, to paste into a bug report";
    report.addEventListener("click", () => openStructureReport(m));
    const save = el("button", "link report-link", "Save the structure as JSON");
    save.title = "Every part, its offset, length, value and explanation, nested as in the tree";
    save.addEventListener("click", () => saveStructureJson(m));
    const inside = el("button", "link report-link", translateText("Search for embedded files", currentLocale()));
    inside.title = translateText("Reads the first 256 MiB in small local slices and searches for known file signatures", currentLocale());
    let activeController: AbortController | null = null;
    inside.addEventListener("click", () => {
      if (activeController) {
        activeController.abort();
        inside.disabled = true;
        const current = fileGroup.querySelector<HTMLElement>(".embedded-search-status .hint");
        if (current) current.textContent = translateText("Cancelling…", currentLocale());
        return;
      }
      fileGroup.querySelector(".embedded-search-results")?.remove();
      const controller = this.beginEmbeddedSearch();
      activeController = controller;
      const status = this.searchProgress("Searching file signatures…");
      status.box.append(el("p", "hint", translateText("Media data are read locally in small pieces, up to the first 256 MiB of this file; nothing is uploaded.", currentLocale())));
      inside.after(status.box);
      inside.textContent = translateText("Cancel search", currentLocale());
      const allRanges: [number, number][] = [[0, m.bytes.length]];
      void (async () => {
        await loadEmbeddedSearchBytes(m, 256 * 1024 * 1024, controller.signal, (loaded, total) => {
          status.bar.value = total > 0 ? Math.min(1, loaded / total) : 1;
          status.detail.textContent = `${translateText("Loading media data…", currentLocale())} · ${formatBytes(loaded)} / ${formatBytes(total)}`;
        });
        if (controller.signal.aborted) throw new DOMException("Embedded file search cancelled", "AbortError");
        status.bar.value = 0;
        status.detail.textContent = translateText("Searching file signatures…", currentLocale());
        return findEmbedded(
          m.bytes,
          readableRanges(m, allRanges),
          m.file.format === "zip" ? ["zip"] : [],
          { signal: controller.signal, onProgress: (progress) => this.updateSearchProgress(status, progress) },
        );
      })()
        .then((result) => {
          if (!status.box.isConnected) return;
          status.box.replaceWith(this.embeddedSearchResults(m, result, "Possible embedded files", hasUnreadBytes(m, allRanges)));
          inside.textContent = translateText("Search for embedded files", currentLocale());
        })
        .catch((error: unknown) => {
          if (!status.box.isConnected) return;
          if (error instanceof DOMException && error.name === "AbortError") {
            status.box.replaceChildren(el("p", "hint", translateText("Search cancelled; unsearched bytes remain.", currentLocale())));
            inside.textContent = translateText("Search for embedded files", currentLocale());
          } else {
            status.box.replaceChildren(el("p", "hint", translateText("Could not search this file.", currentLocale())));
            inside.textContent = translateText("Search for embedded files", currentLocale());
          }
        })
        .finally(() => {
          this.embeddedSearches.delete(controller);
          if (activeController === controller) activeController = null;
          inside.disabled = false;
        });
    });
    fileGroup.append(report, save, inside);
    // Another file beside this one: what one gives away that the other does not.
    const pick = el("button", "link compare-pick", "Compare with another file…");
    const input = el("input");
    input.type = "file";
    input.hidden = true;
    input.addEventListener("change", () => {
      const f = input.files?.[0];
      input.value = "";
      if (f) this.cleaning.compare(f);
    });
    pick.addEventListener("click", () => input.click());
    fileGroup.append(input);
    fileGroup.append(pick);
    later(fileGroup);
    const share = this.file.querySelector<HTMLElement>(".reveals .sharer");
    if (share) more.append(share);
    if (document.body.dataset.appView === "map") more.open = true;
    flow.append(more);

    // For a ZIP, the stream is whichever entry was last played: not the file's.
    if (f.trace && f.format === "png") {
      const [events, literals, matches, output] = f.trace;
      const deflate = el("div", "group");
      deflate.append(el("h2", undefined, "DEFLATE"));
      const d = el("dl", "facts");
      fact(d, "Compressed", `${formatBytes(f.idatBytes)} → ${formatBytes(output)}`);
      if (f.idatBytes > 0) fact(d, "Ratio", `${(output / f.idatBytes).toFixed(2)}×`);
      fact(d, "Steps", events.toLocaleString());
      deflate.append(d);
      later(deflate);

      // Literal vs back-reference split: the one number that says how much
      // LZ77 actually found to reuse.
      const coded = literals + matches;
      if (coded > 0) {
        const share = (matches / coded) * 100;
        const bar = el("div", "split");
        const lit = el("span", "split-lit");
        const mat = el("span", "split-match");
        lit.style.flexGrow = String(literals);
        mat.style.flexGrow = String(matches);
        bar.append(lit, mat);
        const legend = el(
          "p",
          "split-legend",
          `${literals.toLocaleString()} literals · ${matches.toLocaleString()} back-references (${share.toFixed(0)}%)`,
        );
        deflate.append(bar, legend);
      }
    }
    this.file.append(this.nextFile(), this.recent());
  }

  /** The other files opened in this tab, to go back to; hidden when there are none. */
  private recent(): HTMLElement {
    const group = el("div", "group recent-files");
    const files = recentFiles();
    group.hidden = files.length === 0;
    group.append(el("h2", undefined, "Opened in this tab"));
    const ul = el("ul", "recent-list");
    files.forEach((f, i) => {
      const b = el("button", "recent-file");
      b.dataset.recent = String(i);
      b.append(el("span", "recent-name", f.name), el("span", "recent-size", formatBytes(f.size)));
      const li = el("li");
      li.append(b);
      ul.append(li);
    });
    group.append(ul, el("p", "recent-note", "Kept only while this tab is open, and never sent anywhere."));
    return group;
  }

  /** Where to go once done with this file: another one, chosen or pasted. */
  private nextFile(): HTMLElement {
    const group = el("div", "group next-file");
    group.append(el("h2", undefined, "Check another file"));
    const row = el("div", "entry-actions");
    const choose = el("button", "btn", "Choose a file");
    choose.dataset.opens = "picker";
    const paste = el("button", "btn", "Paste a copied picture");
    paste.dataset.paste = "";
    paste.hidden = !navigator.clipboard?.read;
    row.append(choose, paste);
    group.append(row);
    return group;
  }

  /** The answer to the question people arrive with: is it all right, and what does it say? */
  private verdict(m: FileModel): HTMLElement {
    const group = el("div", "group verdict");
    const intro = el("div", "verdict-intro");
    const findings = el("div", "verdict-findings");
    const head = headline(m);
    intro.append(el("p", "verdict-label", "The short answer"));
    const title = el("h2", `verdict-title is-${head.tone}`, head.text);
    title.tabIndex = -1;
    intro.append(title);
    // A message that may be forged: what to do comes before why.
    if (noun(m) === "email" && head.tone === "danger") {
      intro.append(el("p", "verdict-do", "Do not reply, open its files or follow its links. If it matters, ask the sender another way — a number or address you already have."));
    }
    const list = el("ul", "verdict-lines");
    for (const line of verdict(m)) {
      const li = el("li", `verdict-line is-${line.kind}`);
      li.append(el("span", "verdict-kind", VERDICT_KIND_LABELS[line.kind]));
      li.append(el("span", "verdict-text", line.text));
      // A file not read has no part to show.
      if (line.node >= 0 && line.kind !== "unknown") {
        const show = el("button", "verdict-show", "Find in file");
        show.title = "Go to this finding in the file";
        show.addEventListener("click", () => this.showLine(line.node));
        li.append(show);
      }
      list.append(li);
    }
    findings.append(list);
    group.append(intro, findings);
    // What the structure leaves over — data after the end, a gap — may be a
    // whole file of its own: say which, and open it.
    const hiddenParts = m.slices().filter((sl) => sl.role === Role.Hidden).map((sl): [number, number] => [sl.start, sl.start + sl.len]);
    const hiddenRanges = readableRanges(m, hiddenParts);
    if (hiddenParts.length > 0) {
      const controller = this.beginEmbeddedSearch();
      const status = this.searchProgress("Checking hidden data for file signatures…", () => controller.abort());
      findings.append(status.box);
      void findEmbedded(m.bytes, hiddenRanges, [], {
        signal: controller.signal,
        maxBytes: AUTOMATIC_EMBEDDED_SEARCH_BYTES,
        onProgress: (progress) => this.updateSearchProgress(status, progress),
      })
        .then((result) => {
          if (!status.box.isConnected) return;
          status.box.replaceWith(this.embeddedSearchResults(m, result, "Hidden in it", hasUnreadBytes(m, hiddenParts), true));
        })
        .catch(() => {
          if (!status.box.isConnected) return;
          status.box.replaceChildren(el("p", "hint", "Search cancelled; unsearched bytes remain."));
        })
        .finally(() => this.embeddedSearches.delete(controller));
    }
    if (m.file.format !== "unknown") intro.append(el("p", "hint", translateText("Found by reading the file's structure. It is not a virus scan.", currentLocale())));
    if (REPAIRABLE.includes(m.file.format) && verdict(m).some((l) => l.kind === "damage")) findings.append(this.repairer());
    return group;
  }

  /**
   * Where a verdict line points: in the summary, the fact it names, brought
   * into view; otherwise, or for a part with no fact, its bytes.
   */
  private showLine(node: number): void {
    if (document.body.dataset.view === "bytes" || !this.showFact(node)) this.onSelect(node);
  }

  /** Brings the summary's fact for a part into view, if it has one; says whether it did. */
  showFact(node: number): boolean {
    const row = this.file.querySelector<HTMLElement>(`.reveal-list dd[data-node="${node}"]`);
    const label = row?.previousElementSibling as HTMLElement | null;
    if (!row || !label) return false;
    label.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "center" });
    for (const e of [label, row]) {
      e.classList.remove("is-flash");
      void e.offsetWidth;
      e.classList.add("is-flash");
    }
    return true;
  }

  /** Files found inside this one, each to open or save. */
  private insideList(m: FileModel, found: Embedded[], title: string): HTMLElement {
    const box = el("div", "inside");
    const heading = title === "Hidden in it"
      ? `${translateText(title, currentLocale())} · ${translateText("Possible file signatures", currentLocale())}`
      : translateText("Possible file signatures", currentLocale());
    box.append(
      el("p", "inside-title", `${heading}: ${found.length}`),
      el("p", "hint", translateText("These are signature matches, not verified files. Open one to inspect it; this is not a virus scan.", currentLocale())),
    );
    const ul = el("ul", "inside-list");
    const stem = (m.name.split("/").pop() ?? m.name).replace(/\.[^.]*$/, "");
    for (const e of found) {
      const li = el("li");
      const name = `${stem}-at-${e.start.toString(16).toUpperCase()}.${e.ext}`;
      const bytes = () => m.bytes.subarray(e.start, e.end);
      const at = el("button", "link inside-at", `0x${e.start.toString(16).toUpperCase()}`);
      at.title = "Show where it starts";
      at.addEventListener("click", () => this.onSelect(m.nodeAt(e.start)));
      const open = el("button", "btn", "Open");
      open.addEventListener("click", () => this.cleaning.openInside(bytes(), name));
      const save = el("button", "btn", "Save");
      save.addEventListener("click", () => this.cleaning.save(name, bytes().slice()));
      // What it is, where, and how big on one line; what to do with it on the next.
      const actions = el("span", "inside-actions");
      actions.append(open, save);
      const what = translateText(e.what, currentLocale());
      li.append(el("span", "inside-what", `${what.charAt(0).toLocaleUpperCase(currentLocale())}${what.slice(1)}`), at, el("span", "clean-size", formatBytes(e.end - e.start)), actions);
      ul.append(li);
    }
    box.append(ul);
    return box;
  }

  private beginEmbeddedSearch(): AbortController {
    const controller = new AbortController();
    this.embeddedSearches.add(controller);
    return controller;
  }

  private cancelEmbeddedSearches(): void {
    for (const controller of this.embeddedSearches) controller.abort();
    this.embeddedSearches.clear();
  }

  private searchProgress(label: string, cancel?: () => void): { box: HTMLElement; bar: HTMLProgressElement; detail: HTMLElement } {
    const box = el("div", "embedded-search-status");
    box.setAttribute("role", "status");
    box.setAttribute("aria-live", "polite");
    const bar = el("progress");
    bar.max = 1;
    bar.value = 0;
    bar.setAttribute("aria-label", translateText("Search progress", currentLocale()));
    const detail = el("p", "hint", translateText(label, currentLocale()));
    box.append(bar, detail);
    if (cancel) {
      const button = el("button", "link", translateText("Cancel search", currentLocale()));
      button.addEventListener("click", () => {
        button.disabled = true;
        button.textContent = translateText("Cancelling…", currentLocale());
        cancel();
      });
      box.append(button);
    }
    return { box, bar, detail };
  }

  private updateSearchProgress(
    status: { bar: HTMLProgressElement; detail: HTMLElement },
    progress: EmbeddedSearchProgress,
  ): void {
    status.bar.value = progress.total > 0 ? Math.min(1, progress.searched / progress.total) : 1;
    status.detail.textContent = `${formatBytes(progress.searched)} / ${formatBytes(progress.total)} · ${progress.matches} ${translateText("found", currentLocale())}`;
  }

  private embeddedSearchResults(
    m: FileModel,
    result: EmbeddedSearchResult,
    title: string,
    skipsMedia: boolean,
    automatic = false,
  ): HTMLElement {
    const box = el("div", "embedded-search-results");
    if (result.matches.length > 0) box.append(this.insideList(m, result.matches, title));
    else if (skipsMedia && result.searched === 0) {
      const message = title === "Hidden in it"
        ? "The hidden data is not loaded for this view, so it was not checked."
        : "No readable bytes outside the picture and sound were available for this search.";
      box.append(el("p", "hint", translateText(message, currentLocale())));
    }
    else if (title === "Hidden in it") box.append(el("p", "hint", translateText("No known file signatures were found in the hidden bytes checked.", currentLocale())));
    else box.append(el("p", "hint", translateText("No known file signatures were found in the searched bytes.", currentLocale())));

    if (result.limited) {
      const message = result.matchLimitReached
        ? "The result limit was reached; more matches may be present."
        : automatic
          ? "Only part of the hidden data was checked automatically; use Search for embedded files for a wider search."
          : "The search limit was reached; some available bytes were not checked.";
      box.append(el("p", "hint", translateText(message, currentLocale())));
    }
    if (skipsMedia) box.append(el("p", "hint", translateText("Media bytes not loaded for this view were skipped.", currentLocale())));
    return box;
  }

  /** One button that saves a repaired copy, then says what it did. */
  private repairer(): HTMLElement {
    const box = el("div", "repairer");
    const button = el("button", "btn btn-repair", "Try to repair — save a fixed copy");
    button.title = "Makes the copy in this tab: nothing is uploaded";
    box.append(
      button,
      el("p", "hint", "Keeps what survived and puts it back in order: checksums made right, a file cut short given its end, an archive rebuilt from its whole files. Nothing is guessed."),
    );
    button.addEventListener("click", async () => {
      button.disabled = true;
      button.textContent = "Repairing…";
      const r = await this.cleaning.repair();
      button.remove();
      if (r.error) {
        box.append(el("p", "hint", `No repaired copy: ${r.error}.`));
        return;
      }
      box.append(done(`Saved a repaired copy as “${r.name}” — look for it in your downloads. What was done:`));
      const ul = el("ul", "clean-list");
      for (const f of r.fixed) ul.append(el("li", undefined, f.charAt(0).toUpperCase() + f.slice(1)));
      const open = el("button", "btn", "Open the repaired copy");
      open.title = "Check it yourself: the damage should be gone";
      open.addEventListener("click", () => this.cleaning.openRepaired(r.bytes));
      box.append(ul, open);
    });
    return box;
  }

  /** What the file is made of: a bar in file order and a legend by size. */
  /**
   * What the file's metadata gives away. Each fact is a link to the bytes
   * that hold it; the map link sends the coordinates nowhere unless clicked.
   */
  private reveals(m: FileModel): HTMLElement {
    const f = m.file;
    const group = el("div", "group reveals");
    const heading: Record<string, string> = {
      jpeg: "What this photo reveals",
      heif: "What this photo reveals",
      webp: "What this image reveals",
      gif: "What this image reveals",
      tiff: "What this image reveals",
      png: "What this image reveals",
      video: "What this video reveals",
      wasm: "What this module reveals",
      svg: "What this drawing reveals",
      mp3: "What this audio file reveals",
      flac: "What this audio file reveals",
      ogg: "What this audio file reveals",
      wav: "What this audio file reveals",
      unknown: "What this file reveals",
      eml: "What this email reveals",
      msg: "What this email reveals",
    };
    const audioFile = ["mp3", "flac", "ogg", "wav"].includes(f.format);
    const cleanableAudioTags = hasCleanableAudioTags(f.format, f.labels, f.metadataComplete, f.facts);
    const title = el("h2", undefined, audioFile ? "What this audio file reveals" : isAudio(m) ? "What this recording reveals" : (heading[f.format] ?? "What this document reveals"));
    group.append(title);
    // An email's way from its sender to you, drawn, with what is wrong on it.
    if (f.format === "eml") {
      const path = emailPath(m, (id) => this.onSelect(id));
      if (path) group.append(path);
    }
    // The picture it is about, and where it was taken, drawn: what the
    // numbers below mean, before reading them.
    const pictured = ["jpeg", "png", "heif", "webp", "gif", "tiff"].includes(f.format);
    if (pictured || f.location) {
      const hero = el("div", f.location ? "reveal-hero has-map" : "reveal-hero");
      group.append(hero);
      if (pictured) {
        const frame = el("div", "reveal-picture-frame");
        hero.append(frame);
        void decodePicture(m).then((b) => {
          if (!b) return frame.remove();
          // As sharp as the screen shows it: a phone's screen has three
          // pixels to each of the page's.
          const side = Math.min(2048, Math.ceil(Math.max(frame.clientWidth, frame.clientHeight, 360) * (devicePixelRatio || 1)));
          const c = drawn(b, f.format === "jpeg" ? f.orientation : 1, side);
          c.className = "reveal-picture";
          c.setAttribute("role", "img");
          c.setAttribute("aria-label", "The picture");
          frame.append(c);
        });
      }
      if (f.location) {
        const { latitude, longitude } = f.location;
        const place = el("figure", "place-map");
        hero.append(place);
        // Loaded only for a file that says where: the coastline is 20 KB.
        void import("./map")
          .catch(() => null)
          .then((mod) => {
            // A tab open across a new release may not find the map's code:
            // the coordinates below still say where.
            if (!mod) {
              place.remove();
              hero.classList.remove("has-map");
              return;
            }
            const where = `${degrees(latitude, "N", "S")}, ${degrees(longitude, "E", "W")}`;
            place.append(mod.placeMap(latitude, longitude, `A map with a pin where it was taken: ${where}`));
            // The map site is one click away on the Location row; this one asks no one.
            place.append(el("figcaption", undefined, "Where it was taken — drawn here, without asking any map site."));
          });
      }
    }
    if (!f.location && f.facts.length === 0) {
      // A PNG can hold notes that name no one, such as a comment or the
      // time it was changed: still worth a clean copy.
      const notes = ["tEXt", "zTXt", "iTXt", "eXIf", "tIME", "data after the end of the image"];
      const removable = f.format === "png" && m.children(0).some((id) => notes.includes(m.label(id)));
      const none =
        audioFile
          ? f.format === "wav"
            ? "No supported WAV INFO or Broadcast Wave metadata was found."
            : "No supported audio metadata tags were found."
          : f.format === "pdf"
          ? "No document information or XMP: nothing about who wrote it, with what, or when."
          : removable
            ? "Nothing about the camera, the place or who made it, but it holds notes or data that can go."
            : f.format === "video"
              ? isAudio(m)
                ? "No location, device or software in its metadata."
                : "No location, camera or software in its metadata."
              : f.format === "wasm"
                ? "No names, tools, paths or debug info: nothing about how, or by whom, it was built."
              : "No EXIF metadata: nothing about the camera, the time or the place.";
      group.append(el("p", "hint", none));
      if (audioFile && f.metadataComplete === false) {
        group.append(el("p", "hint", "Some audio metadata could not be fully read; the result is partial."));
      }
      if (audioFile && !cleanableAudioTags) {
        group.append(el("p", "hint", f.format === "wav"
          ? "WAV metadata is read-only in this release. Audio playback and clean copies are not provided."
          : "Read-only metadata inspection. Audio playback is not provided. No clean copy can be made unless supported tags are present and fully checked."));
      }
      if (cleanableAudioTags) group.append(createCleaner(m, this.cleaning, group));
      if (removable) group.append(createCleaner(m, this.cleaning, group));
      if (canBlackOut(m)) group.append(this.blackOutButton(m));
      // A screenshot with no metadata still shows what was on the screen.
      const tips = this.tips(m);
      if (tips) group.append(tips);
      return group;
    }

    const metadataGroups = el("div", "metadata-groups reveal-list");
    const groupElements = new Map<MetadataGroupKey, { section: HTMLElement; list: HTMLDListElement; body: HTMLElement }>();
    for (const { key, title } of METADATA_GROUPS) {
      const section = el("section", "metadata-group");
      section.dataset.metadataGroup = key;
      const list = el("dl", "metadata-field-list");
      let body: HTMLElement = section;
      if (key === "additional") {
        const details = el("details", "metadata-additional-details");
        details.append(el("summary", undefined, title), list);
        section.append(details);
        body = details;
      } else {
        section.append(el("h3", "metadata-group-title", title), list);
      }
      metadataGroups.append(section);
      groupElements.set(key, { section, list, body });
    }
    const generalList = groupElements.get("general")!.list;
    fact(generalList, "File type", f.format === "unknown" ? "Not a format hexscope reads" : kindOf(m));
    fact(generalList, "Size", `${formatBytes(m.bytes.length)} · ${m.bytes.length.toLocaleString("en")} bytes`);
    const dimensions = f.ihdr ? [f.ihdr[0], f.ihdr[1]] : f.dimensions;
    if (dimensions) fact(generalList, "Dimensions", `${dimensions[0]} × ${dimensions[1]}`);

    const row = (label: string, text: string, node: number, strong = false, key: MetadataGroupKey = "additional") => {
      const dd = el("dd");
      const link = el("button", strong ? "reveal-link is-strong" : "reveal-link", text);
      link.title = "Show where in the file this is";
      link.addEventListener("click", () => this.onSelect(node));
      dd.append(link);
      dd.dataset.node = String(node);
      groupElements.get(key)!.list.append(el("dt", strong ? "is-strong" : undefined, label), dd);
      return dd;
    };

    if (f.location) {
      const { latitude, longitude, altitude, node } = f.location;
      const where = `${degrees(latitude, "N", "S")}, ${degrees(longitude, "E", "W")}${
        altitude !== null ? ` · ${Math.round(altitude)}\u00a0m` : ""
      }`;
      // Under the group "Location", the row says what it is: the coordinates.
      const dd = row("Coordinates", where, node, true, "location");
      dd.dataset.kind = "location";
      const term = dd.previousElementSibling as HTMLElement | null;
      if (term) term.dataset.kind = "location";
      dd.append(mapLink(latitude, longitude));
    }
    let thumbRow: HTMLElement | null = null;
    for (const fact of f.facts) {
      const covered = fact.kind === "covered";
      const metadataGroup = metadataGroupFor(fact.kind);
      const dd = row(FACT_LABELS[fact.kind] ?? fact.kind, fact.text, fact.node, STRONG.includes(fact.kind), metadataGroup);
      dd.dataset.kind = fact.kind;
      const term = dd.previousElementSibling as HTMLElement | null;
      if (term) term.dataset.kind = fact.kind;
      // A sentence reads better in the body font; only coordinates and codes are set in mono.
      if (!covered) dd.querySelector(".reveal-link")?.classList.remove("is-strong");
      // Hidden text is shown as if selected: that is how someone finds it.
      // Each kind — "hidden: “…”; in white on white: “…”" — marked on its own.
      const ghosts = fact.kind === "hiddentext" ? [...fact.text.matchAll(/([^;“]*?: )“([^”]*)”/g)] : [];
      if (ghosts.length > 0) {
        dd.querySelector(".reveal-link")?.replaceChildren(
          ...ghosts.flatMap((g, i) => [...(i ? ["; "] : []), g[1].trimStart(), el("mark", "ghost-text", g[2])]),
        );
      }
      // Deleted text looks the way Word marks it: struck through.
      const deleted = fact.kind === "deleted" || fact.kind === "earlier" ? /^“(.*)”$/.exec(fact.text) : null;
      if (deleted) {
        const parts = deleted[1].split(" … ").flatMap((t, i) => [...(i ? [" … "] : []), el("del", "deleted-text", t)]);
        dd.querySelector(".reveal-link")?.replaceChildren(...parts);
      }
      // Each attached file opens as a file of its own, with the way back.
      if (fact.kind === "attachments" && f.attachments.length > 0) {
        const open = el("span", "attachment-open");
        f.attachments.forEach((name, i) => {
          const b = el("button", "link", `Open ${name}`);
          b.title = "Opens the attached file here, with the way back to this one";
          b.addEventListener("click", () => this.onOpen(i));
          open.append(b);
        });
        dd.append(open);
      }
      // A photo inside a document that says where: the same map link as a photo's own.
      const place = fact.kind === "photoplace" ? /taken at ([\d.]+)° ([NS]), ([\d.]+)° ([EW])/.exec(fact.text) : null;
      if (place) {
        const lat = Number(place[1]) * (place[2] === "S" ? -1 : 1);
        const lon = Number(place[3]) * (place[4] === "W" ? -1 : 1);
        dd.append(mapLink(lat, lon));
      }
      if (fact.kind === "thumbnail") thumbRow = dd;
      // Text found under a black box is shown the way it was meant to look:
      // blacked out, with what is still there showing through.
      const quoted = covered ? /^(.*?: )“(.*)”$/.exec(fact.text) : null;
      if (quoted) {
        const link = dd.querySelector(".reveal-link");
        const bars = quoted[2].split(" · ").map((t) => el("span", "redacted", t));
        link?.replaceChildren(quoted[1], ...bars);
      }
      // What the clean copy cannot take out stays unstruck.
      // A Word document's comments go with the clean copy, and a LibreOffice
      // document's; a workbook's or a deck's stay.
      const word = f.format === "zip" && (f.labels.includes("word/document.xml") || isOpenDocument(f.labels));
      if (KEPT[f.format]?.includes(fact.kind) && !(word && fact.kind === "comments")) {
        dd.dataset.kept = "";
        term?.setAttribute("data-kept", "");
      }
    }
    for (const { key } of METADATA_GROUPS) {
      const elements = groupElements.get(key)!;
      if (key !== "general" && elements.list.children.length === 0) elements.section.remove();
    }
    const additionalFacts = f.facts.filter((entry) => metadataGroupFor(entry.kind) === "additional");
    if (additionalFacts.some((entry) => STRONG.includes(entry.kind))) {
      groupElements.get("additional")!.body.setAttribute("open", "");
    }
    const metadataSummary = el("nav", "metadata-summary");
    metadataSummary.setAttribute("aria-label", translateText("Metadata fields found", currentLocale()));
    metadataSummary.append(el("p", "metadata-summary-title", "Metadata found"));
    for (const { key, title } of METADATA_GROUPS) {
      if (key === "general") continue;
      const section = groupElements.get(key)!.section;
      const count = Math.floor((section.querySelector("dl")?.children.length ?? 0) / 2);
      if (count === 0) continue;
      const label = translateText(title, currentLocale());
      const jump = el("button", "link metadata-summary-link", `${label} · ${count.toLocaleString(currentLocale())}`);
      jump.type = "button";
      jump.addEventListener("click", () => {
        const details = section.querySelector<HTMLDetailsElement>("details");
        if (details) details.open = true;
        section.scrollIntoView({ behavior: "smooth", block: "nearest" });
        section.querySelector<HTMLButtonElement>(".reveal-link")?.focus({ preventScroll: true });
      });
      metadataSummary.append(jump);
    }
    if (metadataSummary.children.length > 1) group.append(metadataSummary);
    group.append(metadataGroups);
    if (audioFile && f.metadataComplete === false) {
      group.append(el("p", "hint", "Some audio metadata could not be fully read; the result is partial."));
    }
    // The first pages with black boxes over text or pictures, drawn; with
    // their pictures when a box lies on one.
    const overPictures = f.labels.includes("a picture under a black box");
    for (const b of f.blackouts.slice(0, 2)) group.append(blackoutFigure(b, overPictures ? this.picturesOf(m, b.page) : undefined));
    if (thumbRow) groupElements.get("additional")!.body.append(this.thumbnailCheck(m, thumbRow));
    // The one thing to do first, then when it matters and how to stop it
    // next time, then telling others.
    // An encrypted PDF is not rewritten: the copy would drop its protection.
    if (f.facts.some((x) => x.kind === "encryption")) {
      group.append(el("p", "hint", "It is encrypted, so hexscope does not make a clean copy of it."));
    } else if (f.format === "unknown") {
      // Found in its text, not read from its structure: there is nothing to rewrite it by.
      group.append(el("p", "hint", "hexscope does not take this format apart, so it makes no clean copy of it. The program that made the file keeps these names: change them there, or export a copy without them."));
    } else if (f.format === "eml" || f.format === "msg") {
      // A message is read, not sent on as it is: what it says is what the servers wrote.
      group.append(el("p", "hint", "The servers it passed through wrote these lines; an email is not cleaned, but its attached files can be — open one, then save a clean copy of it."));
    } else if (audioFile && !cleanableAudioTags) {
      group.append(el("p", "hint", f.format === "wav"
        ? "WAV metadata is read-only in this release. Audio playback and clean copies are not provided."
        : "Read-only metadata inspection. Audio playback is not provided. No clean copy can be made unless supported tags are present and fully checked."));
    } else {
      group.append(createCleaner(m, this.cleaning, group));
    }
    if (canBlackOut(m)) group.append(this.blackOutButton(m));
    const tips = this.tips(m);
    if (tips) group.append(tips);
    if (categories(m).length > 0) group.append(this.sharer(m));
    return group;
  }

  /** What to do about what was found, or null when there is nothing to say. */
  private tips(m: FileModel): HTMLElement | null {
    const tips = advice(m);
    if (tips.length === 0) return null;
    const box = el("div", "advice");
    box.append(el("p", "advice-title", "What you can do"));
    const ul = el("ul");
    for (const t of tips) {
      const li = el("li", undefined, t.text);
      if (t.guide) {
        const a = el("a", "advice-guide", `${t.guide[1]} →`);
        a.href = t.guide[0];
        li.append(" ", a);
      }
      ul.append(li);
    }
    box.append(ul);
    return box;
  }

  /**
   * Whether the photo's thumbnail is the photo: filled in once both are
   * decoded. A thumbnail that is not gets shown beside the picture, and a
   * line in the verdict; one that is says so quietly in its row.
   */
  private thumbnailCheck(m: FileModel, row: HTMLElement): HTMLElement {
    const box = el("div", "thumbcheck");
    box.hidden = true;
    void checkThumbnail(m).then((c) => {
      if (!c || !box.isConnected) return;
      const orientation = m.file.orientation;
      if (!c.differs) {
        row.append(el("span", "thumb-ok", " · matches the picture"));
      } else {
        const pair = el("div", "thumb-pair");
        for (const [b, caption] of [
          [c.thumbnail, "Thumbnail inside the file"],
          [c.picture, "The picture"],
        ] as const) {
          const fig = el("figure");
          fig.append(drawn(b, orientation, 160), el("figcaption", undefined, caption));
          pair.append(fig);
        }
        const what = c.differs === "shape" ? "cropped" : "edited";
        box.append(
          el("p", "thumb-title", "The thumbnail is not this picture"),
          pair,
          el(
            "p",
            "hint",
            `The photo was ${what} after the camera made its small copy, and the copy was left as it was: it still shows what was taken out, to anyone who opens it. The clean copy removes it.`,
          ),
        );
        box.hidden = false;
        this.addVerdict("hidden", `Hidden in it: the small copy inside the file still shows the photo before it was ${what}.`, c.node);
      }
      c.thumbnail.close();
      c.picture.close();
    });
    return box;
  }

  /** A finding that arrives after the verdict is drawn, placed by how much it matters. */
  private addVerdict(kind: "hidden", text: string, node: number): void {
    const list = this.file.querySelector(".verdict-lines");
    if (!list) return;
    const li = el("li", `verdict-line is-${kind}`);
    li.append(el("span", "verdict-text", text));
    const show = el("button", "verdict-show", "Show me");
    show.addEventListener("click", () => this.onSelect(node));
    li.append(show);
    // As the verdict itself does: "looks healthy" only when nothing is hidden.
    list.querySelector(".is-healthy")?.remove();
    const after = [...list.children].find((x) => !x.matches(".is-damage, .is-hidden"));
    list.insertBefore(li, after ?? null);
  }

  /** Shares what kinds of thing the file gives away, and nothing of what they are. */
  private sharer(m: FileModel): HTMLElement {
    const box = el("div", "sharer");
    const button = el("button", "btn", "Share what it revealed");
    button.title = "A card and a sentence that name only the kinds of thing — no place, name or number";
    const status = el("span", "hint", "Only the kinds of thing, never what they are.");
    button.addEventListener("click", async () => {
      button.disabled = true;
      status.textContent = await share(m);
      button.disabled = false;
    });
    box.append(button, status);
    return box;
  }

  /** Opens the picture to black out what it shows: faces, plates, an address. */
  private blackOutButton(m: FileModel): HTMLElement {
    const codes = m.codeBoxes.length;
    const b = el("button", codes ? "btn blackout-open" : "link blackout-open", codes ? `Black out the QR code${codes > 1 ? "s" : ""}…` : "Black out part of the picture…");
    b.title = codes
      ? "Opens the picture with a box over each code, ready to save as a new picture"
      : "Draw boxes over what the picture itself shows, and save a new picture with them in it";
    b.addEventListener("click", () =>
      void openBlackPicture(m, {
        deliver: (name, bytes, into) => {
          const phone = hasCoarsePointer();
          const sharer = shareButton(name, bytes);
          const shareFallback = sharer?.classList.contains("share-fallback") ?? false;
          if (sharer) {
            into.append(sharer);
            if (shareFallback) into.append(shareFallbackHint());
          }
          if (phone && sharer && !shareFallback) {
            const save = el("button", "btn", "Save it");
            save.dataset.shareSaveFallback = "true";
            save.addEventListener("click", () => this.cleaning.save(name, bytes));
            into.append(save);
          } else if (!phone) {
            this.cleaning.save(name, bytes);
          }
          const open = el("button", "btn", "Open the copy");
          open.addEventListener("click", () => {
            into.closest("dialog")?.close();
            this.cleaning.open(new Blob([bytes as BlobPart]));
          });
          into.append(open);
        },
      }),
    );
    return b;
  }

  /** A message in place of node details, such as why something did not open. */
  showNote(text: string): void {
    this.note = text;
    this.node.replaceChildren(el("p", "problem is-error", text));
  }

  /** Draws the part shown again: its sentence has arrived (explanations.ts). */
  refreshNode(): void {
    // A background explanation must not erase a message the person still needs.
    if (this.note !== null) return;
    if (this.shown) this.showNode(...this.shown);
  }

  showNode(m: FileModel | null, id: number, pinned: boolean): void {
    this.note = null;
    this.shown = [m, id, pinned];
    this.node.replaceChildren();
    if (!m) return;
    if (id < 0) {
      if (document.body.dataset.appView === "files") {
        this.node.append(el("p", "hint", "Select a file to see its details and actions."));
        return;
      }
      this.node.append(
        el(
          "p",
          "hint",
          matchMedia("(hover: none)").matches
            ? "Tap any byte or tree row to see what it is."
            : "Hover any byte or tree row to see what it is. Click to pin it here.",
        ),
      );
      return;
    }

    // In Files the selected entry and its actions come before byte-level details.
    const entry = m.entryOf(id);
    const filesView = document.body.dataset.appView === "files";
    if (filesView && entry >= 0) this.node.append(this.entry(m, entry));

    const crumbs = el("nav", "crumbs");
    this.node.append(el("p", "node-label", "Selected part"));
    const path = m.path(id);
    path.forEach((p, i) => {
      if (i > 0) crumbs.append(el("span", "sep", "›"));
      crumbs.append(el("span", i === path.length - 1 ? "crumb is-current" : "crumb", m.label(p)));
    });
    if (pinned) crumbs.append(el("span", "pin", "pinned"));
    this.node.append(crumbs);

    // First what it is, in plain words; the technical detail follows below.
    const kind = m.kind(id);
    const doc = m.doc(id);
    if (kind >= Kind.Warning) {
      const tone =
        doc?.concern === Concern.Hidden ? "is-hidden" : doc?.concern === Concern.Oddity ? "is-warning" : "is-error";
      this.node.append(el("p", `problem ${tone}`, doc?.text || "These bytes break a rule of the format."));
    } else if (doc?.text) {
      this.node.append(el("p", "explain", doc.text));
    }

    const grid = el("dl", "facts");
    const start = m.start(id);
    const len = m.len(id);
    if (len > 0) {
      fact(grid, "Offset", `${hex(start)} · ${start.toLocaleString()}`, true);
      fact(grid, "Length", len === 1 ? "1 byte" : `${len.toLocaleString("en")} bytes`, true);
    } else {
      fact(grid, "Offset", "— no bytes to point at");
    }
    fact(grid, "Kind", KIND_NAMES[kind]);
    if (m.value(id)) fact(grid, "Value", m.value(id), true);
    if (doc?.cite) {
      const dd = el("dd");
      if (doc.url) {
        const link = el("a", "spec-link", `${doc.cite} ↗`);
        link.href = doc.url;
        link.target = "_blank";
        link.rel = "noopener noreferrer";
        link.title = "The specification that defines this, in a new tab";
        dd.append(link);
      } else {
        dd.textContent = doc.cite;
      }
      grid.append(el("dt", undefined, "Spec"), dd);
    }
    this.node.append(grid);
    const here = !m.missing || m.missing.wanted(start, start + len).length === 0;
    if (len > 0 && len <= MAX_COPIED && here) this.node.append(copyBytes(m.bytes.subarray(start, start + len), m.label(id)));

    if (!filesView && entry >= 0) this.node.append(this.entry(m, entry));
  }

  /** The ZIP entry a node sits in: what it holds, and a way to watch it unpack. */
  private entry(m: FileModel, i: number): HTMLElement {
    const e = m.entry(i);
    const group = el("div", "group entry");
    group.append(el("h2", undefined, "Entry"));
    const grid = el("dl", "facts");
    fact(grid, "Name", m.label(e.node));
    fact(grid, "Data", m.value(e.node));
    if (e.method !== 0 && e.compressed > 0) {
      fact(grid, "Ratio", `${(e.uncompressed / e.compressed).toFixed(2)}×`);
    }
    group.append(grid);

    const traceGroup = el("div", "group archive-trace");
    traceGroup.setAttribute("role", "group");
    traceGroup.setAttribute("aria-label", "Archive Trace");
    traceGroup.append(el("p", "hint", "Loading Archive Trace…"));
    group.append(traceGroup);
    void import("./archive-trace-view").then(({ archiveTraceView }) => {
      if (!this.node.contains(traceGroup)) return;
      traceGroup.replaceWith(archiveTraceView(m, i, {
        nested: this.nested,
        onSelectRange: this.onSelectRange,
        onPlay: this.onPlay,
        onOpen: this.onOpen,
      }));
    }).catch(() => {
      if (this.node.contains(traceGroup)) traceGroup.replaceChildren(el("p", "problem is-error", "Could not load Archive Trace."));
    });
    return group;
  }
}
