// The notebook workspace: a single directory (`<cwd>/notebooks`) that holds all
// `.tsnb` files. The file browser is rooted here and confined to it, so the rest
// of the launch directory is never exposed. All client-supplied paths are
// interpreted relative to this root.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The workspace root, created on first access. Overridable with
/// `PATINA_WORKSPACE`; otherwise `<cwd>/notebooks`.
pub(crate) fn root() -> &'static Path {
    ROOT.get_or_init(|| {
        let root = match std::env::var_os("PATINA_WORKSPACE") {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir().unwrap_or_default().join("notebooks"),
        };
        let _ = std::fs::create_dir_all(&root);
        root.canonicalize().unwrap_or(root)
    })
}

/// Resolve a root-relative path to an absolute path, refusing to escape the
/// workspace (`..` components and absolute paths are rejected). Returns `None`
/// if the path tries to break out.
pub(crate) fn resolve(rel: &str) -> Option<PathBuf> {
    let mut path = root().to_path_buf();
    for comp in rel.split(['/', '\\']) {
        match comp {
            "" | "." => continue,
            ".." => return None,
            _ => path.push(comp),
        }
    }
    Some(path)
}

/// Express an absolute path as a `/`-separated path relative to the workspace
/// root ("" if it *is* the root). Falls back to the input if it isn't under the
/// root.
pub(crate) fn relativize(abs: &Path) -> String {
    abs.strip_prefix(root())
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}
