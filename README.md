# Patina

![patina — a Rust-native interactive notebook for Rust, Python and JavaScript](https://raw.githubusercontent.com/vbasky/patina/main/docs/banner.svg)

**Name:** *patina* is the sheen that forms on metal as it weathers and oxidizes — i.e.
*rust*. A nod to **Rust**, the language the whole stack (server, kernels, even the
JavaScript engine) is built in.

[![CI](https://img.shields.io/github/actions/workflow/status/vbasky/patina/build.yml?branch=main&logo=github&label=CI)](https://github.com/vbasky/patina/actions)
[![License](https://img.shields.io/github/license/vbasky/patina)](LICENSE-MIT)
[![kernels](https://img.shields.io/badge/kernels-Rust%20·%20Python%20·%20JavaScript-c47b3f?logo=rust)](#languages)
[![UI](https://img.shields.io/badge/UI-React-61dafb?logo=react)](browser/ui)

**Patina is a Rust-native interactive notebook.** Cells run in **Rust**, **Python**,
or **JavaScript** — and the stack around them (web server, kernels, wire protocol,
and even the JavaScript engine) is written entirely in Rust. No Jupyter, no separate
kernel-protocol plumbing.

Code and outputs stay separate, run history is preserved instead of clobbered, and
live variables are inspectable.

> The original design, server, and UI are upstream's; Patina replaces the single
> Python kernel with native Rust / Python / JavaScript kernels. See `LICENSE-MIT` /
> `LICENSE-APACHE`.

## Languages

Each notebook runs one language. Pick it when you create the notebook (dropdown in the
file sidebar) or switch later from the editor toolbar — a switch takes effect for the
next kernel, so restart the kernel to change a running one. The server launches the
matching kernel binary.

| Language     | Kernel                  | Engine                        | State across cells     |
| ------------ | ----------------------- | ----------------------------- | ---------------------- |
| Rust         | `patina-kernel`         | `evcxr` (compiled per cell)   | full                   |
| Python       | `patina-kernel-python`  | embedded CPython (`pyo3`)     | full                   |
| JavaScript   | `patina-kernel-js`      | `boa` (pure Rust, no V8)      | `var` / globals only   |

- **Rust** — pull crates inside a cell with `:dep foo = "1"`.
- **Python** — embeds CPython via pyo3; `import` resolves against that Python's packages.
- **JavaScript** — top-level `let`/`const` don't persist across cells (a boa/REPL limit);
  `var` and global assignments do.

## Rich output

A cell renders its last expression — as text, or as HTML / images:

- **Python** (Jupyter-style): pandas `DataFrame`s become HTML tables, matplotlib
  figures are captured as inline PNGs, and any object with
  `_repr_html_` / `_repr_svg_` / `_repr_png_` is rendered. Install the libraries into
  the kernel's Python (`pip install pandas matplotlib`).
- **Rust**: the equivalents are **`polars`** (dataframes) and **`plotters`** (charts).
  plotters' `evcxr_figure(...)` renders inline when returned as the last expression,
  and two built-in helpers display any markup:

  ```rust
  patina_html(&html);   // any HTML fragment — e.g. a polars table
  patina_svg(&svg);     // inline SVG — e.g. a plotters SVGBackend string
  ```

  evcxr persists `let` bindings across cells, so give them a concrete type and avoid a
  trailing `?` (which makes type inference fail): `let df: DataFrame = df!(...).unwrap();`

## Workspace & files

The file browser is rooted at a **`./notebooks`** workspace (override with
`PATINA_WORKSPACE`) — only that folder is shown, with folder navigation and a
breadcrumb. From the sidebar you can:

- **Create** `.tsnb` notebooks in the current folder, in any language.
- **Upload** `.tsnb`, `.md`, or `.ipynb` files. Markdown and Jupyter notebooks are
  converted to `.tsnb` (prose → Markdown cells, code → code cells); an `.ipynb`'s
  language is detected from its kernel metadata.
- **Delete** files.

The light/dark theme follows your OS by default and can be changed from the top bar.

## How it works

```text
 browser UI  ──ws──▶  patina (server, Rust)  ──tcp/bincode──▶  kernel (Rust)
 React, embedded                                               evcxr · pyo3 · boa
 in the binary
```

- **`patina`** — Axum web server: serves the embedded UI, manages the notebook
  workspace, and spawns the per-language kernel as a child process.
- **`patina-kernel` / `-python` / `-js`** — one binary per language, all speaking the
  shared `comm` protocol over TCP. They share a networking run-loop and differ only in
  the executor (`evcxr::CommandContext`, embedded CPython, or `boa`). stdout streams
  live; the last expression renders as text or HTML; live variables feed the inspector.
- **`common`** (`comm`) — the wire protocol (length-delimited bincode), the message
  types, and the shared kernel runtime.

## Getting started (from source)

You need a Rust toolchain, Node.js (for the UI), and a Python 3 install (the Python
kernel embeds CPython through pyo3).

```bash
# 1. Build the UI (it gets embedded into the server binary)
cd browser/ui && npm install && ./build.sh && cd ../..

# 2. Build the server and all three kernels
cargo build

# 3. Run it
./target/debug/patina          # http://127.0.0.1:4050   (use --port to change)
```

Then create a notebook and run a cell with `Shift`+`Enter`:

```rust
let answer: i32 = 40 + 2;
println!("hello from the rust kernel");
answer            // -> 42
```

## Compile speed (Rust cells)

Every `:dep` compiles that crate from source the first time, which is slow for large
crates like polars or plotters. The Rust kernel auto-enables **sccache** when it's on
your `PATH` (`brew install sccache` or `cargo install sccache`), caching compiled
artifacts across cells, kernel restarts, and notebooks — so you pay the build cost
once per machine. Trimming a crate's feature set helps too.

## Status

Experimental. The kernels support cell evaluation, streamed stdout/stderr, text /
HTML / image output, and globals inspection. State save/load and kernel forking are
**not yet** supported (evcxr's compiled context can't be cheaply snapshotted/forked,
and the Python/JS kernels don't persist state to disk).

## License

MIT or Apache-2.0, inherited from Twinsong. See `LICENSE-MIT` and `LICENSE-APACHE`.
