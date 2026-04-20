use blackbox_core::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};
use serde_json::{json, Value};

use crate::daemon_state::DaemonState;

pub mod proxy;
pub mod tools;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Returns `true` if at least one real MCP message was handled (a real stdio
/// client connected and later disconnected). Returns `false` if stdin was
/// immediately at EOF (daemon started as a background service with no
/// console — Windows NUL device, `Start-Process -WindowStyle Hidden`, etc.).
pub async fn run_mcp_stdio(state: DaemonState) -> bool {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    let mut handled_any = false;

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Some MCP clients send HTTP-style headers (Content-Length: ...)
        // We skip these to support those clients robustly.
        if trimmed.to_lowercase().starts_with("content-") {
            continue;
        }

        handled_any = true;

        if let Some(response) = handle_message(&line, &state).await {
            let json = serde_json::to_string(&response).unwrap_or_default();
            let mut out_bytes = json.as_bytes().to_vec();
            out_bytes.push(b'\n');
            let _ = stdout.write_all(&out_bytes).await;
            let _ = stdout.flush().await;
        }
    }

    eprintln!("BlackBox MCP: stdio loop ended (stdin EOF, handled_any={handled_any})");
    handled_any
}

fn extract_method(line: &str) -> String {
    // serde_json serializes as compact: "method":"foo" — no spaces
    for prefix in &[r#""method":""#, r#""method": ""#] {
        if let Some(start) = line.find(prefix) {
            let rest = &line[start + prefix.len()..];
            if let Some(end) = rest.find('"') {
                return rest[..end].to_string();
            }
        }
    }
    "<unknown>".to_string()
}

async fn handle_message(line: &str, state: &DaemonState) -> Option<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(_) => {
            return Some(JsonRpcResponse::error(None, error_codes::PARSE_ERROR, "Parse error".into()))
        }
    };

    let id = req.id.clone();
    let is_notification = id.is_none();

    let response = match req.method.as_str() {
        "initialize" => Some(handle_initialize(id, req.params)),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(tools::handle_tools_call(id, req.params, state).await),
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

    response
}

fn handle_initialize(id: Option<Value>, _params: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "logging": {}
            },
            "serverInfo": {
                "name": "blackbox",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

pub fn handle_tools_list_value() -> serde_json::Value {
    json!({
        "tools": [
            {
                "name": "get_snapshot",
                "description": "Provides a high-level overview of the current system state. Use this as your 'entry point' when starting a task. Returns daemon uptime, detected project type (Rust, Java, Python, etc.), current Git branch, number of dirty files, and terminal buffer size. Ideal for orientation and deciding which specific scanner to run next.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_terminal_buffer",
                "description": "Retrieves the raw captured output from the terminal. Use this to see exactly what the developer saw, including compilation logs, test results, or runtime output. Output is ANSI-cleaned and safely wrapped in XML tags to prevent prompt injection. Best for reading sequential logs or finding specific strings not captured by specialized scanners.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lines": { 
                            "type": "integer", 
                            "description": "Number of most recent lines to fetch. Default is 100, max is 500. Use a smaller number if you just need the last few lines of context." 
                        }
                    }
                }
            },
            {
                "name": "get_project_metadata",
                "description": "Scans the project for developer-relevant metadata. Lists all detected manifest files (like package.json, Cargo.toml, pom.xml) and identifies environment variable keys defined in .env files (values are masked for security). Critical for understanding project dependencies and available configuration without manually reading every file.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "read_file",
                "description": "Reads specific line ranges from files within the project workspace. Use this AFTER identifying a specific file and line number from a stack trace or compilation error. Includes path-traversal protection and automatic line-number slicing to save tokens. Essential for validating fixes or inspecting logic mentioned in logs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Relative path to the file from the project root." },
                        "from_line": { "type": "integer", "description": "Starting line number (1-based index). Default: 1." },
                        "to_line": { "type": "integer", "description": "Ending line number (inclusive, 1-based index). Default: End of file." }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_compressed_errors",
                "description": "The primary tool for error analysis. Uses the Drain template-mining algorithm to group similar errors into 'clusters' and extracts full stack traces from the terminal buffer. Much more token-efficient than reading raw logs. Use this to quickly identify the 'what' and 'where' of recurring issues across thousands of log lines.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { 
                            "type": "integer", 
                            "description": "Maximum number of error clusters to return. Default: 50." 
                        }
                    }
                }
            },
            {
                "name": "get_contextual_diff",
                "description": "Performs surgical analysis of recent changes. It extracts file names from recent stack traces and returns the Git diff hunks ONLY for those relevant files. If no stack traces are found, it identifies changed files and searches for related error clusters. This is the fastest way to understand if a recent code change caused a current failure.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_container_logs",
                "description": "Monitors Docker container events in real-time. Fetches filtered logs (ERROR, WARN, FATAL levels only) from all running containers. If Docker is unavailable, it automatically falls back to terminal error patterns. Use this when you suspect an infrastructure issue or a background service crash that isn't showing in the main terminal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "container_id": { "type": "string", "description": "Optional ID or name to filter logs for a specific container." },
                        "limit": { "type": "integer", "description": "Max number of events to return. Default: 50." }
                    }
                }
            },
            {
                "name": "get_postmortem",
                "description": "Generates a timeline of events leading up to a crash. Groups logs into 1-minute buckets, identifies error spikes, and correlates them with container events and stack traces. Use this to answer 'How did we get here?' after an incident. Ideal for high-level incident response summaries.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "minutes": { 
                            "type": "integer", 
                            "description": "The analysis window in minutes. Default: 30, Max: 1440 (24h). Higher windows consume more memory." 
                        }
                    }
                }
            },
            {
                "name": "get_correlated_errors",
                "description": "Cross-references terminal panics with infrastructure logs. Searches for Docker container crashes or warnings that occurred within a tight time window (e.g., ±5 seconds) of a terminal error. Perfect for identifying 'cascading failures' where a database crash in a container leads to a connection error in the user's terminal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "window_secs": {
                            "type": "integer",
                            "description": "The correlation window in seconds. Default: 5 seconds."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max terminal errors to analyze. Default: 20."
                        }
                    }
                }
            },
            {
                "name": "get_recent_commits",
                "description": "Returns recent git commit history with lightweight metadata (hash, message, author, timestamp, changed files, insertions/deletions). No diffs — token-efficient. Ideal for answering 'which commit could have broken this?' after seeing a stack trace. Use path_filter to restrict history to commits that touched a specific file.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Number of commits to return. Default: 20, max: 100."
                        },
                        "path_filter": {
                            "type": "string",
                            "description": "Optional relative file path — only show commits that touched this file. E.g. 'src/main.rs'."
                        }
                    }
                }
            },
            {
                "name": "watch_log_file",
                "description": "Subscribe to a log file for real-time monitoring. BlackBox will tail the file and feed new lines into the buffer (ANSI-stripped, PII-masked) with source tag 'file:<name>'. Use get_terminal_buffer with terminal='file:<name>' to read file-sourced logs. Auto-detection covers *.log at cwd root and logs/ directory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative or absolute path to the log file to watch."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_watched_files",
                "description": "List all log files currently being watched by the file watcher. Shows relative paths from the project root.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "get_http_errors",
                "description": "Returns HTTP requests that resulted in 4xx/5xx errors, captured by BlackBox's local proxy on port 8769. To use: set HTTP_PROXY=http://127.0.0.1:8769 or add X-Proxy-Target header. Only error responses are stored (max 200). Correlated with terminal errors in get_correlated_errors. Ideal for answering 'what HTTP request triggered this error?'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Max number of HTTP error events to return. Default: 50, max: 200."
                        }
                    }
                }
            },
            {
                "name": "get_process_logs",
                "description": "Returns captured stdout/stderr from processes launched with 'blackbox-run <command>'. Each process appears as a separate terminal stream tagged 'process:<pid>'. Use pid parameter to filter to a specific process, or omit for all processes. Lists all known process PIDs captured. Run 'blackbox-run node server.js' instead of 'node server.js' to enable capture.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": {
                            "type": "integer",
                            "description": "Optional PID to filter logs for a specific process. Omit to get logs from all captured processes."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max number of lines to return. Default: 200, max: 500."
                        }
                    }
                }
            },
            {
                "name": "get_structured_context",
                "description": "Queries structured JSON log events parsed from the terminal buffer. Supports Rust tracing, Node.js pino/bunyan, Go logrus/zap, and Python structlog formats. Use span_id to retrieve the full chain of events for a single request (e.g. 'db query started → db query failed → handler threw'). Without span_id, returns the most recent structured events. Ideal for distributed tracing without Jaeger/Zipkin — works locally with zero infrastructure.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "span_id": {
                            "type": "string",
                            "description": "Optional span ID to filter by. Returns all events from a single request trace. E.g. 'abc123'."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max events to return when no span_id specified. Default: 50, max: 200."
                        }
                    }
                }
            }
        ]
    })
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(id, handle_tools_list_value())
}

