use std::io::{self, BufRead, Write};

use blackbox_core::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};
use serde_json::{json, Value};

use crate::daemon_state::DaemonState;

pub mod tools;

pub async fn run_mcp_stdio(state: DaemonState) {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            _ => continue,
        };

        let response = handle_message(&line, &state).await;
        let mut json = serde_json::to_string(&response).unwrap_or_default();
        json.push('\n');

        let mut out = stdout.lock();
        let _ = out.write_all(json.as_bytes());
        let _ = out.flush();
    }
}

async fn handle_message(line: &str, state: &DaemonState) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(_) => {
            return JsonRpcResponse::error(None, error_codes::PARSE_ERROR, "Parse error".into())
        }
    };

    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(id),
        "tools/call" => tools::handle_tools_call(id, req.params, state).await,
        _ => JsonRpcResponse::error(
            id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", req.method),
        ),
    }
}

fn handle_initialize(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "blackbox",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        json!({
            "tools": [
                {
                    "name": "get_snapshot",
                    "description": "Returns a compact system snapshot: daemon uptime, project type, git status, and buffer stats. Call this first to get a map of the current context.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "get_terminal_buffer",
                    "description": "Returns recent terminal output lines, ANSI-cleaned and wrapped in safety XML tags to prevent prompt injection.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "lines": { "type": "integer", "description": "Number of recent lines to return (default: 100, max: 500)" }
                        }
                    }
                },
                {
                    "name": "get_project_metadata",
                    "description": "Returns detected project manifests (sorted by language priority) and .env key names (values masked).",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "read_file",
                    "description": "Reads a file within the project directory. Use when terminal logs reference a specific file:line error.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Relative or absolute path within the project" },
                            "from_line": { "type": "integer", "description": "Start line (1-based, optional)" },
                            "to_line": { "type": "integer", "description": "End line (1-based, optional)" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "get_compressed_errors",
                    "description": "Returns deduplicated error clusters (Drain algorithm) and parsed stack traces from terminal output. More token-efficient than get_terminal_buffer for error analysis.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "description": "Max clusters to return (default: 50)" }
                        }
                    }
                },
                {
                    "name": "get_contextual_diff",
                    "description": "Returns git diff hunks cross-referenced with files mentioned in recent stack traces. Surgical precision: only diffs files relevant to current errors.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "get_container_logs",
                    "description": "Returns filtered Docker container error events (ERROR/WARN/FATAL only). Returns docker_available: false when Docker is not running.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "container_id": { "type": "string", "description": "Optional container ID or name filter" },
                            "limit": { "type": "integer", "description": "Max events to return (default: 50)" }
                        }
                    }
                },
                {
                    "name": "get_postmortem",
                    "description": "Timeline analysis of the last N minutes of activity. Groups log lines by minute, counts error spikes, and surfaces stack traces within the window. Use after an incident to understand the chain of events.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "minutes": { "type": "integer", "description": "History window in minutes (default: 30, max: 1440)" }
                        }
                    }
                },
                {
                    "name": "get_correlated_errors",
                    "description": "Cross-references terminal errors with Docker container events by timestamp proximity. Reveals whether a terminal panic and a Docker container crash happened within the same time window.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "window_secs": { "type": "integer", "description": "Time window for correlation in seconds (default: 5)" },
                            "limit": { "type": "integer", "description": "Max terminal errors to correlate (default: 20)" }
                        }
                    }
                }
            ]
        }),
    )
}
