export const VERIFICATION_REASONS = [
  "removed",
  "still_present",
  "value_changed_same_kind",
  "no_stable_value",
  "coverage_incomplete",
  "parse_incomplete",
  "unexpected_output",
  "no_comparable_findings",
  "verification_skipped",
] as const;

export type VerificationReason = (typeof VERIFICATION_REASONS)[number];

export interface VerificationItem {
  kind: string;
  reason: VerificationReason;
  unexpected: boolean;
}

export interface VerificationReport {
  schema_version: 1;
  removed: VerificationItem[];
  present: VerificationItem[];
  unchecked: VerificationItem[];
}

const SKIPPED: VerificationReport = {
  schema_version: 1,
  removed: [],
  present: [],
  unchecked: [{ kind: "file", reason: "verification_skipped", unexpected: false }],
};

/** A fail-closed report used when the worker or bridge could not be read. */
export function skippedVerification(): VerificationReport {
  return {
    schema_version: SKIPPED.schema_version,
    removed: [],
    present: [],
    unchecked: [...SKIPPED.unchecked],
  };
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, expected: string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

function items(value: unknown): VerificationItem[] | null {
  if (!Array.isArray(value)) return null;
  const result: VerificationItem[] = [];
  for (const entry of value) {
    if (
      !record(entry) ||
      !exactKeys(entry, ["kind", "reason", "unexpected"]) ||
      typeof entry.kind !== "string" ||
      entry.kind.length === 0 ||
      typeof entry.unexpected !== "boolean" ||
      typeof entry.reason !== "string" ||
      !VERIFICATION_REASONS.includes(entry.reason as VerificationReason)
    ) {
      return null;
    }
    result.push({
      kind: entry.kind,
      reason: entry.reason as VerificationReason,
      unexpected: entry.unexpected,
    });
  }
  return result;
}

/** Decodes the core's versioned JSON and rejects extra fields fail-closed. */
export function decodeVerification(json: string): VerificationReport {
  try {
    const value: unknown = JSON.parse(json);
    if (!record(value) || !exactKeys(value, ["schema_version", "removed", "present", "unchecked"]) || value.schema_version !== 1) {
      return skippedVerification();
    }
    const removed = items(value.removed);
    const present = items(value.present);
    const unchecked = items(value.unchecked);
    if (!removed || !present || !unchecked) return skippedVerification();
    return { schema_version: 1, removed, present, unchecked };
  } catch {
    return skippedVerification();
  }
}

/** A cautious sentence that describes only the checked capability set. */
export function verificationMessage(report: VerificationReport): string {
  if (report.present.length > 0) return "Some findings are still present in the copy.";
  if (report.unchecked.length > 0 && report.removed.length > 0) {
    return "Some findings were removed, but this copy is not fully checked yet.";
  }
  if (report.unchecked.length > 0) return "This copy could not be fully checked yet.";
  if (report.removed.length > 0) return "The checked findings were removed.";
  return "No comparable findings were checked.";
}
