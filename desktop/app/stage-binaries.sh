#!/usr/bin/env bash
# Copy the server + kernels into desktop/app/binaries/ with the target-triple
# suffix Tauri expects for `externalBin` sidecars.
#
#   stage-binaries.sh           # from target/release (for `cargo tauri build`)
#   stage-binaries.sh debug     # from target/debug   (for `cargo tauri dev`)
#
# Run after building the workspace in the matching profile at the repo root.
set -euo pipefail

PROFILE="${1:-release}"
TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(cd "$HERE/../.." && pwd)/target/$PROFILE"
DST="$HERE/binaries"
EXT=""; case "${OSTYPE:-}" in msys*|cygwin*|win*) EXT=".exe" ;; esac

mkdir -p "$DST"
for b in patina patina-kernel patina-kernel-python patina-kernel-js; do
  cp "$SRC/$b$EXT" "$DST/$b-$TRIPLE$EXT"
done
echo "✓ staged sidecars for $TRIPLE → $DST"
