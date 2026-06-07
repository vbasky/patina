use crate::client_messages::{
    DirEntry, DirEntryType, ForkMsg, LoadNotebookMsg, RunCodeMsg, SaveNotebookMsg, ToClientMessage,
    UploadFileMsg, serialize_client_message,
};
use crate::convert;
use crate::kernel::{KernelCtx, spawn_kernel};
use crate::notebook::{
    KernelId, KernelState, Notebook, NotebookId, OutputCell, OutputCellId, OutputValue, Run, RunId,
};
use crate::state::{AppState, AppStateRef};
use crate::storage::{SerializedNotebook, deserialize_notebook, serialize_notebook};
use crate::workspace;
use anyhow::{anyhow, bail};
use axum::extract::ws::Message;
use comm::messages::{ComputeMsg, FromKernelMessage, Language, ToKernelMessage};
use comm::scopes::SerializedGlobals;
use jiff::Timestamp;
use std::path::{Path, PathBuf};
use tokio::spawn;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) fn new_notebook(
    state: &mut AppState,
    state_ref: &AppStateRef,
    mut filename: String,
    language: Language,
    sender: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if !filename.ends_with(".tsnb") {
        filename.push_str(".tsnb");
    }
    let notebook_id = state.new_notebook_id();
    tracing::debug!("Creating new notebook {notebook_id}");
    let mut notebook = Notebook::new(filename, language);
    notebook.set_observer(sender.clone());
    notebook.send_message(ToClientMessage::NewNotebook {
        notebook: notebook.notebook_desc(notebook_id),
    });
    state.add_notebook(notebook_id, notebook);
    let notebook = state.get_notebook_by_id(notebook_id).unwrap();
    save_helper(notebook_id, notebook, state_ref, true)
}

pub(crate) fn start_kernel(
    state: &mut AppState,
    state_ref: &AppStateRef,
    notebook_id: NotebookId,
    run_id: RunId,
    run_title: String,
) -> anyhow::Result<KernelId> {
    let kernel_port = state.kernel_port();
    let language = state.find_notebook_by_id_mut(notebook_id)?.language;
    let env = state.settings().kernel_env(language);
    let notebook = state.find_notebook_by_id_mut(notebook_id)?;
    let kernel_id = KernelId::new(Uuid::new_v4());
    let kernel_ctx = KernelCtx {
        kernel_id,
        notebook_id,
        run_id,
    };
    let run = Run::new(
        run_title,
        Vec::new(),
        KernelState::Init(kernel_ctx.kernel_id),
        SerializedGlobals::default(),
        Timestamp::now(),
    );
    notebook.add_run(run_id, run);
    match spawn_kernel(state_ref, kernel_ctx, kernel_port, language, &env) {
        Ok(kernel) => {
            state.add_kernel(kernel_id, kernel);
        }
        Err(e) => {
            tracing::error!("Starting kernel failed {e}");
            let run = notebook.find_run_by_id_mut(run_id).unwrap();
            run.set_crashed_kernel(e.to_string());
            notebook.send_message(ToClientMessage::KernelCrashed {
                notebook_id,
                run_id,
                message: e.to_string(),
            });
        }
    }
    Ok(kernel_id)
}

pub(crate) fn run_code(state: &mut AppState, msg: RunCodeMsg) -> anyhow::Result<()> {
    tracing::debug!("Runnning code {:?}", msg);
    let notebook = state.find_notebook_by_id_mut(msg.notebook_id)?;
    let run = notebook.find_run_by_id_mut(msg.run_id)?;
    let code = msg.editor_node.to_code_group();
    run.add_output_cell(OutputCell::new(msg.cell_id, msg.editor_node, msg.called_id));
    run.queue_increment();
    if let Some(kernel) = run
        .kernel_id()
        .and_then(|kernel_id| state.get_kernel_by_id_mut(kernel_id))
    {
        kernel.send_message(ToKernelMessage::Compute(ComputeMsg {
            cell_id: msg.cell_id.into_inner(),
            code,
        }))
    }
    Ok(())
}

async fn fork_process(
    state_ref: &AppStateRef,
    path: PathBuf,
    msg: ForkMsg,
    store_reader: oneshot::Receiver<Result<(), String>>,
) -> anyhow::Result<()> {
    let result = store_reader.await?;
    tracing::debug!("Kernel {} saved before forking", msg.run_id);
    result.map_err(|e| anyhow!(e))?;
    let receiver = {
        let mut state = state_ref.lock().unwrap();
        let kernel_id = start_kernel(
            &mut state,
            state_ref,
            msg.notebook_id,
            msg.new_run_id,
            msg.new_run_title,
        )?;
        state
            .get_kernel_by_id_mut(kernel_id)
            .unwrap()
            .load_state(path)
    };
    let result = receiver.await?;
    tracing::debug!("Kernel {} started & loaded", msg.new_run_id);
    let result = result.map_err(|e| anyhow!(e))?;
    let mut state = state_ref.lock().unwrap();
    if let Ok(notebook) = state.find_notebook_by_id_mut(msg.notebook_id) {
        notebook.send_message(ToClientMessage::NewGlobals {
            notebook_id: msg.notebook_id,
            run_id: msg.new_run_id,
            globals: result,
        });
    }
    Ok(())
}

pub(crate) fn fork_run(
    state: &mut AppState,
    state_ref: &AppStateRef,
    msg: ForkMsg,
) -> anyhow::Result<()> {
    tracing::debug!("Forking kernel {:?}", msg);
    let notebook = state.find_notebook_by_id_mut(msg.notebook_id)?;
    let run = notebook.find_run_by_id_mut(msg.run_id)?;
    let (_, path) = tempfile::NamedTempFile::new()?.keep()?;
    let state_ref = state_ref.clone();
    if let Some(kernel) = run
        .kernel_id()
        .and_then(|kernel_id| state.get_kernel_by_id_mut(kernel_id))
    {
        let sender = kernel.store_state(path.clone());
        spawn(async move {
            let notebook_id = msg.notebook_id;
            if let Err(err) = fork_process(&state_ref, path, msg, sender).await {
                let mut state = state_ref.lock().unwrap();
                if let Ok(notebook) = state.find_notebook_by_id_mut(notebook_id) {
                    notebook.send_message(ToClientMessage::Error {
                        message: &format!("Fork failed: {err}"),
                    });
                }
            }
        });
    }
    Ok(())
}

pub(crate) fn process_kernel_message(
    state: &mut AppState,
    kernel_ctx: &KernelCtx,
    msg: FromKernelMessage,
) -> anyhow::Result<()> {
    match msg {
        FromKernelMessage::Login { .. } => bail!("Process is already logged"),
        FromKernelMessage::Output {
            value,
            cell_id,
            flag,
            update,
        } => {
            let value = OutputValue::new(value);
            let notebook = state.find_notebook_by_id_mut(kernel_ctx.notebook_id)?;
            let run = notebook.find_run_by_id_mut(kernel_ctx.run_id)?;
            if flag.is_final() {
                run.queue_decrement();
            }
            let kernel_state = run.kernel_state_desc();
            notebook.send_message(ToClientMessage::Output {
                notebook_id: kernel_ctx.notebook_id,
                run_id: kernel_ctx.run_id,
                cell_id: OutputCellId::new(cell_id),
                value: &value,
                flag,
                update: update.as_ref(),
                kernel_state,
            });
            // TODO: Remove double lookup, this is just because of lifetime problems
            let run = notebook.find_run_by_id_mut(kernel_ctx.run_id)?;
            if let Some(update) = update {
                run.update_globals(update)
            }
            run.add_output(OutputCellId::new(cell_id), value, flag);
        }
        FromKernelMessage::SaveStateResponse { path: _, result } => {
            if let Some(kernel) = state.get_kernel_by_id_mut(kernel_ctx.kernel_id) {
                kernel.on_store_response(result);
            }
        }
        FromKernelMessage::LoadStateResponse { path: _, result } => {
            if let Some(kernel) = state.get_kernel_by_id_mut(kernel_ctx.kernel_id) {
                kernel.on_load_response(result);
            }
        }
    }
    Ok(())
}

fn save_helper(
    notebook_id: NotebookId,
    notebook: &Notebook,
    state_ref: &AppStateRef,
    new_notebook: bool,
) -> anyhow::Result<()> {
    let path = workspace::resolve(&notebook.path)
        .ok_or_else(|| anyhow!("Invalid notebook path: {}", notebook.path))?;
    tracing::debug!("Saving notebook as {}", path.display());
    let serialized_notebook = serialize_notebook(notebook)?;
    let state_ref = state_ref.clone();
    spawn(async move {
        let error = serialized_notebook
            .save(&path)
            .await
            .err()
            .map(|e| e.to_string());
        if let Some(err) = &error {
            tracing::debug!("Saving notebook as {} failed: {}", path.display(), err);
        } else {
            tracing::debug!("Saving notebook as {} finished", path.display());
        }
        let mut state = state_ref.lock().unwrap();
        if !new_notebook {
            if let Some(notebook) = state.get_notebook_by_id(notebook_id) {
                notebook.send_message(ToClientMessage::SaveCompleted { notebook_id, error });
            }
        } else {
            // Refresh the listing for the folder the new notebook landed in.
            let rel_dir = path.parent().map(workspace::relativize).unwrap_or_default();
            if let Ok(message) = query_helper(&mut state, &rel_dir)
                && let Some(notebook) = state.get_notebook_by_id(notebook_id) {
                    notebook.send_raw_message(message)
                }
        }
    });
    Ok(())
}

/// Change a notebook's language. Takes effect for the next kernel spawned
/// (a fresh run or a kernel restart); any already-running kernel is unaffected.
/// The choice is persisted on the notebook's next save.
pub(crate) fn set_language(
    state: &mut AppState,
    notebook_id: NotebookId,
    language: Language,
) -> anyhow::Result<()> {
    let notebook = state.find_notebook_by_id_mut(notebook_id)?;
    notebook.language = language;
    Ok(())
}

/// Persist updated toolchain settings (applied to the next kernel spawned).
pub(crate) fn set_toolchains(state: &mut AppState, settings: crate::settings::Settings) {
    state.set_settings(settings);
}

/// Send the current settings to a client.
pub(crate) fn query_settings(
    state: &AppState,
    sender: &UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let s = state.settings();
    let msg = serialize_client_message(ToClientMessage::Settings {
        rust_toolchain: s.rust_toolchain.clone(),
        python: s.python.clone(),
        node: s.node.clone(),
    })?;
    let _ = sender.send(msg);
    Ok(())
}

pub(crate) fn save_notebook(
    state: &mut AppState,
    state_ref: &AppStateRef,
    msg: SaveNotebookMsg,
) -> anyhow::Result<()> {
    let notebook_id = msg.notebook_id;
    let notebook = state.find_notebook_by_id_mut(notebook_id)?;
    notebook.editor_root = msg.editor_root;
    save_helper(notebook_id, notebook, state_ref, false)
}

pub(crate) fn load_notebook(
    state: &mut AppState,
    state_ref: &AppStateRef,
    msg: LoadNotebookMsg,
    sender: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let path = msg.path;
    tracing::debug!("Loading notebook {}", path);
    if let Some((notebook_id, notebook)) = state.get_notebook_by_path_mut(&path) {
        tracing::debug!("Notebook is already loaded");
        notebook.set_observer(sender);
        notebook.send_message(ToClientMessage::NewNotebook {
            notebook: notebook.notebook_desc(notebook_id),
        });
        return Ok(());
    }
    let abs = workspace::resolve(&path).ok_or_else(|| anyhow!("Invalid path: {path}"))?;
    let state_ref = state_ref.clone();
    spawn(async move {
        match SerializedNotebook::load(&abs)
            .await
            .and_then(|s| deserialize_notebook(&s))
        {
            Err(e) => {
                let _ = sender.send(
                    serialize_client_message(ToClientMessage::Error {
                        message: &format!("Failed to load notebook: {e}"),
                    })
                    .unwrap(),
                );
            }
            Ok(mut notebook) => {
                // TODO: Fix parallel loads
                notebook.set_observer(sender);
                notebook.path = path;
                let mut state = state_ref.lock().unwrap();
                let notebook_id = state.new_notebook_id();
                notebook.send_message(ToClientMessage::NewNotebook {
                    notebook: notebook.notebook_desc(notebook_id),
                });
                state.add_notebook(notebook_id, notebook);
            }
        }
    });
    Ok(())
}

/// Resolve a client-supplied relative directory against the workspace root,
/// confining the result to it. Returns the absolute directory together with its
/// path relative to the root (falls back to the root if the target is missing
/// or isn't a directory).
fn resolve_dir(rel: &str) -> (PathBuf, String) {
    let dir = workspace::resolve(rel)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| workspace::root().to_path_buf());
    let rel_dir = workspace::relativize(&dir);
    (dir, rel_dir)
}

fn query_helper(state: &mut AppState, rel: &str) -> anyhow::Result<Message> {
    let (dir, rel_dir) = resolve_dir(rel);
    let mut entries: Vec<DirEntry> = std::fs::read_dir(&dir)?
        .filter_map(|r| r.ok())
        .filter_map(|entry| {
            let full = entry.path();
            // Path relative to the workspace root, so the client can
            // load/navigate it directly (e.g. "ml/01-agents.tsnb").
            let path = workspace::relativize(&full);
            let file_type = entry.file_type().ok()?;
            let entry_type = if file_type.is_file() && path.ends_with(".tsnb") {
                if state.get_notebook_by_path_mut(&path).is_some() {
                    DirEntryType::LoadedNotebook
                } else {
                    DirEntryType::Notebook
                }
            } else if file_type.is_dir() {
                if path.ends_with(".tsnb.runs") {
                    return None;
                }
                DirEntryType::Dir
            } else {
                DirEntryType::File
            };
            Some(DirEntry { path, entry_type })
        })
        .collect();
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    serialize_client_message(ToClientMessage::DirList {
        dir: &rel_dir,
        entries: &entries,
    })
}

pub(crate) fn query_dir(
    state: &mut AppState,
    sender: &UnboundedSender<Message>,
    rel: &str,
) -> anyhow::Result<()> {
    let message = query_helper(state, rel)?;
    let _ = sender.send(message);
    Ok(())
}

/// The workspace-relative directory containing `rel` ("" for the root).
fn parent_dir(rel: &str) -> String {
    match rel.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

/// Handle an uploaded file: `.tsnb` is stored verbatim, while `.md`/`.ipynb`
/// are converted into a notebook saved alongside as `.tsnb`. The result is
/// opened and the listing refreshed.
pub(crate) fn upload_file(
    state: &mut AppState,
    state_ref: &AppStateRef,
    msg: UploadFileMsg,
    sender: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let ext = Path::new(&msg.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let dir = parent_dir(&msg.path);

    match ext.as_str() {
        "tsnb" => {
            let abs = workspace::resolve(&msg.path)
                .ok_or_else(|| anyhow!("Invalid path: {}", msg.path))?;
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&abs, msg.content.as_bytes())?;
            // Open it, then refresh the folder it landed in.
            load_notebook(
                state,
                state_ref,
                LoadNotebookMsg {
                    path: msg.path.clone(),
                },
                sender.clone(),
            )?;
            query_dir(state, &sender, &dir)?;
        }
        "md" | "ipynb" => {
            let (cells, language) = if ext == "md" {
                (convert::markdown_to_cells(&msg.content), Language::Rust)
            } else {
                (
                    convert::ipynb_to_cells(&msg.content)?,
                    convert::ipynb_language(&msg.content),
                )
            };
            // Save next to the source with a .tsnb extension.
            let out_rel = {
                let p = Path::new(&msg.path).with_extension("tsnb");
                p.to_string_lossy().replace('\\', "/")
            };
            let mut notebook = convert::build_notebook(out_rel, language, cells);
            notebook.set_observer(sender.clone());
            let notebook_id = state.new_notebook_id();
            notebook.send_message(ToClientMessage::NewNotebook {
                notebook: notebook.notebook_desc(notebook_id),
            });
            state.add_notebook(notebook_id, notebook);
            let notebook = state.get_notebook_by_id(notebook_id).unwrap();
            save_helper(notebook_id, notebook, state_ref, true)?;
        }
        other => bail!("Unsupported file type: .{other}"),
    }
    Ok(())
}

/// Delete a file (and a notebook's sidecar `.runs` directory) or an empty
/// subfolder, then refresh the listing.
pub(crate) fn delete_file(
    state: &mut AppState,
    sender: &UnboundedSender<Message>,
    rel: &str,
) -> anyhow::Result<()> {
    let abs = workspace::resolve(rel).ok_or_else(|| anyhow!("Invalid path: {rel}"))?;
    if abs == workspace::root() {
        bail!("Refusing to delete the workspace root");
    }
    if abs.is_dir() {
        std::fs::remove_dir_all(&abs)?;
    } else {
        std::fs::remove_file(&abs)?;
        // Drop the notebook's run sidecar directory if present.
        let runs = abs.with_file_name(format!(
            "{}.runs",
            abs.file_name().unwrap_or_default().to_string_lossy()
        ));
        if runs.is_dir() {
            let _ = std::fs::remove_dir_all(&runs);
        }
    }
    query_dir(state, sender, &parent_dir(rel))
}

pub(crate) fn close_run(
    state: &mut AppState,
    notebook_id: NotebookId,
    run_id: RunId,
) -> anyhow::Result<()> {
    tracing::debug!("Closing run {}", run_id);
    let notebook = state.find_notebook_by_id_mut(notebook_id)?;
    let run = notebook.remove_run_by_id(run_id)?;
    match run.kernel_state() {
        KernelState::Init(kernel_id) | KernelState::Running(kernel_id) => {
            let kernel_id = *kernel_id;
            state.stop_kernel(kernel_id);
        }
        KernelState::Crashed(_) | KernelState::Closed => { /* Do nothing */ }
    }
    Ok(())
}
