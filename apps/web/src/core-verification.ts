import type { ParsedFile } from "./model";
import type { VerificationItem, VerificationReport } from "./verification";

const CORE_REASONS = [
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

type CoreReason = (typeof CORE_REASONS)[number];
type Group = "removed" | "present" | "unchecked";

const presentReasons = new Set<CoreReason>([
  "still_present",
  "value_changed_same_kind",
  "unexpected_output",
]);
const uncheckedReasons = new Set<CoreReason>([
  "no_stable_value",
  "coverage_incomplete",
  "parse_incomplete",
  "no_comparable_findings",
  "verification_skipped",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function hasExactKeys(record: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(record);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(record, key));
}

function isCoreReason(value: unknown): value is CoreReason {
  return typeof value === "string" && CORE_REASONS.includes(value as CoreReason);
}

function belongsInGroup(group: Group, reason: CoreReason): boolean {
  if (group === "removed") return reason === "removed";
  if (group === "present") return presentReasons.has(reason);
  return uncheckedReasons.has(reason);
}

function reasonText(
  reason: CoreReason,
  kind: string,
  format: ParsedFile["format"],
  retainedReasons: Readonly<Record<string, string>>,
): string | undefined {
  switch (reason) {
    case "removed":
      return undefined;
    case "still_present":
      return Object.hasOwn(retainedReasons, kind)
        ? retainedReasons[kind]
        : "This finding is still present in the copy.";
    case "value_changed_same_kind":
      return "A different value remains in the copy.";
    case "no_stable_value":
      return "This finding has no stable value to compare.";
    case "coverage_incomplete":
      return `Hexscope does not have complete ${format.toUpperCase()} coverage for this finding yet.`;
    case "parse_incomplete":
      return "Hexscope could not completely read the copy, so its findings were not checked.";
    case "unexpected_output":
      return "This finding appeared in the copy but was not found in the source file.";
    case "no_comparable_findings":
      return "No comparable findings were available to check.";
    case "verification_skipped":
      return "Check unavailable.";
  }
}

function decodeItem(
  value: unknown,
  group: Group,
  format: ParsedFile["format"],
  labels: Readonly<Record<string, string>>,
  retainedReasons: Readonly<Record<string, string>>,
): VerificationItem | null {
  if (!isRecord(value) || !hasExactKeys(value, ["kind", "reason", "unexpected"])) return null;
  const { kind, reason, unexpected } = value;
  if (
    typeof kind !== "string"
    || !/^[a-z][a-z0-9_-]{0,63}$/.test(kind)
    || !isCoreReason(reason)
    || !belongsInGroup(group, reason)
    || typeof unexpected !== "boolean"
    || unexpected !== (reason === "unexpected_output")
  ) return null;

  const label = Object.hasOwn(labels, kind) ? labels[kind] : kind;
  const explanation = reasonText(reason, kind, format, retainedReasons);
  return {
    kind,
    label,
    ...(explanation ? { reason: explanation } : {}),
    ...(unexpected ? { unexpected: true } : {}),
  };
}

function decodeGroup(
  value: unknown,
  group: Group,
  format: ParsedFile["format"],
  labels: Readonly<Record<string, string>>,
  retainedReasons: Readonly<Record<string, string>>,
): VerificationItem[] | null {
  if (!Array.isArray(value)) return null;
  const decoded: VerificationItem[] = [];
  for (const entry of value) {
    const item = decodeItem(entry, group, format, labels, retainedReasons);
    if (!item) return null;
    decoded.push(item);
  }
  return decoded;
}

/** Decode the shared Rust schema without allowing values or scopes across the UI boundary. */
export function decodeCoreVerification(
  json: string,
  format: ParsedFile["format"],
  labels: Readonly<Record<string, string>>,
  retainedReasons: Readonly<Record<string, string>>,
): VerificationReport | null {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    return null;
  }
  if (
    !isRecord(value)
    || !hasExactKeys(value, ["schema_version", "removed", "present", "unchecked"])
    || value.schema_version !== 1
  ) return null;

  const removed = decodeGroup(value.removed, "removed", format, labels, retainedReasons);
  const present = decodeGroup(value.present, "present", format, labels, retainedReasons);
  const unchecked = decodeGroup(value.unchecked, "unchecked", format, labels, retainedReasons);
  return removed && present && unchecked ? { removed, present, unchecked } : null;
}
