use blackbox_daemon::admin_api;
use blackbox_daemon::buffer;
use blackbox_daemon::daemon_state;
use blackbox_daemon::docker;
use blackbox_daemon::file_watcher;
use blackbox_daemon::http_proxy;
use blackbox_daemon::http_store;
use blackbox_daemon::mcp;
use blackbox_daemon::pii_masker;
use blackbox_daemon::pty_capture;
use blackbox_daemon::scanners;
use blackbox_daemon::structured_store;
use blackbox_daemon::tcp_bridge;
use blackbox_daemon::typed_context;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use buffer::new_buffer;
use daemon_state::DaemonState;
use docker::error_store::new_error_store;
use file_watcher::new_watch_list;
use http_store::new_http_store;
use scanners::drain::new_drain_state;
use structured_store::new_structured_store;

/// How long the primary daemon lingers after its stdio client disconnects
/// before shutting down. This keeps the buffer alive for HTTP clients
/// (Antigravity, blackbox-lab) after an AI session ends.
const GRACE_PERIOD_SECS: u64 = 300; // 5 minutes

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cwd = parse_arg(&args, "--cwd").map(PathBuf::from).unwrap_or_else(|| {
        std::env::current_dir().expect("Cannot determine current directory")
    });

    // ── Singleton detection ────────────────────────────────────────────────────
    // If a primary daemon is already running (port 8768 responds), act as a
    // lightweight MCP proxy: forward stdin → HTTP → stdout, then exit.
    // This lets multiple MCP clients (Claude Code, Antigravity, etc.) each spawn
    // the binary independently while sharing one real daemon instance.
    if mcp::proxy::primary_is_running().await {
        mcp::proxy::run_mcp_proxy().await;
        return;
    }

    eprintln!("BlackBox: starting as primary daemon (cwd={})", cwd.display());

    let bridge_port: u16 = parse_arg(&args, "--port")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8765);

    let state = DaemonState {
        buf: new_buffer(),
        drain: new_drain_state(),
        error_store: new_error_store(),
        http_store: new_http_store(),
        structured: new_structured_store(),
        cwd,
        start_time: Instant::now(),
        watch_list: new_watch_list(),
        docker_reachable: Arc::new(AtomicBool::new(false)),
    };

    // Side tasks — non-fatal: port-in-use just prints and returns.
    tokio::spawn({
        let buf = state.buf.clone();
        let drain = state.drain.clone();
        let structured = state.structured.clone();
        async move {
            tcp_bridge::run_tcp_bridge(buf, drain, structured, bridge_port).await;
            eprintln!("BlackBox: tcp_bridge exited (port {bridge_port} may be in use)");
        }
    });

    // Bind the admin port synchronously so it is reachable the moment any
    // concurrent process calls `primary_is_running()` or Antigravity sends
    // its first HTTP request — before axum even calls accept().
    let admin_listener = match tokio::net::TcpListener::bind("127.0.0.1:8768").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("BlackBox: failed to bind admin port 8768: {e}");
            return;
        }
    };
    tokio::spawn({
        let s = state.clone();
        async move {
            admin_api::run_admin_api_with_listener(s, admin_listener).await;
            eprintln!("BlackBox: admin_api exited");
        }
    });

    tokio::spawn({
        let store = state.error_store.clone();
        let reachable = state.docker_reachable.clone();
        async move {
            docker::run_docker_monitor(store, reachable).await;
            eprintln!("BlackBox: docker_monitor exited");
        }
    });

    tokio::spawn({
        let buf = state.buf.clone();
        let drain = state.drain.clone();
        let structured = state.structured.clone();
        let cwd = state.cwd.clone();
        let watch_list = state.watch_list.clone();
        async move {
            file_watcher::run_file_watcher(buf, drain, structured, cwd, watch_list).await;
        }
    });

    tokio::spawn({
        let http_store = state.http_store.clone();
        async move {
            http_proxy::run_http_proxy(http_store, 8769).await;
            eprintln!("BlackBox: http_proxy exited");
        }
    });

    // Native PTY capture (if requested)
    if args.contains(&"--capture-shell".to_string()) {
        let command = parse_arg(&args, "--shell");
        pty_capture::run_pty_capture(
            state.buf.clone(),
            state.drain.clone(),
            state.structured.clone(),
            command,
        );
        eprintln!("BlackBox: native-pty capture started");
    }

    // MCP stdio task — the primary AI session. When the client closes stdin
    // (EOF), we enter a grace period instead of immediately exiting, so the
    // buffer stays alive for HTTP clients.
    //
    // If stdin was immediately EOF (daemon started as a background service
    // with no console), `run_mcp_stdio` returns false and we skip the grace
    // period — the daemon stays alive indefinitely until Ctrl+C.
    let mcp_task = tokio::spawn({
        let s = state.clone();
        async move {
            mcp::run_mcp_stdio(s).await
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nBlackBox: shutting down via Ctrl+C");
        }
        had_client = mcp_task => {
            let had_client = had_client.unwrap_or(false);
            if had_client {
                // A real stdio client connected and then disconnected.
                // Enter grace period — HTTP clients may still be active.
                eprintln!(
                    "BlackBox: stdio client disconnected — grace period {}s before shutdown",
                    GRACE_PERIOD_SECS
                );
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        eprintln!("\nBlackBox: shutting down via Ctrl+C (grace period)");
                    }
                    _ = tokio::time::sleep(Duration::from_secs(GRACE_PERIOD_SECS)) => {
                        eprintln!("BlackBox: grace period expired, shutting down");
                    }
                }
            } else {
                // stdin was immediately at EOF — started as a background service
                // (e.g. start-daemon.ps1 / WindowStyle Hidden). Stay alive until
                // Ctrl+C so Antigravity and blackbox-lab keep working.
                eprintln!("BlackBox: no stdio client (service mode) — running until Ctrl+C");
                tokio::signal::ctrl_c().await.ok();
                eprintln!("\nBlackBox: shutting down via Ctrl+C");
            }
        }
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}
