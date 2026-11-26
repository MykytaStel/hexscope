import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { wasmFamilies, familyNames, buildArguments } from "./wasm-build-manifest.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const requested = familyNames(process.argv.slice(2));
const target = resolve(root, "target");
const env = { ...process.env, CARGO_TARGET_DIR: target, RUSTFLAGS: `--remap-path-prefix=${process.env.HOME ?? root}=~` };
for (const family of requested) {
  const { crate } = wasmFamilies[family];
  const artifact = resolve(target, "wasm32-unknown-unknown/wasm", crate.replaceAll("-", "_") + ".wasm");
  const out = resolve(root, "apps/web/src/wasm-" + family);
  execFileSync("cargo", buildArguments(family), { cwd: root, env, stdio: "inherit" });
  execFileSync("wasm-bindgen", [artifact, "--target", "web", "--remove-name-section", "--remove-producers-section", "--out-dir", out, "--out-name", "hexscope_" + family], { cwd: root, stdio: "inherit" });
}
