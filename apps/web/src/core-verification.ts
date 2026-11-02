import type { ParsedFile } from "./model";
import type { VerificationItem, VerificationReasonCode, VerificationReport } from "./verification";
import {
  isVerificationReasonCode,
  REASONS_BY_GROUP,
  type VerificationGroup,
  verificationReasonMessage,
} from "./verification-reasons";

type CoreReason = VerificationReasonCode;
type Group = VerificationGroup;

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function hasExactKeys(record: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(record);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(record, key));
}

function isCoreReason(value: unknown): value is CoreReason {
  return isVerificationReasonCode(value);
}

function belongsInGroup(group: Group, reason: CoreReason): boolean {
  return REASONS_BY_GROUP[group].has(reason);
}

function reasonText(
  reason: CoreReason,
  kind: string,
  format: ParsedFile["format"],
  retainedReasons: Readonly<Record<string, string>>,
): string | undefined {
  if (reason === "still_present" && Object.hasOwn(retainedReasons, kind)) {
    return retainedReasons[kind];
  }
  return verificationReasonMessage(reason, format);
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
    reasonCode: reason,
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
