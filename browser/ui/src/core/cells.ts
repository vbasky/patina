// Markdown cells are stored as ordinary code cells whose first line is the
// marker `//md`. This keeps them round-tripping through the existing backend
// (which only knows about code cells) with no protocol/storage changes — the
// frontend renders them as Markdown and never sends them to the kernel.

export const MD_MARKER = "//md";

export function isMarkdownCell(code: string): boolean {
  const t = code.trimStart();
  return t === MD_MARKER || /^\/\/md\b/.test(t);
}

/** The Markdown source (everything after the marker line). */
export function markdownSource(code: string): string {
  const i = code.indexOf("\n");
  return i === -1 ? "" : code.slice(i + 1);
}

export function toMarkdown(code: string): string {
  return isMarkdownCell(code) ? code : `${MD_MARKER}\n${code}`;
}

export function toCode(code: string): string {
  return isMarkdownCell(code) ? markdownSource(code) : code;
}

/** Toggle comment on the current line or selection. */
export function toggleComment(code: string): string {
  const lines = code.split("\n");
  if (lines.length === 1) {
    return code.startsWith("//") ? code.slice(2) : `//${code}`;
  }
  // Check if all non-empty lines are already commented
  const nonEmpty = lines.filter((l) => l.trim() !== "");
  const allCommented = nonEmpty.length > 0 && nonEmpty.every((l) => l.trimStart().startsWith("//"));
  return lines
    .map((l) => {
      if (l.trim() === "") return l;
      if (allCommented) {
        const trimmed = l.trimStart();
        return l.replace(trimmed, trimmed.slice(2));
      }
      return `// ${l}`;
    })
    .join("\n");
}

/** Apply cell magics (%%time, %%capture, %%html) by wrapping code. */
export function applyMagics(code: string, language: string): string {
  const lines = code.split("\n");
  const magics: string[] = [];
  let i = 0;
  while (i < lines.length && lines[i].trimStart().startsWith("%%")) {
    magics.push(lines[i].trim());
    i++;
  }
  if (magics.length === 0) return code;

  let body = lines.slice(i).join("\n");

  for (const m of magics) {
    if (m === "%%time") {
      if (language === "Python") {
        body = `import time as _patina_time\n_patina_t0 = _patina_time.time()\n${body}\nprint(f"Wall time: {_patina_time.time() - _patina_t0:.4f}s")`;
      } else if (language === "Rust") {
        body = `let _patina_t0 = std::time::Instant::now();\n${body}\neprintln!("Wall time: {:?}", _patina_t0.elapsed());`;
      } else {
        body = `console.time("cell");\n${body}\nconsole.timeEnd("cell");`;
      }
    } else if (m === "%%capture") {
      if (language === "Python") {
        body = `import io as _patina_io, sys as _patina_sys\n_patina_stdout = _patina_sys.stdout\n_patina_stderr = _patina_sys.stderr\n_patina_sys.stdout = _patina_io.StringIO()\n_patina_sys.stderr = _patina_io.StringIO()\ntry:\n${body.split("\n").map((l) => "    " + l).join("\n")}\nfinally:\n    print(_patina_sys.stdout.getvalue())\n    print(_patina_sys.stderr.getvalue(), file=_patina_stderr)\n    _patina_sys.stdout = _patina_stdout\n    _patina_sys.stderr = _patina_stderr`;
      }
    } else if (m === "%%html") {
      if (language === "Python") {
        body = `from IPython.display import HTML as _patina_HTML\n_patina_result = None\n${body}\nif _patina_result is not None:\n    _patina_HTML(_patina_result)`;
      } else if (language === "Rust") {
        body = `${body}\n// %%html: use patina_html(...) to render HTML output`;
      }
    }
  }

  return body;
}
