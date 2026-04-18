mod buffer;
mod daemon_state;
mod docker;
mod mcp;
mod pii_masker;
mod scanners;
mod status_server;
mod tcp_bridge;
mod typed_context;
mod admin_api;

use std::path::PathBuf;
use std::time::Instant;

use buffer::new_buffer;
use daemon_state::DaemonState;
use docker::error_store::new_error_store;
use scanners::drain::new_drain_state;

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

    let state = DaemonState {
        buf: new_buffer(),
        drain: new_drain_state(),
        error_store: new_error_store(),
        cwd,
        start_time: Instant::now(),
    };

    let bridge_task = {
        let buf = state.buf.clone();
        let drain = state.drain.clone();
        tokio::spawn(async move {
            tcp_bridge::run_tcp_bridge(buf, drain, bridge_port).await;
        })
    };

    let status_task = {
        let s = state.clone();
        tokio::spawn(async move {
            status_server::run_status_server(s.buf, s.cwd, s.start_time, status_port).await;
        })
    };

    let mcp_task = {
        let s = state.clone();
        tokio::task::spawn_blocking(move || {
            tokio::runtime::Handle::current().block_on(async move {
                mcp::run_mcp_stdio(s).await;
            });
        })
    };

    let admin_task = {
        let s = state.clone();
        tokio::spawn(async move {
            admin_api::run_admin_api(s, 8768).await;
        })
    };

    let docker_task = {
        let store = state.error_store.clone();
        tokio::spawn(async move {
            docker::run_docker_monitor(store).await;
        })
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nBlackBox: shutting down");
        }
        _ = bridge_task => {}
        _ = status_task => {}
        _ = mcp_task => {}
        _ = admin_task => {}
        _ = docker_task => {}
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}
