import { describe, expect, it } from "vitest";
import {
  MAX_VERIFY_BYTES,
  capabilitiesFor,
  fileFindings,
  uncheckedReport,
  verificationEligible,
  verifyFindings,
  verifyPdfSelections,
  type VerificationFinding,
  type VerificationSelection,
} from "./verification";
import type { ParsedFile } from "./model";

const labels = { camera: "Camera", location: "Location", owner: "Owner", ai: "Made with AI" };

function finding(kind: string, value: string | null, format: ParsedFile["format"] = "jpeg", scope: string | null = "file"): VerificationFinding {
  return { format, kind, label: labels[kind as keyof typeof labels] ?? kind, value, scope };
}

function pageSelection(overrides: Partial<VerificationSelection> = {}): VerificationSelection {
  return { page: 1, text: "Secret Name", sourceComplete: true, areaOnly: false, ...overrides };
}

describe("clean-copy verification findings", () => {
  it("collects normalized file facts, duplicate occurrences, and a stable location", () => {
    const findings = fileFindings(
      "jpeg",
      [
        { kind: "camera", text: "  Cafe\u0301 camera  " },
        { kind: "camera", text: "  Cafe\u0301 camera  " },
      ],
      { latitude: 48.8584, longitude: 2.2945, altitude: 35 },
      labels,
    );

    expect(findings).toEqual([
      { format: "jpeg", kind: "camera", label: "Camera", value: "Café camera", scope: "file" },
      { format: "jpeg", kind: "camera", label: "Camera", value: "Café camera", scope: "file" },
      { format: "jpeg", kind: "location", label: "Location", value: "48.8584, 2.2945", scope: "file" },
    ]);
  });

  it("keeps facts with no stable value so the report can say they were not checked", () => {
    expect(fileFindings("jpeg", [{ kind: "owner", text: " \n " }], null, labels)).toEqual([
      { format: "jpeg", kind: "owner", label: "Owner", value: null, scope: "file" },
    ]);
  });

  it("enables only the fixture-backed JPEG file-finding kinds", () => {
    expect(capabilitiesFor("jpeg", ["camera", "serial", "owner", "location", "taken", "thumbnail", "ai"])).toEqual([
      { format: "jpeg", kind: "camera", scope: "file", complete: true },
      { format: "jpeg", kind: "serial", scope: "file", complete: true },
      { format: "jpeg", kind: "owner", scope: "file", complete: true },
      { format: "jpeg", kind: "location", scope: "file", complete: true },
      { format: "jpeg", kind: "taken", scope: "file", complete: false, reason: expect.any(String) },
      { format: "jpeg", kind: "thumbnail", scope: "file", complete: false, reason: expect.any(String) },
      { format: "jpeg", kind: "ai", scope: "file", complete: false, reason: expect.any(String) },
    ]);
    expect(capabilitiesFor("png", ["camera", "location"]).every((capability) => !capability.complete)).toBe(true);
  });

  it("matches duplicate source findings as separate occurrences", () => {
    const report = verifyFindings(
      [finding("camera", "Canon"), finding("camera", "Canon")],
      [finding("camera", "Canon")],
      capabilitiesFor("jpeg", ["camera"]),
      {},
    );

    expect(report).toEqual({
      removed: [{ kind: "camera", label: "Camera" }],
      present: [{ kind: "camera", label: "Camera", unexpected: true }],
      unchecked: [],
    });
  });

  it("keeps equal output-only occurrences as separate residuals", () => {
    const report = verifyFindings(
      [finding("camera", "Canon")],
      [finding("camera", "Canon"), finding("camera", "Canon")],
      capabilitiesFor("jpeg", ["camera"]),
      {},
    );

    expect(report.present).toEqual([
      { kind: "camera", label: "Camera", unexpected: true },
      { kind: "camera", label: "Camera", unexpected: true },
    ]);
  });

  it("explains an intentionally retained matching finding", () => {
    const report = verifyFindings(
      [finding("camera", "Canon")],
      [finding("camera", "Canon")],
      capabilitiesFor("jpeg", ["camera"]),
      { camera: "Kept so the picture stays the right way up." },
    );

    expect(report.present).toEqual([
      { kind: "camera", label: "Camera", reason: "Kept so the picture stays the right way up." },
    ]);
    expect(report.removed).toEqual([]);
  });

  it("reports output-only findings as unexpected residuals without a retention reason", () => {
    const report = verifyFindings(
      [finding("owner", "Alex")],
      [finding("camera", "New camera")],
      capabilitiesFor("jpeg", ["owner", "camera"]),
      { camera: "This reason applies only to a matching source finding." },
    );

    expect(report.removed).toEqual([{ kind: "owner", label: "Owner" }]);
    expect(report.present).toEqual([{ kind: "camera", label: "Camera", unexpected: true }]);
  });

  it("does not infer removal for unsupported, incomplete, or valueless findings", () => {
    const report = verifyFindings(
      [finding("ai", "A private prompt"), finding("owner", "Private owner"), finding("owner", null)],
      [],
      [
        { format: "jpeg", kind: "ai", scope: "file", complete: false, reason: "No complete AI coverage." },
        { format: "jpeg", kind: "owner", scope: "file", complete: false, reason: "The output parse was incomplete." },
      ],
      {},
    );

    expect(report.removed).toEqual([]);
    expect(report.unchecked).toEqual([
      { kind: "ai", label: "Made with AI", reason: "No complete AI coverage." },
      { kind: "owner", label: "Owner", reason: "The output parse was incomplete." },
      { kind: "owner", label: "Owner", reason: expect.any(String) },
    ]);
  });

  it("never serializes a finding value or scope into the report", () => {
    const secret = "48.8584, 2.2945 PRIVATE LOCATION";
    const report = verifyFindings(
      [finding("location", secret)],
      [],
      [],
      {},
    );
    const serialized = JSON.stringify(report);

    expect(serialized).not.toContain(secret);
    expect(report.unchecked[0]).not.toHaveProperty("scope");
    expect(report.unchecked[0]).not.toHaveProperty("value");
    expect(report.unchecked[0]).toMatchObject({ kind: "location", label: "Location" });
  });
});

describe("PDF redaction verification", () => {
  it("does not let the same text on another page keep a selection present", () => {
    expect(verifyPdfSelections([pageSelection()], [
      { page: 1, text: "This page has different text.", complete: true },
      { page: 2, text: "Secret Name", complete: true },
    ])).toEqual({
      removed: [{ kind: "pdf-text", label: "Selected PDF text" }],
      present: [],
      unchecked: [],
    });
  });

  it("conservatively finds repeated text on the selected page despite case and whitespace changes", () => {
    const report = verifyPdfSelections(
      [pageSelection()],
      [{ page: 1, text: "A secret\n  NAME remains here", complete: true }],
    );

    expect(report.removed).toEqual([]);
    expect(report.present).toEqual([{ kind: "pdf-text", label: "Selected PDF text" }]);
  });

  it.each([
    ["an incomplete source page", pageSelection({ sourceComplete: false }), [{ page: 1, text: "Secret Name", complete: true }]],
    ["an incomplete output page", pageSelection(), [{ page: 1, text: "Secret Name", complete: false }]],
    ["a missing output page", pageSelection({ page: 3 }), [{ page: 1, text: "Secret Name", complete: true }]],
  ])("does not call a selection removed when there is %s", (_name, selection, pages) => {
    const report = verifyPdfSelections([selection], pages);
    expect(report.removed).toEqual([]);
    expect(report.unchecked).toHaveLength(1);
  });

  it("leaves picture-only areas unchecked and does not disclose their text", () => {
    const privateText = "password printed inside a scan";
    const report = verifyPdfSelections(
      [pageSelection({ text: privateText, areaOnly: true })],
      [{ page: 1, text: privateText, complete: true }],
    );

    expect(report.removed).toEqual([]);
    expect(report.unchecked[0]).toMatchObject({ kind: "pdf-picture", label: "Text in the selected picture area" });
    expect(report.unchecked[0].reason).toMatch(/OCR/i);
    expect(JSON.stringify(report)).not.toContain(privateText);
  });

  it("marks an empty-text box unchecked rather than removed", () => {
    const report = verifyPdfSelections([pageSelection({ text: "" })], [{ page: 1, text: "", complete: true }]);
    expect(report.removed).toEqual([]);
    expect(report.unchecked).toHaveLength(1);
  });

  it("turns a skipped or failed check into a private unchecked report", () => {
    const report = uncheckedReport([finding("owner", "Private owner")], [pageSelection()], "The copy was too large to check.");
    const serialized = JSON.stringify(report);

    expect(report.removed).toEqual([]);
    expect(report.unchecked).toHaveLength(2);
    expect(report.unchecked.every((item) => item.reason === "The copy was too large to check.")).toBe(true);
    expect(serialized).not.toContain("Private owner");
    expect(serialized).not.toContain("Secret Name");
  });

  it("keeps the redaction and no-OCR explanation when a picture-area check fails", () => {
    const report = uncheckedReport(
      [],
      [pageSelection({ text: "Private words in a scan", areaOnly: true })],
      "The worker did not finish.",
    );

    expect(report.unchecked[0].reason).toContain("The selected area was redacted by the operation");
    expect(report.unchecked[0].reason).toMatch(/OCR/i);
    expect(report.unchecked[0].reason).toContain("The worker did not finish.");
    expect(JSON.stringify(report)).not.toContain("Private words in a scan");
  });
});

describe("verification input size", () => {
  it("accepts inputs below and exactly at the measured maximum, but not above it", () => {
    expect(MAX_VERIFY_BYTES).toBe(10 * 1024 * 1024);
    expect(verificationEligible(MAX_VERIFY_BYTES - 1)).toBe(true);
    expect(verificationEligible(MAX_VERIFY_BYTES)).toBe(true);
    expect(verificationEligible(MAX_VERIFY_BYTES + 1)).toBe(false);
  });
});
