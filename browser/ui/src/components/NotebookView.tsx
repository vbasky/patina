import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  LuEraser,
  LuListTree,
  LuPlay,
  LuRotateCw,
  LuSquare,
} from "react-icons/lu";
import {
  clearOutputs,
  closeRun,
  newRun,
  runAll,
  saveNotebook,
} from "../core/actions";
import type { KernelState, Notebook } from "../core/notebook";
import EditorPanel from "./EditorPanel";
import { usePushNotification } from "./NotificationProvider";
import { useDispatch } from "./StateProvider";
import Workspace from "./Workspace";
import { useSendCommand } from "./WsProvider";

function kernelBadge(state: KernelState | null): {
  color: string;
  label: string;
  message?: string;
} {
  if (!state) return { color: "bg-gray-300", label: "No kernel" };
  switch (state.type) {
    case "Ready":
      return { color: "bg-green-500", label: "Ready" };
    case "Running":
      return { color: "bg-amber-500", label: "Running\u2026" };
    case "Init":
      return { color: "bg-blue-500", label: "Starting\u2026" };
    case "Crashed":
      return { color: "bg-red-500", label: "Crashed", message: state.message };
    case "Closed":
      return { color: "bg-gray-400", label: "Stopped" };
    default:
      return { color: "bg-gray-300", label: "Unknown" };
  }
}

const NotebookView: React.FC<{ notebook: Notebook }> = ({ notebook }) => {
  const dispatch = useDispatch();
  const sendCommand = useSendCommand();
  const pushNotification = usePushNotification();
  const [showGlobals, setShowGlobals] = useState(false);

  const run =
    notebook.runs.find((r) => r.id === notebook.current_run_id) ?? null;
  const badge = kernelBadge(run?.kernel_state ?? null);

  // Auto-start a kernel when a notebook is opened (like Jupyter), so it's Ready
  // before you run anything. Once per notebook; a manual Stop stays stopped.
  const autoStarted = useRef<Set<number>>(new Set());
  useEffect(() => {
    if (
      notebook.current_run_id === null &&
      notebook.runs.length === 0 &&
      !autoStarted.current.has(notebook.id)
    ) {
      autoStarted.current.add(notebook.id);
      newRun(notebook, dispatch, sendCommand);
    }
  }, [notebook, dispatch, sendCommand]);

  // Auto-save: triggered explicitly on editor changes, debounced 2s.
  // Uses a callback passed to EditorPanel, avoiding false triggers
  // from state changes (output arriving, kernel events, etc.).
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const notebookRef = useRef(notebook);
  notebookRef.current = notebook;

  const scheduleSave = useCallback(() => {
    if (notebookRef.current.save_in_progress) return;
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      saveNotebook(notebookRef.current, dispatch, sendCommand);
    }, 2000);
  }, [dispatch, sendCommand]);

  const toolBtn =
    "inline-flex items-center gap-1 rounded px-2 py-1 text-sm text-gray-600 hover:bg-gray-100 disabled:opacity-40 dark:text-gray-300 dark:hover:bg-[#2d2d2d]";

  return (
    <div className="flex h-full w-full flex-col bg-white text-gray-900 dark:bg-[#1e1e1e] dark:text-gray-100">
      {/* Top toolbar: kernel status + controls */}
      <div className="flex items-center gap-3 border-b border-gray-200 bg-white px-3 py-1.5 dark:border-[#3a3a3a] dark:bg-[#252526]">
        <span className="font-mono text-sm font-semibold text-teal-700 dark:text-teal-400">
          {notebook.path?.split("/").pop() || "notebook"}
        </span>
        <span
          className="flex items-center gap-1.5 text-sm text-gray-600 dark:text-gray-300"
          title={badge.message}
        >
          <span
            className={`inline-block h-2.5 w-2.5 rounded-full ${badge.color}`}
          />
          {badge.label}
        </span>
        <button
          type="button"
          onClick={() =>
            runAll(notebook, dispatch, sendCommand, pushNotification)
          }
          title="Run all cells"
          className={toolBtn}
        >
          <LuPlay size={14} /> Run all
        </button>
        <button
          type="button"
          onClick={() => run && clearOutputs(notebook.id, run.id, dispatch)}
          disabled={!run}
          title="Clear all outputs"
          className={toolBtn}
        >
          <LuEraser size={14} /> Clear
        </button>
        <button
          type="button"
          onClick={() =>
            run && closeRun(notebook.id, run.id, dispatch, sendCommand)
          }
          disabled={!run}
          title="Stop kernel"
          className={toolBtn}
        >
          <LuSquare size={14} /> Stop
        </button>
        <button
          type="button"
          onClick={() => newRun(notebook, dispatch, sendCommand)}
          title="Restart kernel (new run)"
          className={toolBtn}
        >
          <LuRotateCw size={14} /> Restart
        </button>
        <div className="flex-1" />
        <button
          type="button"
          onClick={() => setShowGlobals((v) => !v)}
          title="Toggle variables"
          className={`inline-flex items-center gap-1 rounded px-2 py-1 text-sm ${
            showGlobals
              ? "bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300"
              : "text-gray-600 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-[#2d2d2d]"
          }`}
        >
          <LuListTree size={14} /> Variables
        </button>
      </div>

      {/* Body: single-column notebook + optional globals drawer */}
      <div className="flex flex-1 overflow-hidden">
        <div className="flex-1 overflow-auto">
          <EditorPanel
            notebook={notebook}
            run={run}
            onEdit={scheduleSave}
          />
        </div>
        {showGlobals && (
          <div className="w-80 shrink-0 overflow-auto border-l border-gray-200 bg-white p-2 dark:border-[#3a3a3a] dark:bg-[#252526]">
            <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-400">
              Variables
            </div>
            {run ? (
              <Workspace notebook_id={notebook.id} run={run} />
            ) : (
              <div className="text-sm text-gray-400">
                Run a cell to start a kernel.
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default NotebookView;
