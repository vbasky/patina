//! Patina Rust kernel.
//!
//! Speaks the `comm` wire protocol (TCP + length-delimited bincode frames) and
//! evaluates **Rust** cells through an embedded [`evcxr`] evaluation context.
//! The networking half is generic; only the executor (see [`executor`]) is
//! kernel-specific.

mod executor;

use comm::kernel::run_kernel;
use executor::spawn_executor;
use std::path::{Path, PathBuf};

fn main() {
    // evcxr re-invokes THIS binary as its compile/run subprocess. When it does,
    // `runtime_hook` detects the special env var, runs the evcxr runtime, and
    // exits — so it must be the very first thing in `main` and never returns in
    // that case. In a normal kernel launch it returns immediately.
    evcxr::runtime_hook();

    // For a self-contained desktop app: if a bundled Rust toolchain ships with
    // the app, point evcxr's cargo at it so the host needs no Rust install.
    // When absent (the default), the host toolchain is used as before.
    let bundled = configure_bundled_toolchain();

    // Speed up the per-cell rustc compiles: if `sccache` is installed, use it as
    // the rustc wrapper so built crates (incl. `:dep`s) are cached across cells
    // and kernel restarts. Set before any threads spawn (env::set_var safety).
    enable_sccache_if_available();
    // The bundle's own cargo config drives RUSTFLAGS/linker, so don't override it.
    if !bundled {
        tune_build_speed();
    }

    // Render polars DataFrames with ASCII box chars (the inherited eval process
    // reads this): they're in every monospace font's latin set, so tables align
    // even when the UI font lacks Unicode box-drawing glyphs. Respect a user override.
    if std::env::var_os("POLARS_FMT_TABLE_FORMATTING").is_none() {
        // SAFETY: called early in main, before any threads are spawned.
        unsafe { std::env::set_var("POLARS_FMT_TABLE_FORMATTING", "ASCII_FULL_CONDENSED") };
    }

    // Give evcxr a stable, persistent build directory so compiled dependencies
    // (polars, plotters, …) survive kernel restarts. evcxr otherwise builds in a
    // throwaway temp dir and recompiles everything each session (~90s for polars
    // every time). With this, a heavy crate compiles once per machine and later
    // sessions reuse it (~6s). This is the core "compile once, not every restart".
    configure_persistent_build_dir();

    // evcxr runs rust-analyzer in-process, which is extremely chatty at INFO.
    // Default to WARN; honor RUST_LOG if the user wants more.
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .try_init();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    if let Err(e) = rt.block_on(run_kernel(spawn_executor)) {
        eprintln!("patina-kernel error: {e:?}");
        std::process::exit(1);
    }
}

/// Path to a toolchain binary (`<root>/bin/<name>[.exe]`).
fn tool_bin(root: &Path, name: &str) -> PathBuf {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    root.join("bin").join(exe)
}

/// If a bundled, relocatable Rust toolchain is present, route evcxr's cargo at
/// it (so the host needs no Rust install). The toolchain root is taken from
/// `$PATINA_TOOLCHAIN`, else a `toolchain/` directory beside the kernel binary;
/// it's used only if it actually contains `bin/cargo`. A sibling `cargo/`
/// (vendored registry + `config.toml` with linker/offline settings) and
/// prewarmed `target/` cache are picked up automatically, overridable via
/// `$PATINA_CARGO_HOME` / `$PATINA_TARGET_DIR`. Returns true if a bundle is used.
fn configure_bundled_toolchain() -> bool {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let root = std::env::var_os("PATINA_TOOLCHAIN")
        .map(PathBuf::from)
        .or_else(|| exe_dir.as_ref().map(|d| d.join("toolchain")))
        .filter(|p| tool_bin(p, "cargo").exists());
    let Some(root) = root else {
        return false;
    };

    // Prepend the bundled bin dir to PATH (so cargo/rustc/rust-lld resolve here).
    if let Some(path) = std::env::var_os("PATH") {
        let mut parts = vec![root.join("bin")];
        parts.extend(std::env::split_paths(&path));
        if let Ok(joined) = std::env::join_paths(parts) {
            // SAFETY: called early in main, before any threads are spawned.
            unsafe { std::env::set_var("PATH", joined) };
        }
    }
    // SAFETY: called early in main, before any threads are spawned.
    unsafe {
        std::env::set_var("CARGO", tool_bin(&root, "cargo"));
        std::env::set_var("RUSTC", tool_bin(&root, "rustc"));
    }

    // Vendored registry + cargo config (linker, `net.offline`, source-replace).
    let cargo_home = std::env::var_os("PATINA_CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| root.parent().map(|p| p.join("cargo")))
        .filter(|p| p.is_dir());
    if let Some(ch) = cargo_home {
        unsafe { std::env::set_var("CARGO_HOME", ch) };
    }
    // Prewarmed build cache (so the batteries crates don't recompile on first run).
    let target = std::env::var_os("PATINA_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| root.parent().map(|p| p.join("target")))
        .filter(|p| p.is_dir());
    if let Some(t) = target {
        unsafe { std::env::set_var("CARGO_TARGET_DIR", t) };
    }

    eprintln!(
        "patina-kernel: using bundled toolchain at {}",
        root.display()
    );
    true
}

/// Point evcxr at a stable, persistent build directory (via `EVCXR_TMPDIR`) so
/// compiled dependencies persist across kernel restarts instead of being rebuilt
/// in a throwaway temp dir every session.
///
/// The directory is shared per machine, so it's guarded by an advisory file lock:
/// the first kernel takes the lock and uses the shared dir; any concurrent kernel
/// that can't get the lock is left to evcxr's default per-session temp dir (safe,
/// just no cross-restart reuse for that one). Respects a user-set `EVCXR_TMPDIR`.
#[cfg(unix)]
fn configure_persistent_build_dir() {
    use std::os::unix::io::AsRawFd;

    if std::env::var_os("EVCXR_TMPDIR").is_some() {
        return; // user/caller override — don't touch it
    }
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let dir = PathBuf::from(home).join(".patina").join("evcxr").join("rust");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }

    let lock_path = dir.join(".patina-lock");
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
    else {
        return;
    };
    // Non-blocking exclusive lock; auto-released when the process exits (the OS
    // closes the fd). Held for the whole run by leaking the file handle.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    if locked {
        std::mem::forget(file);
        // SAFETY: called early in main, before any threads are spawned.
        unsafe { std::env::set_var("EVCXR_TMPDIR", &dir) };
        eprintln!("patina-kernel: persistent build dir {}", dir.display());
    }
}

#[cfg(not(unix))]
fn configure_persistent_build_dir() {}

/// If `sccache` is on PATH and no rustc wrapper is configured, route compiles
/// through it. Off by default: evcxr's own `:cache` (see `executor.rs`) caches
/// artifacts across sessions *and* preserves dynamic linking, which sccache
/// disables — making cell links much slower. Opt in with `PATINA_SCCACHE=1`.
fn enable_sccache_if_available() {
    if std::env::var_os("PATINA_SCCACHE").is_none() {
        return;
    }
    if std::env::var_os("RUSTC_WRAPPER").is_some() {
        return;
    }
    let available = std::process::Command::new("sccache")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if available {
        // SAFETY: called early in main, before any threads are spawned.
        unsafe { std::env::set_var("RUSTC_WRAPPER", "sccache") };
    }
}

/// Faster per-cell compiles: drop debug info (cells are throwaway) and link with
/// `lld` if it's installed. Respects a user-set RUSTFLAGS.
fn tune_build_speed() {
    if std::env::var_os("RUSTFLAGS").is_some() {
        return;
    }
    let mut flags = String::from("-Cdebuginfo=0");
    let has_lld = std::process::Command::new("ld.lld")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if has_lld {
        flags.push_str(" -Clink-arg=-fuse-ld=lld");
    }
    // SAFETY: called early in main, before any threads are spawned.
    unsafe { std::env::set_var("RUSTFLAGS", flags) };
}
