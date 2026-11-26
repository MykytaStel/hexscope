import { readFileSync } from "node:fs";
import { compareReports } from "./perf-report.mjs";
const args = process.argv.slice(2);
const files = args.filter(a => !a.startsWith("--"));
if (files.length !== 2 || args.some(a => a.startsWith("--") && !["--check","--json"].includes(a) && !a.startsWith("--threshold="))) {
  throw new Error("Usage: pnpm perf:compare baseline.json candidate.json [--threshold=20] [--check] [--json]");
}
const threshold = Number(args.find(a => a.startsWith("--threshold="))?.split("=")[1] ?? 20);
const result = compareReports(...files.map(f => JSON.parse(readFileSync(f,"utf8"))), { maxRegressionPct: threshold });
if (args.includes("--json")) console.log(JSON.stringify(result,null,2));
else {
  console.table(result.rows.map(r => ({ case:r.id, metric:r.metric, baseline:r.baseline, candidate:r.candidate, delta:r.deltaPct === null ? "n/a" : r.deltaPct.toFixed(1)+"%", regression:r.regression })));
  console.log(result.regressions + " regression signals (threshold " + threshold + "%; inspect variance before drawing conclusions)");
}
if (!args.includes("--json")) {
  for (const row of result.unavailable) {
    console.log(`Unavailable evidence: ${row.id}/${row.metric} (missing baseline ${row.baselineMissing}/${row.repeats}, candidate ${row.candidateMissing}/${row.repeats})`);
  }
  if (result.availabilityChanges.length) console.log("Availability changed; collect a comparable baseline before accepting these measurements.");
}
if (args.includes("--check") && (result.regressions || result.availabilityChanges.length)) process.exitCode = 1;
