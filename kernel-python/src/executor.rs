//! CPython executor (pyo3). Runs on a dedicated thread, owns a persistent
//! module-globals dict, and turns `Compute` requests into output messages.

use comm::kernel::{FromExecutorMessage, ToExecutorMessage, collect_leaves};
use comm::messages::{ComputeMsg, Exception, KernelOutputValue, OutputFlag};
use comm::scopes::SerializedGlobals;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Python helpers, defined once per cell run:
///   * `_patina_run`  — exec a leaf, stashing the last top-level expression's
///                      value (IPython-style) in `g['__patina_val']`.
///   * `_patina_collect` — after all leaves, return the rich outputs to display
///                      as a list of `(kind, text)` (kind = "html" | "text"):
///                      any open matplotlib figures (as inline PNGs) followed by
///                      the final value rendered via its display protocol
///                      (`_repr_html_` / `_repr_svg_` / `_repr_png_`, else repr).
///                      pandas DataFrames render as HTML tables for free.
const DRIVER: &str = r#"
def _patina_run(src, g):
    import ast
    tree = ast.parse(src)
    g.pop('__patina_val', None)
    if tree.body and isinstance(tree.body[-1], ast.Expr):
        last = ast.Expression(tree.body.pop().value)
        if tree.body:
            exec(compile(tree, '<cell>', 'exec'), g)
        g['__patina_val'] = eval(compile(last, '<cell>', 'eval'), g)
    else:
        exec(compile(tree, '<cell>', 'exec'), g)


def _patina_mime(val):
    import base64
    for meth, kind in (('_repr_html_', 'html'), ('_repr_svg_', 'html')):
        f = getattr(type(val), meth, None)
        if f is not None:
            try:
                r = f(val)
                if r:
                    return (kind, r if isinstance(r, str) else str(r))
            except Exception:
                pass
    f = getattr(type(val), '_repr_png_', None)
    if f is not None:
        try:
            r = f(val)
            if r:
                data = r if isinstance(r, (bytes, bytearray)) else r.encode()
                b = base64.b64encode(data).decode()
                return ('html', '<img src="data:image/png;base64,%s">' % b)
        except Exception:
            pass
    return ('text', repr(val))


def _patina_collect(g):
    import sys, io, base64
    outs = []
    fig_ids = set()
    if 'matplotlib' in sys.modules:
        try:
            from matplotlib import pyplot as plt
            for num in plt.get_fignums():
                fig = plt.figure(num)
                fig_ids.add(id(fig))
                buf = io.BytesIO()
                fig.savefig(buf, format='png', bbox_inches='tight')
                b = base64.b64encode(buf.getvalue()).decode()
                outs.append(('html', '<img src="data:image/png;base64,%s">' % b))
            plt.close('all')
        except Exception:
            pass
    val = g.pop('__patina_val', None)
    if val is not None and id(val) not in fig_ids:
        outs.append(_patina_mime(val))
    return outs
"#;

pub fn spawn_executor(
    mut rx: UnboundedReceiver<ToExecutorMessage>,
    tx: UnboundedSender<FromExecutorMessage>,
) {
    std::thread::spawn(move || {
        // Persistent namespace shared across all cells.
        let globals: Py<PyDict> = Python::with_gil(|py| PyDict::new(py).unbind());

        while let Some(msg) = rx.blocking_recv() {
            match msg {
                ToExecutorMessage::Compute(m) => run_compute(&globals, &m, &tx),
                ToExecutorMessage::SaveState(path) => {
                    let _ = tx.send(FromExecutorMessage::SaveStateResponse {
                        path,
                        result: Err("State save is not supported by the Python kernel".into()),
                    });
                }
                ToExecutorMessage::LoadState(path) => {
                    let _ = tx.send(FromExecutorMessage::LoadStateResponse {
                        path,
                        result: Err("State load is not supported by the Python kernel".into()),
                    });
                }
            }
        }
    });
}

fn run_compute(
    globals: &Py<PyDict>,
    m: &ComputeMsg,
    tx: &UnboundedSender<FromExecutorMessage>,
) {
    let mut leaves = Vec::new();
    collect_leaves(&m.code, &mut leaves);
    let cell_id = m.cell_id;

    // (captured stdout, rich outputs as (kind,text), optional error value)
    type Outcome = (String, Vec<(String, String)>, Option<KernelOutputValue>);
    let outcome: PyResult<Outcome> = Python::with_gil(|py| {
        let globals = globals.bind(py);

        // Redirect stdout/stderr into a StringIO buffer for the duration.
        let sys = py.import("sys")?;
        let io = py.import("io")?;
        let buf = io.getattr("StringIO")?.call0()?;
        let old_out = sys.getattr("stdout")?;
        let old_err = sys.getattr("stderr")?;
        sys.setattr("stdout", &buf)?;
        sys.setattr("stderr", &buf)?;

        // Define the driver in a throwaway namespace and fetch the functions.
        // Pass it as *globals* so the helpers share one namespace and can call
        // each other (their `__globals__` is this dict).
        let helpers = PyDict::new(py);
        let driver = CString::new(DRIVER).unwrap();
        py.run(driver.as_c_str(), Some(&helpers), None)?;
        let run_fn = helpers.get_item("_patina_run")?.expect("_patina_run defined");
        let collect_fn = helpers
            .get_item("_patina_collect")?
            .expect("_patina_collect defined");

        let mut run_err: Option<PyErr> = None;
        for leaf in &leaves {
            if leaf.code.trim().is_empty() {
                continue;
            }
            if let Err(e) = run_fn.call1((leaf.code.as_str(), globals.clone())) {
                run_err = Some(e);
                break;
            }
        }

        let outs: Vec<(String, String)> = if run_err.is_none() {
            collect_fn.call1((globals.clone(),))?.extract()?
        } else {
            Vec::new()
        };

        // Restore the real streams and read what was captured.
        sys.setattr("stdout", old_out)?;
        sys.setattr("stderr", old_err)?;
        let captured: String = buf.getattr("getvalue")?.call0()?.extract()?;

        let err_value = run_err.map(|e| error_to_value(py, &e));
        Ok((captured, outs, err_value))
    });

    let (captured, outs, err_value) = match outcome {
        Ok(v) => v,
        Err(e) => (String::new(), Vec::new(), Some(Python::with_gil(|py| error_to_value(py, &e)))),
    };

    // Stream captured stdout/stderr first (Running).
    if !captured.is_empty() {
        let _ = tx.send(FromExecutorMessage::Output {
            value: KernelOutputValue::Text { value: captured },
            cell_id,
            flag: OutputFlag::Running,
            update: None,
        });
    }

    let mut globals_snapshot = Some(Python::with_gil(|py| collect_globals(globals.bind(py))));

    if let Some(value) = err_value {
        let _ = tx.send(FromExecutorMessage::Output {
            value,
            cell_id,
            flag: OutputFlag::Fail,
            update: globals_snapshot.take(),
        });
        return;
    }

    // Emit each rich output; figures/intermediate as Running, the final as
    // Success (carrying the globals snapshot). No outputs -> a single None.
    let mut values: Vec<KernelOutputValue> = outs
        .into_iter()
        .map(|(kind, text)| match kind.as_str() {
            "html" => KernelOutputValue::Html { value: text },
            _ => KernelOutputValue::Text { value: text },
        })
        .collect();
    if values.is_empty() {
        values.push(KernelOutputValue::None);
    }
    let last = values.len() - 1;
    for (i, value) in values.into_iter().enumerate() {
        let (flag, update) = if i == last {
            (OutputFlag::Success, globals_snapshot.take())
        } else {
            (OutputFlag::Running, None)
        };
        let _ = tx.send(FromExecutorMessage::Output {
            value,
            cell_id,
            flag,
            update,
        });
    }
}

fn error_to_value(py: Python<'_>, e: &PyErr) -> KernelOutputValue {
    let message = e.to_string();
    let traceback = e
        .traceback(py)
        .and_then(|tb| tb.format().ok())
        .map(|tb| format!("{tb}{message}"))
        .unwrap_or_else(|| message.clone());
    KernelOutputValue::Exception {
        value: Exception { message, traceback },
    }
}

/// Mirrors the UI's `JsonObjectDump` shape (see `core/jobject.ts`).
#[derive(serde::Serialize)]
struct JObject {
    id: u64,
    repr: String,
    value_type: String,
    kind: &'static str,
    children: Vec<(String, u64)>,
}

#[derive(serde::Serialize)]
struct JDump {
    objects: Vec<JObject>,
    root: u64,
}

fn jobject_json(repr: &str, value_type: &str) -> String {
    let dump = JDump {
        objects: vec![JObject {
            id: 1,
            repr: repr.to_string(),
            value_type: value_type.to_string(),
            kind: "",
            children: Vec::new(),
        }],
        root: 1,
    };
    serde_json::to_string(&dump).unwrap_or_else(|_| r#"{"objects":[],"root":0}"#.to_string())
}

/// Expose user-defined globals (skipping dunders/imports-ish underscored names)
/// to the inspector as name -> repr.
fn collect_globals(globals: &Bound<'_, PyDict>) -> SerializedGlobals {
    let mut vars: HashMap<String, Arc<String>> = HashMap::new();
    for (k, v) in globals.iter() {
        let name: String = match k.extract() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if name.starts_with('_') {
            continue;
        }
        let repr = v
            .repr()
            .ok()
            .and_then(|r| r.extract::<String>().ok())
            .unwrap_or_default();
        let value_type = v
            .get_type()
            .name()
            .ok()
            .map(|n| n.to_string())
            .unwrap_or_default();
        vars.insert(name, Arc::new(jobject_json(&repr, &value_type)));
    }
    SerializedGlobals::new("global".to_string(), vars, HashMap::new())
}
