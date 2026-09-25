#!/bin/sh
# Builds the WebAssembly sample: a small Rust module that counts words.
# Built as most Rust modules are, it keeps its function names and the
# producers section, and its panic messages carry the path of the source
# file. The path is rewritten to a sample user's, so the sample shows what a
# real build gives away without giving away anyone's.
#
#   sh scripts/make-sample-wasm.sh
set -e
cd "$(dirname "$0")/sample-wasm"
rustc --edition 2024 --target wasm32-unknown-unknown --crate-type cdylib \
  -C opt-level=s -C panic=abort -C strip=debuginfo \
  --remap-path-prefix="$PWD=/Users/sample/projects/hello" \
  "$PWD/hello.rs" -o ../../apps/web/public/samples/hello.wasm
ls -l ../../apps/web/public/samples/hello.wasm
