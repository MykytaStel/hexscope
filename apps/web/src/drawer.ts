import { areasOf, find, findPreset, pageFigure, PRESETS, searchable, textIn, forgetPictures, toShown, type Match, type Searchable, type Shown } from "./redactor";
import { canBlackOut, openBlackPicture } from "./blackpicture";
import { findEmbedded, type Embedded } from "./embedded";
import { blackoutFigure } from "./blackout";
import { advice } from "./advice";
import { checkThumbnail, decodePicture, drawn } from "./thumbnail";
import { openReport } from "./report";
import { saveStructure } from "./export";
import { categories, share } from "./share";
import { Concern, FileModel, Kind, Role, type PageArea } from "./model";
import { verdict } from "./verdict";
import { announce, done } from "./announce";
import { recentFiles } from "./recent";
import { headline, noun } from "./headline";
import { emailPath } from "./emailpath";
import { makeupGroup } from "./makeup";
import { el, fact, formatBytes, hex } from "./dom";
import { COLOR_TYPES, FACT_LABELS, KEPT, KIND_NAMES, REPAIRABLE, STRONG, degrees, mapLink, openReason, playReason } from "./knowledge";
import { MAX_COPIED, copyBytes } from "./copybytes";
import { copyPictureButton, copyVerification, isAudio, shareButton, type CleanActions } from "./cleancard";
import { createCleaner } from "./cleaner";

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

const METADATA_FACT_GROUPS: Record<Exclude<MetadataGroupKey, "general" | "additional">, Set<string>> = {
  camera: new Set(["camera", "lens", "serial", "owner", "shutter", "uptime"]),
  location: new Set(["place", "photoplace"]),
  dates: new Set(["taken", "created", "modified", "updates", "timezone"]),
  creator: new Set(["software", "author", "editor", "producer", "application", "company", "template", "mailer", "sentfrom", "computer"]),
};

function metadataGroupFor(kind: string): MetadataGroupKey {
  for (const key of ["camera", "location", "dates", "creator"] as const) {
    if (METADATA_FACT_GROUPS[key].has(kind)) return key;
  }
  return "additional";
}

/** Details for one node on the left, facts about the whole file on the right. */
export class Drawer {
  private readonly node: HTMLElement;
  private readonly file: HTMLElement;
  /** How many archives the document on screen sits inside. */
  nested = 0;

  constructor(
    host: HTMLElement,
    private readonly onSelect: (id: number) => void,
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

  private picturesOf(m: FileModel, page: number): Promise<Shown[]> {
    if (m !== this.pictured) {
      forgetPictures();
      this.pictured = m;
      this.pictureCache = new Map();
    }
    let p = this.pictureCache.get(page);
    if (!p) {
      p = this.cleaning.pictures(page).then(toShown).catch(() => []);
      this.pictureCache.set(page, p);
    }
    return p;
  }

  /** Moves the keyboard to what was found, without scrolling the page away. */
  focusVerdict(): void {
    this.file.querySelector<HTMLElement>(".verdict-title")?.focus({ preventScroll: true });
  }

  showFile(m: FileModel | null): void {
    this.file.replaceChildren();
    if (!m) return;
    const f = m.file;

    // What someone came to know first: the verdict, then what the file gives
    // away, then the picture; how it is made, after.
    const answer = this.verdict(m);
    this.file.append(answer);
    // A photo, a video or a PDF always gets the card, if only to say it gives
    // nothing away; an archive only when it is a document with properties.
    if (!["zip", "cfb", "unknown"].includes(f.format) || f.facts.length > 0) {
      this.file.append(this.reveals(m));
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
    if (f.format === "pdf" && !f.facts.some((x) => x.kind === "encryption")) this.file.append(this.redactor(m));
    // Technical detail is available in one place, after the useful answer.
    const more = el("details", "more-details");
    more.append(el("summary", undefined, "For the curious"));
    const later = (e: HTMLElement) => more.append(e);
    more.append(makeupGroup(m, (id) => this.onSelect(id), (id) => this.onHover(id)));
    const picture = this.picture(m);
    if (picture) later(picture);

    const fileGroup = el("div", "group");
    fileGroup.append(el("h2", undefined, "File"));
    const grid = el("dl", "facts");
    fact(grid, "Size", `${formatBytes(m.bytes.length)} · ${m.bytes.length.toLocaleString()} bytes`);
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
    report.addEventListener("click", () => openReport(m));
    const save = el("button", "link report-link", "Save the structure as JSON");
    save.title = "Every part, its offset, length, value and explanation, nested as in the tree";
    save.addEventListener("click", () => saveStructure(m));
    const inside = el("button", "link report-link", "Find files inside");
    inside.title = "Looks through every byte for the start of another file: pictures, archives, documents";
    inside.addEventListener("click", () => {
      const parts = m.missing ? m.missing.read(m.bytes.length) : [[0, m.bytes.length]];
      const found = parts.flatMap(([a, b]) => findEmbedded(m.bytes, a, b, m.file.format === "zip" ? ["zip"] : []));
      const nowhere = m.missing ? "No other file starts anywhere outside the picture and sound." : "No other file starts anywhere in it.";
      const list = found.length > 0 ? this.insideList(m, found, "Files inside") : el("p", "hint", nowhere);
      inside.replaceWith(list);
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
    this.file.append(more);

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
    const head = headline(m);
    group.append(el("p", "verdict-label", "What hexscope found"));
    const title = el("h2", `verdict-title is-${head.tone}`, head.text);
    title.tabIndex = -1;
    group.append(title);
    // A message that may be forged: what to do comes before why.
    if (noun(m) === "email" && head.tone === "danger") {
      group.append(el("p", "verdict-do", "Do not reply, open its files or follow its links. If it matters, ask the sender another way — a number or address you already have."));
    }
    const list = el("ul", "verdict-lines");
    for (const line of verdict(m)) {
      const li = el("li", `verdict-line is-${line.kind}`);
      li.append(el("span", "verdict-text", line.text));
      // A file not read has no part to show.
      if (line.node >= 0 && line.kind !== "unknown") {
        const show = el("button", "verdict-show", "Show me");
        show.addEventListener("click", () => this.showLine(line.node));
        li.append(show);
      }
      list.append(li);
    }
    group.append(list);
    // What the structure leaves over — data after the end, a gap — may be a
    // whole file of its own: say which, and open it.
    const hidden = m
      .slices()
      .filter((sl) => sl.role === Role.Hidden)
      .flatMap((sl) => findEmbedded(m.bytes, sl.start, sl.start + sl.len));
    if (hidden.length > 0) group.append(this.insideList(m, hidden, "Hidden in it"));
    if (m.file.format !== "unknown") group.append(el("p", "hint", "Found by reading the file's structure. It is not a virus scan."));
    if (REPAIRABLE.includes(m.file.format) && verdict(m).some((l) => l.kind === "damage")) group.append(this.repairer());
    return group;
  }

  /**
   * Where a verdict line points: in the summary, the fact it names, brought
   * into view; otherwise, or for a part with no fact, its bytes.
   */
  private showLine(node: number): void {
    const row = this.file.querySelector<HTMLElement>(`.reveal-list dd[data-node="${node}"]`);
    const label = row?.previousElementSibling as HTMLElement | null;
    if (!row || !label || document.body.dataset.view === "bytes") {
      this.onSelect(node);
      return;
    }
    label.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "center" });
    for (const e of [label, row]) {
      e.classList.remove("is-flash");
      void e.offsetWidth;
      e.classList.add("is-flash");
    }
  }

  /** Files found inside this one, each to open or save. */
  private insideList(m: FileModel, found: Embedded[], title: string): HTMLElement {
    const box = el("div", "inside");
    box.append(el("p", "inside-title", `${title}: ${found.length === 1 ? "1 file" : `${found.length} files`}`));
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
      li.append(el("span", "inside-what", `${e.what.charAt(0).toUpperCase()}${e.what.slice(1)}`), " at ", at, el("span", "clean-size", formatBytes(e.end - e.start)), open, save);
      ul.append(li);
    }
    box.append(ul);
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
      png: "What this image reveals",
      video: "What this video reveals",
      wasm: "What this module reveals",
      eml: "What this email reveals",
      msg: "What this email reveals",
    };
    const title = el("h2", undefined, isAudio(m) ? "What this recording reveals" : (heading[f.format] ?? "What this document reveals"));
    group.append(title);
    // An email's way from its sender to you, drawn, with what is wrong on it.
    if (f.format === "eml") {
      const path = emailPath(m, (id) => this.onSelect(id));
      if (path) group.append(path);
    }
    // The picture it is about, and where it was taken, drawn: what the
    // numbers below mean, before reading them.
    const pictured = ["jpeg", "png", "heif", "webp", "gif"].includes(f.format);
    if (pictured || f.location) {
      const hero = el("div", f.location ? "reveal-hero has-map" : "reveal-hero");
      group.append(hero);
      if (pictured) {
        const frame = el("div", "reveal-picture-frame");
        hero.append(frame);
        void decodePicture(m).then((b) => {
          if (!b) return frame.remove();
          const c = drawn(b, f.format === "jpeg" ? f.orientation : 1, 480);
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
        f.format === "pdf"
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
    fact(generalList, "File", f.format.toUpperCase());
    fact(generalList, "Size", `${formatBytes(m.bytes.length)} · ${m.bytes.length.toLocaleString()} bytes`);
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
      const dd = row("Location", where, node, true, "location");
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
      // A Word document's comments go with the clean copy; a workbook's or a deck's stay.
      const word = f.format === "zip" && f.labels.includes("word/document.xml");
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
    group.append(metadataGroups);
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
    } else if (f.format === "eml" || f.format === "msg") {
      // A message is read, not sent on as it is: what it says is what the servers wrote.
      group.append(el("p", "hint", "The servers it passed through wrote these lines; an email is not cleaned, but its attached files can be — open one, then save a clean copy of it."));
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
          const phone = matchMedia("(hover: none) and (pointer: coarse)").matches;
          const sharer = shareButton(name, bytes);
          if (sharer) into.append(sharer);
          if (phone && sharer) {
            const save = el("button", "btn", "Save it");
            save.addEventListener("click", () => this.cleaning.save(name, bytes));
            into.append(save);
          } else {
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

  /** Black out text yourself: search or pick a kind, tick, draw boxes, see the pages, save. */
  private redactor(m: FileModel): HTMLElement {
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
      pages ??= searchable(await this.cleaning.pages());
      incomplete.hidden = pages.every((page) => page.complete);
      return pages;
    };
    const picturesOf = (n: number) => this.picturesOf(m, n);
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
      const r = await this.cleaning.redact(areasOf(marks), selections);
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
      const sharer = shareButton(r.name, r.copy);
      if (sharer) result.append(sharer);
      if (!r.saved) {
        const again = el("button", "btn", "Save it");
        again.addEventListener("click", () => this.cleaning.save(r.name, r.copy));
        result.append(again);
      }
      const open = el("button", "btn", "Open the copy");
      open.title = "Check it yourself: search it for what you blacked out";
      open.addEventListener("click", () => this.cleaning.open(r.copy));
      result.append(open);
      const verification = copyVerification(r);
      if (verification) result.append(verification);
    });
    return group;
  }

  /** A message in place of node details, such as why something did not open. */
  showNote(text: string): void {
    this.node.replaceChildren(el("p", "problem is-error", text));
  }

  showNode(m: FileModel | null, id: number, pinned: boolean): void {
    this.node.replaceChildren();
    if (!m) return;
    if (id < 0) {
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
      this.node.append(el("p", `problem ${tone}`, doc?.text ?? "These bytes break a rule of the format."));
    } else if (doc) {
      this.node.append(el("p", "explain", doc.text));
    }

    const grid = el("dl", "facts");
    const start = m.start(id);
    const len = m.len(id);
    if (len > 0) {
      fact(grid, "Offset", `${hex(start)} · ${start.toLocaleString()}`, true);
      fact(grid, "Length", len === 1 ? "1 byte" : `${len.toLocaleString()} bytes`, true);
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

    const entry = m.entryOf(id);
    if (entry >= 0) this.node.append(this.entry(m, entry));
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

    const actions = el("div", "entry-actions");
    const button = (text: string, reason: string | null, title: string, act: () => void) => {
      const b = el("button", "btn", text);
      b.disabled = reason !== null;
      b.title = reason ?? title;
      b.addEventListener("click", act);
      actions.append(b);
    };
    const playWhy = playReason(e);
    const openWhy = openReason(e, this.nested);
    button("Watch it decompress", playWhy, "Step through this entry's DEFLATE data (P)", () => this.onPlay(i));
    button("Open", openWhy, "Open this entry as a file of its own", () => this.onOpen(i));
    group.append(actions);
    for (const why of new Set([playWhy, openWhy])) if (why) group.append(el("p", "hint", why));
    return group;
  }
}
