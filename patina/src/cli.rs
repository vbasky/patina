use crate::http::http_server_main;
use crate::kernel::init_kernel_manager;
use crate::state::{AppState, generate_key};
use clap::Parser;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "4050")]
    port: u16,

    #[arg(long)]
    key: Option<String>,
}

/// Location of the persisted auth key (`~/.config/patina/secret_key`).
fn key_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("patina").join("secret_key"))
}

/// Reuse a saved auth key across restarts so the browser session doesn't expire
/// every time. Generates and saves one on first run. Falls back to a random
/// in-memory key if the file can't be read/written.
fn persistent_key() -> Option<String> {
    let path = key_file()?;
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim();
        if !existing.is_empty() {
            return Some(existing.to_string());
        }
    }
    let key = generate_key();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &key);
    Some(key)
}

pub async fn server_cli(args: Option<Vec<String>>) {
    /*
       TODO: Implement graceful termination of kernels
       We are not explicitly setting handler when server is called
       from Python
    */
    ctrlc::set_handler(|| std::process::exit(2)).unwrap();
    let args = if let Some(args) = args {
        Args::parse_from(args)
    } else {
        Args::parse()
    };
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            tracing_subscriber::fmt::init();
            // --key wins; otherwise reuse a persisted key so sessions survive restarts.
            let key = args.key.clone().or_else(persistent_key);
            let state = Arc::new(Mutex::new(AppState::new(args.port, key)));
            if let Err(e) = init_kernel_manager(&state).await {
                eprintln!("patina: failed to start kernel manager: {e}");
                std::process::exit(1);
            }
            if let Err(e) = http_server_main(state, args.port).await {
                // Most commonly the port is already taken (another Patina, or a
                // leftover process). Give a clear message instead of a panic.
                let addr_in_use = e
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::AddrInUse);
                if addr_in_use {
                    eprintln!(
                        "patina: port {} is already in use — is Patina already running? \
                         Pass --port <PORT> to use a different one.",
                        args.port
                    );
                } else {
                    eprintln!("patina: server error: {e}");
                }
                std::process::exit(1);
            }
        })
        .await;
}
