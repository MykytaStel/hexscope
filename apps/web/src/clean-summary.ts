import type { VerificationReport } from "./verification";

export interface CleanCopySummary {
  state: "checked" | "unverified";
  removed: number;
  present: number;
  unchecked: number;
  clear: boolean;
}

/** Counts only what the readback report actually established. */
export function cleanCopySummary(report: VerificationReport | undefined): CleanCopySummary {
  if (!report) return { state: "unverified", removed: 0, present: 0, unchecked: 0, clear: false };
  return {
    state: "checked",
    removed: report.removed.length,
    present: report.present.length,
    unchecked: report.unchecked.length,
    clear: report.present.length === 0 && report.unchecked.length === 0,
  };
}

/** A visible value can be crossed out only when all findings of its kind were checked and removed. */
export function confirmedRemovedKinds(report: VerificationReport | undefined): string[] {
  if (!report) return [];
  const notRemoved = new Set([...report.present, ...report.unchecked].map((item) => item.kind));
  return [...new Set(report.removed.filter((item) => !notRemoved.has(item.kind)).map((item) => item.kind))];
}
