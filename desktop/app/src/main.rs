//! Patina desktop shell.
//!
//! A thin Tauri wrapper: on startup it launches the `patina` server (bundled as
//! a Tauri sidecar, with the language kernels alongside it), waits for it to
//! accept connections, then opens a native webview window pointing at the
//! server's localhost URL with a freshly generated auth key. The entire UI is
//! still served by `patina` — Tauri just provides the window and process glue.
//!
//! If a relocatable Rust toolchain and/or Python ship with the app as bundle
//! resources (a `toolchain/` and `python/` dir — see ../build-*.sh and add them
//! to `tauri.conf.json` `bundle.resources`), they're auto-detected here and
//! passed to the server via `PATINA_TOOLCHAIN` / `PATINA_PYTHON`, so the app
//! needs no Rust/Python on the host. When absent, the host toolchains are used.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::Command;

const PORT: u16 = 4050;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // A per-launch auth key, handed to both the server and the webview URL.
            let key = uuid::Uuid::new_v4().simple().to_string();

            // The `patina` server sidecar. The kernel sidecars sit next to it, so
            // the server's `locate_kernel` finds them automatically; env we set
            // here is inherited by those kernel child processes.
            let mut cmd = app
                .shell()
                .sidecar("patina")?
                .args(["--port", &PORT.to_string(), "--key", &key]);
            cmd = with_bundled_runtimes(app, cmd);

            let (_rx, _child) = cmd.spawn()?;

            // Wait until the server is accepting connections, then show the window.
            wait_for_port(PORT, Duration::from_secs(30));
            let url = format!("http://127.0.0.1:{PORT}?k={key}");
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url.parse()?))
                .title("Patina")
                .inner_size(1280.0, 860.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Patina desktop app");
}

/// Point the server (and thus its kernels) at any bundled Rust toolchain /
/// Python shipped as resources. No-op when they aren't bundled.
fn with_bundled_runtimes(app: &App, mut cmd: Command) -> Command {
    if let Some(toolchain) = find_resource(app, "toolchain") {
        cmd = cmd.env("PATINA_TOOLCHAIN", toolchain.to_string_lossy().to_string());
    }
    if let Some(python) = find_resource(app, "python") {
        cmd = cmd.env("PATINA_PYTHON", python.to_string_lossy().to_string());
        // Ensure the bundled libpython is the one loaded by the Python kernel.
        let lib = python.join("lib");
        if lib.is_dir() {
            let lib = lib.to_string_lossy().to_string();
            #[cfg(target_os = "macos")]
            {
                cmd = cmd.env("DYLD_LIBRARY_PATH", prepend_path("DYLD_LIBRARY_PATH", &lib));
            }
            #[cfg(target_os = "linux")]
            {
                cmd = cmd.env("LD_LIBRARY_PATH", prepend_path("LD_LIBRARY_PATH", &lib));
            }
        }
    }
    cmd
}

/// Locate a bundled runtime directory among the layouts Tauri may use for
/// resources (`<resources>/<name>` or `<resources>/resources/<name>`).
fn find_resource(app: &App, name: &str) -> Option<PathBuf> {
    let res = app.path().resource_dir().ok()?;
    [res.join(name), res.join("resources").join(name)]
        .into_iter()
        .find(|p| p.is_dir())
}

/// Prepend `dir` to a `PATH`-style environment variable.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn prepend_path(var: &str, dir: &str) -> String {
    match std::env::var(var) {
        Ok(existing) if !existing.is_empty() => format!("{dir}:{existing}"),
        _ => dir.to_string(),
    }
}

/// Block (up to `timeout`) until something is listening on `127.0.0.1:port`.
fn wait_for_port(port: u16, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}
