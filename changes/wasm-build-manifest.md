### Changed

- Centralize the existing five WASM build specifications and compressed budgets, sharing them across production and diagnostic tooling.
- Use locked Cargo resolution for production WASM builds; preserve feature sets, output names and size limits.
