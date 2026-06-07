// Shiki highlighter — VS Code's engine + themes. Synchronous (so it works as
// react-simple-code-editor's `highlight` prop) via the core build + JS regex
// engine and statically-imported grammar/themes.
import { createHighlighterCoreSync, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import rust from "shiki/langs/rust.mjs";
import githubDark from "shiki/themes/github-dark.mjs";
import githubLight from "shiki/themes/github-light.mjs";
import { isDark } from "./theme";

const hl: HighlighterCore = createHighlighterCoreSync({
  langs: [rust],
  themes: [githubLight, githubDark],
  engine: createJavaScriptRegexEngine(),
});

const loadedLangs = new Set(hl.getLoadedLanguages());

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/**
 * Highlighted HTML as inline-styled `<span>`s that preserve the exact source
 * text (so it overlays the editor textarea and renders inside markdown `<pre>`).
 * Falls back to escaped plain text for unknown languages.
 */
export function highlightInline(code: string, lang = "rust"): string {
  const language = loadedLangs.has(lang) ? lang : null;
  if (!language) return esc(code);
  const theme = isDark() ? "github-dark" : "github-light";
  let lines: ReturnType<typeof hl.codeToTokens>["tokens"];
  try {
    lines = hl.codeToTokens(code, { lang: language, theme }).tokens;
  } catch {
    return esc(code);
  }
  return lines
    .map((line) =>
      line
        .map((t) => {
          let style = `color:${t.color ?? "inherit"}`;
          if (t.fontStyle === 1) style += ";font-style:italic";
          else if (t.fontStyle === 2) style += ";font-weight:bold";
          return `<span style="${style}">${esc(t.content)}</span>`;
        })
        .join(""),
    )
    .join("\n");
}
