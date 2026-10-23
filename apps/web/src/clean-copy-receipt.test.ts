import { describe, expect, it } from "vitest";
import { makeCleanCopyReceipt } from "./clean-copy-receipt";
import { MAX_VERIFY_BYTES, type VerificationReport } from "./verification";

const report: VerificationReport = {
  removed: [{ kind: "location", label: "PRIVATE_LABEL_SENTINEL", reasonCode: "removed" }],
  present: [{ kind: "author", label: "PRIVATE_VALUE_SENTINEL", reason: "PRIVATE_REASON_SENTINEL", reasonCode: "still_present" }],
  unchecked: [{ kind: "custom", label: "PRIVATE_SCOPE_SENTINEL", reason: "PRIVATE_PATH_SENTINEL", reasonCode: "coverage_incomplete" }],
};

describe("clean-copy receipts", () => {
  it("uses the core v1 schema and fingerprints the exact output bytes", async () => {
    const receipt = await makeCleanCopyReceipt("png", new Blob(["abc"]), report);

    expect(receipt).toEqual({
      schema: "hexscope.clean-copy-receipt",
      version: 1,
      hexscope_version: expect.any(String),
      copy: {
        format: "png",
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
      },
      verification: {
        schema_version: 1,
        removed: [{ kind: "location", reason: "removed", unexpected: false }],
        present: [{ kind: "author", reason: "still_present", unexpected: false }],
        unchecked: [{ kind: "custom", reason: "coverage_incomplete", unexpected: false }],
      },
    });
  });

  it("hashes exactly at the measured limit and marks larger copies explicitly", async () => {
    const atLimit = await makeCleanCopyReceipt("png", new Blob([new Uint8Array(MAX_VERIFY_BYTES)]), report);
    expect(atLimit.copy.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(atLimit.copy.fingerprint_reason).toBeUndefined();

    const overLimit = await makeCleanCopyReceipt("png", new Blob([new Uint8Array(MAX_VERIFY_BYTES + 1)]), report);
    expect(overLimit.copy.sha256).toBeNull();
    expect(overLimit.copy.fingerprint_reason).toBe("size_limit");
    expect(overLimit.verification.unchecked).toEqual([
      { kind: "custom", reason: "coverage_incomplete", unexpected: false },
    ]);
  });

  it("never serializes report labels, prose, file names, paths, values, or scopes", async () => {
    const receipt = await makeCleanCopyReceipt("png", new Blob(["bytes"]), report);
    const text = JSON.stringify(receipt);
    for (const secret of [
      "PRIVATE_LABEL_SENTINEL",
      "PRIVATE_VALUE_SENTINEL",
      "PRIVATE_REASON_SENTINEL",
      "PRIVATE_SCOPE_SENTINEL",
      "PRIVATE_PATH_SENTINEL",
      "/Users/alice/private/photo.png",
      "secret-source-name.png",
    ]) {
      expect(text).not.toContain(secret);
    }
  });
});
