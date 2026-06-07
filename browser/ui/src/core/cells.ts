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
