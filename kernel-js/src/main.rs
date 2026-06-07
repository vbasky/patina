//! Patina JavaScript kernel.
//!
//! Speaks the shared `comm` kernel protocol and evaluates **JavaScript** cells
//! with [`boa_engine`] — a pure-Rust JS engine (no V8), so it builds with cargo
//! alone. State persists across cells through one shared `Context`. The
//! networking half is shared (`comm::kernel`); only the executor is JS-specific.

mod executor;

use comm::kernel::run_kernel;
use executor::spawn_executor;

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    if let Err(e) = rt.block_on(run_kernel(spawn_executor)) {
        eprintln!("patina-kernel-js error: {e:?}");
        std::process::exit(1);
    }
}
