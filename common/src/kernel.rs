//! Shared kernel runtime: the networking half every Patina kernel uses.
//!
//! A kernel binary just provides an *executor* — code that owns the
//! language runtime (evcxr, CPython, V8, …) on its own blocking thread and
//! turns [`ToExecutorMessage`]s into [`FromExecutorMessage`]s. [`run_kernel`]
//! handles the rest: connecting to the server, the login handshake, framing,
//! and diffing globals snapshots.

use crate::messages::{ComputeMsg, KernelOutputValue, OutputFlag};
use crate::scopes::SerializedGlobals;
use crate::{
    make_protocol_builder, parse_to_kernel_message, serialize_from_kernel_message,
    messages::{FromKernelMessage, ToKernelMessage},
};
use anyhow::anyhow;
use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use std::path::PathBuf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

/// Requests flowing from the networking loop into the executor thread.
pub enum ToExecutorMessage {
    Compute(ComputeMsg),
    SaveState(PathBuf),
    LoadState(PathBuf),
}

/// Results flowing from the executor thread back to the networking loop.
pub enum FromExecutorMessage {
    Output {
        value: KernelOutputValue,
        cell_id: Uuid,
        flag: OutputFlag,
        update: Option<SerializedGlobals>,
    },
    SaveStateResponse {
        path: PathBuf,
        result: Result<(), String>,
    },
    LoadStateResponse {
        path: PathBuf,
        result: Result<SerializedGlobals, String>,
    },
}

/// Flatten a code group into its leaves in document order.
pub fn collect_leaves<'a>(group: &'a crate::messages::CodeGroup, out: &mut Vec<&'a crate::messages::CodeLeaf>) {
    use crate::messages::CodeNode;
    for child in &group.children {
        match child {
            CodeNode::Group(g) => collect_leaves(g, out),
            CodeNode::Leaf(l) => out.push(l),
        }
    }
}

/// Connect to the server (`KERNEL_CONNECT`), log in as `KERNEL_ID`, and pump
/// messages between the socket and the executor until either side closes.
///
/// `spawn_executor` is given the executor's channel ends and is expected to move
/// the language runtime onto its own thread (it must not block this task).
pub async fn run_kernel<F>(spawn_executor: F) -> anyhow::Result<()>
where
    F: FnOnce(UnboundedReceiver<ToExecutorMessage>, UnboundedSender<FromExecutorMessage>),
{
    let addr =
        std::env::var("KERNEL_CONNECT").map_err(|_| anyhow!("KERNEL_CONNECT is not defined"))?;
    let kernel_id = Uuid::parse_str(
        &std::env::var("KERNEL_ID").map_err(|_| anyhow!("KERNEL_ID is not defined"))?,
    )?;

    let (to_exec_tx, to_exec_rx) = tokio::sync::mpsc::unbounded_channel::<ToExecutorMessage>();
    let (from_exec_tx, mut from_exec_rx) =
        tokio::sync::mpsc::unbounded_channel::<FromExecutorMessage>();
    spawn_executor(to_exec_rx, from_exec_tx);

    let socket = TcpStream::connect(&addr).await?;
    let (mut sender, mut receiver) = make_protocol_builder().new_framed(socket).split();

    sender
        .send(serialize_from_kernel_message(FromKernelMessage::Login { kernel_id })?.into())
        .await?;

    // Executor -> server: translate results, diffing globals against the last
    // snapshot so the inspector only receives changes.
    let send_task = async move {
        let mut last_globals = SerializedGlobals::default();
        while let Some(msg) = from_exec_rx.recv().await {
            let out = match msg {
                FromExecutorMessage::Output {
                    value,
                    cell_id,
                    flag,
                    update,
                } => {
                    let update = update.map(|g| {
                        let u = g.create_update(Some(&last_globals));
                        last_globals = g;
                        u
                    });
                    FromKernelMessage::Output {
                        value,
                        cell_id,
                        flag,
                        update,
                    }
                }
                FromExecutorMessage::SaveStateResponse { path, result } => {
                    FromKernelMessage::SaveStateResponse { path, result }
                }
                FromExecutorMessage::LoadStateResponse { path, result } => {
                    FromKernelMessage::LoadStateResponse { path, result }
                }
            };
            sender.send(serialize_from_kernel_message(out)?.into()).await?;
        }
        Ok::<(), anyhow::Error>(())
    };

    // Server -> executor: forward compute/save/load requests.
    let recv_task = async move {
        while let Some(msg) = receiver.next().await {
            let msg = msg?;
            match parse_to_kernel_message(&msg)? {
                ToKernelMessage::Compute(m) => {
                    let _ = to_exec_tx.send(ToExecutorMessage::Compute(m));
                }
                ToKernelMessage::SaveState(p) => {
                    let _ = to_exec_tx.send(ToExecutorMessage::SaveState(p));
                }
                ToKernelMessage::LoadState(p) => {
                    let _ = to_exec_tx.send(ToExecutorMessage::LoadState(p));
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        r = send_task => r,
        r = recv_task => r,
    }
}
