use crate::client_messages::{
    FromClientMessage, ToClientMessage, parse_client_message, serialize_client_message,
};
use crate::reactor::{
    close_run, delete_file, export_notebook, fork_run, load_notebook, new_notebook, query_dir,
    query_settings, rename_file, run_code, save_notebook, set_language, set_toolchains,
    start_kernel, upload_file,
};
use crate::state::{AppState, AppStateRef};
use anyhow::bail;
use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use futures_util::SinkExt;
use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use serde::Deserialize;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub(crate) async fn http_server_main(state: AppStateRef, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/assets/{name}", get(get_assets))
        .route("/patina.svg", get(patina_logo))
        .route("/ws", any(ws_handler))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;

    {
        let mut stdout = StandardStream::stdout(ColorChoice::Always);
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(173))))?;
        write!(&mut stdout, "\n  Pat")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(37))))?;
        write!(&mut stdout, "ina")?;
        stdout.set_color(ColorSpec::new().set_fg(None))?;
        write!(
            &mut stdout,
            " v{} — Rust notebook",
            env!("CARGO_PKG_VERSION")
        )?;
        stdout.set_color(ColorSpec::new().set_bold(true))?;
        write!(&mut stdout, "\n\n   ➜ http://127.0.0.1:{port}",)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Ansi256(248))))?;
        writeln!(&mut stdout, "?k={}\n", state.lock().unwrap().secret_key())?;
        stdout.reset()?;
    }
    axum::serve(listener, app).await?;
    Ok(())
}

// The built UI assets. The Monaco-based editor makes Vite emit ~180 lazily
// loaded chunks plus web workers (ts.worker, css.worker, …), so the server must
// serve each requested file — not a single bundle. Only the gzip variants are
// embedded (Vite precompresses everything); we serve them with `Content-Encoding:
// gzip`. In debug builds rust-embed reads them from disk; release builds embed.
#[derive(rust_embed::RustEmbed)]
#[folder = "../browser/ui/dist/assets/"]
#[include = "*.gz"]
struct Assets;

fn asset_content_type(name: &str) -> &'static str {
    if name.ends_with(".css") {
        "text/css"
    } else if name.ends_with(".js") {
        "text/javascript"
    } else if name.ends_with(".json") {
        "application/json"
    } else if name.ends_with(".svg") {
        "image/svg+xml"
    } else if name.ends_with(".ttf") {
        "font/ttf"
    } else if name.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

async fn get_assets(Path(name): Path<String>) -> impl IntoResponse {
    match Assets::get(&format!("{name}.gz")) {
        Some(file) => Response::builder()
            .header(header::CONTENT_TYPE, asset_content_type(&name))
            .header(header::CONTENT_ENCODING, "gzip")
            .body(Body::from(file.data.into_owned()))
            .unwrap(),
        None => Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

async fn index(State(state): State<AppStateRef>) -> impl IntoResponse {
    let port = state.lock().unwrap().http_port();
    let html = include_str!("../../browser/ui/dist/index.html")
        .replace("%URL%", format!("ws://127.0.0.1:{port}/ws").as_str());
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap()
}

async fn patina_logo() -> impl IntoResponse {
    Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .body(Body::from(
            include_bytes!("../../browser/ui/dist/patina.svg").as_ref(),
        ))
        .unwrap()
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppStateRef>) -> impl IntoResponse {
    tracing::debug!("New websocket connection");
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_socket(socket, &state).await {
            tracing::error!("Websocket error: {e}");
        }
    })
}

#[derive(Deserialize)]
struct Token {
    token: String,
}

async fn handle_socket(mut socket: WebSocket, state_ref: &AppStateRef) -> anyhow::Result<()> {
    if let Some(msg) = socket.recv().await {
        let msg = msg?;
        if let Message::Text(text) = msg {
            let token = serde_json::from_str::<Token>(&text)?;
            if token.token != state_ref.lock().unwrap().secret_key() {
                tracing::debug!("Invalid authentication");
                bail!("Invalid token");
            }
        } else {
            tracing::error!("Invalid first message");
        }
    } else {
        tracing::debug!("Connection terminated without hello message");
        return Ok(());
    }
    let (sender, receiver) = socket.split();
    let (tx, rx) = unbounded_channel::<Message>();
    let r = tokio::select! {
        r = async {
            forward_sender(sender, rx).await
        } => r,
        r = async {
            recv_client_messages(receiver, state_ref, tx).await
        } => r
    };
    r
}

async fn forward_sender(
    mut sender: SplitSink<WebSocket, Message>,
    mut rx: UnboundedReceiver<Message>,
) -> anyhow::Result<()> {
    while let Some(msg) = rx.recv().await {
        sender.send(msg).await?
    }
    Ok(())
}

fn process_client_message(
    state: &mut AppState,
    state_ref: &AppStateRef,
    sender: &mut UnboundedSender<Message>,
    message: FromClientMessage,
) -> anyhow::Result<()> {
    match message {
        FromClientMessage::CreateNewNotebook(msg) => {
            new_notebook(state, state_ref, msg.filename, msg.language, sender.clone())?
        }
        FromClientMessage::CreateNewKernel(msg) => {
            tracing::debug!("Creating new kernel for notebook {}", msg.notebook_id);
            start_kernel(state, state_ref, msg.notebook_id, msg.run_id, msg.run_title)?;
        }
        FromClientMessage::RunCode(msg) => {
            run_code(state, msg)?;
        }
        FromClientMessage::Fork(msg) => {
            fork_run(state, state_ref, msg)?;
        }
        FromClientMessage::SaveNotebook(msg) => {
            save_notebook(state, state_ref, msg)?;
        }
        FromClientMessage::LoadNotebook(msg) => {
            load_notebook(state, state_ref, msg, sender.clone())?;
        }
        FromClientMessage::QueryDir(msg) => {
            query_dir(state, sender, &msg.path)?;
        }
        FromClientMessage::UploadFile(msg) => {
            upload_file(state, state_ref, msg, sender.clone())?;
        }
        FromClientMessage::DeleteFile(msg) => {
            delete_file(state, sender, &msg.path)?;
        }
        FromClientMessage::RenameFile(msg) => {
            rename_file(state, sender, &msg.path, &msg.new_path)?;
        }
        FromClientMessage::SetLanguage(msg) => {
            set_language(state, msg.notebook_id, msg.language)?;
        }
        FromClientMessage::SetToolchains(settings) => {
            set_toolchains(state, settings);
        }
        FromClientMessage::QuerySettings => {
            query_settings(state, sender)?;
        }
        FromClientMessage::CloseRun(msg) => {
            close_run(state, msg.notebook_id, msg.run_id)?;
        }
        FromClientMessage::KernelList => {
            let _ = sender.send(serialize_client_message(ToClientMessage::Kernels {
                kernels: state.kernel_list(),
            })?);
        }
        FromClientMessage::ExportNotebook(msg) => {
            export_notebook(state, sender, msg.notebook_id)?;
        }
    };
    Ok(())
}

async fn recv_client_messages(
    mut receiver: SplitStream<WebSocket>,
    state_ref: &AppStateRef,
    mut sender: UnboundedSender<Message>,
) -> anyhow::Result<()> {
    while let Some(data) = receiver.next().await {
        let data = data?;
        if let Message::Close(_) = data {
            break;
        }
        let message = parse_client_message(data)?;
        let mut state = state_ref.lock().unwrap();
        if let Err(e) = process_client_message(&mut state, state_ref, &mut sender, message) {
            tracing::error!("Client message processing failed: {e}");
            let _ = sender.send(serialize_client_message(ToClientMessage::Error {
                message: &e.to_string(),
            })?);
        }
    }
    Ok(())
}
