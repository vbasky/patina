//! The evcxr-backed executor: runs on its own OS thread, owns the
//! [`evcxr::EvalContext`], and turns `Compute` requests into output messages.

use comm::kernel::{FromExecutorMessage, ToExecutorMessage, collect_leaves};
use comm::messages::{ComputeMsg, Exception, KernelOutputValue, OutputFlag};
use comm::scopes::SerializedGlobals;
use evcxr::{CommandContext, EvalOutputs};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

/// Display helpers injected into every kernel session. They emit evcxr's
/// content protocol so the result renders as HTML in the notebook. `patina_svg`
/// is for inline SVG (e.g. a `plotters` `SVGBackend` string); `patina_html` for
/// any HTML fragment (e.g. a `polars` table rendered to HTML).
const PRELUDE: &str = r#"
fn patina_html(html: &str) {
    println!("EVCXR_BEGIN_CONTENT text/html\n{}\nEVCXR_END_CONTENT", html);
}
fn patina_svg(svg: &str) {
    println!("EVCXR_BEGIN_CONTENT text/html\n{}\nEVCXR_END_CONTENT", svg);
}
"#;

/// Spin up the evcxr context on a dedicated thread (it is not `Send` and its
/// `eval` blocks while cargo/rustc compiles), draining its stdout/stderr to the
/// server as it runs.
pub fn spawn_executor(
    mut rx: UnboundedReceiver<ToExecutorMessage>,
    tx: UnboundedSender<FromExecutorMessage>,
) {
    std::thread::spawn(move || {
        let (mut context, outputs) = match CommandContext::new() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Failed to start evcxr context: {e}");
                std::process::exit(1);
            }
        };

        // Debug builds compile fastest — every cell goes through rustc, so keep
        // the opt level low. (evcxr's on-disk artifact cache lives on the
        // lower-level CommandContext; wiring it up is a deeper change.)
        let _ = context.set_opt_level("0");

        // Rich-output helpers available in every cell (rendered via evcxr's
        // content protocol). Use them with the Rust equivalents of
        // matplotlib/pandas: `plotters` (charts -> SVG) and `polars`
        // (dataframes). e.g. `patina_svg(&svg_string)` for a plotters chart, or
        // `patina_html(&html)` for any HTML table.
        let _ = context.execute(PRELUDE);

        // Which cell stdout/stderr lines belong to (set before each eval).
        let current: Arc<Mutex<Uuid>> = Arc::new(Mutex::new(Uuid::nil()));

        // Stream the program's stdout/stderr to the UI as it runs (flag =
        // Running). `outputs.{stdout,stderr}` are crossbeam receivers; we move
        // them straight into pump threads so we never name the channel type.
        for stream in [outputs.stdout, outputs.stderr] {
            let tx = tx.clone();
            let current = current.clone();
            std::thread::spawn(move || {
                while let Ok(line) = stream.recv() {
                    let cell_id = *current.lock().unwrap();
                    let _ = tx.send(FromExecutorMessage::Output {
                        value: KernelOutputValue::Text {
                            value: format!("{line}\n"),
                        },
                        cell_id,
                        flag: OutputFlag::Running,
                        update: None,
                    });
                }
            });
        }

        while let Some(msg) = rx.blocking_recv() {
            match msg {
                ToExecutorMessage::Compute(m) => {
                    *current.lock().unwrap() = m.cell_id;
                    let out = run_compute(&mut context, &m);
                    let _ = tx.send(out);
                }
                ToExecutorMessage::SaveState(path) => {
                    let _ = tx.send(FromExecutorMessage::SaveStateResponse {
                        path,
                        result: Err("State save is not supported by the Rust kernel yet".into()),
                    });
                }
                ToExecutorMessage::LoadState(path) => {
                    let _ = tx.send(FromExecutorMessage::LoadStateResponse {
                        path,
                        result: Err("State load is not supported by the Rust kernel yet".into()),
                    });
                }
            }
        }
    });
}

fn run_compute(context: &mut CommandContext, m: &ComputeMsg) -> FromExecutorMessage {
    let mut leaves = Vec::new();
    collect_leaves(&m.code, &mut leaves);

    let mut last_outputs: Option<EvalOutputs> = None;
    let mut error: Option<KernelOutputValue> = None;
    for leaf in &leaves {
        if leaf.code.trim().is_empty() {
            continue;
        }
        match context.execute(&leaf.code) {
            Ok(o) => last_outputs = Some(o),
            Err(e) => {
                error = Some(error_to_value(e));
                break;
            }
        }
    }

    let globals = collect_globals(context);
    match error {
        Some(value) => FromExecutorMessage::Output {
            value,
            cell_id: m.cell_id,
            flag: OutputFlag::Fail,
            update: Some(globals),
        },
        None => {
            let value = last_outputs
                .map(outputs_to_value)
                .unwrap_or(KernelOutputValue::None);
            FromExecutorMessage::Output {
                value,
                cell_id: m.cell_id,
                flag: OutputFlag::Success,
                update: Some(globals),
            }
        }
    }
}

/// Prefer rich HTML (e.g. `EVCXR_BEGIN_CONTENT text/html …`) over the plain
/// `{:?}` value of the cell's final expression.
fn outputs_to_value(o: EvalOutputs) -> KernelOutputValue {
    if let Some(html) = o.content_by_mime_type.get("text/html") {
        KernelOutputValue::Html {
            value: html.clone(),
        }
    } else if let Some(text) = o.content_by_mime_type.get("text/plain") {
        KernelOutputValue::Text {
            value: text.clone(),
        }
    } else {
        KernelOutputValue::None
    }
}

fn error_to_value(e: evcxr::Error) -> KernelOutputValue {
    let (message, traceback) = match &e {
        evcxr::Error::CompilationErrors(errors) => {
            let message = errors
                .iter()
                .map(|c| c.message())
                .collect::<Vec<_>>()
                .join("\n");
            let traceback = errors
                .iter()
                .map(|c| c.rendered())
                .collect::<Vec<_>>()
                .join("\n");
            (
                if message.is_empty() {
                    e.to_string()
                } else {
                    message
                },
                traceback,
            )
        }
        _ => (e.to_string(), e.to_string()),
    };
    KernelOutputValue::Exception {
        value: Exception { message, traceback },
    }
}

/// The globals inspector parses each variable's value as a `JsonObjectDump`
/// (see the UI's `core/jobject.ts`). These mirror that shape.
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

/// evcxr only exposes a variable's name and type (not a runtime repr), so we
/// surface the Rust type as the displayed value.
fn jobject_json(ty: &str) -> String {
    let dump = JDump {
        objects: vec![JObject {
            id: 1,
            repr: ty.to_string(),
            value_type: String::new(),
            kind: "",
            children: Vec::new(),
        }],
        root: 1,
    };
    serde_json::to_string(&dump).unwrap_or_else(|_| r#"{"objects":[],"root":0}"#.to_string())
}

/// Expose evcxr's live variables to the globals inspector (name -> type).
fn collect_globals(context: &CommandContext) -> SerializedGlobals {
    let mut vars: HashMap<String, Arc<String>> = HashMap::new();
    for (name, ty) in context.variables_and_types() {
        vars.insert(name.to_string(), Arc::new(jobject_json(ty)));
    }
    SerializedGlobals::new("global".to_string(), vars, HashMap::new())
}
