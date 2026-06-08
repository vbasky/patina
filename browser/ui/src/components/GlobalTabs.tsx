import { useEffect, useReducer, useState } from "react";
import {
  LuMoon,
  LuNotebook,
  LuPanelLeft,
  LuSun,
  LuSunMoon,
} from "react-icons/lu";
import { buildMenus, flattenCommands } from "../core/commands";
import { cycleTheme, getThemeMode, onThemeChange } from "../core/theme";
import CommandPalette from "./CommandPalette";
import Dialog from "./Dialog";
import NotebookList from "./DirList";
import MenuBar from "./MenuBar";
import NotebookView from "./NotebookView";
import { usePushNotification } from "./NotificationProvider";
import { useDispatch, useGlobalState } from "./StateProvider";
import { useSendCommand } from "./WsProvider";

const NotebookTab = (props: {
  active: boolean;
  name: string;
  onClick: () => void;
}) => (
  <button
    type="button"
    onClick={props.onClick}
    className={`group relative flex items-center gap-2 border-r border-gray-200 px-4 py-2 text-sm transition-colors dark:border-[#3a3a3a] ${
      props.active
        ? "bg-white text-gray-900 dark:bg-[#1e1e1e] dark:text-gray-100"
        : "text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-[#2d2d2d]"
    }`}
  >
    {props.active && (
      <span className="absolute inset-x-0 top-0 h-0.5 bg-teal-500" />
    )}
    <LuNotebook
      size={14}
      className={props.active ? "text-teal-500" : "opacity-60"}
    />
    <span className="max-w-[12rem] truncate">{props.name}</span>
  </button>
);

const ThemeToggle = () => {
  const [mode, setMode] = useState(getThemeMode());
  // Keep the icon in sync with live OS changes while in "system" mode.
  useEffect(() => onThemeChange(() => setMode(getThemeMode())), []);

  const icon =
    mode === "system" ? (
      <LuSunMoon size={16} />
    ) : mode === "dark" ? (
      <LuMoon size={16} />
    ) : (
      <LuSun size={16} />
    );
  const title =
    mode === "system"
      ? "Theme: System — click for Light"
      : mode === "light"
        ? "Theme: Light — click for Dark"
        : "Theme: Dark — click for System";
  return (
    <button
      type="button"
      onClick={() => setMode(cycleTheme())}
      title={title}
      className="mr-2 ml-auto self-center rounded-md p-2 text-gray-500 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-[#2d2d2d]"
    >
      {icon}
    </button>
  );
};

const GlobalTabs = () => {
  const state = useGlobalState();
  const dispatch = useDispatch();
  const sendCommand = useSendCommand();
  const pushNotification = usePushNotification();
  const [showNotebookList, setShowNotebookList] = useState<boolean>(true);
  const [palette, setPalette] = useState<boolean>(false);
  const [, force] = useReducer((x) => x + 1, 0);
  // Re-render (re-highlight code) when the theme changes — manual or live OS.
  useEffect(() => onThemeChange(force), []);

  const notebook = state.selected_notebook;
  const run =
    notebook?.runs.find((r) => r.id === notebook.current_run_id) ?? null;

  const newNotebook = () =>
    dispatch({
      type: "set_dialog",
      dialog: {
        title: "New notebook name",
        value: "",
        okText: "Create a new notebook",
        onCancel: () => {},
        onConfirm: (value: string) =>
          sendCommand({ type: "CreateNewNotebook", filename: value }),
      },
    });

  const menus = buildMenus({
    notebook: notebook ?? null,
    run,
    dispatch,
    sendCommand,
    pushNotification,
    ui: { newNotebook, openPalette: () => setPalette(true), rerender: force },
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (
        (e.metaKey || e.ctrlKey) &&
        e.shiftKey &&
        e.key.toLowerCase() === "c"
      ) {
        e.preventDefault();
        setPalette((p) => !p);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-full w-full flex-col bg-white text-gray-900 dark:bg-[#1e1e1e] dark:text-gray-100">
      {state.dialog && <Dialog config={state.dialog} />}
      {palette && (
        <CommandPalette
          commands={flattenCommands(menus)}
          onClose={() => setPalette(false)}
        />
      )}
      <MenuBar menus={menus} />

      {/* Tab bar */}
      <div className="flex items-stretch border-b border-gray-200 bg-gray-100 dark:border-[#3a3a3a] dark:bg-[#252526]">
        <button
          type="button"
          onClick={() => setShowNotebookList((v) => !v)}
          title="Toggle sidebar"
          className={`px-3 py-2 text-gray-500 transition-colors hover:bg-gray-200 dark:text-gray-400 dark:hover:bg-[#2d2d2d] ${
            showNotebookList ? "text-teal-600 dark:text-teal-400" : ""
          }`}
        >
          <LuPanelLeft size={16} />
        </button>
        <div className="flex flex-1 items-stretch overflow-x-auto">
          {state.notebooks.map((nb) => (
            <NotebookTab
              key={nb.id}
              active={nb.id === state.selected_notebook?.id}
              name={nb.path}
              onClick={() =>
                dispatch({ type: "set_selected_notebook", id: nb.id })
              }
            />
          ))}
        </div>
        <ThemeToggle />
      </div>

      {/* Body */}
      <div className="flex min-h-0 flex-1">
        {showNotebookList && <NotebookList />}
        {state.selected_notebook === null ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 text-gray-400">
            <img
              src="/patina.svg"
              width={64}
              height={64}
              alt=""
              className="opacity-80"
            />
            <div className="text-sm">Select a notebook to get started</div>
          </div>
        ) : (
          <NotebookView notebook={state.selected_notebook} />
        )}
      </div>
    </div>
  );
};

export default GlobalTabs;
