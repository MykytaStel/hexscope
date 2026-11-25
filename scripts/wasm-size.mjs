// Same gzip command and byte budgets on macOS and GitHub Actions.
import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = fileURLToPath(new URL("../apps/web/dist/assets/", import.meta.url));
const assets = readdirSync(dir).sort();
const groups = [
  ["media", /^hexscope_media_bg-.*\.wasm$/, 178000],
  ["pdf", /^hexscope_pdf_bg-.*\.wasm$/, 229000],
  ["docs", /^hexscope_docs_bg-.*\.wasm$/, 240000],
  ["audio", /^hexscope_audio_bg-.*\.wasm$/, 168000],
  ["raster", /^hexscope_raster.*\.wasm$/, 216000],
];
function measure(name, file, budget) {
  const path = join(dir, file);
  const gzipBytes = execFileSync("gzip", ["-9c", path], { maxBuffer: 16 * 1024 * 1024 }).length;
  return { name, file, rawBytes: statSync(path).size, gzipBytes, budget, headroom: budget - gzipBytes };
}
const rows = groups.map(([name, pattern, budget]) => {
  const files = assets.filter((file) => pattern.test(file));
  if (files.length !== 1) throw new Error(`Expected exactly one ${name} WASM asset, got ${files.length}`);
  return measure(name, files[0], budget);
});
const explanations = assets.filter((file) => /^explain-.*\.js$/.test(file));
if (explanations.length === 0) throw new Error("No explanation assets found");
for (const file of explanations) rows.push(measure(file, file, 34000));
if (process.argv.includes("--json")) console.log(JSON.stringify(rows, null, 2));
else for (const row of rows) {
  console.log(`${row.name}: ${row.gzipBytes} / ${row.budget} gzip bytes (headroom ${row.headroom}, raw ${row.rawBytes})`);
}
if (process.argv.includes("--check") && rows.some((row) => row.headroom < 0)) process.exitCode = 1;
