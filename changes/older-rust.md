### Fixed

- **`cargo install hexscope-cli` failed on a compiler a few months old**:
  0.7.1 needed a feature only the newest Rust accepts, so Rust 1.94 (March
  2026), for one, stopped with "`if let` guards are experimental". It builds
  again with Rust 1.88 and later, and the crates now say they need 1.88, so
  an older compiler gets a clear message instead. The prebuilt commands and
  Homebrew were not affected.
