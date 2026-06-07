//! Patina Python kernel.
//!
//! Speaks the shared `comm` kernel protocol and evaluates **Python** cells in an
//! embedded CPython interpreter (via pyo3). State persists across cells through a
//! single module-globals dict. The networking half is shared (`comm::kernel`);
//! only the executor here is Python-specific.

mod executor;

use comm::kernel::run_kernel;
use executor::spawn_executor;

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    if let Err(e) = rt.block_on(run_kernel(spawn_executor)) {
        eprintln!("patina-kernel-python error: {e:?}");
        std::process::exit(1);
    }
}
