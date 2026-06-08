import Editor from "@monaco-editor/react";
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { monacoLang, monacoTheme } from "../core/monaco-setup";

interface MonacoCellProps {
  value: string;
  language: string;
  id: string;
  onChange: (code: string) => void;
  onRun: () => void;
  onAdvanceAndRun: () => void;
  onFocus: () => void;
  onBlur: () => void;
  onEscape: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

function cursorAtFirstLine(
  editor: import("monaco-editor").editor.IStandaloneCodeEditor,
): boolean {
  const pos = editor.getPosition();
  if (!pos) return false;
  return pos.lineNumber === 1 && pos.column === 1;
}

function cursorAtLastLine(
  editor: import("monaco-editor").editor.IStandaloneCodeEditor,
): boolean {
  const model = editor.getModel();
  if (!model) return false;
  const pos = editor.getPosition();
  if (!pos) return false;
  const lastLine = model.getLineCount();
  if (pos.lineNumber !== lastLine) return false;
  const lastCol = model.getLineMaxColumn(lastLine);
  return pos.column === lastCol;
}

const MonacoCell: React.FC<MonacoCellProps> = ({
  value,
  language,
  id,
  onChange,
  onRun,
  onAdvanceAndRun,
  onFocus,
  onBlur,
  onEscape,
  onMoveUp,
  onMoveDown,
}) => {
  const [theme, setTheme] = useState(monacoTheme());
  const containerRef = useRef<HTMLDivElement>(null);
  const onFocusRef = useRef(onFocus);
  const onBlurRef = useRef(onBlur);
  const onRunRef = useRef(onRun);
  const onAdvanceRef = useRef(onAdvanceAndRun);
  const onEscapeRef = useRef(onEscape);
  const onMoveUpRef = useRef(onMoveUp);
  const onMoveDownRef = useRef(onMoveDown);

  // Keep refs in sync
  onFocusRef.current = onFocus;
  onBlurRef.current = onBlur;
  onRunRef.current = onRun;
  onAdvanceRef.current = onAdvanceAndRun;
  onEscapeRef.current = onEscape;
  onMoveUpRef.current = onMoveUp;
  onMoveDownRef.current = onMoveDown;

  useEffect(() => {
    const update = () => setTheme(monacoTheme());
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const tid = setInterval(() => {
      const t = monacoTheme();
      setTheme((prev) => (prev !== t ? t : prev));
    }, 300);
    return () => clearInterval(tid);
  }, []);

  const handleMount = useCallback(
    (
      editor: import("monaco-editor").editor.IStandaloneCodeEditor,
      monaco: typeof import("monaco-editor"),
    ) => {
      editor.addAction({
        id: "patina-run-cell",
        label: "Run Cell",
        keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter],
        run: () => onRunRef.current(),
      });
      editor.addAction({
        id: "patina-run-advance",
        label: "Run and Advance",
        keybindings: [
          monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.Enter,
        ],
        run: () => onAdvanceRef.current(),
      });
      editor.addAction({
        id: "patina-escape",
        label: "Deselect Cell",
        keybindings: [monaco.KeyCode.Escape],
        run: () => onEscapeRef.current(),
      });

      editor.onKeyDown((e) => {
        if (e.keyCode === monaco.KeyCode.UpArrow && cursorAtFirstLine(editor)) {
          e.preventDefault();
          e.stopPropagation();
          onMoveUpRef.current();
        }
        if (
          e.keyCode === monaco.KeyCode.DownArrow &&
          cursorAtLastLine(editor)
        ) {
          e.preventDefault();
          e.stopPropagation();
          onMoveDownRef.current();
        }
      });

      editor.onDidFocusEditorText(() => onFocusRef.current());
      editor.onDidBlurEditorText(() => onBlurRef.current());

      // Suppress Monaco's own context menu suggestions
      editor.updateOptions({ contextmenu: false });

      setTimeout(() => editor.focus(), 0);
    },
    [],
  );

  const langId = monacoLang(language);

  return (
    <div id={id} ref={containerRef} className="patina-monaco-cell">
      <Editor
        height="auto"
        language={langId}
        value={value}
        theme={theme}
        onChange={(v) => onChange(v ?? "")}
        onMount={handleMount}
        loading={
          <div className="h-8 animate-pulse rounded bg-gray-100 dark:bg-[#2d2d2d]" />
        }
        options={{
          minimap: { enabled: false },
          lineNumbers: "on",
          lineNumbersMinChars: 3,
          glyphMargin: false,
          folding: true,
          lineDecorationsWidth: 0,
          renderLineHighlight: "all",
          scrollBeyondLastLine: false,
          scrollbar: {
            vertical: "hidden",
            horizontal: "hidden",
            alwaysConsumeMouseWheel: false,
          },
          overviewRulerLanes: 0,
          hideCursorInOverviewRuler: true,
          overviewRulerBorder: false,
          wordWrap: "on",
          automaticLayout: true,
          fixedOverflowWidgets: true,
          fontSize: 12.5,
          fontFamily:
            '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
          lineHeight: 20,
          padding: { top: 10, bottom: 10 },
          tabSize: 4,
          insertSpaces: true,
          suggest: { showWords: true, showSnippets: false },
          quickSuggestions: true,
          bracketPairColorization: { enabled: true },
          matchBrackets: "always",
          autoClosingBrackets: "always",
          autoClosingQuotes: "always",
          copyWithSyntaxHighlighting: true,
          rulers: [],
        }}
      />
    </div>
  );
};

export default MonacoCell;
