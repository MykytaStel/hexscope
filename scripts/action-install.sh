#!/usr/bin/env bash
# Puts the hexscope command in "$1/bin" for the GitHub Action. At a released
# version (uses: MykytaStel/hexscope@v0.5.1) it is the release's own build for
# this runner, checked against its checksum and the signed record of the
# workflow that built it. Anywhere else — a branch, a commit, a version from
# before releases carried builds, a build that does not check out — it is
# compiled from the Action's own copy of the repository, as before.
set -euo pipefail

root=$1
case "${RUNNER_OS:-}/${RUNNER_ARCH:-}" in
  Linux/X64) target=x86_64-unknown-linux-musl ;;
  Linux/ARM64) target=aarch64-unknown-linux-musl ;;
  macOS/ARM64) target=aarch64-apple-darwin ;;
  macOS/X64) target=x86_64-apple-darwin ;;
  Windows/X64) target=x86_64-pc-windows-msvc ;;
  *) target= ;;
esac
exe=hexscope
[ "${RUNNER_OS:-}" = Windows ] && exe=hexscope.exe

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

released() {
  [[ -n "$target" && "${ACTION_REF:-}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || return 1
  local name="hexscope-${ACTION_REF#v}-$target.tar.gz"
  local url="https://github.com/$ACTION_REPOSITORY/releases/download/$ACTION_REF/$name"
  local tmp
  tmp=$(mktemp -d)
  curl -fsSL --retry 3 -o "$tmp/$name" "$url" && curl -fsSL --retry 3 -o "$tmp/$name.sha256" "$url.sha256" || {
    echo "::notice::hexscope $ACTION_REF has no build for $target; building it."
    return 1
  }
  if [ "$(sha256 "$tmp/$name")" != "$(cut -d' ' -f1 "$tmp/$name.sha256")" ]; then
    echo "::warning::The downloaded hexscope does not match its checksum; building it instead."
    return 1
  fi
  # Signed by GitHub for the workflow that built it in this repository.
  if command -v gh >/dev/null && ! gh attestation verify "$tmp/$name" --repo "$ACTION_REPOSITORY" >/dev/null 2>&1; then
    echo "::warning::The downloaded hexscope has no attestation from $ACTION_REPOSITORY; building it instead."
    return 1
  fi
  mkdir -p "$root/bin"
  tar -xzf "$tmp/$name" -C "$root/bin" "./$exe"
  echo "hexscope $ACTION_REF for $target, as released."
}

released || cargo install --locked --quiet --root "$root" --path "$GITHUB_ACTION_PATH/crates/hexscope-cli"
"$root/bin/$exe" --version
