#!/usr/bin/env bash
# Assemble a relocatable Rust toolchain bundle for a self-contained Patina
# desktop app, so the end user needs no Rust install. Run once per target
# platform on a machine that already has `rustup`.
#
# Output layout (point the kernel at it with PATINA_TOOLCHAIN=<out>/toolchain;
# the sibling cargo/ and target/ are picked up automatically):
#
#   <out>/toolchain/   relocatable rustc + cargo + std (+ rust-lld)
#   <out>/cargo/       CARGO_HOME: config.toml (+ vendored registry, if offline)
#   <out>/target/      prewarmed build cache for the batteries crates
#
# Modes:
#   online  (default) — host needs no Rust, but :dep still fetches from crates.io
#                       over the network; the batteries crates are prebuilt.
#   offline (--offline) — vendors a fixed crate set and sets net.offline, so the
#                         app works with no network — but ONLY the vendored
#                         crates are available (Rust Playground style).
set -euo pipefail

RUST_VERSION="${RUST_VERSION:-1.96.0}"      # pin to match the dev toolchain
OUT="${OUT:-$(cd "$(dirname "$0")" && pwd)/bundle}"
OFFLINE=0
[[ "${1:-}" == "--offline" ]] && OFFLINE=1

# Versions must match the kernel's batteries preload (kernel/src/executor.rs).
BATTERIES=(
  'ndarray = "0.16"'
  'plotters = { version = "0.3", features = ["evcxr"] }'
  'polars = { version = "0.46", features = ["fmt", "lazy"] }'
)

HOST_TRIPLE="$(rustc -vV | sed -n 's/host: //p')"
echo "▶ Rust $RUST_VERSION for $HOST_TRIPLE → $OUT (offline=$OFFLINE)"
rm -rf "$OUT"; mkdir -p "$OUT/cargo"

# 1. Relocatable toolchain (minimal profile = rustc + cargo + std; ships rust-lld)
rustup toolchain install "$RUST_VERSION" --profile minimal --no-self-update >/dev/null
SYSROOT="$(rustup run "$RUST_VERSION" rustc --print sysroot)"
cp -a "$SYSROOT" "$OUT/toolchain"
echo "  toolchain: $(du -sh "$OUT/toolchain" | cut -f1)"

# 2. A throwaway crate that depends on the batteries, used to vendor + prewarm.
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/src"; : > "$WORK/src/lib.rs"
{
  echo '[package]'; echo 'name = "patina-batteries"'; echo 'version = "0.0.0"'
  echo 'edition = "2021"'; echo; echo '[dependencies]'
  printf '%s\n' "${BATTERIES[@]}"
} > "$WORK/Cargo.toml"

# 3. cargo config: drop debug info, and (offline) replace crates-io with a vendor dir.
CFG="$OUT/cargo/config.toml"
{
  echo '[build]'
  echo 'rustflags = ["-Cdebuginfo=0"]'
  echo
  echo '# Linker: prefer the bundled rust-lld where the platform supports it.'
  echo '# macOS still needs the system linker/SDK for native frameworks — the'
  echo '# fully-no-host story there requires a bundled sysroot (see README).'
} > "$CFG"

export CARGO_HOME="$OUT/cargo"
export CARGO_TARGET_DIR="$OUT/target"
export PATH="$OUT/toolchain/bin:$PATH"

if [[ "$OFFLINE" == "1" ]]; then
  echo "▶ Vendoring crates (offline mode)…"
  ( cd "$WORK" && cargo generate-lockfile )
  ( cd "$WORK" && cargo vendor "$OUT/cargo/vendor" ) >> "$CFG"
  { echo; echo '[net]'; echo 'offline = true'; } >> "$CFG"
fi

# 4. Prewarm: compile the batteries once into the bundled target cache.
echo "▶ Prewarming build cache (compiles polars/plotters/ndarray — slow once)…"
( cd "$WORK" && cargo build --release )

echo "✓ Bundle ready: $OUT"
echo "  Launch with:  PATINA_TOOLCHAIN=$OUT/toolchain ./patina"
