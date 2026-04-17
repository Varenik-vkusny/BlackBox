mod buffer;
mod mcp;
mod scanners;
mod status_server;
mod tcp_bridge;

use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cwd = parse_arg(&args, "--cwd").map(PathBuf::from).unwrap_or_else(|| {
        std::env::current_dir().expect("Cannot determine current directory")
    });
    let bridge_port: u16 = parse_arg(&args, "--port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8765);
    let status_port: u16 = parse_arg(&args, "--status-port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8766);

    let buf = buffer::new_buffer();
    let start_time = Instant::now();

    let buf_bridge = buf.clone();
    let buf_status = buf.clone();
    let buf_mcp = buf.clone();
    let cwd_status = cwd.clone();
    let cwd_mcp = cwd.clone();

    let bridge_task = tokio::spawn(async move {
        tcp_bridge::run_tcp_bridge(buf_bridge, bridge_port).await;
    });

    let status_task = tokio::spawn(async move {
        status_server::run_status_server(buf_status, cwd_status, start_time, status_port).await;
    });

    let mcp_task = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async move {
            mcp::run_mcp_stdio(buf_mcp, cwd_mcp, start_time).await;
        });
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nBlackBox: shutting down");
        }
        _ = bridge_task => {}
        _ = status_task => {}
        _ = mcp_task => {}
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}
