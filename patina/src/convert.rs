// Convert foreign notebook/document formats into Patina notebooks.
//
//   * `.md`     — prose becomes Markdown cells; fenced code blocks become
//                 runnable code cells (rust, python, js/javascript/typescript).
//                 Other-language fences stay in the prose.
//   * `.ipynb`  — Jupyter cells map directly (code → code, markdown/raw → md).
//
// Markdown cells are ordinary code cells whose first line is the `//md` marker
// (matching the frontend convention in core/cells.ts).

use crate::notebook::{EditorCell, EditorGroup, EditorId, EditorNode, Notebook, ScopeType};
use anyhow::Context;
use comm::messages::Language;
use uuid::Uuid;

const MD_MARKER: &str = "//md";

/// A parsed cell: `(is_markdown, text)`.
type Cell = (bool, String);

fn editor_cell(is_markdown: bool, text: String) -> EditorNode {
    let code = if is_markdown {
        format!("{MD_MARKER}\n{text}")
    } else {
        text
    };
    EditorNode::Cell(EditorCell {
        id: EditorId::new(Uuid::new_v4()),
        code,
    })
}

/// Build a notebook from parsed cells. Empty input yields a single empty cell.
pub(crate) fn build_notebook(path: String, language: Language, cells: Vec<Cell>) -> Notebook {
    let mut children: Vec<EditorNode> = cells
        .into_iter()
        .filter(|(is_md, text)| !(*is_md && text.trim().is_empty()))
        .map(|(is_md, text)| editor_cell(is_md, text))
        .collect();
    if children.is_empty() {
        children.push(editor_cell(false, String::new()));
    }
    let editor_root = EditorGroup {
        id: EditorId::new(Uuid::new_v4()),
        name: "project".to_string(),
        scope: ScopeType::Own,
        children,
    };
    let mut editor_open_nodes = Vec::new();
    editor_root.collect_group_ids(&mut editor_open_nodes);
    Notebook {
        editor_root,
        editor_open_nodes,
        path,
        language,
        runs: Default::default(),
        run_order: Vec::new(),
        observer: None,
    }
}

/// Map a Jupyter `metadata.kernelspec.language` (or `language_info.name`) to a
/// Patina language, defaulting to Rust.
pub(crate) fn ipynb_language(json: &str) -> Language {
    let doc: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Language::Rust,
    };
    let name = doc
        .pointer("/metadata/kernelspec/language")
        .or_else(|| doc.pointer("/metadata/language_info/name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match name.as_str() {
        "python" => Language::Python,
        "typescript" | "ts" | "javascript" | "js" | "node" => Language::TypeScript,
        _ => Language::Rust,
    }
}

/// Map a Markdown fence language tag to a Patina language (or None for non-code).
fn code_lang(lang: &str) -> Option<Language> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "" | "rust" | "rs" => Some(Language::Rust),
        "python" | "py" => Some(Language::Python),
        "javascript" | "js" | "typescript" | "ts" => Some(Language::TypeScript),
        _ => None,
    }
}

/// Split Markdown into Markdown/code cells. Fenced code blocks (rust, python,
/// js, etc.) become runnable code cells; everything else stays Markdown.
pub(crate) fn markdown_to_cells(md: &str) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut md_buf: Vec<&str> = Vec::new();
    let mut code_buf: Vec<&str> = Vec::new();
    let mut in_code = false; // inside a recognized code fence
    let mut in_other = false; // inside a fence we keep in the prose

    let flush_md = |buf: &mut Vec<&str>, cells: &mut Vec<Cell>| {
        let text = buf.join("\n");
        buf.clear();
        if !text.trim().is_empty() {
            cells.push((true, text.trim_end().to_string()));
        }
    };

    for line in md.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```");
        if in_code {
            if is_fence {
                cells.push((false, code_buf.join("\n")));
                code_buf.clear();
                in_code = false;
            } else {
                code_buf.push(line);
            }
        } else if in_other {
            md_buf.push(line);
            if is_fence {
                in_other = false;
            }
        } else if is_fence {
            let lang = &trimmed[3..];
            if code_lang(lang).is_some() {
                flush_md(&mut md_buf, &mut cells);
                in_code = true;
            } else {
                md_buf.push(line);
                in_other = true;
            }
        } else {
            md_buf.push(line);
        }
    }
    // Flush any trailing buffers (handles unclosed fences gracefully).
    if !code_buf.is_empty() {
        cells.push((false, code_buf.join("\n")));
    }
    flush_md(&mut md_buf, &mut cells);
    cells
}

/// Join an ipynb `source` field, which may be a string or an array of lines.
fn join_source(source: &serde_json::Value) -> String {
    match source {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(lines) => lines
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Convert a Jupyter `.ipynb` document into cells.
pub(crate) fn ipynb_to_cells(json: &str) -> anyhow::Result<Vec<Cell>> {
    let doc: serde_json::Value =
        serde_json::from_str(json).context("notebook is not valid JSON")?;
    let cells = doc
        .get("cells")
        .and_then(|c| c.as_array())
        .context("ipynb has no `cells` array")?;
    Ok(cells
        .iter()
        .filter_map(|cell| {
            let kind = cell.get("cell_type")?.as_str()?;
            let text = join_source(cell.get("source")?);
            // code → code cell; markdown/raw → markdown cell.
            Some((kind != "code", text))
        })
        .collect())
}

/// Convert a Patina notebook to Jupyter .ipynb JSON.
pub(crate) fn notebook_to_ipynb(notebook: &Notebook) -> String {
    let cells: Vec<serde_json::Value> = notebook
        .editor_root
        .children
        .iter()
        .filter_map(|node| {
            let (is_md, code) = match node {
                EditorNode::Cell(cell) => (
                    cell.code.starts_with("//md"),
                    if cell.code.starts_with("//md") {
                        cell.code.replacen("//md\n", "", 1)
                    } else {
                        cell.code.clone()
                    },
                ),
                _ => return None,
            };
            let mut cell = serde_json::Map::new();
            if is_md {
                cell.insert("cell_type".into(), "markdown".into());
                cell.insert("source".into(), vec![code].into());
            } else {
                cell.insert("cell_type".into(), "code".into());
                cell.insert("source".into(), vec![code].into());
                cell.insert("outputs".into(), serde_json::Value::Array(vec![]));
                cell.insert(
                    "execution_count".into(),
                    serde_json::Value::Null,
                );
            }
            cell.insert("metadata".into(), serde_json::Map::new().into());
            Some(serde_json::Value::Object(cell))
        })
        .collect();

    let language = match notebook.language {
        Language::Python => "python",
        Language::TypeScript => "javascript",
        _ => "rust",
    };

    let notebook_json = serde_json::json!({
        "nbformat": 4,
        "nbformat_minor": 5,
        "metadata": {
            "kernelspec": {
                "display_name": language,
                "language": language,
                "name": language,
            },
            "language_info": {
                "name": language,
            },
        },
        "cells": cells,
    });

    serde_json::to_string_pretty(&notebook_json).unwrap_or_default()
}
