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
            init_kernel_manager(&state).await.unwrap();
            http_server_main(state, args.port).await.unwrap();
        })
        .await;
}
