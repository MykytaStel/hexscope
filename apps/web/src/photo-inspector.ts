// The visible image's stored properties, kept apart from its metadata and
// privacy findings. Every optional detail is derived from the parse tree.
import type { FileModel } from "./model";
import { el } from "./dom";
import { COLOR_TYPES } from "./knowledge";
import { currentLocale, translateText } from "./i18n";

const PHOTO_FORMATS = new Set(["jpeg", "png", "heif", "webp", "gif", "tiff"]);
const tr = (text: string) => translateText(text, currentLocale());

function sourceNode(m: FileModel, pattern: RegExp): number {
  for (let id = 0; id < m.count; id++) {
    if (pattern.test(m.label(id)) || pattern.test(m.value(id))) return id;
  }
  return -1;
}

function appendProperty(
  list: HTMLDListElement,
  label: string,
  value: string,
  node: number,
  select: (node: number) => void,
): void {
  const term = el("dt", undefined, tr(label));
  const detail = el("dd", "photo-property-value");
  if (node >= 0) {
    const jump = el("button", "photo-property-link", value);
    jump.type = "button";
    jump.title = tr("Show the source part in Bytes");
    jump.addEventListener("click", () => select(node));
    detail.append(jump);
  } else detail.textContent = value;
  list.append(term, detail);
}

/** Read-only view of visible image properties; null for non-picture formats. */
export function photoInspector(m: FileModel, select: (node: number) => void): HTMLElement | null {
  const f = m.file;
  if (!PHOTO_FORMATS.has(f.format)) return null;

  const section = el("section", "photo-inspector");
  section.append(el("h3", undefined, "Image properties"));
  section.append(el("p", "hint photo-inspector-hint", "Visible image properties are separate from metadata and privacy findings below."));
  const list = el("dl", "photo-property-list");
  let sequenceNote: HTMLElement | null = null;

  const dimensions = f.ihdr ? [f.ihdr[0], f.ihdr[1]] : f.dimensions;
  if (dimensions) {
    const [storedWidth, storedHeight] = dimensions;
    const swapsAxes = [5, 6, 7, 8].includes(f.orientation);
    const shown = swapsAxes ? `${storedHeight} × ${storedWidth}` : `${storedWidth} × ${storedHeight}`;
    const label = swapsAxes ? "Displayed pixel dimensions" : "Pixel dimensions";
    appendProperty(list, label, shown, sourceNode(m, /^(?:size|dimensions|canvas|logical screen descriptor)$/i), select);
  }

  if (f.ihdr) {
    const [, , depth, color] = f.ihdr;
    appendProperty(
      list,
      "Pixel encoding",
      `${COLOR_TYPES[color] ?? `colour type ${color}`} · ${depth}-bit`,
      sourceNode(m, /^IHDR$/i),
      select,
    );
  }

  const orientationNode = sourceNode(m, /orientation/i);
  if (orientationNode >= 0 && f.orientation !== 1) {
    appendProperty(list, "Stored EXIF orientation", String(f.orientation), orientationNode, select);
  }

  const profileNode = sourceNode(m, /(?:ICC|colour profile|color profile|nclx)/i);
  if (profileNode >= 0) {
    appendProperty(list, "Colour profile", "An embedded profile or colour description is present", profileNode, select);
  }

  const explicitFrames: number[] = [];
  const webpFrames: number[] = [];
  let animationMarker = false;
  let associatedImageNode = -1;
  for (let id = 0; id < m.count; id++) {
    const label = m.label(id);
    const value = m.value(id);
    if (/^frame(?:\s+\d+)?$/i.test(label)) explicitFrames.push(id);
    if (/^ANMF$/i.test(label)) webpFrames.push(id);
    if (/^(?:ANIM|acTL)$/i.test(label) || /image sequence/i.test(value)) animationMarker = true;
    if (associatedImageNode < 0 && /(?:auxiliary image|gain map|depth map|picture listed in the MPF index|embedded thumbnail)/i.test(`${label} ${value}`)) {
      associatedImageNode = id;
    }
  }
  const frameNodes = explicitFrames.length > 0 ? explicitFrames : webpFrames;
  if (frameNodes.length > 1 || animationMarker) {
    appendProperty(
      list,
      frameNodes.length > 0 ? "Frames" : "Animation or image sequence",
      frameNodes.length > 0 ? frameNodes.length.toLocaleString(currentLocale()) : "Present",
      frameNodes[0] ?? sourceNode(m, /(?:ANIM|acTL|sequence)/i),
      select,
    );
    sequenceNote = el("p", "hint", "Frame data is listed in the structure. This preview is a browser-rendered still and does not play or decode the sequence.");
  }

  if (associatedImageNode >= 0) {
    appendProperty(list, "Associated image data", "An auxiliary image, thumbnail or multi-picture item is present", associatedImageNode, select);
  }

  if (list.children.length === 0) {
    section.remove();
    return null;
  }
  section.append(list);
  if (sequenceNote) section.append(sequenceNote);
  return section;
}
