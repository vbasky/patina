//! boa_engine executor. Runs on a dedicated thread, owns one persistent
//! `Context` (so globals/`var`s survive across cells), and turns `Compute`
//! requests into output messages. `console.*` output is buffered in JS and read
//! back after each run.

use boa_engine::{Context, Source, js_string};
use comm::kernel::{FromExecutorMessage, ToExecutorMessage, collect_leaves};
use comm::messages::{ComputeMsg, Exception, KernelOutputValue, OutputFlag};
use comm::scopes::SerializedGlobals;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Installs a `console` that appends formatted lines to `__patina_out` (read and
/// cleared after each cell), so we can stream program output to the UI.
const PRELUDE: &str = r#"
globalThis.__patina_out = [];
(function () {
  const fmt = (a) => a.map((x) => {
    if (typeof x === 'string') return x;
    try { return JSON.stringify(x); } catch (e) { return String(x); }
  }).join(' ');
  const push = (...a) => { globalThis.__patina_out.push(fmt(a)); };
  globalThis.console = { log: push, info: push, warn: push, error: push, debug: push };
})();
"#;

const DRAIN: &str =
    "(()=>{const s=globalThis.__patina_out.join('\\n');globalThis.__patina_out=[];return s;})()";

const GLOBAL_KEYS: &str = "Object.getOwnPropertyNames(globalThis).join('\\u0001')";

pub fn spawn_executor(
    mut rx: UnboundedReceiver<ToExecutorMessage>,
    tx: UnboundedSender<FromExecutorMessage>,
) {
    std::thread::spawn(move || {
        let mut ctx = Context::default();
        // Install console capture, then snapshot the global names so we can later
        // tell user-declared globals apart from built-ins.
        let _ = ctx.eval(Source::from_bytes(PRELUDE));
        let baseline: HashSet<String> = global_keys(&mut ctx).into_iter().collect();

        while let Some(msg) = rx.blocking_recv() {
            match msg {
                ToExecutorMessage::Compute(m) => run_compute(&mut ctx, &baseline, &m, &tx),
                ToExecutorMessage::SaveState(path) => {
                    let _ = tx.send(FromExecutorMessage::SaveStateResponse {
                        path,
                        result: Err("State save is not supported by the JS kernel".into()),
                    });
                }
                ToExecutorMessage::LoadState(path) => {
                    let _ = tx.send(FromExecutorMessage::LoadStateResponse {
                        path,
                        result: Err("State load is not supported by the JS kernel".into()),
                    });
                }
            }
        }
    });
}

fn run_compute(
    ctx: &mut Context,
    baseline: &HashSet<String>,
    m: &ComputeMsg,
    tx: &UnboundedSender<FromExecutorMessage>,
) {
    let mut leaves = Vec::new();
    collect_leaves(&m.code, &mut leaves);
    let cell_id = m.cell_id;

    let mut value = KernelOutputValue::None;
    let mut error: Option<KernelOutputValue> = None;
    for leaf in &leaves {
        if leaf.code.trim().is_empty() {
            continue;
        }
        // Strip TypeScript to JavaScript (oxc) before boa evaluates it.
        let js = match crate::transpile::ts_to_js(&leaf.code) {
            Ok(js) => js,
            Err(message) => {
                error = Some(KernelOutputValue::Exception {
                    value: Exception {
                        traceback: message.clone(),
                        message,
                    },
                });
                break;
            }
        };
        match ctx.eval(Source::from_bytes(js.as_str())) {
            Ok(v) => {
                value = if v.is_undefined() || v.is_null() {
                    KernelOutputValue::None
                } else {
                    KernelOutputValue::Text {
                        value: v.display().to_string(),
                    }
                };
            }
            Err(e) => {
                let message = e.to_string();
                error = Some(KernelOutputValue::Exception {
                    value: Exception {
                        traceback: message.clone(),
                        message,
                    },
                });
                break;
            }
        }
    }

    // Drain buffered console output and stream it first.
    let captured = ctx
        .eval(Source::from_bytes(DRAIN))
        .ok()
        .and_then(|v| v.to_string(ctx).ok())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    if !captured.is_empty() {
        let _ = tx.send(FromExecutorMessage::Output {
            value: KernelOutputValue::Text { value: captured },
            cell_id,
            flag: OutputFlag::Running,
            update: None,
        });
    }

    let globals = collect_globals(ctx, baseline);
    let (value, flag) = match error {
        Some(e) => (e, OutputFlag::Fail),
        None => (value, OutputFlag::Success),
    };
    let _ = tx.send(FromExecutorMessage::Output {
        value,
        cell_id,
        flag,
        update: Some(globals),
    });
}

fn global_keys(ctx: &mut Context) -> Vec<String> {
    ctx.eval(Source::from_bytes(GLOBAL_KEYS))
        .ok()
        .and_then(|v| v.to_string(ctx).ok())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default()
        .split('\u{1}')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
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

fn jobject_json(repr: &str) -> String {
    let dump = JDump {
        objects: vec![JObject {
            id: 1,
            repr: repr.to_string(),
            value_type: String::new(),
            kind: "",
            children: Vec::new(),
        }],
        root: 1,
    };
    serde_json::to_string(&dump).unwrap_or_else(|_| r#"{"objects":[],"root":0}"#.to_string())
}

/// Globals declared by the user since startup (`var`/global assignments;
/// top-level `let`/`const` aren't enumerable so won't appear).
fn collect_globals(ctx: &mut Context, baseline: &HashSet<String>) -> SerializedGlobals {
    let mut vars: HashMap<String, Arc<String>> = HashMap::new();
    let global = ctx.global_object();
    for key in global_keys(ctx) {
        if baseline.contains(&key) || key.starts_with("__patina") {
            continue;
        }
        let repr = global
            .get(js_string!(key.as_str()), ctx)
            .ok()
            .map(|v| v.display().to_string())
            .unwrap_or_default();
        vars.insert(key, Arc::new(jobject_json(&repr)));
    }
    SerializedGlobals::new("global".to_string(), vars, HashMap::new())
}
