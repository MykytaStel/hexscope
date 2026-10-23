import type { ParsedFile, PhotoFact, PhotoLocation } from "./model";

export { decodeCoreVerification } from "./core-verification";

/**
 * 10 MiB cap from the Playwright Pixel 7 profile: a deterministic 9.78 MiB
 * PNG completed parse → clean → reparse in 2.55 s. Chromium's process-tree
 * RSS peaked at 502.6 MiB (no renderer-only metric was available); the 9.5 MiB
 * JS-heap reading excludes Wasm memory. This is emulation evidence, not a
 * physical-device claim. Copy handoff runs before verification starts.
 */
export const MAX_VERIFY_BYTES = 10 * 1024 * 1024;

export interface VerificationFinding {
  format: ParsedFile["format"];
  kind: string;
  label: string;
  value: string | null;
  scope: string | null;
}

export interface VerificationCoverage {
  format: ParsedFile["format"];
  kind: string;
  scope: string;
  complete: boolean;
  reason?: string;
}

export interface VerificationItem {
  kind: string;
  label: string;
  reason?: string;
  /** Stable core code kept beside localized presentation text for receipts. */
  reasonCode?: VerificationReasonCode;
  unexpected?: boolean;
}

export type VerificationReasonCode =
  | "removed"
  | "still_present"
  | "value_changed_same_kind"
  | "no_stable_value"
  | "coverage_incomplete"
  | "parse_incomplete"
  | "unexpected_output"
  | "no_comparable_findings"
  | "verification_skipped";

export interface VerificationReport {
  removed: VerificationItem[];
  present: VerificationItem[];
  unchecked: VerificationItem[];
}

export interface VerificationSelection {
  /** Counted from 1, as in the PDF parser. */
  page: number;
  /** Kept in the worker only; it is never copied to a report item. */
  text: string;
  sourceComplete: boolean;
  /** The selection covers picture pixels, with no searchable glyphs. */
  areaOnly: boolean;
}

export interface VerificationPage {
  page: number;
  text: string;
  complete: boolean;
}

export interface VerificationFileOutput {
  format: ParsedFile["format"];
  facts: readonly Pick<PhotoFact, "kind" | "text">[];
  location: Pick<PhotoLocation, "latitude" | "longitude"> | null;
}

const FILE_SCOPE = "file";
const JPEG_COMPLETE_FILE_KINDS = new Set([
  // The JPEG cleaner regression covers these findings disappearing on a
  // complete, error-free reparse. Other EXIF kinds stay unchecked until the
  // regression fixture exercises their output coverage too.
  "camera",
  "serial",
  "owner",
  "location",
]);

const noStableValue = "This finding has no stable value to compare.";
const stillPresent = "This finding is still present in the copy.";
const noCompleteCoverage = (format: string) => `hexscope does not have complete ${format.toUpperCase()} coverage for this finding yet.`;

function stableValue(value: string | null | undefined): string | null {
  if (typeof value !== "string") return null;
  const normalized = value.normalize("NFC").trim();
  return normalized.length > 0 ? normalized : null;
}

/** Collect comparable file-level facts and the photo's stable coordinate identity. */
export function fileFindings(
  format: ParsedFile["format"],
  facts: readonly Pick<PhotoFact, "kind" | "text">[],
  location: Pick<PhotoLocation, "latitude" | "longitude"> | null,
  labels: Readonly<Record<string, string>>,
): VerificationFinding[] {
  const findings = facts.map(({ kind, text }) => ({
    format,
    kind,
    label: labels[kind] ?? kind,
    value: stableValue(text),
    scope: FILE_SCOPE,
  }));

  if (location && Number.isFinite(location.latitude) && Number.isFinite(location.longitude)) {
    findings.push({
      format,
      kind: "location",
      label: labels.location ?? "Location",
      value: `${location.latitude}, ${location.longitude}`,
      scope: FILE_SCOPE,
    });
  }
  return findings;
}

/** Return explicit file-level parser coverage; every other pair is incomplete. */
export function capabilitiesFor(format: ParsedFile["format"], kinds: Iterable<string>): VerificationCoverage[] {
  const seen = new Set<string>();
  const coverage: VerificationCoverage[] = [];
  for (const kind of kinds) {
    if (seen.has(kind)) continue;
    seen.add(kind);
    const complete = format === "jpeg" && JPEG_COMPLETE_FILE_KINDS.has(kind);
    coverage.push({
      format,
      kind,
      scope: FILE_SCOPE,
      complete,
      ...(complete ? {} : { reason: noCompleteCoverage(format) }),
    });
  }
  return coverage;
}

function identity(finding: VerificationFinding): string | null {
  const value = stableValue(finding.value);
  if (!value || !finding.scope) return null;
  return JSON.stringify([finding.format, finding.kind, finding.scope, value]);
}

function fieldIdentity(finding: VerificationFinding): string | null {
  if (!finding.scope) return null;
  return JSON.stringify([finding.format, finding.kind, finding.scope]);
}

function item(
  finding: Pick<VerificationFinding, "kind" | "label">,
  reason?: string,
  unexpected?: boolean,
  reasonCode?: VerificationReasonCode,
): VerificationItem {
  return {
    kind: finding.kind,
    label: finding.label,
    ...(reason ? { reason } : {}),
    ...(reasonCode ? { reasonCode } : {}),
    ...(unexpected ? { unexpected: true } : {}),
  };
}

/** Compare file findings as a multiset without returning their values or scopes. */
export function verifyFindings(
  source: readonly VerificationFinding[],
  output: readonly VerificationFinding[],
  coverage: readonly VerificationCoverage[],
  retainedReasons: Readonly<Record<string, string>>,
): VerificationReport {
  const report: VerificationReport = { removed: [], present: [], unchecked: [] };
  const outputUsed = new Array<boolean>(output.length).fill(false);
  const sourceIdentities = source.map(identity);
  const outputIdentities = output.map(identity);
  const sourceFields = source.map(fieldIdentity);
  const outputFields = output.map(fieldIdentity);
  const exactOutputs = new Map<string, number[]>();
  const outputsByField = new Map<string, number[]>();
  const unknownOutputsByField = new Map<string, number[]>();
  const sourcesByField = new Map<string, number[]>();
  const coverageByField = new Map<string, VerificationCoverage>();
  const sourceResults: ({ group: "present" | "unchecked"; item: VerificationItem } | undefined)[] =
    new Array(source.length);

  for (const capability of coverage) {
    const key = JSON.stringify([capability.format, capability.kind, capability.scope]);
    coverageByField.set(key, capability);
  }
  output.forEach((_, index) => {
    const exact = outputIdentities[index];
    if (exact) {
      const group = exactOutputs.get(exact) ?? [];
      group.push(index);
      exactOutputs.set(exact, group);
    }
    const field = outputFields[index];
    if (field) {
      const group = outputsByField.get(field) ?? [];
      group.push(index);
      outputsByField.set(field, group);
      if (!outputIdentities[index]) {
        const unknown = unknownOutputsByField.get(field) ?? [];
        unknown.push(index);
        unknownOutputsByField.set(field, unknown);
      }
    }
  });
  sourceFields.forEach((field, index) => {
    if (!field) return;
    const group = sourcesByField.get(field) ?? [];
    group.push(index);
    sourcesByField.set(field, group);
  });

  // Reserve every exact occurrence first. A changed value must not consume an
  // output that exactly matches a later source finding.
  const exactCursors = new Map<string, number>();
  source.forEach((original, index) => {
    const exact = sourceIdentities[index];
    if (!exact) return;
    const candidates = exactOutputs.get(exact);
    if (!candidates) return;
    let cursor = exactCursors.get(exact) ?? 0;
    while (cursor < candidates.length && outputUsed[candidates[cursor]]) cursor++;
    const matched = candidates[cursor];
    if (matched === undefined) return;
    outputUsed[matched] = true;
    exactCursors.set(exact, cursor + 1);
    sourceResults[index] = {
      group: "present",
      item: item(original, retainedReasons[original.kind] ?? stillPresent, false, "still_present"),
    };
  });

  // Pair stable leftovers of the same format, kind, and scope as changed
  // values. Each side advances through indexed queues instead of rescanning
  // every finding for each source item.
  const fieldCursors = new Map<string, number>();
  source.forEach((original, index) => {
    if (sourceResults[index] || !sourceIdentities[index]) return;
    const field = sourceFields[index];
    if (!field) return;
    const candidates = outputsByField.get(field);
    if (!candidates) return;
    let cursor = fieldCursors.get(field) ?? 0;
    while (
      cursor < candidates.length
      && (outputUsed[candidates[cursor]] || !outputIdentities[candidates[cursor]])
    ) cursor++;
    const matched = candidates[cursor];
    if (matched === undefined) {
      fieldCursors.set(field, cursor);
      return;
    }
    outputUsed[matched] = true;
    fieldCursors.set(field, cursor + 1);
    sourceResults[index] = {
      group: "present",
      item: item(original, "A different value remains in the copy.", false, "value_changed_same_kind"),
    };
  });

  // A same-kind output with no stable value makes unmatched source findings
  // uncertain; pair those occurrences instead of claiming they were removed.
  for (const [field, candidates] of unknownOutputsByField) {
    const unmatchedSources = sourcesByField.get(field) ?? [];
    const unmatched = unmatchedSources.filter((sourceIndex) => !sourceResults[sourceIndex]);
    for (const sourceIndex of unmatched) {
      sourceResults[sourceIndex] = {
        group: "unchecked",
        item: item(source[sourceIndex], noStableValue, false, "no_stable_value"),
      };
    }
    let remaining = unmatched.length;
    for (const outputIndex of candidates) {
      if (remaining === 0) break;
      if (outputUsed[outputIndex]) continue;
      outputUsed[outputIndex] = true;
      remaining--;
    }
  }

  source.forEach((original, index) => {
    const result = sourceResults[index];
    if (result) {
      report[result.group].push(result.item);
      return;
    }
    if (!sourceIdentities[index]) {
      report.unchecked.push(item(original, noStableValue, false, "no_stable_value"));
      return;
    }
    const field = sourceFields[index];
    const capability = field ? coverageByField.get(field) : undefined;
    if (!capability?.complete) {
      report.unchecked.push(item(original, capability?.reason ?? noCompleteCoverage(original.format), false, "coverage_incomplete"));
      return;
    }
    report.removed.push(item(original, undefined, false, "removed"));
  });

  output.forEach((finding, index) => {
    if (outputUsed[index]) return;
    if (!outputIdentities[index]) report.unchecked.push(item(finding, noStableValue, false, "no_stable_value"));
    else report.present.push(item(finding, undefined, true, "unexpected_output"));
  });

  if (report.removed.length + report.present.length + report.unchecked.length === 0) {
    report.unchecked.push({ kind: "verification", label: "Copy verification", reason: "No comparable findings were available to check.", reasonCode: "no_comparable_findings" });
  }
  return report;
}

/** Convert one parsed output observation to findings and apply tested capabilities. */
export function verifyFileOutput(
  source: readonly VerificationFinding[],
  output: VerificationFileOutput,
  labels: Readonly<Record<string, string>>,
  retainedReasons: Readonly<Record<string, string>>,
  extraOutputFacts: readonly Pick<PhotoFact, "kind" | "text">[] = [],
): VerificationReport {
  const outputFindings = fileFindings(output.format, [...output.facts, ...extraOutputFacts], output.location, labels);
  return verifyFindings(source, outputFindings, capabilitiesFor(output.format, source.map(({ kind }) => kind)), retainedReasons);
}

const PDF_TEXT_KIND = "pdf-text";
const PDF_TEXT_LABEL = "Selected PDF text";
const PDF_AREA_KIND = "pdf-picture";
const PDF_AREA_LABEL = "Text in the selected picture area";
const PDF_AREA_UNCHECKED_REASON =
  "The selected area was redacted by the operation, but text inside its pixels was not checked because hexscope does not use OCR.";

function normalizedPdfText(text: string): string {
  return text.normalize("NFC").toLowerCase().split(/\s+/u).filter(Boolean).join(" ");
}

function uncheckedSelection(
  selection: VerificationSelection,
  reason: string,
  reasonCode: VerificationReasonCode,
): VerificationItem {
  return selection.areaOnly
    ? item({ kind: PDF_AREA_KIND, label: PDF_AREA_LABEL }, reason, false, reasonCode)
    : item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }, reason, false, reasonCode);
}

/** Check each selected text occurrence only against the output page it came from. */
export function verifyPdfSelections(
  selections: readonly VerificationSelection[],
  pages: readonly VerificationPage[],
): VerificationReport {
  const report: VerificationReport = { removed: [], present: [], unchecked: [] };
  for (const selection of selections) {
    if (selection.areaOnly) {
      report.unchecked.push(uncheckedSelection(selection, PDF_AREA_UNCHECKED_REASON, "coverage_incomplete"));
      continue;
    }

    const needle = normalizedPdfText(selection.text);
    if (!needle) {
      report.unchecked.push(uncheckedSelection(selection, "No searchable text was available for this selection.", "no_stable_value"));
      continue;
    }
    if (!selection.sourceComplete) {
      report.unchecked.push(uncheckedSelection(selection, "The source page was incomplete, so this selection could not be checked.", "parse_incomplete"));
      continue;
    }

    const page = pages.find((candidate) => candidate.page === selection.page);
    if (!page) {
      report.unchecked.push(uncheckedSelection(selection, "The corresponding output page could not be checked.", "parse_incomplete"));
      continue;
    }
    if (!page.complete) {
      report.unchecked.push(uncheckedSelection(selection, "The output page was incomplete, so this selection could not be checked.", "parse_incomplete"));
      continue;
    }

    const haystack = normalizedPdfText(page.text);
    if (haystack.includes(needle)) report.present.push(item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }, undefined, false, "still_present"));
    else report.removed.push(item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }, undefined, false, "removed"));
  }

  if (report.removed.length + report.present.length + report.unchecked.length === 0) {
    report.unchecked.push({ kind: "verification", label: "Copy verification", reason: "No comparable findings were available to check.", reasonCode: "no_comparable_findings" });
  }
  return report;
}

/** Conservatively mark every supplied finding unchecked after a skipped or failed verification. */
export function uncheckedReport(
  source: readonly VerificationFinding[],
  selections: readonly VerificationSelection[],
  reason: string,
): VerificationReport {
  const code: VerificationReasonCode = reason.startsWith("hexscope could not completely read")
    ? "parse_incomplete"
    : "verification_skipped";
  const unchecked = [
    ...source.map((finding) => item(finding, reason, false, code)),
    ...selections.map((selection) =>
      uncheckedSelection(
        selection,
        selection.areaOnly ? `${PDF_AREA_UNCHECKED_REASON} ${reason}` : reason,
        selection.areaOnly ? "coverage_incomplete" : code,
      ),
    ),
  ];
  if (unchecked.length === 0) unchecked.push({ kind: "verification", label: "Copy verification", reason, reasonCode: code });
  return { removed: [], present: [], unchecked };
}

export interface VerificationReply {
  type: string;
  report?: VerificationReport;
}

function compactReport(value: unknown): VerificationReport | null {
  if (!value || typeof value !== "object") return null;
  const report = value as Record<string, unknown>;
  const items = (list: unknown): VerificationItem[] | null => {
    if (!Array.isArray(list)) return null;
    const compact: VerificationItem[] = [];
    for (const value of list) {
      if (!value || typeof value !== "object") return null;
      const candidate = value as Record<string, unknown>;
      if (typeof candidate.kind !== "string" || typeof candidate.label !== "string") return null;
      compact.push({
        kind: candidate.kind,
        label: candidate.label,
        ...(typeof candidate.reason === "string" ? { reason: candidate.reason } : {}),
        ...(isVerificationReasonCode(candidate.reasonCode) ? { reasonCode: candidate.reasonCode } : {}),
        ...(candidate.unexpected === true ? { unexpected: true } : {}),
      });
    }
    return compact;
  };
  const removed = items(report.removed);
  const present = items(report.present);
  const unchecked = items(report.unchecked);
  return removed && present && unchecked ? { removed, present, unchecked } : null;
}

function isVerificationReasonCode(value: unknown): value is VerificationReasonCode {
  return typeof value === "string" && [
    "removed", "still_present", "value_changed_same_kind", "no_stable_value",
    "coverage_incomplete", "parse_incomplete", "unexpected_output",
    "no_comparable_findings", "verification_skipped",
  ].includes(value);
}

/** Run a bounded worker check and map every skip, failure, or malformed reply to a private unchecked report. */
export async function checkCopySafely(
  sizeBytes: number,
  source: readonly VerificationFinding[],
  selections: readonly VerificationSelection[],
  inspect: () => Promise<VerificationReply>,
): Promise<VerificationReport> {
  if (!verificationEligible(sizeBytes)) {
    return uncheckedReport(source, selections, "This copy is larger than the 10 MiB verification limit.");
  }
  try {
    const reply = await inspect();
    const report = reply.type === "verification" ? compactReport(reply.report) : null;
    if (report) return report;
  } catch {
    // Worker errors can contain file-derived text. Keep only a general reason.
  }
  return uncheckedReport(source, selections, "Check unavailable.");
}

/** Whether the output can be reparsed within the measured browser budget. */
export function verificationEligible(sizeBytes: number, maxBytes = MAX_VERIFY_BYTES): boolean {
  return Number.isSafeInteger(sizeBytes) && sizeBytes >= 0 && Number.isSafeInteger(maxBytes) && maxBytes >= 0 && sizeBytes <= maxBytes;
}
