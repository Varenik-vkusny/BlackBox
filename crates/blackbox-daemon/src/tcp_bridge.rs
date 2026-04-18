use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;

use crate::buffer::{push_line_and_drain, SharedBuffer};
use crate::scanners::drain::SharedDrainState;

pub async fn run_tcp_bridge(buf: SharedBuffer, drain: SharedDrainState, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await
        .unwrap_or_else(|e| panic!("TCP bridge failed to bind {addr}: {e}"));

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
        push_line_and_drain(&buf, &drain, line);
    }
}
