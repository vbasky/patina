//! Patina desktop shell.
//!
//! A thin Tauri wrapper: on startup it launches the `patina` server (bundled as
//! a Tauri sidecar, with the language kernels alongside it), waits for it to
//! accept connections, then opens a native webview window pointing at the
//! server's localhost URL with a freshly generated auth key. The entire UI is
//! still served by `patina` — Tauri just provides the window and process glue.
//!
//! If you ship a bundled Rust toolchain / Python (see ../build-*.sh), set
//! `PATINA_TOOLCHAIN` / `PATINA_PYTHON` (e.g. to resources resolved via
//! `app.path()`) before spawning the sidecar so the app needs nothing from the
//! host.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpStream;
use std::time::{Duration, Instant};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_shell::ShellExt;

const PORT: u16 = 4050;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // A per-launch auth key, handed to both the server and the webview URL.
            let key = uuid::Uuid::new_v4().simple().to_string();

            // Launch the `patina` server sidecar. The kernel sidecars sit next to
            // it, so the server's `locate_kernel` finds them automatically.
            let (_rx, _child) = app
                .shell()
                .sidecar("patina")?
                .args(["--port", &PORT.to_string(), "--key", &key])
                .spawn()?;

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
