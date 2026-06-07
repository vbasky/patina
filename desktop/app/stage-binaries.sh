#!/usr/bin/env bash
# Copy the release server + kernels into desktop/app/binaries/ with the
# target-triple suffix Tauri expects for `externalBin` sidecars.
# Run after `cargo build --release` at the repo root.
set -euo pipefail

TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(cd "$HERE/../.." && pwd)/target/release"
DST="$HERE/binaries"
EXT=""; case "${OSTYPE:-}" in msys*|cygwin*|win*) EXT=".exe" ;; esac

mkdir -p "$DST"
for b in patina patina-kernel patina-kernel-python patina-kernel-js; do
  cp "$SRC/$b$EXT" "$DST/$b-$TRIPLE$EXT"
done
echo "✓ staged sidecars for $TRIPLE → $DST"
