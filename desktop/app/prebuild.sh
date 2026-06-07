#!/usr/bin/env bash
# Tauri `beforeBuildCommand`: produce everything the bundled app needs so the
# end user needs no host Rust/Python and pays no first-run download.
#
#   1. build the UI (embedded into the patina server)
#   2. build the server + kernels (release)
#   3. stage them as Tauri sidecars
#   4. precompile the batteries into a relocatable, *vendored* Rust toolchain
#      bundle (offline; the crates are present, no crates.io needed)
#   5. fetch a relocatable Python
#   6. copy the runtime bundle into resources/ so Tauri ships it
#
# NOTE: the compiled `target/` cache is NOT shipped — cargo keys its cache on
# absolute paths, so it wouldn't hit on the user's machine. The app warms the
# compile once on first launch into a writable ~/.patina/runtime (offline, using
# the vendored crates), which is fast and one-time. See src/main.rs.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
DESKTOP="$HERE/.."

echo "▶ [1/6] building UI"
( cd "$ROOT/browser/ui" && npm install --no-audit --no-fund && ./build.sh )

echo "▶ [2/6] building server + kernels (release)"
( cd "$ROOT" && cargo build --release )

echo "▶ [3/6] staging sidecars"
"$HERE/stage-binaries.sh" release

# The toolchain bundle + Python are expensive to produce (rustup install,
# vendoring, a full polars compile). They don't change between dev iterations, so
# build them only when missing. Force a rebuild with PATINA_REBUILD_BUNDLE=1.
if [[ "${PATINA_REBUILD_BUNDLE:-0}" == "1" ]]; then
  rm -rf "$DESKTOP/bundle"
fi

if [[ ! -x "$DESKTOP/bundle/toolchain/bin/cargo" ]]; then
  echo "▶ [4/6] precompiling + vendoring Rust batteries (offline)"
  "$DESKTOP/build-rust-bundle.sh" --offline
else
  echo "▶ [4/6] Rust bundle present — skipping (PATINA_REBUILD_BUNDLE=1 to force)"
fi

if [[ ! -d "$DESKTOP/bundle/python" ]]; then
  echo "▶ [5/6] fetching relocatable Python"
  "$DESKTOP/build-python-bundle.sh"
else
  echo "▶ [5/6] Python present — skipping"
fi

echo "▶ [6/6] syncing runtime into resources/"
for part in toolchain cargo python; do
  rm -rf "$HERE/resources/$part"
  cp -a "$DESKTOP/bundle/$part" "$HERE/resources/$part"
done

echo "✓ prebuild complete → $HERE/resources"
