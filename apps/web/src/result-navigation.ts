import type { FileModel } from "./model";
import { currentLocale, translateText } from "./i18n";
import { el, formatBytes } from "./dom";
import { kindOf } from "./files";

export type CoverageState = "complete" | "partial" | "unsupported";

/** Details beside the filename when the byte view is selected. */
export function fileInfoChips(m: FileModel): string[] {
  const chips: string[] = [];
  const f = m.file;
  if (f.format === "png" && f.ihdr) {
    const [w, h, depth, color] = f.ihdr;
    const names: Record<number, string> = { 0: "Grey", 2: "RGB", 3: "Palette", 4: "Grey+α", 6: "RGBA" };
    chips.push("PNG", `${w}×${h}`, `${names[color] ?? `type ${color}`} ${depth}-bit`);
  } else if (f.format === "png") {
    chips.push("PNG");
  } else if (f.format === "jpeg") {
    chips.push("JPEG");
    if (f.dimensions) chips.push(`${f.dimensions[0]}×${f.dimensions[1]}`);
    if (f.facts.length > 0 || f.location) chips.push("EXIF");
  } else if (f.format === "webp" || f.format === "gif") {
    chips.push(...m.value(0).split(" · "));
    if (f.facts.length > 0 || f.location) chips.push("Metadata");
  } else if (f.format === "tiff") {
    chips.push("TIFF");
    if (f.dimensions) chips.push(`${f.dimensions[0]}×${f.dimensions[1]}`);
    if (f.facts.length > 0 || f.location) chips.push("EXIF");
  } else if (f.format === "heif") {
    chips.push(m.value(0).split(" · ")[0]);
    if (f.dimensions) chips.push(`${f.dimensions[0]}×${f.dimensions[1]}`);
    if (f.facts.length > 0 || f.location) chips.push("EXIF");
  } else if (f.format === "video") {
    chips.push(...m.value(0).split(" · "));
    if (f.location) chips.push("Location");
  } else if (f.format === "pdf") {
    chips.push(...m.value(0).split(" · "));
  } else if (f.format === "zip") {
    const kind = kindOf(m);
    chips.push(...(kind === "ZIP" ? ["ZIP", m.value(0)] : [kind]));
  } else if (f.format === "rtf") {
    chips.push("Rich Text");
  } else if (f.format === "svg") {
    chips.push("SVG");
  } else if (f.format === "office97" || f.format === "cfb") {
    chips.push(m.value(0).charAt(0).toUpperCase() + m.value(0).slice(1));
  } else if (f.format === "eml" || f.format === "msg") {
    chips.push(f.format === "msg" ? "Outlook message" : "Email");
    if (f.attachments.length > 0) chips.push(f.attachments.length === 1 ? "1 attachment" : `${f.attachments.length} attachments`);
  } else if (f.format === "wasm") {
    chips.push(...m.value(0).split(" · "));
  } else {
    chips.push("Unrecognised format");
  }
  chips.push(formatBytes(m.bytes.length), `parsed in ${f.parseMs.toFixed(1)} ms`);
  return chips;
}

export function fillFileInfoChips(host: HTMLElement, m: FileModel): void {
  host.replaceChildren(...fileInfoChips(m).flatMap((text, index) => {
    const chip = document.createElement("span");
    chip.textContent = text;
    return index ? [document.createTextNode("  ·  "), chip] : [chip];
  }));
}

/** Keeps the result's category index adjacent to the categories it moves through. */
export function refreshVerdictIndex(findings: HTMLElement): void {
  const previous = findings.querySelector<HTMLElement>(".verdict-index");
  const activeKey = previous?.contains(document.activeElement)
    ? document.activeElement?.closest<HTMLElement>(".verdict-index-link")?.dataset.verdictGroup
    : undefined;
  const sections = (["action", "personal", "notes", "health"] as const)
    .map((key) => findings.querySelector<HTMLElement>(`.verdict-group[data-verdict-group="${key}"]`))
    .filter((section): section is HTMLElement => !!section);
  if (sections.length < 2) {
    previous?.remove();
    return;
  }

  const nav = el("nav", "verdict-index");
  nav.setAttribute("aria-label", translateText("Finding groups", currentLocale()));
  for (const section of sections) {
    const title = section.querySelector<HTMLElement>(".verdict-group-title");
    if (!title) continue;
    const count = section.querySelectorAll(".verdict-line").length;
    const button = el("button", "verdict-index-link");
    button.type = "button";
    button.dataset.verdictGroup = section.dataset.verdictGroup;
    button.setAttribute("aria-controls", section.id);
    button.title = translateText("Jump to this group", currentLocale());
    button.append(
      el("span", "verdict-index-name", title.textContent ?? ""),
      el("span", "verdict-index-count", count.toLocaleString(currentLocale())),
    );
    button.addEventListener("click", () => {
      section.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
      title.focus({ preventScroll: true });
    });
    nav.append(button);
  }
  if (previous) previous.replaceWith(nav);
  else findings.prepend(nav);
  if (activeKey) nav.querySelector<HTMLButtonElement>(`.verdict-index-link[data-verdict-group="${activeKey}"]`)?.focus({ preventScroll: true });
}

/** Small header control for the read-coverage explanation on the result page. */
export function coverageStatusButton(
  m: FileModel,
  appNavigation: HTMLElement,
  drawer: HTMLElement,
): HTMLButtonElement {
  const f = m.file;
  const hasUncheckedParts = f.coverageStatus !== "complete" || f.missing.length > 0 || f.metadataComplete === false;
  const state: CoverageState = f.coverageStatus === "unsupported" ? "unsupported" : hasUncheckedParts ? "partial" : "complete";
  const labels = state === "complete"
    ? ["Fully read", "Read"]
    : state === "partial"
      ? ["Partly checked", "Partial read"]
      : ["No format parser", "No parser"];
  const button = document.createElement("button");
  button.className = "file-coverage";
  button.type = "button";
  button.dataset.status = state;
  button.setAttribute("aria-controls", "drawer");
  button.setAttribute("aria-describedby", "analysis-coverage-description");
  button.setAttribute(
    "aria-label",
    `${translateText(`Analysis status: ${labels[0]}`, currentLocale())} · ${translateText(labels[1], currentLocale())}`,
  );
  button.title = translateText("Show what was and was not checked", currentLocale());
  const dot = document.createElement("span");
  dot.className = "file-coverage-dot";
  dot.setAttribute("aria-hidden", "true");
  const full = document.createElement("span");
  full.className = "file-coverage-label file-coverage-full";
  full.textContent = translateText(labels[0], currentLocale());
  const compact = document.createElement("span");
  compact.className = "file-coverage-label file-coverage-compact";
  compact.textContent = translateText(labels[1], currentLocale());
  button.append(dot, full, compact);
  button.addEventListener("click", () => {
    const overview = appNavigation.querySelector<HTMLButtonElement>('[data-app-view="overview"]');
    if (overview?.getAttribute("aria-current") !== "page") overview?.click();
    const target = state !== "complete"
      ? drawer.querySelector<HTMLElement>(".evidence-unchecked-link") ?? drawer.querySelector<HTMLElement>(".verdict-coverage")
      : drawer.querySelector<HTMLElement>(".verdict-coverage");
    if (!target) return;
    target.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "center" });
    target.focus({ preventScroll: true });
  });
  return button;
}
