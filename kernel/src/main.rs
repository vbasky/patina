//! Patina Rust kernel.
//!
//! Speaks the `comm` wire protocol (TCP + length-delimited bincode frames) and
//! evaluates **Rust** cells through an embedded [`evcxr`] evaluation context.
//! The networking half is generic; only the executor (see [`executor`]) is
//! kernel-specific.

mod executor;

use comm::kernel::run_kernel;
use executor::spawn_executor;

fn main() {
    // evcxr re-invokes THIS binary as its compile/run subprocess. When it does,
    // `runtime_hook` detects the special env var, runs the evcxr runtime, and
    // exits — so it must be the very first thing in `main` and never returns in
    // that case. In a normal kernel launch it returns immediately.
    evcxr::runtime_hook();

    // Speed up the per-cell rustc compiles: if `sccache` is installed, use it as
    // the rustc wrapper so built crates (incl. `:dep`s) are cached across cells
    // and kernel restarts. Set before any threads spawn (env::set_var safety).
    enable_sccache_if_available();
    tune_build_speed();

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

/// If `sccache` is on PATH and no rustc wrapper is configured, route compiles
/// through it. Caches compiled artifacts, cutting evcxr's per-cell latency.
fn enable_sccache_if_available() {
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
