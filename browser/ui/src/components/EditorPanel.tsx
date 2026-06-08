import {
  type EditorCell,
  type EditorGroupNode,
  type EditorNode,
  type EditorNodeId,
  EditorScope,
  type Language,
  type Notebook,
  type OutputCell,
  type Run,
} from "../core/notebook";

const LANGUAGES: Language[] = ["Rust", "Python", "TypeScript"];

import { marked } from "marked";
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  LuChevronDown,
  LuChevronRight,
  LuChevronUp,
  LuCircleAlert,
  LuCircleCheck,
  LuClock,
  LuCode,
  LuFolderPlus,
  LuGripVertical,
  LuLoaderCircle,
  LuPlay,
  LuPlus,
  LuSave,
  LuText,
  LuTrash2,
} from "react-icons/lu";
import { TbCircleDashed } from "react-icons/tb";

import { v4 as uuidv4 } from "uuid";
import {
  newEditorCode,
  newEditorGroup,
  removeEditorNode,
  runCode,
  saveNotebook,
  findNodeById,
  findPathById,
} from "../core/actions";
import {
  isMarkdownCell,
  markdownSource,
  toCode,
  toMarkdown,
} from "../core/cells";
import { focusId } from "../core/focus";
import { highlightInline } from "../core/highlighter";
import { NodeToolbar } from "./EditorToolbar";
import { LanguageIcon } from "./LanguageIcon";
import MonacoCell from "./MonacoCell";
import { usePushNotification } from "./NotificationProvider";
import { OutputValueView } from "./OutputCell";
import { useDispatch } from "./StateProvider";
import { useSendCommand } from "./WsProvider";

function latestOutputFor(
  run: Run | null,
  cellId: EditorNodeId,
): { cell: OutputCell; execCount: number } | null {
  if (!run) return null;
  for (let i = run.output_cells.length - 1; i >= 0; i--) {
    if (run.output_cells[i].called_id === cellId) {
      return { cell: run.output_cells[i], execCount: i + 1 };
    }
  }
  return null;
}

const InlineOutput: React.FC<{ cell: OutputCell; execCount: number }> = ({
  cell,
  execCount,
}) => {
  const [startMs] = useState(() => Date.now());
  const [elapsed, setElapsed] = useState(0);
  const final = cell.flag === "Success" || cell.flag === "Fail";

  useEffect(() => {
    if (cell.flag === "Pending" || cell.flag === "Running") {
      const id = setInterval(() => {
        setElapsed(Date.now() - startMs);
      }, 100);
      return () => clearInterval(id);
    }
    // On completion, capture the final elapsed time
    setElapsed(Date.now() - startMs);
  }, [cell.flag, startMs]);

  const pending = cell.flag === "Pending" || cell.flag === "Running";
  const label = pending ? "[*]" : `[${execCount}]`;
  const statusIcon = () => {
    switch (cell.flag) {
      case "Pending":
        return <LuClock className="text-blue-400" size={13} />;
      case "Running":
        return (
          <LuLoaderCircle className="animate-spin text-blue-500" size={13} />
        );
      case "Fail":
        return <LuCircleAlert className="text-red-500" size={13} />;
      default:
        return <LuCircleCheck className="text-green-600" size={13} />;
    }
  };
  const hasContent = cell.values.some((v) => v.type !== "None");
  return (
    <div className="flex mt-1 mb-3">
      <div className="w-14 shrink-0 pr-2 pt-1.5 flex items-start justify-end gap-1 font-mono text-[11px] text-gray-400 select-none">
        {statusIcon()}
        <span>{label}</span>
      </div>
      <div className="flex-1 min-w-0">
        {(pending || final) && (
          <div className="mb-0.5 text-[11px] text-gray-400">
            {pending ? (
              <span>
                {hasContent ? "running… " : "compiling… "}
                {(elapsed / 1000).toFixed(1)}s
              </span>
            ) : (
              <span>{(elapsed / 1000).toFixed(1)}s</span>
            )}
          </div>
        )}
        {hasContent && (
          <div className="rounded bg-gray-50 px-2 py-1 border border-gray-100 dark:bg-[#2d2d2d]/60 dark:border-[#3a3a3a]">
            {cell.values.map((v, i) => (
              <OutputValueView key={i} value={v} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

// A thin "+ Code / + Markdown" inserter between/after cells.
// Also serves as a drop target for drag-and-drop reordering.
const InsertBar: React.FC<{
  onInsert: (markdown: boolean) => void;
  onDropCell?: (dragId: string) => void;
}> = ({ onInsert, onDropCell }) => {
  const [dragOver, setDragOver] = useState(false);
  return (
    <div
      className={`group/insert relative my-0.5 flex h-4 items-center justify-center rounded transition-colors ${dragOver ? "bg-blue-100 dark:bg-blue-900/30 h-8" : ""}`}
      onDragOver={(e) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragOver(false);
        const dragId = e.dataTransfer.getData("text/plain");
        if (dragId && onDropCell) onDropCell(dragId);
      }}
    >
      <div className="absolute inset-x-0 top-1/2 border-t border-dashed border-gray-200 opacity-0 group-hover/insert:opacity-100 dark:border-[#3a3a3a]" />
      <div className="z-10 flex gap-1 opacity-0 transition group-hover/insert:opacity-100">
        <button
          type="button"
          onClick={() => onInsert(false)}
          className="inline-flex items-center gap-1 rounded border border-gray-200 bg-white px-2 py-0.5 text-[11px] text-gray-600 hover:bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-300 dark:hover:bg-[#3a3a3a]"
        >
          <LuPlus size={11} /> Code
        </button>
        <button
          type="button"
          onClick={() => onInsert(true)}
          className="inline-flex items-center gap-1 rounded border border-gray-200 bg-white px-2 py-0.5 text-[11px] text-gray-600 hover:bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-300 dark:hover:bg-[#3a3a3a]"
        >
          <LuPlus size={11} /> Markdown
        </button>
      </div>
    </div>
  );
};

const EditorNamedNodeRenderer: React.FC<{
  notebook: Notebook;
  run: Run | null;
  path: EditorNodeId[];
  node: EditorGroupNode;
  depth: number;
  orderedNodes: EditorNode[];
  onEdit: () => void;
}> = ({ notebook, run, path, node, depth, orderedNodes, onEdit }) => {
  const dispatch = useDispatch();
  const sendCommand = useSendCommand();
  const pushNotification = usePushNotification();

  const isSelected = notebook.selected_editor_node_id === node.id;
  const isOpen = notebook.editor_open_nodes.has(node.id);
  return (
    <div className="w-full my-1">
      <div
        id={node.id}
        tabIndex={-1}
        className={`flex justify-between select-none rounded px-2 py-1 mb-1 text-gray-500 font-semibold focus:outline-0 ${isSelected ? "bg-blue-200" : "hover:bg-blue-50"}`}
        onClick={() => {
          document.getElementById(node.id)?.focus();
        }}
        onFocus={() =>
          dispatch({
            type: "select_editor_node",
            notebook_id: notebook.id,
            editor_node_id: node.id,
          })
        }
        onBlur={() =>
          dispatch({
            type: "select_editor_node",
            notebook_id: notebook.id,
            editor_node_id: null,
          })
        }
        onKeyDown={(e) => {
          if (e.key === "ArrowUp") {
            e.preventDefault();
            moveToNode(node, orderedNodes, true);
          }
          if (e.key === "ArrowDown") {
            e.preventDefault();
            moveToNode(node, orderedNodes, false);
          }
          if (
            (e.key === "ArrowLeft" && isOpen) ||
            (e.key === "ArrowRight" && !isOpen)
          ) {
            e.preventDefault();
            dispatch({
              type: "toggle_editor_node",
              notebook_id: notebook.id,
              node_id: node.id,
            });
          }
          if (e.ctrlKey && e.key === "Enter") {
            e.preventDefault();
            runCode(path, notebook, dispatch, sendCommand, pushNotification);
          }
        }}
      >
        <div className="mr-4 flex items-center">
          <button
            type="button"
            className="mr-1"
            onClick={(e) => {
              e.stopPropagation();
              dispatch({
                type: "toggle_editor_node",
                notebook_id: notebook.id,
                node_id: node.id,
              });
            }}
          >
            {isOpen ? (
              <LuChevronDown size={16} />
            ) : (
              <LuChevronRight size={16} />
            )}
          </button>
          {node.scope === EditorScope.Own && depth > 0 && (
            <div className="mr-1 text-purple-400">
              <TbCircleDashed size={18} />
            </div>
          )}
          {node.name}
        </div>
        <div>
          {isSelected && (
            <NodeToolbar
              className=""
              node={node}
              notebook={notebook}
              path={path}
              isRoot={depth === 0}
            />
          )}
        </div>
      </div>
      {isOpen && (
        <div className="ml-2">
          {node.children.length === 0 && (
            <div className="flex ml-5">
              <div className="italic mr-3">Group is empty</div>
              <NodeButton2
                onClick={() => {
                  newEditorGroup(notebook, node, path, "child", dispatch);
                }}
              >
                <div className="inline-flex items-center">
                  <LuFolderPlus className="mr-2" />
                  Add group
                </div>
              </NodeButton2>
              <NodeButton2
                onClick={() => {
                  newEditorCode(notebook, path, "child", dispatch);
                }}
              >
                <div className="inline-flex items-center">
                  <LuPlus className="mr-2" />
                  Add code
                </div>
              </NodeButton2>
            </div>
          )}
          {node.children.map((child) => {
            const p = [...path, child.id];
            if (child.type === "Group") {
              return (
                <EditorNamedNodeRenderer
                  key={child.id}
                  notebook={notebook}
                  run={run}
                  path={p}
                  node={child}
                  depth={depth + 1}
                  orderedNodes={orderedNodes}
                  onEdit={onEdit}
                />
              );
            }
            return (
              <EditorCellRenderer
                key={child.id}
                notebook={notebook}
                run={run}
                path={p}
                cell={child}
                orderedNodes={orderedNodes}
                onEdit={onEdit}
              />
            );
          })}
        </div>
      )}
    </div>
  );
};

function moveToNode(
  node: EditorNode,
  orderedNodes: EditorNode[],
  is_up: boolean,
) {
  let idx = orderedNodes.indexOf(node);
  if (is_up) {
    if (idx <= 0) return;
    idx -= 1;
  } else {
    if (idx === orderedNodes.length - 1) return;
    idx += 1;
  }
  const newId = orderedNodes[idx].id;
  const element = document.getElementById(newId);
  if (!element) return;
  const textArea = element.getElementsByTagName("textarea")[0];
  if (textArea) {
    textArea.focus();
    const pos = is_up ? textArea.value.length : 0;
    textArea.setSelectionRange(pos, pos);
  } else {
    element.focus();
  }
}

const DragHandle: React.FC<{
  cell: EditorCell;
}> = ({ cell }) => {
  const onDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData("text/plain", cell.id);
    e.dataTransfer.effectAllowed = "move";
    (e.currentTarget as HTMLElement).closest(".group\\/cell")?.classList.add("opacity-50");
  };
  const onDragEnd = (e: React.DragEvent) => {
    (e.currentTarget as HTMLElement).closest(".group\\/cell")?.classList.remove("opacity-50");
  };
  return (
    <div
      draggable
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      className="mt-1 mr-0.5 flex-shrink-0 opacity-0 group-hover/cell:opacity-100 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 cursor-grab active:cursor-grabbing p-0.5 rounded transition-opacity"
      title="Drag to reorder"
    >
      <LuGripVertical size={14} />
    </div>
  );
};

const EditorCellRenderer: React.FC<{
  notebook: Notebook;
  run: Run | null;
  path: EditorNodeId[];
  cell: EditorCell;
  orderedNodes: EditorNode[];
  onEdit: () => void;
}> = ({ notebook, run, path, cell, orderedNodes, onEdit }) => {
  const dispatch = useDispatch();
  const sendCommand = useSendCommand();
  const pushNotification = usePushNotification();
  const isSelected = notebook.selected_editor_node_id === cell.id;
  const isMd = isMarkdownCell(cell.code);
  const isCollapsed = notebook.collapsed_cells?.has(cell.id) ?? false;
  const output = latestOutputFor(run, cell.id);
  // Syntax-highlight fenced code blocks inside rendered markdown (marked emits
  // <pre><code class="language-…">, but doesn't run a highlighter itself).
  const mdRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = mdRef.current;
    if (!root) return;
    root
      .querySelectorAll<HTMLElement>('pre code[class*="language-"]')
      .forEach((el) => {
        const lang = el.className.match(/language-(\w+)/)?.[1] ?? "rust";
        el.innerHTML = highlightInline(el.textContent ?? "", lang);
      });
  });

  const insertAfter = (markdown: boolean) => {
    const id = uuidv4();
    dispatch({
      type: "new_editor_node",
      notebook_id: notebook.id,
      path,
      editor_node: { type: "Cell", id, code: markdown ? "//md\n" : "" },
      insert_type: "after",
    });
    dispatch({
      type: "select_editor_node",
      notebook_id: notebook.id,
      editor_node_id: id,
    });
    setTimeout(() => focusId(id), 0);
  };

  const advanceOrCreate = () => {
    const idx = orderedNodes.indexOf(cell);
    if (idx >= 0 && idx < orderedNodes.length - 1) {
      moveToNode(cell, orderedNodes, false);
    } else {
      insertAfter(false);
    }
  };

  const runThis = () => {
    if (isMd) {
      dispatch({
        type: "select_editor_node",
        notebook_id: notebook.id,
        editor_node_id: null,
      });
      (document.activeElement as HTMLElement | null)?.blur();
    } else {
      runCode(path, notebook, dispatch, sendCommand, pushNotification);
    }
  };

  const deleteThis = () => removeEditorNode(notebook, path, dispatch);

  const deselectThis = () => {
    dispatch({
      type: "select_editor_node",
      notebook_id: notebook.id,
      editor_node_id: null,
    });
    (document.activeElement as HTMLElement | null)?.blur();
  };

  const toggleType = () =>
    dispatch({
      type: "update_editor_node",
      notebook_id: notebook.id,
      path,
      node_update: { code: isMd ? toCode(cell.code) : toMarkdown(cell.code) },
    });

  const toggleCollapse = () =>
    dispatch({
      type: "toggle_cell_collapse",
      notebook_id: notebook.id,
      cell_id: cell.id,
    });

  // Drop a dragged cell after this one.
  const handleDropCell = (dragId: string) => {
    if (dragId === cell.id) return;
    const dragNode = findNodeById(notebook.editor_root, dragId);
    const dragPath = findPathById(notebook.editor_root, dragId);
    if (!dragNode || !dragPath) return;
    // Remove from old position and re-insert after this cell.
    dispatch({
      type: "remove_editor_node",
      notebook_id: notebook.id,
      path: dragPath,
    });
    dispatch({
      type: "new_editor_node",
      notebook_id: notebook.id,
      path,
      editor_node: dragNode,
      insert_type: "after",
    });
    dispatch({
      type: "select_editor_node",
      notebook_id: notebook.id,
      editor_node_id: dragId,
    });
  };

  // Collapsed markdown view (rendered or not) — show first line only.
  if (isCollapsed && isMd) {
    const firstLine = markdownSource(cell.code).split("\n")[0] || cell.code;
    return (
      <div className="flex items-start group/cell pl-1 my-0.5">
        <DragHandle cell={cell} />
        <div className="flex-1 min-w-0">
          <div
            className="patina-md cursor-pointer rounded px-3 py-1 bg-gray-50/50 border border-gray-100 hover:bg-gray-50 dark:bg-[#2d2d2d]/30 dark:border-[#3a3a3a] dark:hover:bg-[#2d2d2d]"
            onClick={() => toggleCollapse()}
            title="Click to expand"
          >
            <span className="text-xs text-gray-500 line-clamp-1 font-mono truncate block">
              {firstLine || "(empty markdown)"}
            </span>
          </div>
          <InsertBar onInsert={insertAfter} onDropCell={handleDropCell} />
        </div>
      </div>
    );
  }

  // Rendered markdown view (not selected) — click to edit.
  if (isMd && !isSelected) {
    const html = marked.parse(
      markdownSource(cell.code) || "*empty markdown*",
    ) as string;
    return (
      <div className="flex items-start group/cell pl-1">
        <DragHandle cell={cell} />
        <div className="flex-1 min-w-0">
          <div className="flex items-start">
            <button
              type="button"
              onClick={toggleCollapse}
              title="Collapse cell"
              className="mt-1 mr-1 opacity-0 group-hover/cell:opacity-100 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 p-0.5 rounded"
            >
              <LuChevronUp size={12} />
            </button>
            <div
              ref={mdRef}
              className="patina-md cursor-text rounded px-3 py-2 hover:bg-gray-50 dark:text-gray-100 dark:hover:bg-[#2d2d2d] flex-1"
              onClick={() => {
                dispatch({
                  type: "select_editor_node",
                  notebook_id: notebook.id,
                  editor_node_id: cell.id,
                });
                setTimeout(() => focusId(cell.id), 0);
              }}
              dangerouslySetInnerHTML={{ __html: html }}
            />
          </div>
          <InsertBar onInsert={insertAfter} onDropCell={handleDropCell} />
        </div>
      </div>
    );
  }

  // Collapsed code cell (selected or not) — show first line only.
  if (isCollapsed && !isMd) {
    const firstLine = cell.code.split("\n")[0] || "(empty)";
    return (
      <div className="flex items-start group/cell pl-1 my-0.5">
        <DragHandle cell={cell} />
        <div className="flex-1 min-w-0">
          <div
            className="cursor-pointer rounded px-3 py-1 bg-gray-50/50 border border-gray-100 hover:bg-gray-50 dark:bg-[#2d2d2d]/30 dark:border-[#3a3a3a] dark:hover:bg-[#2d2d2d]"
            onClick={() => toggleCollapse()}
            title="Click to expand"
          >
            <span className="text-xs text-gray-500 font-mono truncate block">
              {firstLine}
              {cell.code.includes("\n") ? " …" : ""}
            </span>
          </div>
          {output && (
            <InlineOutput cell={output.cell} execCount={output.execCount} />
          )}
          <InsertBar onInsert={insertAfter} onDropCell={handleDropCell} />
        </div>
      </div>
    );
  }

  return (
    <div
      className={`relative flex items-start pl-1 group/cell ${isSelected ? "border-l-6 border-blue-300" : "border-l-6 border-transparent"}`}
    >
      <DragHandle cell={cell} />
      <div className="flex-1 min-w-0">
        {isSelected && (
          <NodeToolbar
            className="z-10 absolute top-0 right-0 mt-2 mr-2"
            node={cell}
            notebook={notebook}
            path={path}
            isRoot={false}
          />
        )}
        <div
          className={`mb-1 overflow-hidden rounded-md border bg-white dark:bg-[#252526] ${isMd ? "border-amber-300 dark:border-amber-500/50" : "border-gray-300 dark:border-[#3a3a3a]"}`}
        >
        <MonacoCell
          id={cell.id}
          value={cell.code}
          language={notebook.language}
          onChange={(code) => {
            dispatch({
              type: "update_editor_node",
              notebook_id: notebook.id,
              path,
              node_update: { code: code },
            });
            onEdit();
          }}
          onRun={runThis}
          onAdvanceAndRun={() => {
            runThis();
            advanceOrCreate();
          }}
          onFocus={() =>
            dispatch({
              type: "select_editor_node",
              notebook_id: notebook.id,
              editor_node_id: cell.id,
            })
          }
          onBlur={() => {}}
          onEscape={deselectThis}
          onMoveUp={() => moveToNode(cell, orderedNodes, true)}
          onMoveDown={() => moveToNode(cell, orderedNodes, false)}
        />
        <div className="flex items-center justify-between border-t border-gray-200 bg-gray-50 px-2 py-1 text-xs dark:border-[#3a3a3a] dark:bg-[#2d2d2d]">
          <div className="flex items-center gap-2">
            <button
              type="button"
              title={
                isMd
                  ? "Render (Ctrl/Shift+Enter)"
                  : "Run cell (Ctrl/Shift+Enter)"
              }
              onClick={runThis}
              className="inline-flex items-center font-medium text-gray-600 hover:text-green-700 dark:text-gray-300 dark:hover:text-green-400"
            >
              <LuPlay className="mr-1" size={12} />
              {isMd ? "Render" : "Run"}
              <span className="ml-2 text-gray-400">⇧⏎</span>
            </button>
            <button
              type="button"
              title="Collapse cell"
              onClick={toggleCollapse}
              className="inline-flex items-center gap-1 hover:text-purple-600"
            >
              <LuChevronUp size={13} />
            </button>
          </div>
          <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
            <button
              type="button"
              title={isMd ? "Convert to code" : "Convert to markdown"}
              onClick={toggleType}
              className="inline-flex items-center gap-1 hover:text-amber-700"
            >
              {isMd ? <LuCode size={13} /> : <LuText size={13} />}
              {isMd ? "Code" : "MD"}
            </button>
            <button
              type="button"
              title="Delete cell"
              onClick={deleteThis}
              className="hover:text-red-600"
            >
              <LuTrash2 size={13} />
            </button>
          </div>
        </div>
      </div>
      {!isMd && output && (
        <InlineOutput cell={output.cell} execCount={output.execCount} />
      )}
      <InsertBar onInsert={insertAfter} onDropCell={handleDropCell} />
      </div>
    </div>
  );
};

function crawlOpen(
  node: EditorNode,
  opens: Set<EditorNodeId>,
  out: EditorNode[],
) {
  out.push(node);
  if (node.type === "Group" && opens.has(node.id)) {
    for (const child of node.children) {
      crawlOpen(child, opens, out);
    }
  }
}

const EditorPanel: React.FC<{
  notebook: Notebook;
  run: Run | null;
  onEdit: () => void;
}> = ({ notebook, run, onEdit }) => {
  const dispatch = useDispatch();
  const sendCommand = useSendCommand();
  const onSave = useCallback(() => {
    saveNotebook(notebook, dispatch, sendCommand);
  }, [notebook, dispatch, sendCommand]);

  // Flatten the root group: render its children as a plain cell list.
  const root = notebook.editor_root;
  const orderedNodes: EditorNode[] = [];
  for (const child of root.children) {
    crawlOpen(child, notebook.editor_open_nodes, orderedNodes);
  }

  const addAtEnd = (markdown: boolean) => {
    const id = uuidv4();
    dispatch({
      type: "new_editor_node",
      notebook_id: notebook.id,
      path: [],
      editor_node: { type: "Cell", id, code: markdown ? "//md\n" : "" },
      insert_type: "child",
    });
    dispatch({
      type: "select_editor_node",
      notebook_id: notebook.id,
      editor_node_id: id,
    });
    setTimeout(() => focusId(id), 0);
  };

  return (
    <div className="h-full">
      <div className="sticky top-0 z-10 flex items-center gap-2 bg-white/90 px-2 py-1 backdrop-blur dark:bg-[#1e1e1e]/90">
        <ToolButton onClick={onSave}>
          {notebook.save_in_progress ? (
            <LuLoaderCircle className="h-4 w-4 animate-spin" />
          ) : (
            <LuSave className="h-4 w-4" />
          )}
        </ToolButton>
        <span className="ml-auto flex items-center">
          <LanguageIcon language={notebook.language} size={15} />
        </span>
        <select
          value={notebook.language}
          title="Notebook language — applies to the next kernel (restart to switch a running one)"
          onChange={(e) => {
            const language = e.target.value as Language;
            dispatch({
              type: "set_notebook_language",
              notebook_id: notebook.id,
              language,
            });
            sendCommand({
              type: "SetLanguage",
              notebook_id: notebook.id,
              language,
            });
          }}
          className="rounded-md border border-gray-200 bg-white px-2 py-0.5 text-[11px] font-medium text-gray-600 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-300"
        >
          {LANGUAGES.map((l) => (
            <option key={l} value={l}>
              {l}
            </option>
          ))}
        </select>
      </div>

      <div className="mx-auto max-w-4xl px-2 pb-32 pt-2">
        {root.children.length === 0 && (
          <div className="py-8 text-center text-sm text-gray-400">
            Empty notebook — add a cell below.
          </div>
        )}
        {root.children.map((child) => {
          const p = [child.id];
          if (child.type === "Group") {
            return (
              <EditorNamedNodeRenderer
                key={child.id}
                notebook={notebook}
                run={run}
                path={p}
                node={child}
                depth={0}
                orderedNodes={orderedNodes}
                onEdit={onEdit}
              />
            );
          }
          return (
            <EditorCellRenderer
              key={child.id}
              notebook={notebook}
              run={run}
              path={p}
              cell={child}
              orderedNodes={orderedNodes}
              onEdit={onEdit}
            />
          );
        })}

        <div className="mt-3 flex justify-center gap-2">
          <button
            type="button"
            onClick={() => addAtEnd(false)}
            className="inline-flex items-center gap-1 rounded border border-gray-200 bg-white px-3 py-1 text-sm text-gray-600 hover:bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-300 dark:hover:bg-[#3a3a3a]"
          >
            <LuPlus size={14} /> Code
          </button>
          <button
            type="button"
            onClick={() => addAtEnd(true)}
            className="inline-flex items-center gap-1 rounded border border-gray-200 bg-white px-3 py-1 text-sm text-gray-600 hover:bg-gray-50 dark:border-[#3a3a3a] dark:bg-[#2d2d2d] dark:text-gray-300 dark:hover:bg-[#3a3a3a]"
          >
            <LuPlus size={14} /> Markdown
          </button>
        </div>
      </div>
    </div>
  );
};

const ToolButton: React.FC<{
  onClick: () => void;
  children: React.ReactNode;
}> = ({ onClick, children }) => (
  <button
    type="button"
    onClick={onClick}
    className="rounded bg-gray-200 px-3 py-2 text-black hover:bg-gray-300 dark:bg-[#2d2d2d] dark:text-gray-100 dark:hover:bg-[#3a3a3a]"
  >
    {children}
  </button>
);

const NodeButton2: React.FC<{
  onClick: () => void;
  children: React.ReactNode;
}> = ({ onClick, children }) => (
  <button
    type="button"
    onClick={(e) => {
      e.preventDefault();
      e.stopPropagation();
      onClick();
    }}
    className="text-gray-700 bg-gray-200 py-1 px-3 mr-1 rounded hover:bg-gray-400"
  >
    {children}
  </button>
);

export default EditorPanel;
