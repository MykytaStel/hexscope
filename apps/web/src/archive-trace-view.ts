import { el, fact, hex } from "./dom";
import { openReason, playReason } from "./knowledge";
import type { ArchiveTraceRange, FileModel, ZipEntryInfo } from "./model";

export interface ArchiveTraceActions {
  nested: number;
  onSelectRange(start: number, len: number): void;
  onPlay(entry: number): void;
  onOpen(entry: number): void;
}

function rangeStep(
  title: string,
  range: ArchiveTraceRange | null,
  absent: string,
  onSelectRange: ArchiveTraceActions["onSelectRange"],
): HTMLLIElement {
  const step = el("li", "archive-trace-step");
  step.append(el("h3", undefined, title));
  const label = `Show ${title.toLowerCase()} bytes`;
  const button = el("button", "btn", label);
  button.disabled = range ? range.len <= 0 : true;
  button.title = range ? (range.len > 0 ? label : "No bytes are available for this range.") : absent;
  button.addEventListener("click", () => {
    if (range) onSelectRange(range.start, range.len);
  });
  step.append(button);
  if (range) {
    step.append(el("p", "archive-trace-range", `${hex(range.start)}–${hex(range.start + range.len)}`));
    step.append(el("p", "archive-trace-byte-count", `${range.len.toLocaleString("en")} bytes`));
  } else {
    step.append(el("p", "hint", absent));
  }
  return step;
}

function actionButton(text: string, reason: string | null, title: string, act: () => void): HTMLButtonElement {
  const button = el("button", "btn", text);
  button.disabled = reason !== null;
  button.title = reason ?? title;
  button.addEventListener("click", act);
  return button;
}

function outputStep(entry: ZipEntryInfo, i: number, actions: ArchiveTraceActions): HTMLLIElement {
  const step = el("li", "archive-trace-step");
  step.append(el("h3", undefined, "Unpacked output"));
  const reasonToPlay = playReason(entry);
  const reasonToOpen = openReason(entry, actions.nested);
  const outputActions = el("div", "entry-actions");
  if (entry.playable) {
    outputActions.append(actionButton("Watch it decompress", reasonToPlay, "Step through this entry's DEFLATE data (P)", () => actions.onPlay(i)));
  }
  if (entry.openable) outputActions.append(actionButton("Open", reasonToOpen, "Open this entry as a file of its own", () => actions.onOpen(i)));
  if (outputActions.childElementCount > 0) step.append(outputActions);
  if (reasonToPlay) step.append(el("p", "hint", reasonToPlay));
  if (reasonToOpen) step.append(el("p", "hint", reasonToOpen));
  return step;
}

/** Draws the byte-level path from a ZIP directory record to an entry's output. */
export function archiveTraceView(m: FileModel, i: number, actions: ArchiveTraceActions): HTMLElement {
  const entry = m.entry(i);
  const trace = m.archiveTrace(i);
  const group = el("div", "group archive-trace");
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", "Archive Trace");
  group.append(el("h2", undefined, "Archive Trace"));

  const steps = el("ol", "archive-trace-steps");
  steps.append(rangeStep("Central-directory record", trace.centralRecord, "No central-directory record is linked to this entry.", actions.onSelectRange));
  const header = rangeStep("Local header", trace.localHeader, "The local-header range is unavailable.", actions.onSelectRange);
  if (trace.localHeaderWarning) {
    header.append(el("p", "hint", "The parser flagged bytes in this local header. Compare the highlighted range with the central record."));
  }
  steps.append(header);

  const compressed = rangeStep("Compressed data", trace.compressedData, "The compressed-data range is unavailable.", actions.onSelectRange);
  if (trace.compressedData) {
    const sizes = el("dl", "facts archive-trace-sizes");
    fact(sizes, "Available bytes", trace.compressedData.len.toLocaleString());
    fact(sizes, "Declared bytes", entry.compressed.toLocaleString());
    compressed.append(sizes);
    if (!trace.dataComplete) compressed.append(el("p", "hint", "Only part of the declared compressed data is present."));
  }
  steps.append(compressed);
  steps.append(outputStep(entry, i, actions));

  group.append(steps);
  return group;
}
