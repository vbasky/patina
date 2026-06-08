use crate::notebook::{
    EditorGroup, EditorId, KernelId, NotebookId, OutputCell, OutputCellId, OutputValue, RunId,
};
use axum::extract::ws::Message;
use comm::messages::{Language, OutputFlag};
use comm::scopes::{SerializedGlobals, SerializedGlobalsUpdate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum FromClientMessage {
    CreateNewNotebook(CreateNewNotebookMsg),
    CreateNewKernel(CreateNewKernelMsg),
    RunCode(RunCodeMsg),
    SaveNotebook(SaveNotebookMsg),
    LoadNotebook(LoadNotebookMsg),
    QueryDir(QueryDirMsg),
    UploadFile(UploadFileMsg),
    DeleteFile(DeleteFileMsg),
    RenameFile(RenameFileMsg),
    SetLanguage(SetLanguageMsg),
    SetToolchains(crate::settings::Settings),
    QuerySettings,
    CloseRun(NotebookRunMsg),
    KernelList,
    Fork(ForkMsg),
    ExportNotebook(ExportNotebookMsg),
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExportNotebookMsg {
    pub notebook_id: NotebookId,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NotebookRunMsg {
    pub notebook_id: NotebookId,
    pub run_id: RunId,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateNewNotebookMsg {
    pub filename: String,
    #[serde(default)]
    pub language: Language,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateNewKernelMsg {
    pub notebook_id: NotebookId,
    pub run_id: RunId,
    pub run_title: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunCodeMsg {
    pub notebook_id: NotebookId,
    pub run_id: RunId,
    pub cell_id: OutputCellId,
    pub editor_node: EditorGroup,
    pub called_id: EditorId,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ForkMsg {
    pub notebook_id: NotebookId,
    pub run_id: RunId,
    pub new_run_id: RunId,
    pub new_run_title: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveNotebookMsg {
    pub notebook_id: NotebookId,
    pub editor_root: EditorGroup,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoadNotebookMsg {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QueryDirMsg {
    /// Directory to list, relative to the project root ("" = root).
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UploadFileMsg {
    /// Destination path relative to the workspace root, including the original
    /// extension (.tsnb/.md/.ipynb). Converted files are saved alongside as
    /// `.tsnb`.
    pub path: String,
    /// UTF-8 file contents.
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteFileMsg {
    /// Path to delete, relative to the workspace root.
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenameFileMsg {
    /// Existing path, relative to the workspace root.
    pub path: String,
    /// New path, relative to the workspace root.
    pub new_path: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetLanguageMsg {
    pub notebook_id: NotebookId,
    pub language: Language,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum KernelStateDesc {
    Init,
    Ready,
    Running,
    Crashed { message: String },
    Closed,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunDesc<'a> {
    pub id: RunId,
    pub title: &'a str,
    pub output_cells: &'a [OutputCell],
    pub kernel_state: KernelStateDesc,
    pub globals: &'a SerializedGlobals,
}

#[derive(Debug, Serialize)]
pub(crate) struct NotebookDesc<'a> {
    pub id: NotebookId,
    pub path: &'a str,
    pub language: Language,
    pub editor_root: &'a EditorGroup,
    pub editor_open_nodes: &'a [EditorId],
    pub runs: Vec<RunDesc<'a>>,
}

#[derive(Debug, Serialize)]
pub(crate) enum DirEntryType {
    Notebook,
    LoadedNotebook,
    File,
    Dir,
}

#[derive(Debug, Serialize)]
pub(crate) struct DirEntry {
    pub path: String,
    pub entry_type: DirEntryType,
}

#[derive(Debug, Serialize)]
pub(crate) struct KernelInfo {
    pub kernel_id: KernelId,
    pub notebook_id: NotebookId,
    pub run_id: RunId,
    pub pid: u32,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum ToClientMessage<'a> {
    Error {
        message: &'a str,
    },
    NewNotebook {
        notebook: NotebookDesc<'a>,
    },
    KernelReady {
        notebook_id: NotebookId,
        run_id: RunId,
    },
    KernelCrashed {
        notebook_id: NotebookId,
        run_id: RunId,
        message: String,
    },
    Output {
        notebook_id: NotebookId,
        run_id: RunId,
        cell_id: OutputCellId,
        value: &'a OutputValue,
        flag: OutputFlag,
        update: Option<&'a SerializedGlobalsUpdate>,
        kernel_state: KernelStateDesc,
    },
    NewGlobals {
        notebook_id: NotebookId,
        run_id: RunId,
        globals: SerializedGlobals,
    },
    SaveCompleted {
        notebook_id: NotebookId,
        error: Option<String>,
    },
    DirList {
        /// The directory these entries belong to, relative to the project
        /// root ("" = root).
        dir: &'a str,
        entries: &'a [DirEntry],
    },
    Settings {
        rust_toolchain: Option<String>,
        python: Option<String>,
        node: Option<String>,
    },
    Kernels {
        kernels: Vec<KernelInfo>,
    },
    ExportData {
        filename: &'a str,
        data: &'a str,
    },
}

pub(crate) fn parse_client_message(message: Message) -> anyhow::Result<FromClientMessage> {
    Ok(serde_json::from_str::<FromClientMessage>(
        message.to_text()?,
    )?)
}

pub(crate) fn serialize_client_message(message: ToClientMessage) -> anyhow::Result<Message> {
    Ok(Message::Text(serde_json::to_string(&message)?.into()))
}
