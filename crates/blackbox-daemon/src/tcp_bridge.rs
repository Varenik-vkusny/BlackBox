use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;

use crate::buffer::{push_line_and_drain, SharedBuffer};
use crate::scanners::drain::SharedDrainState;

/// JSON envelope sent by the VS Code extension: `{"t":"terminal name","d":"data chunk"}`.
/// Plain-text lines (shell hooks, old clients) are handled as-is with no terminal tag.
#[derive(Deserialize)]
struct TcpEnvelope {
    t: Option<String>,
    d: String,
}

pub async fn run_tcp_bridge(buf: SharedBuffer, drain: SharedDrainState, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("TCP bridge failed to bind {addr}: {e}. External logs will not be captured via TCP.");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let buf = buf.clone();
                let drain = drain.clone();
                tokio::spawn(async move {
                    handle_connection(stream, buf, drain).await;
                });
            }
            Err(_) => break,
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    buf: SharedBuffer,
    drain: SharedDrainState,
) {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(env) = serde_json::from_str::<TcpEnvelope>(&line) {
            // JSON envelope from VS Code extension — carries terminal name.
            for data_line in env.d.lines() {
                push_line_and_drain(&buf, &drain, data_line.to_string(), env.t.clone());
            }
        } else {
            // Plain text — shell hooks or legacy clients. No terminal tag.
            push_line_and_drain(&buf, &drain, line, None);
        }
    }
}
