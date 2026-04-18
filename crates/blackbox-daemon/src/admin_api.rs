use axum::{
    extract::{State, Query},
    routing::{get, post},
    Json, Router,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use std::net::SocketAddr;

use crate::daemon_state::DaemonState;
use crate::buffer::{buffer_len, push_line_and_drain};
use crate::scanners::git::scan_git;
use crate::scanners::manifests::scan_manifests;
use blackbox_core::types::{StatusResponse, ProjectKind};

#[derive(Debug, Deserialize)]
pub struct LogParams {
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct LogResponse {
    pub lines: Vec<String>,
}

pub async fn run_admin_api(state: DaemonState, port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/terminal", get(get_terminal_logs))
        .route("/api/compressed", get(get_compressed_logs))
        .route("/api/docker", get(get_docker_logs))
        .route("/api/diff", get(get_diff))
        .route("/api/postmortem", get(get_postmortem_handler))
        .route("/api/correlated", get(get_correlated_handler))
        .route("/api/inject", post(inject_log))
        .route("/api/clear", post(clear_logs))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Admin API failed to bind {addr}: {e}");
            return;
        }
    };
    axum::serve(listener, app).await.unwrap();
}

async fn get_status(State(state): State<DaemonState>) -> impl IntoResponse {
    let (branch_str, dirty) = scan_git(&state.cwd);
    let git_branch = match branch_str.as_str() {
        "unknown" | "HEAD (detached)" => None,
        other => Some(other.to_string()),
    };

    let manifests = scan_manifests(&state.cwd);
    let project_type = manifests
        .first()
        .map(|m| m.manifest_type.clone())
        .unwrap_or(ProjectKind::Unknown);

    Json(StatusResponse {
        uptime_secs: state.start_time.elapsed().as_secs(),
        buffer_lines: buffer_len(&state.buf),
        git_branch,
        git_dirty_files: dirty,
        project_type,
        has_recent_errors: crate::buffer::has_recent_errors(&state.buf),
    })
}

async fn get_terminal_logs(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let buf = state.buf.read().unwrap();
    let lines = buf.iter().rev().take(limit).map(|l| l.text.clone()).collect::<Vec<_>>();
    Json(LogResponse { lines })
}

async fn get_compressed_logs(State(state): State<DaemonState>) -> impl IntoResponse {
    let res = crate::mcp::tools::handle_tools_call(None, Some(serde_json::json!({"name": "get_compressed_errors"})), &state).await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "drain failed" }))
    }
}

async fn get_docker_logs(State(state): State<DaemonState>) -> impl IntoResponse {
    let store = state.error_store.read().unwrap();
    let containers = store.container_ids();
    let events = store.get_events(None, 50);
    Json(serde_json::json!({
        "containers": containers,
        "events": events,
        "docker_available": !containers.is_empty()
    }))
}

async fn get_diff(State(state): State<DaemonState>) -> impl IntoResponse {
    let res = crate::mcp::tools::handle_tools_call(None, Some(serde_json::json!({"name": "get_contextual_diff"})), &state).await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "diff failed" }))
    }
}

async fn get_postmortem_handler(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let minutes = params.limit.unwrap_or(30) as u64;
    let res = crate::mcp::tools::handle_tools_call(
        None,
        Some(serde_json::json!({"name": "get_postmortem", "arguments": {"minutes": minutes}})),
        &state,
    )
    .await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "postmortem failed" })),
    }
}

async fn get_correlated_handler(State(state): State<DaemonState>) -> impl IntoResponse {
    let res = crate::mcp::tools::handle_tools_call(
        None,
        Some(serde_json::json!({"name": "get_correlated_errors", "arguments": {}})),
        &state,
    )
    .await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "correlation failed" })),
    }
}

#[derive(Deserialize)]
struct InjectRequest {
    text: String,
}

async fn inject_log(
    State(state): State<DaemonState>,
    Json(payload): Json<InjectRequest>,
) -> impl IntoResponse {
    // Split on newlines so multi-line stack traces become separate LogLines,
    // which is required for the stack trace state-machine parser to detect them.
    // Use push_line_and_drain so the Drain clustering state also gets updated.
    for line in payload.text.split('\n') {
        if !line.trim().is_empty() {
            push_line_and_drain(&state.buf, &state.drain, line.to_string());
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn clear_logs(State(state): State<DaemonState>) -> impl IntoResponse {
    let mut buf = state.buf.write().unwrap();
    buf.clear();
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}
