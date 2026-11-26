/** Versioned numeric evidence; comparison deliberately refuses different workloads. */
export function median(values) {
  if (!values.length || values.some(v => !Number.isFinite(v))) throw new Error("Expected finite samples");
  const ordered = [...values].sort((a,b) => a-b);
  const i = Math.floor(ordered.length / 2);
  return ordered.length % 2 ? ordered[i] : (ordered[i-1] + ordered[i]) / 2;
}

function validate(report) {
  if (report?.schema !== "hexscope.performance" || report.version !== 1 || !["wasm","browser"].includes(report.kind) ||
      !report.environment || !Number.isInteger(report.environment.repeats) || report.environment.repeats < 1 ||
      !Array.isArray(report.cases) || !report.cases.length) throw new Error("Unsupported performance report");
  const ids = new Set();
  for (const c of report.cases) {
    if (typeof c.id !== "string" || ids.has(c.id) || !Number.isSafeInteger(c.inputBytes) || c.inputBytes < 0 ||
        !/^[a-f0-9]{64}$/.test(c.inputSha256) || !Array.isArray(c.metrics) || c.metrics.length !== report.environment.repeats) throw new Error("Invalid performance case");
    ids.add(c.id);
    const keys = Object.keys(c.metrics[0]).sort().join(",");
    if (!keys || c.metrics.some(m => Object.keys(m).sort().join(",") !== keys || Object.values(m).some(v => v !== null && (!Number.isFinite(v) || v < 0)))) throw new Error("Invalid metric samples");
  }
}
const stable = o => JSON.stringify(Object.entries(o).sort(([a],[b]) => a.localeCompare(b)));
export function compareReports(baseline, candidate, { maxRegressionPct = 20 } = {}) {
  validate(baseline); validate(candidate);
  if (!Number.isFinite(maxRegressionPct) || maxRegressionPct < 0) throw new Error("Invalid regression threshold");
  if (baseline.kind !== candidate.kind || stable(baseline.environment) !== stable(candidate.environment)) throw new Error("Incompatible benchmark environments");
  const current = new Map(candidate.cases.map(c => [c.id, c]));
  if (current.size !== baseline.cases.length) throw new Error("Different benchmark cases");
  const rows = [];
  const unavailable = [];
  const availabilityChanges = [];
  for (const old of baseline.cases) {
    const next = current.get(old.id);
    if (!next || old.inputBytes !== next.inputBytes || old.inputSha256 !== next.inputSha256) throw new Error("Different workload: " + old.id);
    const keys = Object.keys(old.metrics[0]).sort();
    if (keys.join(",") !== Object.keys(next.metrics[0]).sort().join(",")) throw new Error("Different metrics: " + old.id);
    for (const metric of keys) {
      const before = old.metrics.map(m => m[metric]), after = next.metrics.map(m => m[metric]);
      if (metric === "cleanProduced" && (new Set([...before,...after]).size !== 1 || ![0,1].includes(before[0]))) throw new Error("Different or unstable cleaner outcome: " + old.id);
      const baselineMissing = before.filter(v => v === null).length;
      const candidateMissing = after.filter(v => v === null).length;
      if (baselineMissing || candidateMissing) {
        const detail = { id: old.id, metric, baselineMissing, candidateMissing, repeats: before.length };
        unavailable.push(detail);
        // Even a change from partial to complete evidence is a contract
        // change; collect a new baseline rather than comparing unlike samples.
        if (baselineMissing !== candidateMissing) availabilityChanges.push(detail);
        rows.push({ id: old.id, metric, baseline: null, candidate: null, deltaPct: null, regression: false, ...detail });
        continue;
      }
      const b = median(before), c = median(after);
      const deltaPct = b === 0 ? (c === 0 ? 0 : null) : (c-b)/b*100;
      // Counts of worker generations are diagnostic, not universally lower-is-better.
      const diagnostic = ["workerCount","terminatedWorkers","cleanProduced","copyBytes"].includes(metric);
      const regression = !diagnostic && (deltaPct === null ? c > b : deltaPct > maxRegressionPct);
      rows.push({ id: old.id, metric, baseline: b, candidate: c, deltaPct, regression });
    }
  }
  return { rows, regressions: rows.filter(r => r.regression).length, unavailable, availabilityChanges };
}
