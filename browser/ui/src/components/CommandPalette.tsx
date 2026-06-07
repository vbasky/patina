import { useEffect, useMemo, useRef, useState } from "react";
import type { Command } from "../core/commands";

const CommandPalette: React.FC<{
  commands: Command[];
  onClose: () => void;
}> = ({ commands, onClose }) => {
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    const ql = q.toLowerCase();
    return commands.filter((c) => c.enabled && c.label.toLowerCase().includes(ql));
  }, [q, commands]);

  const clamped = Math.min(idx, Math.max(0, filtered.length - 1));
  const runAt = (i: number) => {
    const c = filtered[i];
    if (c) {
      c.run();
      onClose();
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center bg-black/30 pt-24"
      onClick={onClose}
    >
      <div
        className="w-[36rem] max-w-[90vw] overflow-hidden rounded-lg border border-gray-200 bg-white shadow-2xl dark:border-[#3a3a3a] dark:bg-[#2d2d2d]"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setIdx(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setIdx((i) => Math.min(i + 1, filtered.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setIdx((i) => Math.max(i - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              runAt(clamped);
            } else if (e.key === "Escape") {
              e.preventDefault();
              onClose();
            }
          }}
          placeholder="Type a command…"
          className="w-full border-b border-gray-200 bg-transparent px-4 py-3 outline-none dark:border-[#3a3a3a] dark:text-gray-100"
        />
        <div className="max-h-80 overflow-auto py-1">
          {filtered.length === 0 && (
            <div className="px-4 py-3 text-sm text-gray-400">No commands</div>
          )}
          {filtered.map((c, i) => (
            <button
              key={c.id}
              onMouseEnter={() => setIdx(i)}
              onClick={() => runAt(i)}
              className={`flex w-full items-center justify-between px-4 py-2 text-left text-sm dark:text-gray-100 ${
                i === clamped ? "bg-blue-100 dark:bg-[#37373d]" : ""
              }`}
            >
              <span>{c.label}</span>
              {c.shortcut && (
                <span className="text-xs text-gray-400">{c.shortcut}</span>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
};

export default CommandPalette;
