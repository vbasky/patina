//! User settings, persisted to `~/.config/patina/settings.json`.
//!
//! The toolchain paths let a user point each kernel at a specific Rust
//! toolchain / Python interpreter / Node install from the app. When set, they
//! take precedence over auto-detection (they're applied as the kernel's
//! `PATINA_TOOLCHAIN` / `PATINA_PYTHON` / `PATINA_NODE` environment).

use comm::messages::Language;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Settings {
    /// Path to a Rust toolchain root (containing `bin/cargo`).
    #[serde(default)]
    pub rust_toolchain: Option<String>,
    /// Path to a Python install root (used as `PYTHONHOME`).
    #[serde(default)]
    pub python: Option<String>,
    /// Path to a Node install root (reserved; the JS kernel uses `boa`).
    #[serde(default)]
    pub node: Option<String>,
}

/// Same config location as the auth key (`~/.config/patina/settings.json`).
fn settings_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("patina").join("settings.json"))
}

fn trimmed(o: &Option<String>) -> Option<String> {
    o.as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Settings {
    pub fn load() -> Self {
        settings_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = settings_file() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Environment to set on a kernel of `language` so it uses the configured
    /// toolchain. Empty when nothing is configured for that language.
    pub fn kernel_env(&self, language: Language) -> Vec<(String, String)> {
        let mut env = Vec::new();
        match language {
            Language::Rust => {
                if let Some(p) = trimmed(&self.rust_toolchain) {
                    env.push(("PATINA_TOOLCHAIN".to_string(), p));
                }
            }
            Language::Python => {
                // On macOS the kernel binary links libpython via @rpath. Set
                // the library path so the dynamic linker can find it (needed
                // even without explicit config). Also set PATINA_PYTHON to
                // point at the same Python install, so the embedded interpreter
                // finds its stdlib and site-packages correctly.
                let python_root = trimmed(&self.python).or_else(|| {
                    let output = std::process::Command::new("python3")
                        .args(["-c", "import sys; print(sys.prefix)"])
                        .output()
                        .ok()?;
                    if output.status.success() {
                        let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !prefix.is_empty() {
                            return Some(prefix);
                        }
                    }
                    None
                });

                if let Some(p) = python_root {
                    let lib = format!("{p}/lib");
                    let var = if cfg!(target_os = "macos") {
                        "DYLD_LIBRARY_PATH"
                    } else {
                        "LD_LIBRARY_PATH"
                    };
                    let combined = match std::env::var(var) {
                        Ok(cur) if !cur.is_empty() => format!("{lib}:{cur}"),
                        _ => lib,
                    };
                    env.push((var.to_string(), combined));
                    env.push(("PATINA_PYTHON".to_string(), p));
                }
            }
            Language::TypeScript => {
                if let Some(p) = trimmed(&self.node) {
                    env.push(("PATINA_NODE".to_string(), p));
                }
            }
        }
        env
    }
}
