import type { FileModel } from "./model";
import { el, formatBytes } from "./dom";
import { currentLocale, translateText } from "./i18n";

const REF_TYPES = new Set(["dimg", "thmb", "auxl", "cdsc", "prem", "auxC", "altr", "base"]);
const MAX_TIMING_RUNS = 120;
const MAX_GRAPH_ROWS = 200;
const MAX_SEQUENCE_TRACKS = 8;
const tr = (text: string) => translateText(text, currentLocale());

export interface HeifItem {
  id: number;
  type: string;
  name: string;
  node: number;
  constructionMethod: number | null;
  primary: boolean;
  role: string;
}

export interface HeifReference {
  type: string;
  from: number;
  to: number[];
}

/** A bounded, value-free index of the HEIF item/reference tree for the map view. */
export function heifGraph(m: FileModel): {
  items: HeifItem[];
  itemCount: number;
  refs: HeifReference[];
  referenceCount: number;
  sequence: boolean;
} | null {
  if (m.file.format !== "heif") return null;
  const children = new Map<number, number[]>();
  for (let id = 1; id < m.count; id++) {
    const parent = m.file.parents[id];
    const list = children.get(parent) ?? [];
    list.push(id);
    children.set(parent, list);
  }
  const direct = (id: number) => children.get(id) ?? [];
  const items = new Map<number, HeifItem>();
  const methods = new Map<number, number>();
  let primaryId: number | null = null;
  let sequence = false;

  for (let id = 1; id < m.count; id++) {
    const label = m.label(id);
    const value = m.value(id);
    if (label === "majorBrand" && value === "avis") sequence = true;
    if (label === "compatibleBrands" && value.split(", ").includes("avis")) sequence = true;
    if (label === "itemId" && m.label(m.file.parents[id]) === "pitm") {
      const n = Number(value);
      if (Number.isSafeInteger(n)) primaryId = n;
    }
    if (label === "infe") {
      let itemId: number | null = null;
      let type = "?";
      let name = "";
      for (const child of direct(id)) {
        const childLabel = m.label(child);
        const childValue = m.value(child);
        if (childLabel === "itemId") itemId = Number(childValue);
        else if (childLabel === "itemType") type = childValue;
        else if (childLabel === "itemName" || childLabel === "contentType") name = childValue;
      }
      if (itemId !== null && Number.isSafeInteger(itemId)) {
        items.set(itemId, { id: itemId, type, name, node: -1, constructionMethod: null, primary: false, role: "item" });
      }
    }
    const placed = /^item (\d+) · ([^ (]+)/.exec(label);
    if (placed) {
      const itemId = Number(placed[1]);
      const old = items.get(itemId);
      if (old) old.node = id;
      else items.set(itemId, { id: itemId, type: placed[2], name: "", node: id, constructionMethod: null, primary: false, role: "item" });
    }
    const location = /^item (\d+) location$/.exec(label);
    if (location) {
      const itemId = Number(location[1]);
      for (const child of direct(id)) {
        if (m.label(child) === "constructionMethod") methods.set(itemId, Number(m.value(child)));
        if (m.label(child).startsWith("construction method ")) {
          const method = /^construction method (\d+)/.exec(m.label(child));
          if (method) methods.set(itemId, Number(method[1]));
        }
      }
    }
  }

  const refs: HeifReference[] = [];
  for (let id = 1; id < m.count; id++) {
    const type = m.label(id);
    if (!REF_TYPES.has(type)) continue;
    const match = /^item (\d+) →\s*(.*)$/.exec(m.value(id));
    if (!match) continue;
    const to = match[2].split(",").map((part) => Number(part.trim())).filter(Number.isSafeInteger);
    refs.push({ type, from: Number(match[1]), to });
  }

  const primary = primaryId;
  const derived = new Set(refs.filter((ref) => ref.type === "dimg").map((ref) => ref.from));
  const tiles = new Set(refs.filter((ref) => ref.type === "dimg").flatMap((ref) => ref.to));
  const thumbnails = new Set(refs.filter((ref) => ref.type === "thmb").map((ref) => ref.from));
  const auxiliary = new Set(refs.filter((ref) => ref.type === "auxl").map((ref) => ref.from));
  for (const item of items.values()) {
    item.primary = item.id === primary;
    item.role = item.primary ? "primary image" : derived.has(item.id) ? "derived image" : tiles.has(item.id) ? "tile" : thumbnails.has(item.id) ? "thumbnail" : auxiliary.has(item.id) ? "auxiliary image" : /^(Exif|mime)$/.test(item.type) ? "metadata" : "item";
    item.constructionMethod = methods.get(item.id) ?? null;
  }
  const allItems = [...items.values()].sort((a, b) => a.id - b.id);
  return {
    items: allItems.slice(0, MAX_GRAPH_ROWS),
    itemCount: allItems.length,
    refs: refs.slice(0, MAX_GRAPH_ROWS),
    referenceCount: refs.length,
    sequence,
  };
}

function descendants(m: FileModel, node: number): number[] {
  const out: number[] = [];
  const stack = [...m.children(node)].reverse();
  while (stack.length) {
    const id = stack.pop()!;
    out.push(id);
    const kids = m.children(id);
    for (let i = kids.length - 1; i >= 0; i--) stack.push(kids[i]);
  }
  return out;
}

export interface SequenceTiming {
  handler: string;
  timescale: number;
  sampleCount: number;
  seconds: number;
  runs: { samples: number; delta: number }[];
}

/** Extract bounded sample timing without requiring a browser DOM. */
export function sequenceTimings(m: FileModel): { tracks: SequenceTiming[]; trackCount: number } {
  const tracks: SequenceTiming[] = [];
  let trackCount = 0;
  for (let id = 0; id < m.count; id++) {
    if (m.label(id) !== "trak" || !m.value(id).startsWith("AV1 image sequence")) continue;
    trackCount++;
    if (tracks.length >= MAX_SEQUENCE_TRACKS) continue;

    const ids = descendants(m, id);
    const handler = ids.map((child) => m.label(child) === "handlerType" ? m.value(child) : "").find(Boolean) ?? "unknown";
    const rawTimescale = Number(ids.map((child) => m.label(child) === "timescale" ? m.value(child) : "").find(Boolean));
    const timescale = Number.isSafeInteger(rawTimescale) && rawTimescale > 0 ? rawTimescale : 0;
    const runs: SequenceTiming["runs"] = [];
    for (const timing of ids.filter((child) => m.label(child) === "stts")) {
      const fields = Array.from(m.children(timing));
      let samples: number | null = null;
      for (const field of fields) {
        if (m.label(field) === "sampleCount") samples = Number(m.value(field));
        else if (m.label(field) === "sampleDelta" && samples !== null) {
          const delta = Number(m.value(field));
          const weight = samples * delta;
          if (Number.isSafeInteger(samples) && Number.isSafeInteger(delta) && samples >= 0 && delta >= 0 && Number.isSafeInteger(weight)) {
            runs.push({ samples, delta });
          }
          samples = null;
        }
      }
    }
    const sampleCount = runs.reduce((n, run) => Number.isSafeInteger(n + run.samples) ? n + run.samples : n, 0);
    const ticks = runs.reduce((n, run) => {
      const next = n + run.samples * run.delta;
      return Number.isSafeInteger(next) ? next : n;
    }, 0);
    tracks.push({ handler, timescale, sampleCount, seconds: timescale > 0 ? ticks / timescale : 0, runs });
  }
  return { tracks, trackCount };
}

function sequenceView(m: FileModel, sequence: boolean): HTMLElement | null {
  if (!sequence) return null;
  const section = el("section", "heif-sequence");
  section.append(el("h3", undefined, tr("AVIF image sequence")));
  section.append(el("p", "hint", tr("This file declares the AVIS sequence brand. The timeline is read from sample timing; it does not play or decode frames.")));

  const timing = sequenceTimings(m);
  if (!timing.tracks.length) {
    section.append(el("p", "hint", tr("The sequence track or its timing table could not be read completely.")));
    return section;
  }
  if (timing.trackCount > timing.tracks.length) {
    section.append(el("p", "hint", `${tr("Showing the first")} ${timing.tracks.length} ${tr("of")} ${timing.trackCount} ${tr("tracks")}.`));
  }
  timing.tracks.forEach(({ handler, timescale, sampleCount, seconds, runs }, index) => {
    const row = el("div", "heif-sequence-track");
    row.append(el("div", "heif-sequence-label", `${tr("Track")} ${index + 1} · ${handler}`));
    row.append(el("div", "heif-sequence-stats", `${sampleCount.toLocaleString(currentLocale())} ${tr("samples")} · ${seconds > 0 ? `${seconds.toFixed(3)} s` : tr("duration unavailable")}`));
    if (runs.length) {
      const bar = el("div", "heif-sequence-timeline");
      bar.setAttribute("role", "img");
      bar.setAttribute("aria-label", `${tr("Approximate sequence timeline")}: ${sampleCount} ${tr("samples")}, ${seconds.toFixed(3)} s`);
      const shown = runs.length <= MAX_TIMING_RUNS ? runs : runs.slice(0, MAX_TIMING_RUNS - 1);
      const weights = shown.map((run) => ({
        ...run,
        ticks: run.samples * run.delta,
        combined: false,
      }));
      if (runs.length > MAX_TIMING_RUNS) {
        const remaining = runs.slice(MAX_TIMING_RUNS - 1);
        weights.push({
          samples: remaining.reduce((n, run) => n + run.samples, 0),
          delta: 0,
          ticks: remaining.reduce((n, run) => n + run.samples * run.delta, 0),
          combined: true,
        });
      }
      for (const [runIndex, run] of weights.entries()) {
        const part = el("span", "heif-sequence-run");
        part.style.flexGrow = String(Math.max(1, run.ticks));
        part.title = `${run.combined ? `${tr("Combined timing runs")} · ` : ""}${run.samples.toLocaleString(currentLocale())} ${tr("samples")}${timescale > 0 ? ` · ${run.combined ? (run.ticks / timescale).toFixed(3) : (run.delta / timescale).toFixed(3)} ${run.combined ? tr("total") : tr("each")}` : ""}`;
        part.setAttribute("aria-hidden", "true");
        part.dataset.run = String(runIndex + 1);
        bar.append(part);
      }
      row.append(bar);
    } else {
      row.append(el("p", "hint", tr("No complete sample timing runs were found.")));
    }
    section.append(row);
  });
  section.append(el("p", "hint", tr("Duration is estimated from the media sample table. Track edit lists are not applied.")));
  return section;
}

/** HEIF item and reference graph, plus a timing map for AVIF image sequences. */
export function heifMapGroup(m: FileModel, select: (id: number) => void): HTMLElement | null {
  const graph = heifGraph(m);
  if (!graph) return null;
  const group = el("section", "group heif-map");
  group.append(el("h2", undefined, tr("HEIF item graph")));
  group.append(el("p", "hint", tr("Items can share properties, refer to other images, or derive one picture from several tiles.")));

  const list = el("ul", "heif-items");
  for (const item of graph.items) {
    const li = el("li", "heif-item");
    const head = el("div", "heif-item-head");
    head.append(el("strong", undefined, `#${item.id} · ${item.type}`), el("span", "heif-item-role", tr(item.role)));
    li.append(head);
    if (item.name) li.append(el("span", "heif-item-name", item.name));
    const status = item.constructionMethod === 2
      ? tr("Constructed from other items · bytes not assembled")
      : item.node >= 0 && m.len(item.node) > 0
        ? `${tr("Source bytes mapped")} · ${formatBytes(m.len(item.node))}`
        : tr("Source bytes not mapped");
    li.append(el("span", "heif-item-status", status));
    if (item.node >= 0 && m.len(item.node) > 0) {
      const jump = el("button", "link heif-item-jump", tr("Show source bytes"));
      jump.type = "button";
      jump.addEventListener("click", () => select(item.node));
      li.append(jump);
    }
    list.append(li);
  }
  if (!graph.items.length) list.append(el("li", "hint", tr("No item information could be read.")));
  group.append(list);
  if (graph.itemCount > graph.items.length) {
    group.append(el("p", "hint", `${tr("Showing the first")} ${graph.items.length} ${tr("of")} ${graph.itemCount} ${tr("items")}.`));
  }

  if (graph.refs.length) {
    const relations = el("div", "heif-relations");
    relations.append(el("h3", undefined, tr("Relationships")));
    const ul = el("ul");
    for (const ref of graph.refs) {
      const li = el("li");
      li.append(
        el("span", "heif-relation-type", tr(ref.type)),
        el("span", "heif-relation-arrow", `#${ref.from} → ${ref.to.map((id) => `#${id}`).join(", ") || tr("no target")}`),
      );
      ul.append(li);
    }
    relations.append(ul);
    group.append(relations);
    if (graph.referenceCount > graph.refs.length) {
      group.append(el("p", "hint", `${tr("Showing the first")} ${graph.refs.length} ${tr("of")} ${graph.referenceCount} ${tr("relationships")}.`));
    }
  }
  const sequence = sequenceView(m, graph.sequence);
  if (sequence) group.append(sequence);
  return group;
}
