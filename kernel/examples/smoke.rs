//! Standalone smoke test for the Patina Rust kernel.
//!
//! Mimics the server's kernel manager: binds a port, launches `patina-kernel`
//! pointed at it, accepts the Login, sends one `Compute`, and prints the
//! streamed outputs. Run with: `cargo run -p patina-kernel --example smoke`.

use comm::messages::{
    CodeGroup, CodeLeaf, CodeNode, CodeScope, ComputeMsg, FromKernelMessage, ToKernelMessage,
};
use comm::{parse_from_kernel_message, serialize_to_kernel_message};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use uuid::Uuid;

fn write_frame(s: &mut impl Write, data: &[u8]) {
    s.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
    s.write_all(data).unwrap();
    s.flush().unwrap();
}

fn read_frame(s: &mut impl Read) -> Option<Vec<u8>> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len).ok()?;
    let mut buf = vec![0u8; u32::from_le_bytes(len) as usize];
    s.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let kernel_id = Uuid::new_v4();

    // sibling of this example binary: target/debug/examples/smoke -> target/debug/patina-kernel
    let exe = std::env::current_exe().unwrap();
    let kernel_bin = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("patina-kernel");
    eprintln!("[harness] launching {kernel_bin:?}");
    let mut child = Command::new(&kernel_bin)
        .env("KERNEL_CONNECT", format!("127.0.0.1:{port}"))
        .env("KERNEL_ID", kernel_id.to_string())
        .spawn()
        .expect("failed to spawn patina-kernel");

    let (mut stream, _) = listener.accept().unwrap();
    eprintln!("[harness] kernel connected");

    match parse_from_kernel_message(&read_frame(&mut stream).unwrap()).unwrap() {
        FromKernelMessage::Login { kernel_id: kid } => {
            assert_eq!(kid, kernel_id);
            eprintln!("[harness] got Login (kernel_id matches)");
        }
        other => panic!("expected Login, got {other:?}"),
    }

    let cell_id = Uuid::new_v4();
    let code = CodeGroup {
        children: vec![CodeNode::Leaf(CodeLeaf {
            id: Uuid::new_v4(),
            code: "let answer: i32 = 40 + 2;\nprintln!(\"hello from the rust kernel\");\nanswer"
                .to_string(),
        })],
        scope: CodeScope::Inherit,
    };
    eprintln!("[harness] sending Compute (expect stdout 'hello…' then value 42)");
    write_frame(
        &mut stream,
        &serialize_to_kernel_message(ToKernelMessage::Compute(ComputeMsg { cell_id, code }))
            .unwrap(),
    );

    loop {
        let Some(frame) = read_frame(&mut stream) else {
            eprintln!("[harness] stream closed");
            break;
        };
        match parse_from_kernel_message(&frame).unwrap() {
            FromKernelMessage::Output {
                value,
                flag,
                update,
                ..
            } => {
                eprintln!("[harness] OUTPUT flag={flag:?} value={value:?}");
                if let Some(u) = update {
                    eprintln!("[harness]   globals update: {u:?}");
                }
                if flag.is_final() {
                    eprintln!("[harness] FINAL — success");
                    break;
                }
            }
            other => eprintln!("[harness] other: {other:?}"),
        }
    }

    let _ = child.kill();
}
