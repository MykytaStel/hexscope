import { test } from "node:test";
import assert from "node:assert/strict";
import { median, compareReports } from "./perf-report.mjs";
const report = (samples, environment = { repeats: samples.length, profile: "desktop" }) => ({
  schema: "hexscope.performance", version: 1, kind: "browser", environment,
  cases: [{ id: "movie", inputBytes: 10, inputSha256: "a".repeat(64), metrics: samples.map(readyMs => ({ readyMs })) }],
});
test("medians are order independent and even samples use the midpoint", () => {
  assert.equal(median([9,1,5]),5);
  assert.equal(median([8,2]),5);
  assert.throws(() => median([]));
});
test("comparison reports regression and uses median rather than one outlier", () => {
  const result = compareReports(report([10,10,1000]), report([15,15,15]));
  assert.equal(result.rows[0].deltaPct,50);
  assert.equal(result.regressions,1);
});
test("refuses different environment, workload, missing metrics and duplicate cases", () => {
  const a = report([1,2]);
  assert.throws(() => compareReports(a, report([1,2], { repeats:2, profile:"phone" })), /environment/);
  const b = structuredClone(a);
  b.cases[0].inputSha256 = "b".repeat(64);
  assert.throws(() => compareReports(a,b), /workload/);
  const c = structuredClone(a);
  c.cases[0].metrics[1] = {};
  assert.throws(() => compareReports(a,c), /metric/);
  const d = structuredClone(a);
  d.cases.push(d.cases[0]);
  assert.throws(() => compareReports(a,d), /case/);
});
test("zero baselines and unavailable measurements remain explicit", () => {
  assert.equal(compareReports(report([0]),report([5])).regressions,1);
  assert.equal(compareReports(report([null]),report([5])).rows[0].deltaPct,null);
});

test("rejects changed cleaner outcomes but treats copy size as descriptive", () => {
  const a = report([1,2]);
  a.cases[0].metrics = [{cleanProduced:1,copyBytes:2},{cleanProduced:1,copyBytes:2}];
  const b = structuredClone(a);
  b.cases[0].metrics.forEach(m => m.copyBytes = 5);
  assert.equal(compareReports(a,b).regressions,0);
  b.cases[0].metrics.forEach(m => m.cleanProduced = 0);
  assert.throws(() => compareReports(a,b), /cleaner outcome/);
});

test("availability changes cannot silently pass an evidence gate", () => {
  const lost = compareReports(report([10, 11, 12]), report([10, null, 12]));
  assert.equal(lost.regressions, 0);
  assert.deepEqual(lost.availabilityChanges, [{ id: "movie", metric: "readyMs", baselineMissing: 0, candidateMissing: 1, repeats: 3 }]);
  const gained = compareReports(report([null]), report([5]));
  assert.equal(gained.availabilityChanges.length, 1);
  const unchanged = compareReports(report([null]), report([null]));
  assert.equal(unchanged.availabilityChanges.length, 0);
  assert.equal(unchanged.unavailable.length, 1);
});

test("partial samples are not represented as comparable medians", () => {
  const result = compareReports(report([1, null, 3]), report([2, 3, null]));
  assert.equal(result.availabilityChanges.length, 0);
  assert.equal(result.unavailable.length, 1);
  assert.equal(result.rows[0].baseline, null);
  assert.equal(result.rows[0].candidate, null);
});
