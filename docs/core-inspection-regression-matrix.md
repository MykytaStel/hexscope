# Core inspection regression matrix — 0.24-D

This first shared regression gate complements format-specific tests.
It checks recognized damaged framing for PNG, JPEG, GIF and WebP, plus ZIP
and SVG when their respective features are enabled. It also checks an unknown
format, a known valid JPEG and aggregate coverage counts.

```sh
cargo test -p hexscope-core --all-features --test inspection_regression_matrix
cargo test -p hexscope-core --no-default-features --test inspection_regression_matrix
```

The matrix is deliberately small: no claim of exhaustive parser validation,
anonymization, decoder correctness or real-world corpus coverage. It must pass
alongside existing format, property, fuzz, CLI, browser and independent
verification gates. Native and WASM parity and physical-device measurements
are separate evidence requirements.

The workspace version stays at 0.23.0 until the 0.24 release gate holds.
This record describes proposed tests, not executed results.
