mod buffer;
mod daemon_state;
mod docker;
mod file_watcher;
mod http_proxy;
mod http_store;
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
use file_watcher::new_watch_list;
use http_store::new_http_store;
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
        http_store: new_http_store(),
        cwd,
        start_time: Instant::now(),
        watch_list: new_watch_list(),
    };

    // Side tasks — these are non-fatal: if a port is already in use
    // (e.g. another daemon instance) the task returns but the daemon stays up.
    tokio::spawn({
        let buf = state.buf.clone();
        let drain = state.drain.clone();
        async move {
            tcp_bridge::run_tcp_bridge(buf, drain, bridge_port).await;
            eprintln!("BlackBox: tcp_bridge exited (port {bridge_port} may be in use)");
        }
    });

    tokio::spawn({
        let s = state.clone();
        async move {
            status_server::run_status_server(s.buf, s.cwd, s.start_time, status_port).await;
            eprintln!("BlackBox: status_server exited (port {status_port} may be in use)");
        }
    });

    tokio::spawn({
        let s = state.clone();
        async move {
            admin_api::run_admin_api(s, 8768).await;
            eprintln!("BlackBox: admin_api exited");
        }
    });

    tokio::spawn({
        let store = state.error_store.clone();
        async move {
            docker::run_docker_monitor(store).await;
            eprintln!("BlackBox: docker_monitor exited");
        }
    });

    tokio::spawn({
        let buf = state.buf.clone();
        let drain = state.drain.clone();
        let cwd = state.cwd.clone();
        let watch_list = state.watch_list.clone();
        async move {
            file_watcher::run_file_watcher(buf, drain, cwd, watch_list).await;
        }
    });

    tokio::spawn({
        let http_store = state.http_store.clone();
        async move {
            http_proxy::run_http_proxy(http_store, 8769).await;
            eprintln!("BlackBox: http_proxy exited");
        }
    });

    // The daemon lifetime is tied to the MCP stdio session (stdin EOF = done)
    // or until the user sends Ctrl+C.
    let mcp_task = tokio::spawn({
        let s = state.clone();
        async move {
            mcp::run_mcp_stdio(s).await;
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nBlackBox: shutting down via Ctrl+C");
        }
        _ = mcp_task => {
            // stdin was closed by the MCP client — normal shutdown
        }
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}
