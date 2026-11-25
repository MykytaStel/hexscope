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
