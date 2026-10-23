import { expect, it } from "vitest";
import { batchCleanCopyReceipt, batchReceipt, copyReceiptRow, receiptRow } from "./batch-receipt";
import { makeCleanCopyReceipt } from "./clean-copy-receipt";

it("aggregates value-free core reports without leaking unexpected JSON fields", () => {
  const row = receiptRow(2, "written", '{"schema_version":1,"removed":[{"kind":"camera","reason":"removed","unexpected":false,"value":"secret"}],"present":[],"unchecked":[],"filename":"private.jpg"}');
  const report = batchReceipt([row, receiptRow(3, "not_created")]);
  expect(report).toMatchObject({ total: 2, written: 1, failed: 1, removed: 1, unchecked: 1 });
  expect(JSON.stringify(report)).not.toMatch(/secret|private.jpg|filename|value/);
});
it("malformed and unavailable checks stay unchecked", () => {
  expect(receiptRow(1, "written", '{"removed":"bad"}').verification.unchecked).toHaveLength(1);
  const privatePath = receiptRow(1, "written", JSON.stringify({
    schema_version: 1,
    removed: [{ kind: "../../secret/photo.jpg", reason: "removed", unexpected: false }],
    present: [],
    unchecked: [],
  }));
  expect(privatePath.verification.unchecked).toHaveLength(1);
  expect(JSON.stringify(privatePath)).not.toContain("secret");
});

it("keeps the default v1 receipt contract exact", () => {
  const row = receiptRow(1, "written", '{"schema_version":1,"removed":[],"present":[],"unchecked":[]}');
  expect(batchReceipt([row])).toEqual({
    schema: "hexscope.batch-receipt",
    version: 1,
    total: 1,
    written: 1,
    failed: 0,
    removed: 0,
    present: 0,
    unchecked: 0,
    files: [{
      index: 1,
      operation_state: "written",
      verification: { schema_version: 1, removed: [], present: [], unchecked: [] },
    }],
  });
});

it("builds the fingerprinted batch envelope from indexed single-copy receipts", async () => {
  const verification = {
    removed: [{ kind: "location", label: "private", reasonCode: "removed" as const }],
    present: [],
    unchecked: [],
  };
  const receipt = await makeCleanCopyReceipt("jpeg", new Blob(["copy bytes"]), verification);
  const report = batchCleanCopyReceipt([
    copyReceiptRow(7, "written", receipt),
    copyReceiptRow(9, "not_created"),
  ]);

  expect(report).toMatchObject({
    schema: "hexscope.batch-clean-copy-receipt",
    version: 1,
    total: 2,
    written: 1,
    failed: 1,
    removed: 1,
    present: 0,
    unchecked: 0,
    files: [
      { index: 7, operation_state: "written", receipt },
      { index: 9, operation_state: "not_created", receipt: null },
    ],
  });
  expect(JSON.stringify(report)).not.toContain("private");
});

it("explains when an opt-in batch copy could not be fingerprinted", () => {
  expect(copyReceiptRow(4, "written")).toEqual({
    index: 4,
    operation_state: "written",
    receipt: null,
    fingerprint_reason: "unavailable",
  });
  expect(copyReceiptRow(5, "not_created")).toEqual({
    index: 5,
    operation_state: "not_created",
    receipt: null,
  });
});
