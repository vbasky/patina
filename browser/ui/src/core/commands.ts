import { Dispatch } from "react";
import { PushNotification } from "../components/NotificationProvider";
import {
  clearOutputs,
  closeRun,
  copyCell,
  cutCell,
  deleteCell,
  insertCell,
  moveCell,
  newRun,
  pasteCell,
  runAll,
  runCellById,
  saveNotebook,
} from "./actions";
import { SendCommand } from "./messages";
import { Notebook, Run } from "./notebook";
import { StateAction } from "./state";
import { cycleTheme } from "./theme";

export interface Command {
  id: string;
  label: string;
  shortcut?: string;
  enabled: boolean;
  run: () => void;
}

export interface CommandMenu {
  menu: string;
  items: (Command | "separator")[];
}

export interface CmdCtx {
  notebook: Notebook | null;
  run: Run | null;
  dispatch: Dispatch<StateAction>;
  sendCommand: SendCommand;
  pushNotification: PushNotification;
  ui: {
    newNotebook: () => void;
    openPalette: () => void;
    rerender: () => void;
  };
}

const disabled = { id: "", label: "", enabled: false, run: () => {} };

export function buildMenus(ctx: CmdCtx): CommandMenu[] {
  const nb = ctx.notebook;
  const sel = nb?.selected_editor_node_id ?? null;
  const hasNb = !!nb;
  const hasSel = !!nb && !!sel;
  const hasRun = !!nb && !!ctx.run;

  const cmd = (
    id: string,
    label: string,
    enabled: boolean,
    run: () => void,
    shortcut?: string,
  ): Command => ({ id, label, enabled, run, shortcut });

  return [
    {
      menu: "File",
      items: [
        cmd("file.new", "New Notebook…", true, ctx.ui.newNotebook),
        cmd(
          "file.save",
          "Save",
          hasNb,
          () => nb && saveNotebook(nb, ctx.dispatch, ctx.sendCommand),
          "⌘S",
        ),
        "separator",
        { ...disabled, id: "file.open", label: "Open .ipynb…  (soon)" },
        { ...disabled, id: "file.export", label: "Export as .ipynb…  (soon)" },
      ],
    },
    {
      menu: "Edit",
      items: [
        cmd("edit.cut", "Cut Cell", hasSel, () => nb && sel && cutCell(nb, sel, ctx.dispatch), "X"),
        cmd("edit.copy", "Copy Cell", hasSel, () => nb && sel && copyCell(nb, sel), "C"),
        cmd("edit.paste", "Paste Cell Below", hasNb, () => nb && pasteCell(nb, sel, ctx.dispatch), "V"),
        cmd("edit.delete", "Delete Cell", hasSel, () => nb && sel && deleteCell(nb, sel, ctx.dispatch), "D D"),
        "separator",
        cmd("edit.up", "Move Cell Up", hasSel, () => nb && sel && moveCell(nb, sel, "up", ctx.dispatch), "⌃↑"),
        cmd("edit.down", "Move Cell Down", hasSel, () => nb && sel && moveCell(nb, sel, "down", ctx.dispatch), "⌃↓"),
        "separator",
        cmd("edit.insertCode", "Insert Code Cell", hasNb, () => nb && insertCell(nb, sel, false, ctx.dispatch)),
        cmd("edit.insertMd", "Insert Markdown Cell", hasNb, () => nb && insertCell(nb, sel, true, ctx.dispatch)),
      ],
    },
    {
      menu: "Run",
      items: [
        cmd(
          "run.selected",
          "Run Selected Cell",
          hasSel,
          () => nb && sel && runCellById(nb, sel, ctx.dispatch, ctx.sendCommand, ctx.pushNotification),
          "⌃⏎",
        ),
        cmd("run.all", "Run All Cells", hasNb, () => nb && runAll(nb, ctx.dispatch, ctx.sendCommand, ctx.pushNotification)),
        "separator",
        cmd("run.clear", "Clear All Outputs", hasRun, () => nb && ctx.run && clearOutputs(nb.id, ctx.run.id, ctx.dispatch)),
      ],
    },
    {
      menu: "Kernel",
      items: [
        cmd("kernel.restart", "Restart Kernel", hasNb, () => nb && newRun(nb, ctx.dispatch, ctx.sendCommand)),
        cmd("kernel.stop", "Stop Kernel", hasRun, () => nb && ctx.run && closeRun(nb.id, ctx.run.id, ctx.dispatch, ctx.sendCommand)),
      ],
    },
    {
      menu: "View",
      items: [
        cmd("view.palette", "Command Palette…", true, ctx.ui.openPalette, "⌘⇧C"),
        cmd("view.theme", "Switch Theme (System/Light/Dark)", true, () => {
          cycleTheme();
          ctx.ui.rerender();
        }),
      ],
    },
  ];
}

export function flattenCommands(menus: CommandMenu[]): Command[] {
  const out: Command[] = [];
  for (const m of menus) {
    for (const it of m.items) {
      if (it !== "separator" && it.id) {
        out.push({ ...it, label: `${m.menu}: ${it.label}` });
      }
    }
  }
  return out;
}
