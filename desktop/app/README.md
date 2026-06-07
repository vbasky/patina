# Patina desktop (Tauri)

A thin [Tauri](https://tauri.app) shell that turns Patina into a double-click app.
On launch it spawns the bundled `patina` server (with the kernels alongside it as
sidecars) and opens a native webview at the server's localhost URL with a fresh
auth key. The UI is still served by `patina`; Tauri provides the window + process
glue and the installer/bundle.

> **Status: scaffold.** This was authored but not built in CI — building a Tauri
> app needs the Tauri CLI and a system webview, and bundles are per-platform.
> Treat the steps below as the build recipe; adjust config for your Tauri 2.x
> point release if needed.

## Prerequisites

- Rust + the Tauri CLI: `cargo install tauri-cli --version "^2"`
- Node.js (to build the UI)
- A system webview (WebView2 on Windows; WebKit on macOS/Linux — see Tauri's
  prerequisites page)

## Build

```bash
# from the repo root
cd browser/ui && npm install && ./build.sh && cd ../..   # 1. build the UI
cargo build --release                                    # 2. build server + kernels
desktop/app/stage-binaries.sh                            # 3. copy them as triple-suffixed sidecars
cargo tauri icon browser/ui/public/patina.svg \
  --output desktop/app/icons                             # 4. generate app icons (once)

cd desktop/app && cargo tauri build                      # 5. produce the installer/bundle
```

The result is a native installer (`.dmg`/`.app`, `.msi`/`.exe`, `.deb`/AppImage)
under `desktop/app/target/release/bundle/`.

## Self-contained (no host Rust/Python)

Ship the bundled toolchain / interpreter (see `../build-rust-bundle.sh` and
`../build-python-bundle.sh`) as Tauri **resources**, then in `src/main.rs` set
`PATINA_TOOLCHAIN` / `PATINA_PYTHON` (resolved via `app.path().resource_dir()`)
in the sidecar's environment before `.spawn()`. The kernels pick those up
automatically. Without that, the app relies on the host's Rust/Python toolchains
(the JavaScript kernel always works standalone).

## Files

- `src/main.rs` — spawn the server sidecar, wait for the port, open the webview.
- `tauri.conf.json` — bundle config + `externalBin` sidecars.
- `capabilities/default.json` — permission to spawn the `patina` sidecar.
- `stage-binaries.sh` — stage `target/release` binaries as Tauri sidecars.
