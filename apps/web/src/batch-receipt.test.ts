import { expect, it } from "vitest";
import { batchReceipt, receiptRow } from "./batch-receipt";

it("aggregates value-free core reports without leaking unexpected JSON fields", () => {
  const row = receiptRow(2, "written", '{"schema_version":1,"removed":[{"kind":"camera","reason":"removed","unexpected":false,"value":"secret"}],"present":[],"unchecked":[],"filename":"private.jpg"}');
  const report = batchReceipt([row, receiptRow(3, "not_created")]);
  expect(report).toMatchObject({ total: 2, written: 1, failed: 1, removed: 1, unchecked: 1 });
  expect(JSON.stringify(report)).not.toMatch(/secret|private.jpg|filename|value/);
});
it("malformed and unavailable checks stay unchecked", () => {
  expect(receiptRow(1, "written", '{"removed":"bad"}').verification.unchecked).toHaveLength(1);
});
