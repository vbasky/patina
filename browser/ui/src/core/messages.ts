import type { Dispatch } from "react";
import type { NotificationType } from "../components/NotificationProvider";
import type {
  EditorGroupNode as EditorGroup,
  EditorNode,
  EditorNodeId,
  KernelState,
  Language,
  NotebookDesc,
  NotebookId,
  OutputCellFlag,
  OutputValue,
  RunId,
} from "./notebook";
import type { DirEntry, StateAction } from "./state";

export type SendCommand = (message: FromClientMessage) => void;

interface NewNotebookMsg {
  type: "NewNotebook";
  notebook: NotebookDesc;
}

interface KernelReadyMsg {
  type: "KernelReady";
  notebook_id: NotebookId;
  run_id: RunId;
}

interface KernelCrashedMsg {
  type: "KernelCrashed";
  notebook_id: NotebookId;
  run_id: RunId;
  message: string;
}

export interface SerializedGlobalsUpdate {
  variables: { string: string | null };
  name: string;
  children: { string: SerializedGlobalsUpdate };
}

export interface SerializedGlobals {
  variables: { string: string };
  name: string;
  children: { string: SerializedGlobalsUpdate };
}

interface OutputMsg {
  type: "Output";
  notebook_id: NotebookId;
  run_id: RunId;
  cell_id: EditorNodeId;
  flag: OutputCellFlag;
  value: OutputValue;
  update: null | SerializedGlobalsUpdate;
  kernel_state: KernelState;
}

interface NewGlobalsMsg {
  type: "NewGlobals";
  notebook_id: NotebookId;
  run_id: RunId;
  globals: SerializedGlobals;
}

interface SaveCompletedMsg {
  type: "SaveCompleted";
  notebook_id: NotebookId;
  error: string | null;
}

interface DirList {
  type: "DirList";
  dir: string;
  entries: DirEntry[];
}

interface Error {
  type: "Error";
  message: string;
}

export interface Toolchains {
  rust_toolchain: string | null;
  python: string | null;
  node: string | null;
}

interface SettingsMsg extends Toolchains {
  type: "Settings";
}

interface ExportDataMsg {
  type: "ExportData";
  filename: string;
  data: string;
}

export type ToClientMessage =
  | Error
  | NewNotebookMsg
  | KernelReadyMsg
  | KernelCrashedMsg
  | OutputMsg
  | NewGlobalsMsg
  | SaveCompletedMsg
  | SettingsMsg
  | DirList
  | ExportDataMsg;

interface CreateNewNotebookMsg {
  type: "CreateNewNotebook";
  filename: string;
  language?: Language;
}

interface CreateNewKernelMsg {
  type: "CreateNewKernel";
  notebook_id: NotebookId;
  run_title: string;
  run_id: string;
}

interface SaveNotebookMsg {
  type: "SaveNotebook";
  notebook_id: NotebookId;
  editor_root: EditorGroup;
}

interface RunCodeMsg {
  type: "RunCode";
  notebook_id: NotebookId;
  run_id: RunId;
  cell_id: EditorNodeId;
  editor_node: EditorNode;
  called_id: EditorNodeId;
}

interface LoadNotebookMsg {
  type: "LoadNotebook";
  path: string;
}

interface QueryDirMsg {
  type: "QueryDir";
  // Directory to list, relative to the project root ("" = root).
  path: string;
}

interface UploadFileMsg {
  type: "UploadFile";
  // Destination path relative to the workspace root, incl. extension.
  path: string;
  content: string;
}

interface DeleteFileMsg {
  type: "DeleteFile";
  path: string;
}

interface RenameFileMsg {
  type: "RenameFile";
  path: string;
  new_path: string;
}

interface SetToolchainsMsg extends Toolchains {
  type: "SetToolchains";
}

interface QuerySettingsMsg {
  type: "QuerySettings";
}

interface SetLanguageMsg {
  type: "SetLanguage";
  notebook_id: NotebookId;
  language: Language;
}

interface CloseRunMsg {
  type: "CloseRun";
  notebook_id: NotebookId;
  run_id: RunId;
}

interface ForkRunMsg {
  type: "Fork";
  notebook_id: NotebookId;
  run_id: RunId;
  new_run_id: RunId;
  new_run_title: string;
}

interface ExportNotebookMsg {
  type: "ExportNotebook";
  notebook_id: NotebookId;
}

export type FromClientMessage =
  | CreateNewNotebookMsg
  | CreateNewKernelMsg
  | RunCodeMsg
  | CloseRunMsg
  | ForkRunMsg
  | LoadNotebookMsg
  | QueryDirMsg
  | UploadFileMsg
  | DeleteFileMsg
  | RenameFileMsg
  | SetLanguageMsg
  | SetToolchainsMsg
  | QuerySettingsMsg
  | SaveNotebookMsg
  | ExportNotebookMsg;

export function processMessage(
  message: ToClientMessage,
  dispatch: Dispatch<StateAction>,
  pushNotification: (text: string, type: NotificationType) => void,
) {
  switch (message.type) {
    case "NewNotebook": {
      /// Because the root node is alwas EditorNamedNode
      /// so server does not send type
      /// But JS bad system of enums force us to fill the type
      message.notebook.editor_root.type = "Group";
      dispatch({
        type: "add_notebook",
        notebook: message.notebook,
      });
      break;
    }
    case "KernelReady": {
      dispatch({
        type: "kernel_changed",
        notebook_id: message.notebook_id,
        run_id: message.run_id,
        kernel_state: { type: "Ready" },
      });
      break;
    }
    case "KernelCrashed": {
      dispatch({
        type: "kernel_changed",
        notebook_id: message.notebook_id,
        run_id: message.run_id,
        kernel_state: { type: "Crashed", message: message.message },
      });
      break;
    }
    case "Output": {
      dispatch({
        type: "new_output",
        notebook_id: message.notebook_id,
        run_id: message.run_id,
        cell_id: message.cell_id,
        flag: message.flag,
        value: message.value,
        update: message.update,
        kernel_state: message.kernel_state,
      });
      break;
    }
    case "NewGlobals": {
      dispatch({
        type: "new_globals",
        notebook_id: message.notebook_id,
        run_id: message.run_id,
        globals: message.globals,
      });
      break;
    }
    case "SaveCompleted": {
      dispatch({
        type: "save_notebook",
        notebook_id: message.notebook_id,
        save_in_progress: false,
      });
      if (message.error) {
        pushNotification(message.error, "error");
      } else {
        pushNotification("Notebook saved", "success");
      }
      break;
    }
    case "DirList": {
      dispatch({
        type: "set_dir_entries",
        dir: message.dir,
        entries: message.entries,
      });
      break;
    }
    case "Settings": {
      dispatch({
        type: "set_settings",
        settings: {
          rust_toolchain: message.rust_toolchain,
          python: message.python,
          node: message.node,
        },
      });
      break;
    }
    case "Error": {
      pushNotification(message.message, "error");
      break;
    }
    case "ExportData": {
      const blob = new Blob([message.data], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = message.filename;
      a.click();
      URL.revokeObjectURL(url);
      pushNotification("Exported to .ipynb", "success");
      break;
    }
    default: {
      console.log("Unknown message", message);
    }
  }
}
