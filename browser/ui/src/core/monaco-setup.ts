import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import { isDark } from "./theme";

// Use local Monaco build (no CDN) — required since the server embeds everything.
loader.config({ monaco });

let themeRegistered = false;

function registerThemes() {
  if (themeRegistered) return;
  themeRegistered = true;

  monaco.editor.defineTheme("patina-light", {
    base: "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: "6a9955" },
      { token: "keyword", foreground: "0000ff" },
      { token: "string", foreground: "a31515" },
      { token: "number", foreground: "098658" },
      { token: "type", foreground: "267f99" },
      { token: "function", foreground: "795e26" },
      { token: "variable", foreground: "001080" },
    ],
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#1e1e1e",
      "editor.lineHighlightBackground": "#f0f0f0",
      "editorLineNumber.foreground": "#999999",
      "editorLineNumber.activeForeground": "#1e1e1e",
      "editor.selectionBackground": "#add6ff",
      "editorCursor.foreground": "#000000",
      "editor.inactiveSelectionBackground": "#e5ebf1",
      "editorWidget.background": "#ffffff",
      "editorWidget.border": "#c8c8c8",
    },
  });

  monaco.editor.defineTheme("patina-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "6a9955" },
      { token: "keyword", foreground: "569cd6" },
      { token: "string", foreground: "ce9178" },
      { token: "number", foreground: "b5cea8" },
      { token: "type", foreground: "4ec9b0" },
      { token: "function", foreground: "dcdcaa" },
      { token: "variable", foreground: "9cdcfe" },
    ],
    colors: {
      "editor.background": "#252526",
      "editor.foreground": "#d4d4d4",
      "editor.lineHighlightBackground": "#2d2d2d",
      "editorLineNumber.foreground": "#858585",
      "editorLineNumber.activeForeground": "#c6c6c6",
      "editor.selectionBackground": "#264f78",
      "editorCursor.foreground": "#aeafad",
      "editor.inactiveSelectionBackground": "#3a3d41",
      "editorWidget.background": "#252526",
      "editorWidget.border": "#454545",
    },
  });
}

// Call early to ensure themes are ready before any editor mounts.
registerThemes();

export function monacoTheme(): string {
  return isDark() ? "patina-dark" : "patina-light";
}

export function monacoLang(lang: string): string {
  switch (lang) {
    case "Rust":
      return "rust";
    case "Python":
      return "python";
    case "TypeScript":
    case "JavaScript":
      return "typescript";
    default:
      return "rust";
  }
}
