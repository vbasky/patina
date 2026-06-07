//! Patina Python kernel.
//!
//! Speaks the shared `comm` kernel protocol and evaluates **Python** cells in an
//! embedded CPython interpreter (via pyo3). State persists across cells through a
//! single module-globals dict. The networking half is shared (`comm::kernel`);
//! only the executor here is Python-specific.

mod executor;

use comm::kernel::run_kernel;
use executor::spawn_executor;
use std::path::{Path, PathBuf};

fn main() {
    // For a self-contained desktop app: if a relocatable Python ships with the
    // app, point the embedded interpreter's stdlib at it so the host needs no
    // Python. Must run before the interpreter initializes (any `Python::with_gil`).
    configure_bundled_python();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    if let Err(e) = rt.block_on(run_kernel(spawn_executor)) {
        eprintln!("patina-kernel-python error: {e:?}");
        std::process::exit(1);
    }
}

/// If a bundled, relocatable Python is present, set `PYTHONHOME` so the embedded
/// interpreter uses its standard library instead of the host's. The root comes
/// from `$PATINA_PYTHON`, else a `python/` directory beside the kernel binary;
/// used only if it looks like a Python install (has a `lib/` dir). Honors an
/// explicit `$PYTHONHOME`. (Loading the bundle's `libpython` is the launcher's
/// job — see the desktop bundle script.)
fn configure_bundled_python() {
    if std::env::var_os("PYTHONHOME").is_some() {
        return;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let root = std::env::var_os("PATINA_PYTHON")
        .map(PathBuf::from)
        .or_else(|| exe_dir.as_ref().map(|d| d.join("python")))
        .filter(|p| p.join("lib").is_dir());
    if let Some(root) = root {
        // SAFETY: called at the very start of main, before any threads spawn.
        unsafe { std::env::set_var("PYTHONHOME", &root) };
        eprintln!(
            "patina-kernel-python: using bundled Python at {}",
            root.display()
        );
    }
}
