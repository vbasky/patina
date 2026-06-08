import { useRef, useState } from "react";
import {
  LuChevronLeft,
  LuFile,
  LuFileText,
  LuFolder,
  LuHouse,
  LuNotebook,
  LuPencil,
  LuPlus,
  LuSettings,
  LuTrash2,
  LuUpload,
} from "react-icons/lu";
import { loadNotebook } from "../core/actions";
import type { Language } from "../core/notebook";
import type { DirEntry } from "../core/state";
import { ConfirmModal } from "./ConfirmModal";
import { LanguageIcon } from "./LanguageIcon";
import { SettingsModal } from "./SettingsModal";
import { useDispatch, useGlobalState } from "./StateProvider";
import { useSendCommand } from "./WsProvider";

const LANGUAGES: Language[] = ["Rust", "Python", "TypeScript"];

const basename = (p: string) => p.split("/").pop() ?? p;
const parentOf = (p: string) => p.split("/").slice(0, -1).join("/");

const iconFor = (entry: DirEntry) => {
  switch (entry.entry_type) {
    case "LoadedNotebook":
      return <LuNotebook size={15} className="text-emerald-500" />;
    case "Notebook":
      return <LuNotebook size={15} className="text-teal-500" />;
    case "Dir":
      return <LuFolder size={15} className="text-amber-500" />;
    default:
      return entry.path.endsWith(".md") ? (
        <LuFileText size={15} className="text-blue-400" />
      ) : (
        <LuFile size={15} className="text-gray-400" />
      );
  }
};

const NotebookList = () => {
  const state = useGlobalState();
  const sendCommand = useSendCommand();
  const dispatch = useDispatch();
  const fileInput = useRef<HTMLInputElement>(null);
  const [newLang, setNewLang] = useState<Language>("Rust");
  const [showSettings, setShowSettings] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<DirEntry | null>(null);

  const dir = state.current_dir;
  const openDir = (path: string) => sendCommand({ type: "QueryDir", path });

  // Upload .tsnb / .md / .ipynb into the current folder (md & ipynb are
  // converted to .tsnb server-side).
  const onUpload = async (files: FileList | null) => {
    if (!files) return;
    for (const file of Array.from(files)) {
      const content = await file.text();
      sendCommand({
        type: "UploadFile",
        path: dir ? `${dir}/${file.name}` : file.name,
        content,
      });
    }
  };

  const confirmDeleteEntry = () => {
    if (confirmDelete) {
      sendCommand({ type: "DeleteFile", path: confirmDelete.path });
      setConfirmDelete(null);
    }
  };

  // Rename a file/folder via the name dialog, keeping it in the same folder.
  const renameEntry = (entry: DirEntry) => {
    const oldName = basename(entry.path);
    const parent = parentOf(entry.path);
    dispatch({
      type: "set_dialog",
      dialog: {
        title: "Rename",
        value: oldName,
        okText: "Rename",
        onCancel: () => {},
        onConfirm: (value: string) => {
          const next = value.trim();
          if (!next || next === oldName) return;
          sendCommand({
            type: "RenameFile",
            path: entry.path,
            new_path: parent ? `${parent}/${next}` : next,
          });
        },
      },
    });
  };

  // Hide dotfiles by their basename (entry paths are full, root-relative).
  const entries = state.dir_entries.filter(
    (e) => !basename(e.path).startsWith("."),
  );
  entries.sort(
    (a, b) =>
      (a.entry_type === "Dir" ? 0 : 1) - (b.entry_type === "Dir" ? 0 : 1),
  );

  const onEntryClick = (entry: DirEntry) => {
    if (entry.entry_type === "Dir") {
      openDir(entry.path);
    } else if (
      entry.entry_type === "Notebook" ||
      entry.entry_type === "LoadedNotebook"
    ) {
      loadNotebook(state, entry.path, dispatch, sendCommand);
    }
  };

  const newNotebook = () =>
    dispatch({
      type: "set_dialog",
      dialog: {
        title: "New notebook name",
        value: "",
        okText: "Create a new notebook",
        onCancel: () => {},
        onConfirm: (value: string) =>
          sendCommand({
            type: "CreateNewNotebook",
            // Create inside the folder currently being browsed.
            filename: dir ? `${dir}/${value}` : value,
            language: newLang,
          }),
      },
    });

  return (
    <div className="flex h-full w-72 flex-col border-r border-gray-200 bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#252526]">
      {/* Brand */}
      <div className="flex items-center gap-2 px-3 pt-3 pb-2">
        <img
          src="/patina.svg"
          width={22}
          height={22}
          alt=""
          className="rounded"
        />
        <span className="font-semibold tracking-tight text-gray-800 dark:text-gray-100">
          Patina
        </span>
        <input
          ref={fileInput}
          type="file"
          multiple
          accept=".tsnb,.md,.ipynb"
          className="hidden"
          onChange={(e) => {
            onUpload(e.target.files);
            e.target.value = "";
          }}
        />
        <button
          type="button"
          onClick={() => fileInput.current?.click()}
          title="Upload .tsnb / .md / .ipynb"
          className="ml-auto rounded-md p-1.5 text-gray-500 hover:bg-gray-200 dark:text-gray-400 dark:hover:bg-[#2d2d2d]"
        >
          <LuUpload size={16} />
        </button>
        <button
          type="button"
          onClick={newNotebook}
          title="New notebook"
          className="rounded-md p-1.5 text-gray-500 hover:bg-gray-200 dark:text-gray-400 dark:hover:bg-[#2d2d2d]"
        >
          <LuPlus size={16} />
        </button>
        <button
          type="button"
          onClick={() => setShowSettings(true)}
          title="Toolchain settings"
          className="rounded-md p-1.5 text-gray-500 hover:bg-gray-200 dark:text-gray-400 dark:hover:bg-[#2d2d2d]"
        >
          <LuSettings size={16} />
        </button>
      </div>
      {showSettings && <SettingsModal onClose={() => setShowSettings(false)} />}
      {confirmDelete && (
        <ConfirmModal
          message={`Delete ${
            confirmDelete.entry_type === "Dir" ? "folder" : "file"
          } "${basename(confirmDelete.path)}"?`}
          onConfirm={confirmDeleteEntry}
          onCancel={() => setConfirmDelete(null)}
        />
      )}

      <div className="flex items-center justify-between gap-2 px-3 pb-2">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-400">
          Files
        </span>
        {/* Language for newly created notebooks. */}
        <label className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wider text-gray-400">
          New
          <LanguageIcon language={newLang} size={15} />
          <select
            value={newLang}
            onChange={(e) => setNewLang(e.target.value as Language)}
            title="Language for new notebooks"
            className="cursor-pointer rounded-md border border-gray-300 bg-white px-3 py-2 text-sm font-normal normal-case text-gray-700 hover:bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-200 dark:hover:bg-[#3a3a3a]"
          >
            {LANGUAGES.map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </select>
        </label>
      </div>

      {/* Breadcrumb: clickable path segments back to the root. */}
      {dir && (
        <div className="flex items-center gap-1 overflow-x-auto px-3 pb-1 text-xs text-gray-500 dark:text-gray-400">
          <button
            type="button"
            onClick={() => openDir("")}
            title="Project root"
            className="rounded p-0.5 hover:bg-gray-200 dark:hover:bg-[#2d2d2d]"
          >
            <LuHouse size={13} />
          </button>
          {dir.split("/").map((seg, i, segs) => {
            const path = segs.slice(0, i + 1).join("/");
            return (
              <span key={path} className="flex items-center gap-1">
                <span className="text-gray-300 dark:text-gray-600">/</span>
                <button
                  type="button"
                  onClick={() => openDir(path)}
                  className="truncate rounded px-0.5 hover:bg-gray-200 dark:hover:bg-[#2d2d2d]"
                >
                  {seg}
                </button>
              </span>
            );
          })}
        </div>
      )}

      <ul className="flex-1 space-y-0.5 overflow-auto px-2 pb-3">
        {/* "Up one level" row when inside a subfolder. */}
        {dir && (
          <li
            onClick={() => openDir(parentOf(dir))}
            className="group flex cursor-pointer items-center gap-2.5 rounded-md border-l-2 border-transparent px-2.5 py-1.5 text-sm text-gray-500 transition-colors hover:bg-gray-200/70 dark:text-gray-400 dark:hover:bg-[#2d2d2d]"
          >
            <LuChevronLeft size={15} className="text-gray-400" />
            <span className="truncate">..</span>
          </li>
        )}
        {entries.map((entry) => {
          const selected = entry.path === state.selected_notebook?.path;
          const clickable = entry.entry_type !== "File";
          return (
            <li
              key={entry.path}
              onClick={() => onEntryClick(entry)}
              className={`group flex items-center gap-2.5 rounded-md border-l-2 px-2.5 py-1.5 text-sm transition-colors ${
                selected
                  ? "border-teal-500 bg-teal-50 font-medium text-teal-800 dark:bg-[#37373d] dark:text-teal-200"
                  : `border-transparent text-gray-700 dark:text-gray-300 ${
                      clickable
                        ? "cursor-pointer hover:bg-gray-200/70 dark:hover:bg-[#2d2d2d]"
                        : "cursor-default opacity-80"
                    }`
              }`}
            >
              {iconFor(entry)}
              <span className="flex-1 truncate">{basename(entry.path)}</span>
              <button
                type="button"
                title="Rename"
                onClick={(e) => {
                  e.stopPropagation();
                  renameEntry(entry);
                }}
                className="shrink-0 rounded p-0.5 text-gray-400 opacity-0 transition hover:bg-gray-200 hover:text-gray-700 group-hover:opacity-100 dark:hover:bg-[#3a3a3a] dark:hover:text-gray-200"
              >
                <LuPencil size={13} />
              </button>
              <button
                type="button"
                title="Delete"
                onClick={(e) => {
                  e.stopPropagation();
                  setConfirmDelete(entry);
                }}
                className="shrink-0 rounded p-0.5 text-gray-400 opacity-0 transition hover:bg-red-100 hover:text-red-600 group-hover:opacity-100 dark:hover:bg-red-900/40"
              >
                <LuTrash2 size={13} />
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
};

export default NotebookList;
