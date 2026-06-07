import type { EditorNodeId } from "./notebook";

// Focus an editor node by id: prefer its inner <textarea> (code/markdown
// editors), falling back to the node element itself (e.g. group headers).
export function focusId(id: EditorNodeId) {
  const element = document.getElementById(id)!;
  const textArea = element?.getElementsByTagName("textarea")[0];
  if (textArea) {
    textArea.focus();
  } else {
    element?.focus();
  }
}
