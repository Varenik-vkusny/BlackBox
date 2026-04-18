use blackbox_core::protocol::{error_codes, JsonRpcResponse};
use serde_json::{json, Value};

use crate::buffer::{buffer_len, get_last_n, has_recent_errors};
use crate::daemon_state::DaemonState;
use crate::scanners::drain::{get_error_clusters, total_error_line_count};
use crate::scanners::stacktrace::{extract_source_files, extract_stack_traces};

pub async fn handle_tools_call(
    id: Option<Value>,
    params: Option<Value>,
    state: &DaemonState,
) -> JsonRpcResponse {
    let params = params.unwrap_or(json!({}));
    let tool_name = match params["name"].as_str() {
        Some(n) => n.to_string(),
        None => {
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Missing tool name".into(),
            )
        }
    };
    let args = match params["arguments"].clone() {
        Value::Null => json!({}),
        v => v,
    };

    match tool_name.as_str() {
        "get_snapshot" => get_snapshot(id, state).await,
        "get_terminal_buffer" => get_terminal_buffer(id, state, &args),
        "get_project_metadata" => get_project_metadata(id, state).await,
        "read_file" => read_file(id, state, &args),
        "get_compressed_errors" => get_compressed_errors(id, state, &args).await,
        "get_contextual_diff" => get_contextual_diff(id, state).await,
        "get_container_logs" => get_container_logs(id, state, &args).await,
        "get_postmortem" => get_postmortem(id, state, &args).await,
        "get_correlated_errors" => get_correlated_errors(id, state, &args).await,
        _ => JsonRpcResponse::error(
            id,
            error_codes::METHOD_NOT_FOUND,
            format!("Unknown tool: {tool_name}"),
        ),
    }
}

// ── Shared data collectors (used by primary handlers and fallbacks) ──────────

fn collect_terminal_buffer_data(state: &DaemonState, n: usize) -> Value {
    let lines = get_last_n(&state.buf, n);
    let content: String = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    let output = crate::typed_context::wrap_untrusted(&content, "vscode_bridge");
    json!({ "content": output, "lines_returned": lines.len() })
}

async fn collect_compressed_errors_data(state: &DaemonState, limit: usize) -> Value {
    let clusters = get_error_clusters(&state.drain, limit);
    let total = total_error_line_count(&state.drain);
    let lines = get_last_n(&state.buf, 500);
    let stack_traces = tokio::task::spawn_blocking(move || extract_stack_traces(&lines))
        .await
        .unwrap_or_default();
    json!({
        "clusters": clusters,
        "stack_traces": stack_traces,
        "total_error_lines": total
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

async fn get_snapshot(id: Option<Value>, state: &DaemonState) -> JsonRpcResponse {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let cwd = state.cwd.clone();
    let (branch, dirty_files, project_type) =
        tokio::task::spawn_blocking(move || {
            let (branch, dirty) = crate::scanners::git::scan_git(&cwd);
            let manifests = crate::scanners::manifests::scan_manifests(&cwd);
            let pt = manifests
                .first()
                .map(|m| format!("{:?}", m.manifest_type).to_lowercase())
                .unwrap_or_else(|| "unknown".into());
            (branch, dirty, pt)
        })
        .await
        .unwrap_or_else(|_| ("unknown".into(), 0, "unknown".into()));

    JsonRpcResponse::success(
        id,
        json!({
            "daemon_uptime_secs": uptime_secs,
            "project_type": project_type,
            "git_branch": branch,
            "git_dirty_files": dirty_files,
            "buffer_lines": buffer_len(&state.buf),
            "has_recent_errors": has_recent_errors(&state.buf)
        }),
    )
}

fn get_terminal_buffer(id: Option<Value>, state: &DaemonState, args: &Value) -> JsonRpcResponse {
    let n = args["lines"].as_u64().unwrap_or(100).min(500) as usize;
    let data = collect_terminal_buffer_data(state, n);
    let lines_returned = data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned == 0 {
        return JsonRpcResponse::success(
            id,
            json!({
                "content": "",
                "lines_returned": 0,
                "fallback_source": "none",
                "fallback_reason": "terminal buffer is empty — no data has been captured yet"
            }),
        );
    }

    JsonRpcResponse::success(id, data)
}

async fn get_project_metadata(id: Option<Value>, state: &DaemonState) -> JsonRpcResponse {
    let cwd = state.cwd.clone();
    let (manifests, env_keys) = tokio::task::spawn_blocking(move || {
        let manifests = crate::scanners::manifests::scan_manifests(&cwd);
        let env_keys = crate::scanners::env::scan_env_keys(&cwd);
        (manifests, env_keys)
    })
    .await
    .unwrap_or_default();

    JsonRpcResponse::success(id, json!({ "manifests": manifests, "env_keys": env_keys }))
}

fn read_file(id: Option<Value>, state: &DaemonState, args: &Value) -> JsonRpcResponse {
    let path_str = match args["path"].as_str() {
        Some(p) => p,
        None => return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing path".into()),
    };

    let requested = state.cwd.join(path_str);
    let canonical_cwd = match std::fs::canonicalize(&state.cwd) {
        Ok(p) => p,
        Err(_) => return JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, "Cannot resolve cwd".into()),
    };
    let canonical_req = match std::fs::canonicalize(&requested) {
        Ok(p) => p,
        Err(_) => return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, format!("File not found: {path_str}")),
    };
    if !canonical_req.starts_with(&canonical_cwd) {
        return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, "Path traversal not allowed".into());
    }

    let content = match std::fs::read_to_string(&canonical_req) {
        Ok(c) => c,
        Err(e) => return JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, format!("Read error: {e}")),
    };

    let lines: Vec<&str> = content.lines().collect();
    let from = args["from_line"].as_u64().map(|n| (n as usize).saturating_sub(1)).unwrap_or(0);
    let to = args["to_line"].as_u64().map(|n| (n as usize).min(lines.len())).unwrap_or(lines.len());
    let slice = lines[from.min(lines.len())..to.min(lines.len())].join("\n");

    JsonRpcResponse::success(id, json!({
        "path": path_str,
        "content": slice,
        "from_line": from + 1,
        "to_line": to
    }))
}

async fn get_compressed_errors(
    id: Option<Value>,
    state: &DaemonState,
    args: &Value,
) -> JsonRpcResponse {
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
    let data = collect_compressed_errors_data(state, limit).await;

    let has_clusters = data["clusters"].as_array().map_or(false, |a| !a.is_empty());
    let has_traces = data["stack_traces"].as_array().map_or(false, |a| !a.is_empty());

    if !has_clusters && !has_traces {
        // Fallback: return raw terminal buffer so AI has something to work with.
        let buf_data = collect_terminal_buffer_data(state, 100);
        let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

        if lines_returned == 0 {
            return JsonRpcResponse::success(
                id,
                json!({
                    "clusters": [],
                    "stack_traces": [],
                    "total_error_lines": 0,
                    "fallback_source": "none",
                    "fallback_reason": "no error clusters or stack traces found, and terminal buffer is empty — no data captured yet"
                }),
            );
        }

        return JsonRpcResponse::success(
            id,
            json!({
                "clusters": [],
                "stack_traces": [],
                "total_error_lines": data["total_error_lines"],
                "fallback_source": "terminal_buffer",
                "fallback_reason": "no error clusters or stack traces found in drain — showing raw terminal buffer instead; consider injecting logs via the terminal bridge",
                "terminal_buffer": buf_data
            }),
        );
    }

    JsonRpcResponse::success(id, data)
}

async fn get_contextual_diff(id: Option<Value>, state: &DaemonState) -> JsonRpcResponse {
    let cwd = state.cwd.clone();
    let lines = get_last_n(&state.buf, 500);

    let diff_result = tokio::task::spawn_blocking(move || {
        let traces = extract_stack_traces(&lines);
        let error_files = extract_source_files(&traces);

        let changed = crate::scanners::git::get_changed_files(&cwd);
        // Normalize both sides so relative vs absolute and \ vs / don't cause misses
        let changed_paths: std::collections::HashSet<String> = changed
            .iter()
            .map(|f| crate::scanners::git::normalize_path(&f.path, &cwd))
            .collect();

        let relevant: Vec<String> = error_files
            .into_iter()
            .map(|f| crate::scanners::git::normalize_path(&f, &cwd))
            .filter(|f| changed_paths.contains(f))
            .collect();

        let (hunks, truncated) = if relevant.is_empty() {
            (vec![], false)
        } else {
            crate::scanners::git::get_diff_hunks(&cwd, &relevant)
        };

        (hunks, relevant, truncated)
    })
    .await;

    let (hunks, relevant, truncated) = match diff_result {
        Ok(v) => v,
        Err(_) => return JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, "Diff failed".into()),
    };

    if !hunks.is_empty() {
        return JsonRpcResponse::success(
            id,
            json!({
                "diff_hunks": hunks,
                "files_cross_referenced": relevant,
                "truncated": truncated,
                "fallback_source": "none"
            }),
        );
    }

    // Diff was empty — fall back to compressed errors for useful context.
    let err_data = collect_compressed_errors_data(state, 50).await;
    let has_clusters = err_data["clusters"].as_array().map_or(false, |a| !a.is_empty());
    let has_traces = err_data["stack_traces"].as_array().map_or(false, |a| !a.is_empty());

    if has_clusters || has_traces {
        return JsonRpcResponse::success(
            id,
            json!({
                "diff_hunks": [],
                "files_cross_referenced": relevant,
                "truncated": false,
                "fallback_source": "compressed_errors",
                "fallback_reason": "no stack trace files matched dirty git files — showing error clusters and stack traces instead",
                "clusters": err_data["clusters"],
                "stack_traces": err_data["stack_traces"],
                "total_error_lines": err_data["total_error_lines"]
            }),
        );
    }

    // Last resort: raw terminal buffer.
    let buf_data = collect_terminal_buffer_data(state, 100);
    let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned > 0 {
        return JsonRpcResponse::success(
            id,
            json!({
                "diff_hunks": [],
                "files_cross_referenced": relevant,
                "truncated": false,
                "fallback_source": "terminal_buffer",
                "fallback_reason": "no diff context and no error clusters found — showing raw terminal buffer; check if logs are flowing via the VS Code terminal bridge",
                "terminal_buffer": buf_data
            }),
        );
    }

    JsonRpcResponse::success(
        id,
        json!({
            "diff_hunks": [],
            "files_cross_referenced": [],
            "truncated": false,
            "fallback_source": "none",
            "fallback_reason": "no diff context, no error clusters, and terminal buffer is empty — no data available yet"
        }),
    )
}

async fn get_container_logs(id: Option<Value>, state: &DaemonState, args: &Value) -> JsonRpcResponse {
    let container_id = args["container_id"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;

    let (containers, events, docker_available) = {
        let store = state.error_store.read().unwrap();
        let containers = store.container_ids();
        let docker_available = !containers.is_empty();
        let events = store.get_events(container_id, limit);
        (containers, events, docker_available)
    };

    if docker_available && !events.is_empty() {
        return JsonRpcResponse::success(
            id,
            json!({
                "containers": containers,
                "events": events,
                "docker_available": true,
                "fallback_source": "none"
            }),
        );
    }

    // Docker running but no errors stored yet — still report correctly.
    if docker_available {
        return JsonRpcResponse::success(
            id,
            json!({
                "containers": containers,
                "events": [],
                "docker_available": true,
                "fallback_source": "none",
                "fallback_reason": "Docker is connected and containers are tracked, but no ERROR/WARN/FATAL events have been captured yet"
            }),
        );
    }

    // Docker not reachable — fall back to compressed errors then terminal buffer.
    let err_data = collect_compressed_errors_data(state, 50).await;
    let has_clusters = err_data["clusters"].as_array().map_or(false, |a| !a.is_empty());
    let has_traces = err_data["stack_traces"].as_array().map_or(false, |a| !a.is_empty());

    if has_clusters || has_traces {
        return JsonRpcResponse::success(
            id,
            json!({
                "containers": [],
                "events": [],
                "docker_available": false,
                "fallback_source": "compressed_errors",
                "fallback_reason": "Docker is not reachable — showing terminal error clusters instead; start Docker Desktop if container monitoring is needed",
                "clusters": err_data["clusters"],
                "stack_traces": err_data["stack_traces"],
                "total_error_lines": err_data["total_error_lines"]
            }),
        );
    }

    let buf_data = collect_terminal_buffer_data(state, 100);
    let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned > 0 {
        return JsonRpcResponse::success(
            id,
            json!({
                "containers": [],
                "events": [],
                "docker_available": false,
                "fallback_source": "terminal_buffer",
                "fallback_reason": "Docker is not reachable and no error clusters found — showing raw terminal buffer",
                "terminal_buffer": buf_data
            }),
        );
    }

    JsonRpcResponse::success(
        id,
        json!({
            "containers": [],
            "events": [],
            "docker_available": false,
            "fallback_source": "none",
            "fallback_reason": "Docker is not reachable and terminal buffer is empty — no data available"
        }),
    )
}

// ── Phase 3 tools ─────────────────────────────────────────────────────────────

pub async fn get_postmortem(id: Option<Value>, state: &DaemonState, args: &Value) -> JsonRpcResponse {
    let minutes = args["minutes"].as_u64().unwrap_or(30).clamp(1, 1440);
    let now = crate::buffer::now_ms();
    let cutoff_ms = now.saturating_sub(minutes * 60 * 1_000);

    let lines = {
        let guard = state.buf.read().unwrap();
        guard
            .iter()
            .filter(|l| l.timestamp_ms >= cutoff_ms)
            .cloned()
            .collect::<Vec<_>>()
    };

    // Group log lines into 1-minute buckets (offset from cutoff, not wall clock)
    let mut buckets: std::collections::BTreeMap<u64, (usize, usize, String)> =
        std::collections::BTreeMap::new();
    for line in &lines {
        let minute = (line.timestamp_ms - cutoff_ms) / 60_000;
        let entry = buckets.entry(minute).or_insert((0, 0, String::new()));
        entry.0 += 1; // total lines
        let lower = line.text.to_lowercase();
        if lower.contains("error") || lower.contains("panic") || lower.contains("fatal") {
            entry.1 += 1; // error lines
        }
        entry.2 = line.text.clone(); // keep last line as sample
    }

    let timeline: Vec<Value> = buckets
        .into_iter()
        .map(|(minute, (line_count, error_count, sample))| {
            json!({
                "minute_offset": minute,
                "line_count": line_count,
                "error_count": error_count,
                "sample": sample
            })
        })
        .collect();

    // Docker events within the same window
    let docker_in_window = {
        let store = state.error_store.read().unwrap();
        store
            .get_events(None, 200)
            .into_iter()
            .filter(|e| e.timestamp_ms >= cutoff_ms)
            .count()
    };

    // Stack traces from the window
    let traces = {
        let lines_clone = lines.clone();
        tokio::task::spawn_blocking(move || extract_stack_traces(&lines_clone))
            .await
            .unwrap_or_default()
    };

    JsonRpcResponse::success(
        id,
        json!({
            "window_minutes": minutes,
            "total_lines": lines.len(),
            "timeline": timeline,
            "docker_events_in_window": docker_in_window,
            "stack_traces": traces,
            "fallback_source": "none"
        }),
    )
}

pub async fn get_correlated_errors(id: Option<Value>, state: &DaemonState, args: &Value) -> JsonRpcResponse {
    let window_secs = args["window_secs"].as_u64().unwrap_or(5);
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    // Collect recent terminal error lines
    let terminal_errors: Vec<_> = {
        let guard = state.buf.read().unwrap();
        guard
            .iter()
            .filter(|l| {
                let lower = l.text.to_lowercase();
                lower.contains("error") || lower.contains("panic") || lower.contains("fatal")
            })
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(limit)
            .collect()
    };

    // For each terminal error, find Docker events within ±window_secs
    let all_docker = {
        let store = state.error_store.read().unwrap();
        store.get_events(None, 500)
    };

    let correlations: Vec<Value> = terminal_errors
        .iter()
        .map(|tl| {
            let nearby: Vec<Value> = all_docker
                .iter()
                .filter(|de| {
                    let diff = if de.timestamp_ms > tl.timestamp_ms {
                        de.timestamp_ms - tl.timestamp_ms
                    } else {
                        tl.timestamp_ms - de.timestamp_ms
                    };
                    diff <= window_secs * 1_000
                })
                .map(|de| json!({"source": de.source, "text": de.text, "level": de.level}))
                .collect();
            json!({
                "terminal_line": tl.text,
                "timestamp_ms": tl.timestamp_ms,
                "correlated_docker_events": nearby
            })
        })
        .collect();

    let has_correlations = correlations.iter().any(|c| {
        c["correlated_docker_events"]
            .as_array()
            .map_or(false, |a| !a.is_empty())
    });

    JsonRpcResponse::success(
        id,
        json!({
            "correlations": correlations,
            "has_cross_source_correlations": has_correlations,
            "window_secs": window_secs,
            "fallback_source": if has_correlations { "none" } else { "terminal_only" }
        }),
    )
}
