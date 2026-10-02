import type { ParsedFile, PhotoFact, PhotoLocation } from "./model";

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
  unexpected?: boolean;
}

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
const noCompleteCoverage = (format: string) => `Hexscope does not have complete ${format.toUpperCase()} coverage for this finding yet.`;

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

function item(finding: Pick<VerificationFinding, "kind" | "label">, reason?: string, unexpected?: boolean): VerificationItem {
  return {
    kind: finding.kind,
    label: finding.label,
    ...(reason ? { reason } : {}),
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
  const outputUsed = new Set<number>();

  for (const original of source) {
    const originalIdentity = identity(original);
    const matchingIndex = originalIdentity === null
      ? -1
      : output.findIndex((candidate, index) => !outputUsed.has(index) && identity(candidate) === originalIdentity);

    if (matchingIndex >= 0) {
      outputUsed.add(matchingIndex);
      const reason = retainedReasons[original.kind];
      report.present.push(item(original, reason, !reason));
      continue;
    }

    if (originalIdentity === null) {
      report.unchecked.push(item(original, noStableValue));
      continue;
    }

    const capability = coverage.find(
      (candidate) => candidate.format === original.format && candidate.kind === original.kind && candidate.scope === original.scope,
    );
    if (!capability?.complete) {
      report.unchecked.push(item(original, capability?.reason ?? noCompleteCoverage(original.format)));
      continue;
    }

    report.removed.push(item(original));
  }

  output.forEach((finding, index) => {
    if (outputUsed.has(index)) return;
    if (identity(finding) === null) report.unchecked.push(item(finding, noStableValue));
    else report.present.push(item(finding, undefined, true));
  });

  if (report.removed.length + report.present.length + report.unchecked.length === 0) {
    report.unchecked.push({ kind: "verification", label: "Copy verification", reason: "No comparable findings were available to check." });
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
  "The selected area was redacted by the operation, but text inside its pixels was not checked because Hexscope does not use OCR.";

function normalizedPdfText(text: string): string {
  return text.normalize("NFC").toLowerCase().split(/\s+/u).filter(Boolean).join(" ");
}

function uncheckedSelection(selection: VerificationSelection, reason: string): VerificationItem {
  return selection.areaOnly
    ? item({ kind: PDF_AREA_KIND, label: PDF_AREA_LABEL }, reason)
    : item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }, reason);
}

/** Check each selected text occurrence only against the output page it came from. */
export function verifyPdfSelections(
  selections: readonly VerificationSelection[],
  pages: readonly VerificationPage[],
): VerificationReport {
  const report: VerificationReport = { removed: [], present: [], unchecked: [] };
  for (const selection of selections) {
    if (selection.areaOnly) {
      report.unchecked.push(uncheckedSelection(selection, PDF_AREA_UNCHECKED_REASON));
      continue;
    }

    const needle = normalizedPdfText(selection.text);
    if (!needle) {
      report.unchecked.push(uncheckedSelection(selection, "No searchable text was available for this selection."));
      continue;
    }
    if (!selection.sourceComplete) {
      report.unchecked.push(uncheckedSelection(selection, "The source page was incomplete, so this selection could not be checked."));
      continue;
    }

    const page = pages.find((candidate) => candidate.page === selection.page);
    if (!page) {
      report.unchecked.push(uncheckedSelection(selection, "The corresponding output page could not be checked."));
      continue;
    }
    if (!page.complete) {
      report.unchecked.push(uncheckedSelection(selection, "The output page was incomplete, so this selection could not be checked."));
      continue;
    }

    const haystack = normalizedPdfText(page.text);
    if (haystack.includes(needle)) report.present.push(item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }));
    else report.removed.push(item({ kind: PDF_TEXT_KIND, label: PDF_TEXT_LABEL }));
  }

  if (report.removed.length + report.present.length + report.unchecked.length === 0) {
    report.unchecked.push({ kind: "verification", label: "Copy verification", reason: "No comparable findings were available to check." });
  }
  return report;
}

/** Conservatively mark every supplied finding unchecked after a skipped or failed verification. */
export function uncheckedReport(
  source: readonly VerificationFinding[],
  selections: readonly VerificationSelection[],
  reason: string,
): VerificationReport {
  const unchecked = [
    ...source.map((finding) => item(finding, reason)),
    ...selections.map((selection) =>
      uncheckedSelection(selection, selection.areaOnly ? `${PDF_AREA_UNCHECKED_REASON} ${reason}` : reason),
    ),
  ];
  if (unchecked.length === 0) unchecked.push({ kind: "verification", label: "Copy verification", reason });
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
  return uncheckedReport(source, selections, "Copy check unavailable.");
}

/** Whether the output can be reparsed within the measured browser budget. */
export function verificationEligible(sizeBytes: number, maxBytes = MAX_VERIFY_BYTES): boolean {
  return Number.isSafeInteger(sizeBytes) && sizeBytes >= 0 && Number.isSafeInteger(maxBytes) && maxBytes >= 0 && sizeBytes <= maxBytes;
}
