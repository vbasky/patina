import { useState } from "react";
import type { CommandMenu } from "../core/commands";

const MenuBar: React.FC<{ menus: CommandMenu[] }> = ({ menus }) => {
  const [open, setOpen] = useState<string | null>(null);

  return (
    <div
      className="relative z-40 flex items-center border-b border-gray-200 bg-gray-50 px-2 text-sm text-gray-700 dark:border-[#3a3a3a] dark:bg-[#252526] dark:text-gray-200"
      onMouseLeave={() => setOpen(null)}
    >
      {menus.map((m) => (
        <div key={m.menu} className="relative">
          <button
            type="button"
            className={`rounded px-3 py-1.5 hover:bg-gray-200 dark:hover:bg-[#2d2d2d] ${
              open === m.menu ? "bg-gray-200 dark:bg-[#2d2d2d]" : ""
            }`}
            onClick={() => setOpen(open === m.menu ? null : m.menu)}
            onMouseEnter={() => open && setOpen(m.menu)}
          >
            {m.menu}
          </button>
          {open === m.menu && (
            <div className="absolute left-0 top-full z-50 min-w-60 rounded-md border border-gray-200 bg-white py-1 shadow-lg dark:border-[#3a3a3a] dark:bg-[#2d2d2d]">
              {m.items.map((it, i) =>
                it === "separator" ? (
                  <div
                    key={i}
                    className="my-1 border-t border-gray-200 dark:border-[#3a3a3a]"
                  />
                ) : (
                  <button
                    type="button"
                    key={it.id || i}
                    disabled={!it.enabled}
                    onClick={() => {
                      it.run();
                      setOpen(null);
                    }}
                    className="flex w-full items-center justify-between gap-8 px-3 py-1.5 text-left hover:bg-blue-50 disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent dark:hover:bg-[#3a3a3a]"
                  >
                    <span>{it.label}</span>
                    {it.shortcut && (
                      <span className="text-xs text-gray-400">
                        {it.shortcut}
                      </span>
                    )}
                  </button>
                ),
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
};

export default MenuBar;
