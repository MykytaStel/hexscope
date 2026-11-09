// Family-specific summaries assembled only from the parser's existing tree
// and facts. These views add navigation, not a second reading or safety model.
import { el } from "./dom";
import { FACT_LABELS } from "./knowledge";
import { Kind, type FileModel } from "./model";
import { currentLocale, translateText } from "./i18n";
import { svgStaticPreview } from "./svg-preview";

const tr = (text: string) => translateText(text, currentLocale());
const MAX_ROWS = 80;
const MAX_TRACK_NODES = 4096;
const MAX_STRUCTURE_SCAN = 100_000;

function remember(nodes: number[], id: number): void {
  if (nodes.length <= MAX_ROWS) nodes.push(id);
}

function clip(text: string): string {
  const clean = text.replace(/\s+/g, " ").trim();
  return clean.length > 220 ? `${clean.slice(0, 219)}…` : clean;
}

function nodeList(
  m: FileModel,
  title: string,
  nodes: number[],
  select: (node: number) => void,
): HTMLElement | null {
  if (nodes.length === 0) return null;
  const details = el("details", "format-evidence-group");
  details.append(el("summary", undefined, `${tr(title)} · ${nodes.length > MAX_ROWS ? `${MAX_ROWS}+` : nodes.length.toLocaleString(currentLocale())}`));
  const list = el("ul", "format-evidence-list");
  for (const id of nodes.slice(0, MAX_ROWS)) {
    const li = el("li");
    const label = m.label(id);
    const value = clip(m.value(id));
    const button = el("button", "format-evidence-link", value ? `${label}: ${value}` : label);
    button.type = "button";
    button.title = tr("Show this part in the file structure");
    button.addEventListener("click", () => select(id));
    li.append(button);
    list.append(li);
  }
  if (nodes.length > MAX_ROWS) list.append(el("li", "hint", tr("More parts are available in the file structure.")));
  details.append(list);
  return details;
}

function factList(
  m: FileModel,
  title: string,
  kinds: readonly string[],
  select: (node: number) => void,
): HTMLElement | null {
  const facts = m.file.facts.filter((item) => kinds.includes(item.kind));
  if (facts.length === 0) return null;
  const details = el("details", "format-evidence-group");
  details.append(el("summary", undefined, `${tr(title)} · ${facts.length.toLocaleString(currentLocale())}`));
  const list = el("ul", "format-evidence-list");
  for (const item of facts.slice(0, MAX_ROWS)) {
    const li = el("li");
    const button = el("button", "format-evidence-link", `${tr(FACT_LABELS[item.kind] ?? item.kind)}: ${item.text}`);
    button.type = "button";
    button.title = tr("Show this finding in the file structure");
    button.addEventListener("click", () => select(item.node));
    li.append(button);
    list.append(li);
  }
  if (facts.length > MAX_ROWS) list.append(el("li", "hint", tr("More parts are available in the file structure.")));
  details.append(list);
  return details;
}

function section(title: string, intro: string): HTMLElement {
  const root = el("section", "format-inspector");
  root.append(el("h3", undefined, tr(title)), el("p", "hint format-inspector-intro", tr(intro)));
  return root;
}

function scanLimitNote(m: FileModel, root: HTMLElement): void {
  if (m.count > MAX_STRUCTURE_SCAN) {
    root.append(el("p", "hint", tr("This summary checked the first 100,000 structure nodes; later nodes remain in the file structure.")));
  }
}

function descendants(m: FileModel, root: number, limit = MAX_TRACK_NODES): number[] {
  const result: number[] = [];
  const queue: number[] = [];
  for (const id of m.children(root)) {
    if (queue.length >= limit) break;
    queue.push(id);
  }
  for (let i = 0; i < queue.length && result.length < limit; i++) {
    const id = queue[i];
    result.push(id);
    for (const nested of m.children(id)) {
      if (queue.length >= limit) break;
      queue.push(nested);
    }
  }
  return result;
}

function child(m: FileModel, root: number, label: string): number {
  return Array.from(m.children(root)).find((id) => m.label(id) === label) ?? -1;
}

function numberValue(m: FileModel, id: number): number | null {
  if (id < 0) return null;
  const raw = m.value(id).trim();
  if (!raw) return null;
  const value = Number(raw);
  return Number.isFinite(value) ? value : null;
}

function videoTracks(m: FileModel, select: (node: number) => void): HTMLElement {
  const root = section("Tracks and stored timing", "Track details come from the container headers. Hexscope does not decode or play the video or sound.");
  const tracks: number[] = [];
  for (let id = 0; id < Math.min(m.count, MAX_STRUCTURE_SCAN); id++) if (m.label(id) === "trak" && tracks.length <= 64) tracks.push(id);
  const list = el("ol", "format-track-list");
  for (const [index, id] of tracks.slice(0, 64).entries()) {
    const nodes = descendants(m, id);
    const handler = nodes.find((node) => m.label(node) === "handlerType");
    const handlerValue = handler === undefined ? "" : m.value(handler).toLowerCase();
    const kind = handlerValue === "vide" ? "Video track" : handlerValue === "soun" ? "Audio track" : handlerValue ? "Other track" : "Track type not read";
    const mediaHeader = nodes.find((node) => m.label(node) === "mdhd");
    let timing = "";
    if (mediaHeader !== undefined) {
      const duration = numberValue(m, child(m, mediaHeader, "duration"));
      const scale = numberValue(m, child(m, mediaHeader, "timescale"));
      if (duration !== null && scale !== null && scale > 0) {
        const seconds = duration / scale;
        timing = `${tr("Duration")} ${seconds.toLocaleString(currentLocale(), { maximumFractionDigits: 2 })} s`;
      }
    }
    const li = el("li", "format-track");
    const button = el("button", "format-evidence-link", [`${tr("Track")} ${index + 1}`, tr(kind), handlerValue && kind === "Other track" ? handlerValue : "", timing].filter(Boolean).join(" · "));
    button.type = "button";
    button.title = tr("Show this track in the file structure");
    button.addEventListener("click", () => select(id));
    li.append(button);
    list.append(li);
  }
  if (tracks.length === 0) {
    root.append(el("p", "hint", tr("No track boxes were found in the readable structure.")));
  } else {
    root.append(list);
    if (tracks.length > 64) root.append(el("p", "hint", tr("More tracks are available in the file structure.")));
  }
  scanLimitNote(m, root);
  return root;
}

function audioStructure(m: FileModel, select: (node: number) => void): HTMLElement {
  const root = section("Audio structure", "Tags, cover art and container blocks are listed separately from the compressed sound. Hexscope does not play audio.");
  const artwork: number[] = [];
  const metadata: number[] = [];
  const streams: number[] = [];
  const incomplete: number[] = [];
  for (let id = 1; id < Math.min(m.count, MAX_STRUCTURE_SCAN); id++) {
    const label = m.label(id).toLowerCase();
    if (m.kind(id) === Kind.Warning || m.kind(id) === Kind.Error || /unsupported|not inspected|exceeds the inspection limit|truncated|could not be read|multiplexed|interleaved/.test(label)) remember(incomplete, id);
    else if (/picture|cover art|attached picture/.test(label)) remember(artwork, id);
    else if (/id3|apev2|vorbis comment|metadata block|info text|broadcast wave|tag block/.test(label)) remember(metadata, id);
    else if (/streaminfo|logical stream|ogg page|audio data|mpeg audio|audio frame|riff|wave chunk/.test(label)) remember(streams, id);
  }
  const groups = [
    nodeList(m, "Cover art and pictures", artwork, select),
    nodeList(m, "Tags and metadata blocks", metadata, select),
    nodeList(m, "Audio stream and container", streams, select),
    nodeList(m, "Unsupported or incomplete parts", incomplete, select),
  ].filter((item): item is HTMLElement => item !== null);
  if (m.file.metadataComplete === false || m.file.coverageStatus !== "complete") {
    root.append(el("p", "problem is-warning", tr("Some audio structure was not fully read; see the coverage and evidence details.")));
  }
  root.append(...groups);
  if (groups.length === 0) root.append(el("p", "hint", tr("No additional audio blocks were listed by the parser.")));
  scanLimitNote(m, root);
  return root;
}

function svgContents(m: FileModel, select: (node: number) => void): HTMLElement {
  const root = section("SVG contents", "This map does not execute scripts or fetch linked files. Embedded pictures keep their own metadata.");
  root.append(svgStaticPreview(m.bytes));
  const scripts: number[] = [];
  const images: number[] = [];
  for (let id = 1; id < Math.min(m.count, MAX_STRUCTURE_SCAN); id++) {
    const label = m.label(id).toLowerCase();
    if (label === "script" || label === "event handler") remember(scripts, id);
    if (label === "image" || label === "embedded picture") remember(images, id);
  }
  const groups = [
    nodeList(m, "Scripts and event handlers", scripts, select),
    factList(m, "External references", ["links"], select),
    nodeList(m, "Image elements", images, select),
    factList(m, "Embedded-picture findings", ["embedded", "photoplace"], select),
  ].filter((item): item is HTMLElement => item !== null);
  root.append(...groups);
  if (groups.length === 0) root.append(el("p", "hint", tr("No script, link or image entries were listed by the parser.")));
  scanLimitNote(m, root);
  return root;
}

function emailEvidence(m: FileModel, select: (node: number) => void): HTMLElement {
  const root = section("Email evidence", "Header results and link targets are read from the saved message. Authentication results are claims in the file, not verified by Hexscope; links are not visited.");
  const groups = [
    factList(m, "Sender and transport clues", ["sentfrom", "computer", "mailer", "timezone", "replyto"], select),
    factList(m, "Authentication claims", ["authclaim"], select),
    factList(m, "Links and attachments", ["weblinks", "linkmismatch", "linkidn", "attachments", "riskyfile"], select),
  ].filter((item): item is HTMLElement => item !== null);
  root.append(...groups);
  if (groups.length === 0) root.append(el("p", "hint", tr("No sender, route, link or attachment findings were listed by the parser.")));
  return root;
}

function wasmInterface(m: FileModel, select: (node: number) => void): HTMLElement {
  const root = section("Module interface", "Imports name services a module asks from its host; exports name services it offers. Hexscope does not run WebAssembly code.");
  const imports: number[] = [];
  const exports: number[] = [];
  const dependencies: number[] = [];
  for (let id = 1; id < Math.min(m.count, MAX_STRUCTURE_SCAN); id++) {
    const label = m.label(id);
    if (label.startsWith("import ")) remember(imports, id);
    else if (label.startsWith("export ")) remember(exports, id);
    else if (/sourceMappingURL|external_debug_info|dependency|linked module/i.test(label)) remember(dependencies, id);
  }
  const groups = [
    nodeList(m, "Imports required from the host", imports, select),
    nodeList(m, "Exports offered to the host", exports, select),
    nodeList(m, "External debug references", dependencies, select),
  ].filter((item): item is HTMLElement => item !== null);
  root.append(...groups);
  if (groups.length === 0) root.append(el("p", "hint", tr("No imports, exports or external debug references were listed by the parser.")));
  scanLimitNote(m, root);
  return root;
}

function unknownFile(): HTMLElement {
  const root = section("Unrecognized file structure", "Hexscope has no structural reader for this input. Use Search for embedded files to see the exact checked byte range and search limit. Text and signature clues are not a complete inspection or a safety verdict.");
  return root;
}

/** A context panel backed exclusively by existing parser facts and nodes. */
export function formatInspector(m: FileModel, select: (node: number) => void): HTMLElement | null {
  switch (m.file.format) {
    case "video": return videoTracks(m, select);
    case "mp3":
    case "flac":
    case "ogg":
    case "wav": return audioStructure(m, select);
    case "svg": return svgContents(m, select);
    case "eml":
    case "msg": return emailEvidence(m, select);
    case "wasm": return wasmInterface(m, select);
    case "unknown": return unknownFile();
    default: return null;
  }
}
