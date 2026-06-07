#!/usr/bin/env bash
# Fetch a relocatable CPython (python-build-standalone) for a self-contained
# Patina desktop app, so the end user needs no Python install. Run once per
# target platform.
#
# Output: <out>/python/  — a relocatable interpreter (bin/, lib/, …). Launch the
# app with PATINA_PYTHON pointing at it; the kernel sets PYTHONHOME accordingly
# and installs the batteries (numpy/pandas/matplotlib) into ~/.patina/pyenv.
#
#   PATINA_PYTHON=<out>/python ./patina
#
# Fully host-free note: with the default build, the kernel still loads the
# *host* libpython at runtime (it was linked against it). For true independence,
# rebuild the kernel against this interpreter so it links the bundled libpython:
#
#   PYO3_PYTHON=<out>/python/bin/python3 cargo build --release -p patina-kernel-python
#
# then ship <out>/python and set PATINA_PYTHON (and, per-OS, the library path:
# macOS DYLD_LIBRARY_PATH=<out>/python/lib, Linux LD_LIBRARY_PATH=<out>/python/lib).
set -euo pipefail

PY_VERSION="${PY_VERSION:-3.14}"            # major.minor to match the kernel
OUT="${OUT:-$(cd "$(dirname "$0")" && pwd)/bundle}"
REPO="astral-sh/python-build-standalone"

# Map the host to a python-build-standalone target triple.
os="$(uname -s)"; arch="$(uname -m)"
case "$os/$arch" in
  Darwin/arm64)   TRIPLE="aarch64-apple-darwin" ;;
  Darwin/x86_64)  TRIPLE="x86_64-apple-darwin" ;;
  Linux/x86_64)   TRIPLE="x86_64-unknown-linux-gnu" ;;
  Linux/aarch64)  TRIPLE="aarch64-unknown-linux-gnu" ;;
  *) echo "Unsupported host $os/$arch — see github.com/$REPO/releases" >&2; exit 1 ;;
esac
echo "▶ CPython $PY_VERSION ($TRIPLE) → $OUT/python"

# Find the latest 'install_only' asset for this version + triple.
api="https://api.github.com/repos/$REPO/releases/latest"
url="$(curl -fsSL "$api" \
  | grep -o "https://[^\"]*cpython-${PY_VERSION}\.[0-9]*+[0-9]*-${TRIPLE}-install_only\.tar\.gz" \
  | head -1)"
[ -n "$url" ] || { echo "no matching asset for $PY_VERSION/$TRIPLE" >&2; exit 1; }
echo "  downloading $(basename "$url")"

mkdir -p "$OUT"; rm -rf "$OUT/python"
curl -fsSL "$url" | tar -xz -C "$OUT"     # extracts to $OUT/python
"$OUT/python/bin/python3" --version

echo "✓ Python bundle ready: $OUT/python"
echo "  Launch with:  PATINA_PYTHON=$OUT/python ./patina"
