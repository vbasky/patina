import { useState } from "react";
import { LuX } from "react-icons/lu";
import { useDispatch, useGlobalState } from "./StateProvider";
import { useSendCommand } from "./WsProvider";

const norm = (s: string): string | null => {
  const t = s.trim();
  return t === "" ? null : t;
};

const Field: React.FC<{
  label: string;
  hint: string;
  placeholder: string;
  value: string;
  onChange: (v: string) => void;
}> = ({ label, hint, placeholder, value, onChange }) => (
  <label className="block">
    <div className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-200">
      {label}
    </div>
    <input
      type="text"
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      spellCheck={false}
      className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:border-[#3a3a3a] dark:bg-[#1e1e1e] dark:text-gray-100"
    />
    <div className="mt-1 text-[11px] text-gray-400">{hint}</div>
  </label>
);

// Lets the user point each kernel at a specific toolchain. Empty = auto-detect.
export const SettingsModal: React.FC<{ onClose: () => void }> = ({
  onClose,
}) => {
  const state = useGlobalState();
  const dispatch = useDispatch()!;
  const sendCommand = useSendCommand()!;
  const [rust, setRust] = useState(state.settings.rust_toolchain ?? "");
  const [python, setPython] = useState(state.settings.python ?? "");
  const [node, setNode] = useState(state.settings.node ?? "");

  const save = () => {
    const settings = {
      rust_toolchain: norm(rust),
      python: norm(python),
      node: norm(node),
    };
    sendCommand({ type: "SetToolchains", ...settings });
    dispatch({ type: "set_settings", settings });
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="mx-4 w-full max-w-lg rounded-md bg-white text-gray-900 shadow-xl dark:bg-[#252526] dark:text-gray-100">
        <div className="flex items-center justify-between p-4">
          <h2 className="text-lg font-bold">Toolchains</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-200"
          >
            <LuX size={20} />
          </button>
        </div>

        <div className="space-y-4 px-4 pb-2">
          <p className="text-xs text-gray-500 dark:text-gray-400">
            Point each kernel at a specific toolchain. Applied to the next kernel
            started (restart a running one). Leave blank to auto-detect.
          </p>
          <Field
            label="Rust toolchain"
            hint="Root containing bin/cargo (e.g. ~/.rustup/toolchains/stable-…)."
            placeholder="auto-detect (host)"
            value={rust}
            onChange={setRust}
          />
          <Field
            label="Python"
            hint="Python install root, used as PYTHONHOME."
            placeholder="auto-detect (host)"
            value={python}
            onChange={setPython}
          />
          <Field
            label="Node"
            hint="Node install root. Reserved — the JavaScript kernel uses boa."
            placeholder="auto-detect (host)"
            value={node}
            onChange={setNode}
          />
        </div>

        <div className="flex justify-end gap-3 rounded-b-md bg-gray-50 p-4 dark:bg-[#2d2d2d]">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-gray-300 px-4 py-2 text-gray-700 hover:bg-gray-100 dark:border-[#3a3a3a] dark:text-gray-200 dark:hover:bg-[#3a3a3a]"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={save}
            className="rounded-md bg-blue-600 px-4 py-2 text-white hover:bg-blue-700"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
};
