# Patina

![patina — a Rust-native interactive notebook for Rust, Python and JavaScript](https://raw.githubusercontent.com/vbasky/patina/main/docs/banner.png)

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

## How it works

When you press `Shift+Enter` in the browser, here's what happens start to finish:

```
┌──────────────┐     WebSocket (JSON)     ┌──────────────┐     TCP (bincode)     ┌──────────────┐
│  Browser UI  │ ◄──────────────────────► │   Server     │ ◄───────────────────► │   Kernel     │
│   (React)    │                          │   (Axum)     │                       │   Process    │
│              │                          │              │                       │              │
│  ─ cell code ──►  RunCode               │              │  ──► Compute(cell)    │              │
│              │                          │              │                       │              │
│              │   Output ◄────────────── │              │  ◄── Output(text)     │              │
│              │   Output ◄────────────── │              │  ◄── Output(html)     │              │
│              │   Output ◄────────────── │              │  ◄── Output(text)     │              │
│              │                          │              │                       │              │
```

**Step by step:**

1. **Browser** sends a `RunCode` message over WebSocket containing the notebook id,
   run id, cell id, and editor tree. The server queues the request on that run.

2. **Server** sends `Compute { cell_id, code }` over TCP to the kernel process (already
   spawned and connected). The wire format is length-delimited bincode — compact and
   binary.

3. **Kernel** executes the cell in the language runtime (`evcxr` for Rust, embedded
   CPython for Python, `boa` for JS). As it runs, it streams back `Output` messages:
   - `Text` — stdout/stderr lines. The server accumulates consecutive text fragments
     so the browser can render them in real time as the cell runs.
   - `Html` — rich output: a pandas DataFrame table, a matplotlib chart, a plotters
     graph. Rendered immediately in the browser as innerHTML.
   - `Exception` — compilation error or runtime exception with a traceback.
   Each message carries an `OutputFlag`: `Running` (intermediate), `Success`, or `Fail`.

4. **Server** forwards each output to the browser over WebSocket (as JSON), tagged
   with the cell and run id. The browser UI appends streaming text, replaces final
   results, and shows error panels.

5. After the final output, the kernel also sends a **globals update** — the set of
   variables changed by the cell (name, type, `repr()` value). The browser renders
   these in the inspector sidebar.

**Protocols:**

| Direction | Transport | Format | Purpose |
| --------- | --------- | ------ | ------- |
| Browser → Server | WebSocket | JSON | User actions (run code, save, fork, navigate files) |
| Server → Browser | WebSocket | JSON | Output, notebook state, globals, errors |
| Server → Kernel | TCP | bincode | Compute requests, save/load state |
| Kernel → Server | TCP | bincode | Output values, globals updates, state responses |

**State model:**

- Kernels are **child OS processes** — a kernel crash never takes down the server.
- Each notebook has multiple **runs**. Each run has its own kernel process and its own
  timeline of output cells. You can fork a run: the kernel saves its globals, a new
  kernel loads them, and execution branches from that point.
- The server holds all state behind an `Arc<Mutex<AppState>>`. Lock is held briefly
  (message dispatch only), never across I/O.

## Rich output

A cell's result is not just a plain string — the last expression is rendered in the
richest available format. The kernel chooses from four output types:

| Type | Wire format | When |
| ---- | ----------- | ---- |
| `Html` | HTML string | A DataFrame, chart, or custom `_repr_html_()` |
| `Text` | Plain string | stdout/stderr or plain `repr()` |
| `Exception` | `{message, traceback}` | Compile error or runtime exception |
| `None` | *(absent)* | Statement with no return value |

### Python

Uses IPython's display protocol, captured by an injected driver script:

```python
import pandas as pd
import matplotlib.pyplot as plt

df = pd.DataFrame([[10, 20], [30, 40]], columns=["A", "B"])
df                              # → HTML table (via _repr_html_)

plt.plot([1, 2, 3], [1, 4, 9])
plt.title("plot")
plt.show()                      # → inline PNG image

"My string"                     # → plain text

1 / 0                           # → Exception with traceback
```

The last open matplotlib figure is auto-captured as a base64-encoded PNG injected into
HTML. Any object with `_repr_html_()`, `_repr_svg_()`, or `_repr_png_()` is rendered
accordingly. `numpy`, `pandas`, and `matplotlib` come preloaded (see [Batteries
included](#batteries-included)).

### Rust

`evcxr` compiles each cell. The kernel detects evcxr's `text/html` content protocol
and maps it to `Html` output. Two helper functions are injected into every cell:

```rust
// Render arbitrary HTML — useful for polars DataFrames
println!("{:?}", df);           // → Text (stdout)
patina_html(&html);             // → Html, rendered in the browser

// Render inline SVG — useful for plotters charts
use plotters::prelude::*;
let chart = evcxr_figure(640, 480, |root| {
    root.fill(&WHITE)?;
    Ok(())
});
chart                           // → Html (inline SVG)

patina_svg(&svg);              // → Html (inline SVG)
```

`polars`, `plotters` (with evcxr support), and `ndarray` come preloaded. evcxr persists
`let` bindings across cells — use a concrete type annotation and avoid a trailing `?`
(which causes type inference to fail): `let df: DataFrame = df!(...).unwrap();`

### JavaScript

The `boa` engine (pure Rust) captures `console.*` output and renders the final
expression value as text:

```javascript
console.log("hello");
[1, 2, 3]                       // → Text: "[ 1, 2, 3 ]"
```

Rich output (HTML/PNG/SVG) is not yet supported in JS.

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
answer            // → 42
```

## Batteries included

The common data libraries are available, but **off by default** so a fresh Rust
notebook starts instantly (a plain cell compiles in ~1–2s instead of waiting on a
large crate). Opt in per notebook:

- **Rust** — add the crate with a `:dep` line, e.g.
  `:dep polars = { version = "0.46", features = ["fmt"] }`, then
  `use polars::prelude::*;`. It compiles once per machine and is reused across
  restarts (see *Compile speed*). To instead preload `polars`, `plotters`
  (with `evcxr` support) and `ndarray` at kernel startup, set `PATINA_BATTERIES=1`.
- **Python** — `numpy`, `pandas`, and `matplotlib` install (binary wheels) into a
  Patina-managed virtualenv at `~/.patina/pyenv` and are added to the kernel's
  path. This avoids touching the system/Homebrew Python (externally managed,
  PEP 668).

## Compile speed (Rust cells)

A Rust notebook compiles real code, so a large crate like polars takes ~a minute
the *first* time. Patina makes that a one-time cost rather than a per-run one:

- **Persistent build dir** — the kernel points evcxr at a stable, exclusively
  locked `~/.patina/evcxr/rust` so compiled dependencies survive kernel restarts.
  evcxr otherwise builds in a throwaway temp dir; this takes polars from ~90s
  *every* session to ~6–10s after the first compile. (Plain cargo already reuses
  deps across builds — evcxr just needed a stable directory.)
- **One-time warm-up** — evcxr embeds rust-analyzer for cross-cell variable
  persistence; it indexes the sysroot once per kernel session (~30s). The kernel
  reports "starting" until that's done, so your first cell runs fast instead of
  stalling mid-work. Keep a kernel alive and you pay it once per session.
- **sccache** (optional) — set `PATINA_SCCACHE=1` to also route rustc through
  sccache if it's on `PATH`. Off by default: it disables evcxr's dynamic linking
  (slower cell links) and overlaps with the persistent build dir above.

## Desktop app

Patina ships as a native desktop app via [Tauri](https://tauri.app) (`desktop/app`):
a thin native window around the embedded server, with the server and all three
language kernels bundled as sidecars. Notebooks live in `~/Documents/Patina`
(seeded with examples on first run).

```bash
cd desktop/app
cargo tauri dev                              # run it during development
PATINA_REBUILD_BUNDLE=1 cargo tauri build    # build installers (.app/.dmg, .deb/.AppImage)
```

`cargo tauri build` runs `prebuild.sh` (the configured `beforeBuildCommand`), which:

1. builds the UI and the release server + kernels, and stages them as Tauri sidecars;
2. assembles the **offline runtime** — a relocatable Rust toolchain with the
   batteries crates vendored (`build-rust-bundle.sh --offline`) and a relocatable
   CPython (`build-python-bundle.sh`) — and copies it into `resources/`.

On first launch the app mirrors the vendored runtime into a writable
`~/.patina/runtime` and warm-compiles the batteries once, so notebooks run with no
host Rust/Python and no network. The costly bundle steps are cached between builds;
`PATINA_REBUILD_BUNDLE=1` forces a fresh bundle. (`prebuild.sh` is Unix-only today;
Windows packaging isn't wired up yet.) The underlying mechanism is detailed below.

CI (`.github/workflows/desktop.yml`) builds the installers on macOS + Linux and
attaches them to the GitHub release on tags. macOS builds are **code-signed and
notarized** when the Apple secrets are configured (see the workflow header for the
list); the hardened-runtime entitlements in `desktop/app/entitlements.plist` are
what let the signed app still compile/`dlopen` cell code and load the bundled
`libpython`.

## Bundled toolchain (self-contained desktop app)

The Rust kernel needs a Rust toolchain to compile cells. To ship a desktop app
that doesn't depend on the host having Rust, **bundle one**: build a relocatable
toolchain (+ vendored batteries crates and a prewarmed cache) and point the kernel
at it. No evcxr fork required — the kernel just configures the environment its
cargo sees.

```bash
desktop/build-rust-bundle.sh            # online: host needs no Rust; :dep still uses the network
desktop/build-rust-bundle.sh --offline  # offline: only the vendored crates, no network (Playground-style)

PATINA_TOOLCHAIN=desktop/bundle/toolchain ./target/debug/patina
```

The kernel uses `$PATINA_TOOLCHAIN` (else a `toolchain/` dir beside the kernel
binary) and auto-picks-up a sibling `cargo/` (vendored registry + config) and
`target/` (prewarmed cache); override with `$PATINA_CARGO_HOME` / `$PATINA_TARGET_DIR`.
Caveat: on macOS, linking native crates still needs the system linker/SDK unless
you also bundle a sysroot — the only fully-host-free Rust path is a wasm executor
(a much larger change).

**Python** ships self-contained the same way — fetch a relocatable interpreter
([python-build-standalone](https://github.com/astral-sh/python-build-standalone))
and point the kernel at it:

```bash
desktop/build-python-bundle.sh          # downloads a relocatable CPython
PATINA_PYTHON=desktop/bundle/python ./target/debug/patina
```

The kernel sets `PYTHONHOME` to `$PATINA_PYTHON` (else a `python/` dir beside the
kernel binary). For *full* independence rebuild the kernel against that interpreter
(`PYO3_PYTHON=…/python/bin/python3 cargo build -p patina-kernel-python`) so it links
the bundled `libpython`. The **JavaScript** (boa) kernel already needs nothing from
the host.

## Status

Experimental. The kernels support cell evaluation, streamed stdout/stderr, text /
HTML / image output, and globals inspection. State save/load and kernel forking are
**not yet** supported (evcxr's compiled context can't be cheaply snapshotted/forked,
and the Python/JS kernels don't persist state to disk).

## License

MIT or Apache-2.0, inherited from Twinsong. See `LICENSE-MIT` and `LICENSE-APACHE`.
