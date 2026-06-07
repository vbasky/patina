<p align="center">
    <img width="160" src="browser/ui/public/patina.svg">
</p>

# Patina

**Patina is a Rust-only interactive notebook.** It runs **Rust** cells directly —
no Python, no Jupyter — by embedding the [`evcxr`](https://github.com/evcxr/evcxr)
evaluation engine as a native kernel. The server, the kernel, and the protocol
are all Rust; the only non-Rust piece is the browser UI.

Code and outputs are separated, results history is preserved, and memory state is
inspectable — but the language you actually compute in is Rust.

> Patina is a fork of **[Twinsong](https://github.com/spirali/twinsong)** by Ada Böhm.
> The original design, server, and UI are upstream's; Patina swaps the Python kernel
> for a native Rust one. See `LICENSE-MIT` / `LICENSE-APACHE`.

## Why

Jupyter is clumsy for Rust: it leans on a Python host, a separate kernel process
protocol, and a document model where re-running a cell clobbers history. Patina
keeps that cleaner model and swaps the language: every cell is Rust,
compiled and run incrementally by `evcxr`, with `:dep` support for pulling crates.

## How it works

```
 browser UI  --ws-->  patina (server, Rust)  --tcp/bincode-->  patina-kernel (Rust)
                                                                  +- evcxr::EvalContext
```

- **`patina`** — the web server.
- **`patina-kernel`** — a new crate: a kernel that speaks the same `comm` wire
  protocol but evaluates Rust via an embedded `evcxr::EvalContext`. stdout streams
  live, the final expression renders as text or HTML (`EVCXR_BEGIN_CONTENT`), and
  `evcxr`'s live variables feed the globals inspector.
- **`common`** — the shared protocol (`comm`), unchanged.

## Getting started (from source)

You need a Rust toolchain (with `cargo`) and Node.js for the UI.

```bash
# 1. Build the frontend (embedded into the server binary)
cd browser/ui && npm install && ./build.sh && cd ../..

# 2. Build the native stack (server + Rust kernel)
cargo build            # builds `patina` and `patina-kernel` (default-members)

# 3. Run it
./target/debug/patina          # opens at http://127.0.0.1:4050
```

The server launches `patina-kernel` automatically (it looks next to the server
binary, or at `$PATINA_KERNEL`). Create a notebook, type Rust in a cell, and run it:

```rust
let answer: i32 = 40 + 2;
println!("hello from the rust kernel");
answer            // -> 42
```

Add crates inside a cell with evcxr's directive:

```rust
:dep ndarray = "0.16"
```

## Languages

Each notebook picks a language (kernel) — choose it when creating the notebook
(dropdown in the file sidebar) or switch later from the dropdown in the editor
toolbar (applies to the next kernel; restart to switch a running one). Uploaded
`.ipynb` files infer their language from the kernel metadata.

- **Rust** — cells run through `evcxr`; add crates with `:dep`.
- **Python** — embedded CPython (pyo3); state persists across cells.
- **JavaScript** — the pure-Rust [`boa`](https://github.com/boa-dev/boa) engine
  (no V8). `var`/global state persists; top-level `let`/`const` may not carry
  across cells.

## Rich output

A cell renders its last expression. Beyond text, both kernels can emit HTML:

**Python** — works like Jupyter's inline backend:

- **pandas** `DataFrame`s render as HTML tables automatically (`_repr_html_`).
- **matplotlib** figures are captured as inline PNGs (just `plt.plot(...)`).
- Any object implementing `_repr_html_` / `_repr_svg_` / `_repr_png_` is shown.

**Rust** — the equivalents are **`polars`** (dataframes) and **`plotters`**
(charts). Two helpers are available in every cell to emit rich content:

```rust
patina_html(&html);   // any HTML fragment — e.g. a polars table rendered to HTML
patina_svg(&svg);     // inline SVG — e.g. a plotters SVGBackend string
```

For example, render a `plotters` chart to an SVG `String` and pass it to
`patina_svg(...)`; print a `polars` `DataFrame` with `println!("{df}")` for a text
table, or build an HTML table and call `patina_html(...)`.

## Example notebooks

Patina ships with a set of ready-to-run notebooks in
[`notebooks/`](./notebooks/) — a from-scratch tour of modern AI (agents,
RAG, attention, a net that learns XOR, …), all offline and Rust-only. They show up
automatically in the file browser: the workspace defaults to `./notebooks` (override
with `PATINA_WORKSPACE`), so just launch Patina and open one.

## Status

Experimental, like its upstream. The kernels support cell evaluation, streamed
stdout/stderr, text/HTML output, and globals inspection. State save/load and
kernel forking are **not yet** supported (evcxr's compiled context can't be
cheaply snapshotted/forked, and the Python/JS kernels don't persist state to
disk either).

## License

MIT or Apache-2.0, inherited from Twinsong. See `LICENSE-MIT` and `LICENSE-APACHE`.
