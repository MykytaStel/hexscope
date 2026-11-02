// The core owns reason codes, report groups and their default wording.
import source from "../../../crates/hexscope-core/src/verification-reasons.tsv?raw";
import type { VerificationReasonCode } from "./verification";

export type VerificationGroup = "removed" | "present" | "unchecked";

interface ReasonWords {
  code: VerificationReasonCode;
  group: VerificationGroup;
  message: string;
}

const groups = new Set<VerificationGroup>(["removed", "present", "unchecked"]);

const rows: ReasonWords[] = source
  .split(/\r?\n/)
  .filter((line) => line.length > 0 && !line.startsWith("#"))
  .map((line) => {
    const [code, group, message = ""] = line.split("\t");
    if (
      !code
      || !/^[a-z][a-z0-9_]*$/.test(code)
      || !groups.has(group as VerificationGroup)
    ) throw new Error("Invalid verification reason row in the core catalogue");
    return { code: code as VerificationReasonCode, group: group as VerificationGroup, message };
  });

const byCode = new Map(rows.map((row) => [row.code, row] as const));
if (byCode.size !== rows.length) throw new Error("Duplicate verification reason code in the core catalogue");

export const VERIFICATION_REASON_CODES: readonly VerificationReasonCode[] = rows.map(({ code }) => code);

export const REASONS_BY_GROUP: Record<VerificationGroup, ReadonlySet<VerificationReasonCode>> = {
  removed: new Set(rows.filter((row) => row.group === "removed").map(({ code }) => code)),
  present: new Set(rows.filter((row) => row.group === "present").map(({ code }) => code)),
  unchecked: new Set(rows.filter((row) => row.group === "unchecked").map(({ code }) => code)),
};

export function isVerificationReasonCode(value: unknown): value is VerificationReasonCode {
  return typeof value === "string" && byCode.has(value as VerificationReasonCode);
}

export function verificationReasonMessage(
  code: VerificationReasonCode,
  format: string,
): string | undefined {
  const template = byCode.get(code)?.message;
  return template ? template.replace("{format}", format.toUpperCase()) : undefined;
}
