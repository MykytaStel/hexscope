// Single source for production and diagnostic WASM build identities.
// Budgets are existing compressed limits, not targets to raise.
export const wasmFamilies = Object.freeze({
  media: Object.freeze({ crate: "hexscope-wasm", features: "hdr,svg,strings", budget: 178000 }),
  pdf: Object.freeze({ crate: "hexscope-wasm", features: "pdf", budget: 229000 }),
  docs: Object.freeze({ crate: "hexscope-wasm", features: "office,mail,module", budget: 240000 }),
  audio: Object.freeze({ crate: "hexscope-wasm", features: "audio", budget: 168000 }),
  raster: Object.freeze({ crate: "hexscope-raster", features: "web", budget: 216000 }),
});

export function familyNames(requested) {
  if (!requested.length || requested.some(f => !Object.hasOwn(wasmFamilies, f)) || new Set(requested).size !== requested.length) {
    throw new Error("Expected unique WASM families: " + Object.keys(wasmFamilies).join(", "));
  }
  return requested;
}

export function buildArguments(family) {
  const { crate, features } = wasmFamilies[family];
  return ["build", "--locked", "-p", crate, "--target", "wasm32-unknown-unknown", "--profile", "wasm", "--no-default-features", "--features", features];
}

export function artifactPattern(family, diagnostic = false) {
  // Bindgen's diagnostic artifacts are un-hashed. Production Vite assets
  // are hashed; retain the raster build's historical naming allowance.
  if (diagnostic) return new RegExp("^hexscope_" + family + "_bg\\.wasm$");
  return family === "raster" ? /^hexscope_raster.*\.wasm$/ : new RegExp("^hexscope_" + family + "_bg-.*\\.wasm$");
}
