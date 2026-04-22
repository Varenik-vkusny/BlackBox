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
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SpanParams {
    pub span_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct LogLineResp {
    pub text: String,
    pub timestamp_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_terminal: Option<String>,
}

#[derive(Serialize)]
pub struct LogResponse {
    pub lines: Vec<LogLineResp>,
}

fn build_router(state: DaemonState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/terminal", get(get_terminal_logs))
        .route("/api/compressed", get(get_compressed_logs))
        .route("/api/docker", get(get_docker_logs))
        .route("/api/diff", get(get_diff))
        .route("/api/postmortem", get(get_postmortem_handler))
        .route("/api/correlated", get(get_correlated_handler))
        .route("/api/http-errors", get(get_http_errors_handler))
        .route("/api/watched", get(get_watched_handler))
        .route("/api/commits", get(get_commits_handler))
        .route("/api/structured", get(get_structured_handler))
        .route("/api/inject", post(inject_log))
        .route("/api/clear", post(clear_logs))
        .route("/api/watch", post(watch_file))
        .route("/mcp", post(mcp_http_handler))
        .layer(cors)
        .with_state(state)
}

/// Called from main.rs with a pre-bound listener so the port is reserved
/// synchronously before any background tasks are spawned.
pub async fn run_admin_api_with_listener(
    state: DaemonState,
    listener: tokio::net::TcpListener,
) {
    let app = build_router(state);
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Admin API server error: {e}");
    }
}

pub async fn run_admin_api(state: DaemonState, port: u16) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Admin API failed to bind {addr}: {e}");
            return;
        }
    };
    run_admin_api_with_listener(state, listener).await;
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
    let buf = state.buf.ring.read().unwrap();
    let lines = buf.iter().rev().take(limit).map(|l| LogLineResp {
        text: l.text.clone(),
        timestamp_ms: l.timestamp_ms,
        source_terminal: l.source_terminal.clone(),
    }).collect::<Vec<_>>();
    Json(LogResponse { lines })
}

async fn get_compressed_logs(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let args = match params.source {
        Some(s) => serde_json::json!({"source": s, "limit": params.limit.unwrap_or(50)}),
        None => serde_json::json!({"limit": params.limit.unwrap_or(50)}),
    };
    let res = crate::mcp::tools::handle_tools_call(None, Some(serde_json::json!({"name": "get_compressed_errors", "arguments": args})), &state).await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "drain failed" }))
    }
}

async fn get_docker_logs(State(state): State<DaemonState>) -> impl IntoResponse {
    let docker_available = state.docker_reachable.load(std::sync::atomic::Ordering::Relaxed);
    let store = state.error_store.read().unwrap();
    let containers = store.container_ids();
    let events = store.get_events(None, 50);
    Json(serde_json::json!({
        "containers": containers,
        "events": events,
        "docker_available": docker_available
    }))
}

async fn get_diff(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let args = match params.source {
        Some(s) => serde_json::json!({"terminal": s}),
        None => serde_json::json!({}),
    };
    let res = crate::mcp::tools::handle_tools_call(None, Some(serde_json::json!({"name": "get_contextual_diff", "arguments": args})), &state).await;
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
    terminal: Option<String>,
}

async fn inject_log(
    State(state): State<DaemonState>,
    Json(payload): Json<InjectRequest>,
) -> impl IntoResponse {
    for line in payload.text.split('\n') {
        if !line.trim().is_empty() {
            push_line_and_drain(&state.buf, &state.drain, &state.structured, line.to_string(), payload.terminal.clone());
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

async fn clear_logs(State(state): State<DaemonState>) -> impl IntoResponse {
    let mut buf = state.buf.ring.write().unwrap();
    buf.clear();
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

#[derive(Deserialize)]
struct WatchRequest {
    path: String,
}

async fn watch_file(
    State(state): State<DaemonState>,
    Json(payload): Json<WatchRequest>,
) -> impl IntoResponse {
    let abs_path = if std::path::Path::new(&payload.path).is_absolute() {
        std::path::PathBuf::from(&payload.path)
    } else {
        state.cwd.join(&payload.path)
    };

    if !abs_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("file not found: {}", payload.path) })),
        );
    }

    let mut list = state.watch_list.write().unwrap();
    if list.contains(&abs_path) {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "already_watching" })));
    }
    list.push(abs_path);
    (StatusCode::OK, Json(serde_json::json!({ "status": "watching", "path": payload.path })))
}

async fn get_http_errors_handler(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50) as u64;
    let res = crate::mcp::tools::handle_tools_call(
        None,
        Some(serde_json::json!({"name": "get_http_errors", "arguments": {"limit": limit}})),
        &state,
    )
    .await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "http_errors failed" })),
    }
}

async fn get_watched_handler(State(state): State<DaemonState>) -> impl IntoResponse {
    let res = crate::mcp::tools::handle_tools_call(
        None,
        Some(serde_json::json!({"name": "get_watched_files"})),
        &state,
    )
    .await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "watched_files failed" })),
    }
}

async fn get_commits_handler(
    State(state): State<DaemonState>,
    Query(params): Query<LogParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20) as u64;
    let res = crate::mcp::tools::handle_tools_call(
        None,
        Some(serde_json::json!({"name": "get_recent_commits", "arguments": {"limit": limit}})),
        &state,
    )
    .await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "commits failed" })),
    }
}

async fn get_structured_handler(
    State(state): State<DaemonState>,
    Query(params): Query<SpanParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50) as u64;
    let args = match params.span_id {
        Some(sid) => serde_json::json!({"name": "get_structured_context", "arguments": {"span_id": sid, "limit": limit}}),
        None => serde_json::json!({"name": "get_structured_context", "arguments": {"limit": limit}}),
    };
    let res = crate::mcp::tools::handle_tools_call(None, Some(args), &state).await;
    match res.result {
        Some(v) => Json(v),
        None => Json(serde_json::json!({ "error": "structured failed" })),
    }
}

// ── MCP Streamable HTTP transport ────────────────────────────────────────────
//
// POST /mcp  accepts a single JSON-RPC 2.0 message and returns a JSON response.
// This is the simplest conforming implementation of the MCP Streamable HTTP
// transport: no SSE streaming, just request → response JSON.
//
// The client (Antigravity) is configured via:
//   "blackbox": { "serverUrl": "http://127.0.0.1:8768/mcp" }

async fn mcp_http_handler(
    State(state): State<DaemonState>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    use blackbox_core::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};
    use axum::http::header;

    let req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("BlackBox MCP-HTTP: parse error");
            let resp = JsonRpcResponse::error(None, error_codes::PARSE_ERROR, "Parse error".into());
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                serde_json::to_string(&resp).unwrap_or_default(),
            );
        }
    };

    let id = req.id.clone();
    let is_notification = id.is_none();

    let response: Option<JsonRpcResponse> = match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse::success(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {}, "logging": {} },
                "serverInfo": { "name": "blackbox", "version": env!("CARGO_PKG_VERSION") }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(JsonRpcResponse::success(id, serde_json::json!({}))),
        "tools/list" => {
            // Re-use the same list as the stdio transport
            let list_resp = crate::mcp::handle_tools_list_value();
            Some(JsonRpcResponse::success(id, list_resp))
        }
        "tools/call" => {
            let resp = crate::mcp::tools::handle_tools_call(id, req.params, &state).await;
            Some(resp)
        }
        _ => {
            if is_notification {
                None
            } else {
                Some(JsonRpcResponse::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("Method not found: {}", req.method),
                ))
            }
        }
    };

    match response {
        Some(resp) => {
            let body = serde_json::to_string(&resp).unwrap_or_default();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                body,
            )
        }
        // Notifications → 202 Accepted with empty body
        None => (StatusCode::ACCEPTED, [(header::CONTENT_TYPE, "application/json")], String::new()),
    }
}

