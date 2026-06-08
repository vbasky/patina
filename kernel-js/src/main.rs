//! Patina TypeScript kernel.
//!
//! Speaks the shared `comm` kernel protocol and evaluates **TypeScript** cells:
//! each cell is type-stripped to JavaScript with [`oxc`] (pure Rust) and run with
//! [`boa_engine`] — a pure-Rust JS engine (no V8) — so it builds with cargo alone
//! and needs no Node/V8. State persists across cells through one shared `Context`.
//! The networking half is shared (`comm::kernel`); only the executor is specific.

mod executor;
mod transpile;

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
