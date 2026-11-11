// Reproducible same-origin OCR assets from the lockfile, never a runtime CDN.
import { createRequire } from "node:module";
import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const require = createRequire(join(root, "apps/web/package.json"));
const engine = dirname(require.resolve("tesseract.js/package.json"));
const core = dirname(createRequire(join(engine, "package.json")).resolve("tesseract.js-core/package.json"));
const out = join(root, "apps/web/public/ocr-v7-data1");
mkdirSync(out, { recursive: true });
for (const name of readdirSync(core).filter((name) => name.endsWith(".wasm.js"))) {
  copyFileSync(join(core, name), join(out, name));
}
for (const name of ["worker.min.js", "worker.min.js.LICENSE.txt"]) copyFileSync(join(engine, "dist", name), join(out, name));
for (const lang of ["eng", "ukr"]) {
  const data = dirname(require.resolve(`@tesseract.js-data/${lang}/package.json`));
  copyFileSync(join(data, "4.0.0", `${lang}.traineddata.gz`), join(out, `${lang}.traineddata.gz`));
}
copyFileSync(join(core, "LICENSE"), join(out, "CORE-LICENSE.txt"));
