use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;

use crate::buffer::{push_line, SharedBuffer};

pub async fn run_tcp_bridge(buf: SharedBuffer, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await
        .unwrap_or_else(|e| panic!("TCP bridge failed to bind {addr}: {e}"));

    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let buf = buf.clone();
                tokio::spawn(async move {
                    handle_connection(stream, buf).await;
                });
            }
            Err(_) => break,
        }
    }
}

async fn handle_connection(stream: tokio::net::TcpStream, buf: SharedBuffer) {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        push_line(&buf, line);
    }
}
