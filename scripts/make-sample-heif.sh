#!/bin/sh
# Writes HEIF fixtures from the sample photo with macOS sips, which encodes
# with Apple's own HEIF encoder: a small HEIC, a 4032x3024 HEIC that the
# encoder grids into tiles as an iPhone does, and an AVIF. The EXIF, GPS
# included, is carried over from the JPEG. The gridded one is also the web
# app's HEIC sample.
#
#   sh scripts/make-sample-heif.sh
set -e
here=$(cd "$(dirname "$0")" && pwd)
src="$here/../apps/web/public/samples/photo.jpg"
out="$here/../crates/hexscope-core/tests/fixtures"
tmp=$(mktemp -d)
sips -s format heic "$src" --out "$out/photo.heic" >/dev/null
sips -s format avif "$src" --out "$out/photo.avif" >/dev/null
sips -z 3024 4032 "$src" --out "$tmp/big.jpg" >/dev/null
sips -s format heic "$tmp/big.jpg" --out "$out/photo-grid.heic" >/dev/null
rm -r "$tmp"
cp "$out/photo-grid.heic" "$here/../apps/web/public/samples/photo.heic"
ls -la "$out"/photo.heic "$out"/photo.avif "$out"/photo-grid.heic
