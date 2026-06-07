//! Patina desktop shell.
//!
//! A thin Tauri wrapper: on startup it launches the `patina` server (bundled as
//! a Tauri sidecar, with the language kernels alongside it), waits for it to
//! accept connections, then opens a native webview window pointing at the
//! server's localhost URL with a freshly generated auth key. The entire UI is
//! still served by `patina` — Tauri just provides the window and process glue.
//!
//! If a relocatable Rust toolchain and/or Python ship with the app as bundle
//! resources (`toolchain/`, `cargo/` and `python/` dirs — produced by
//! ../prebuild.sh / ../build-*.sh and listed in `tauri.conf.json`
//! `bundle.resources`), they're auto-detected here and passed to the server via
//! `PATINA_TOOLCHAIN` / `PATINA_PYTHON`, so the app needs no Rust/Python on the
//! host. When absent, the host toolchains are used.
//!
//! Because the `.app` is read-only but cargo must write its build cache, an
//! offline (vendored) Rust bundle is mirrored into a writable `~/.patina/runtime`
//! and the batteries crates are warm-compiled there once on first launch — so
//! the first notebook run reuses cached deps instead of compiling polars from
//! scratch. See [`setup_rust_runtime`].

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
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

        // The toolchain binaries are read-only inside the signed `.app`, but
        // cargo must *write* its build cache. If the bundle also vendored the
        // batteries crates (offline mode), set up a writable runtime under
        // `~/.patina/runtime`: a CARGO_HOME whose config replaces crates.io with
        // the read-only vendored sources, and a writable target dir — then warm
        // the batteries compile once so the first notebook run is fast.
        if let Some(cargo_res) = find_resource(app, "cargo") {
            if cargo_res.join("vendor").is_dir() {
                if let Some(rt) = setup_rust_runtime(app, &toolchain, &cargo_res) {
                    cmd = cmd
                        .env("PATINA_CARGO_HOME", rt.cargo_home.to_string_lossy().to_string())
                        .env("PATINA_TARGET_DIR", rt.target_dir.to_string_lossy().to_string());
                }
            }
        }
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

/// A writable Rust runtime derived from the read-only bundle resources.
struct RustRuntime {
    cargo_home: PathBuf,
    target_dir: PathBuf,
}

/// The batteries crates the kernel preloads — MUST stay in sync with
/// `kernel/src/executor.rs` and `desktop/build-rust-bundle.sh`, or the warm
/// compile's dependency fingerprints won't match what evcxr later builds (and
/// the cache won't be reused).
const BATTERIES: &[&str] = &[
    "ndarray = \"0.16\"",
    "plotters = { version = \"0.3\", features = [\"evcxr\"] }",
    "polars = { version = \"0.46\", features = [\"fmt\", \"lazy\"] }",
];

/// Prepare `~/.patina/runtime` as a writable home for cargo: a `CARGO_HOME`
/// whose config replaces crates.io with the bundle's read-only vendored sources
/// (offline), and an empty target dir. Kicks a one-time background warm compile
/// of the batteries so their artifacts are cached before the first cell runs.
/// Returns the paths to hand the kernel, or `None` on any I/O failure (the app
/// still runs — the kernel just falls back to compiling on demand).
fn setup_rust_runtime(app: &App, toolchain: &Path, cargo_res: &Path) -> Option<RustRuntime> {
    let root = app.path().home_dir().ok()?.join(".patina").join("runtime");
    let cargo_home = root.join("cargo");
    let target_dir = root.join("target");
    std::fs::create_dir_all(&cargo_home).ok()?;
    std::fs::create_dir_all(&target_dir).ok()?;

    // Writable cargo config: offline, vendored sources from the read-only
    // Resources dir, and the same rustflags the prewarm used so fingerprints
    // (and therefore the cached artifacts) match what evcxr builds.
    let vendor = cargo_res.join("vendor");
    let config = format!(
        "[build]\nrustflags = [\"-Cdebuginfo=0\"]\n\n\
         [net]\noffline = true\n\n\
         [source.crates-io]\nreplace-with = \"vendored-sources\"\n\n\
         [source.vendored-sources]\ndirectory = {:?}\n",
        vendor.to_string_lossy(),
    );
    std::fs::write(cargo_home.join("config.toml"), config).ok()?;

    // Warm once (idempotent via a sentinel), off the UI thread.
    let warmed = root.join(".warmed");
    if !warmed.exists() {
        let toolchain = toolchain.to_path_buf();
        let cargo_home = cargo_home.clone();
        let target_dir = target_dir.clone();
        let work = root.join("warm");
        std::thread::spawn(move || {
            if warm_batteries(&toolchain, &cargo_home, &target_dir, &work).is_ok() {
                let _ = std::fs::write(&warmed, b"");
            }
        });
    }

    Some(RustRuntime {
        cargo_home,
        target_dir,
    })
}

/// Compile a throwaway crate that depends on the batteries into `target_dir`,
/// using the bundled cargo. cargo caches each dependency by fingerprint in the
/// shared target dir, so evcxr's later per-cell builds reuse these artifacts and
/// only compile the thin cell crate.
fn warm_batteries(
    toolchain: &Path,
    cargo_home: &Path,
    target_dir: &Path,
    work: &Path,
) -> std::io::Result<()> {
    std::fs::create_dir_all(work.join("src"))?;
    std::fs::write(work.join("src/lib.rs"), b"")?;
    let mut manifest = String::from(
        "[package]\nname = \"patina-warm\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    );
    for dep in BATTERIES {
        manifest.push_str(dep);
        manifest.push('\n');
    }
    std::fs::write(work.join("Cargo.toml"), manifest)?;

    let cargo = toolchain.join("bin").join(if cfg!(windows) {
        "cargo.exe"
    } else {
        "cargo"
    });
    std::process::Command::new(cargo)
        .arg("build")
        .arg("--release")
        .current_dir(work)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTUP_TOOLCHAIN", "") // ignore any host rustup proxy
        .status()?;
    Ok(())
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
