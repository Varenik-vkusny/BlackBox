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

    let result = match tool_name.as_str() {
        "get_snapshot" => get_snapshot(state).await,
        "get_terminal_buffer" => get_terminal_buffer(state, &args),
        "get_project_metadata" => get_project_metadata(state).await,
        "read_file" => read_file(state, &args),
        "get_compressed_errors" => get_compressed_errors(state, &args).await,
        "get_contextual_diff" => get_contextual_diff(state, &args).await,
        "get_container_logs" => get_container_logs(state, &args).await,
        "get_postmortem" => get_postmortem(state, &args).await,
        "get_correlated_errors" => get_correlated_errors(state, &args).await,
        "get_recent_commits" => get_recent_commits(state, &args).await,
        "watch_log_file" => watch_log_file(state, &args),
        "get_watched_files" => get_watched_files(state),
        "get_http_errors" => get_http_errors(state, &args),
        "get_structured_context" => get_structured_context(state, &args),
        "get_process_logs" => get_process_logs(state, &args),
        _ => return JsonRpcResponse::error(
            id,
            error_codes::METHOD_NOT_FOUND,
            format!("Unknown tool: {tool_name}"),
        ),
    };

    JsonRpcResponse::success(
        id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                }
            ],
            "isError": false
        }),
    )
}

// ── Shared data collectors (used by primary handlers and fallbacks) ──────────

fn collect_terminal_buffer_data(state: &DaemonState, n: usize, terminal: Option<&str>) -> Value {
    let terminals = crate::buffer::list_terminals(&state.buf);
    let lines = get_last_n(&state.buf, n, terminal);
    let content: String = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    let source = terminal.unwrap_or("vscode_bridge");
    let output = crate::typed_context::wrap_untrusted(&content, source);
    json!({ "content": output, "lines_returned": lines.len(), "terminals": terminals })
}

async fn collect_compressed_errors_data(
    state: &DaemonState,
    limit: usize,
    source: Option<&str>,
) -> Value {
    let clusters = get_error_clusters(&state.drain, limit, source);
    let total = total_error_line_count(&state.drain);
    let lines = get_last_n(&state.buf, 500, source);
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

async fn get_snapshot(state: &DaemonState) -> Value {
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

    json!({
        "daemon_uptime_secs": uptime_secs,
        "project_type": project_type,
        "git_branch": branch,
        "git_dirty_files": dirty_files,
        "buffer_lines": buffer_len(&state.buf),
        "has_recent_errors": has_recent_errors(&state.buf)
    })
}

fn get_terminal_buffer(state: &DaemonState, args: &Value) -> Value {
    let n = args["lines"].as_u64().unwrap_or(100).min(500) as usize;
    let terminal = args["terminal"].as_str();
    let data = collect_terminal_buffer_data(state, n, terminal);
    let lines_returned = data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned == 0 {
        return json!({
            "content": "",
            "lines_returned": 0,
            "fallback_source": "none",
            "fallback_reason": "terminal buffer is empty — no data has been captured yet"
        });
    }

    data
}

async fn get_project_metadata(state: &DaemonState) -> Value {
    let cwd = state.cwd.clone();
    let (manifests, env_keys) = tokio::task::spawn_blocking(move || {
        let manifests = crate::scanners::manifests::scan_manifests(&cwd);
        let env_keys = crate::scanners::env::scan_env_keys(&cwd);
        (manifests, env_keys)
    })
    .await
    .unwrap_or_default();

    json!({ "manifests": manifests, "env_keys": env_keys })
}

fn read_file(state: &DaemonState, args: &Value) -> Value {
    let path_str = match args["path"].as_str() {
        Some(p) => p,
        None => return json!({ "error": "Missing path" }),
    };

    let requested = state.cwd.join(path_str);
    let canonical_cwd = match std::fs::canonicalize(&state.cwd) {
        Ok(p) => p,
        Err(_) => return json!({ "error": "Cannot resolve cwd" }),
    };
    let canonical_req = match std::fs::canonicalize(&requested) {
        Ok(p) => p,
        Err(_) => return json!({ "error": format!("File not found: {path_str}") }),
    };
    if !canonical_req.starts_with(&canonical_cwd) {
        return json!({ "error": "Path traversal not allowed" });
    }

    let content = match std::fs::read_to_string(&canonical_req) {
        Ok(c) => c,
        Err(e) => return json!({ "error": format!("Read error: {e}") }),
    };

    let lines: Vec<&str> = content.lines().collect();
    let from = args["from_line"].as_u64().map(|n| (n as usize).saturating_sub(1)).unwrap_or(0);
    let to = args["to_line"].as_u64().map(|n| (n as usize).min(lines.len())).unwrap_or(lines.len());
    let slice = lines[from.min(lines.len())..to.min(lines.len())].join("\n");

    json!({
        "path": path_str,
        "content": slice,
        "from_line": from + 1,
        "to_line": to
    })
}

async fn get_compressed_errors(
    state: &DaemonState,
    args: &Value,
) -> Value {
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
    let source = args["source"].as_str();
    let data = collect_compressed_errors_data(state, limit, source).await;

    let has_clusters = data["clusters"].as_array().is_some_and(|a| !a.is_empty());
    let has_traces = data["stack_traces"].as_array().is_some_and(|a| !a.is_empty());

    if !has_clusters && !has_traces {
        // Fallback: return raw terminal buffer so AI has something to work with.
        let buf_data = collect_terminal_buffer_data(state, 100, None);
        let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

        if lines_returned == 0 {
            return json!({
                "clusters": [],
                "stack_traces": [],
                "total_error_lines": 0,
                "fallback_source": "none",
                "fallback_reason": "no error clusters or stack traces found, and terminal buffer is empty — no data captured yet"
            });
        }

        return json!({
            "clusters": [],
            "stack_traces": [],
            "total_error_lines": data["total_error_lines"],
            "fallback_source": "terminal_buffer",
            "fallback_reason": "no error clusters or stack traces found in drain — showing raw terminal buffer instead; consider injecting logs via the terminal bridge",
            "terminal_buffer": buf_data
        });
    }

    data
}

async fn get_contextual_diff(state: &DaemonState, args: &Value) -> Value {
    let cwd = state.cwd.clone();
    let terminal = args["terminal"].as_str();
    let lines = get_last_n(&state.buf, 500, terminal);

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
        Err(_) => return json!({ "error": "Diff failed" }),
    };

    if !hunks.is_empty() {
        return json!({
            "diff_hunks": hunks,
            "files_cross_referenced": relevant,
            "truncated": truncated,
            "fallback_source": "none"
        });
    }

    // Diff was empty — fall back to compressed errors for useful context.
    let err_data = collect_compressed_errors_data(state, 50, terminal).await;
    let has_clusters = err_data["clusters"].as_array().is_some_and(|a| !a.is_empty());
    let has_traces = err_data["stack_traces"].as_array().is_some_and(|a| !a.is_empty());

    if has_clusters || has_traces {
        return json!({
            "diff_hunks": [],
            "files_cross_referenced": relevant,
            "truncated": false,
            "fallback_source": "compressed_errors",
            "fallback_reason": "no stack trace files matched dirty git files — showing error clusters and stack traces instead",
            "clusters": err_data["clusters"],
            "stack_traces": err_data["stack_traces"],
            "total_error_lines": err_data["total_error_lines"]
        });
    }

    // Last resort: raw terminal buffer.
    let buf_data = collect_terminal_buffer_data(state, 100, None);
    let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned > 0 {
        return json!({
            "diff_hunks": [],
            "files_cross_referenced": relevant,
            "truncated": false,
            "fallback_source": "terminal_buffer",
            "fallback_reason": "no diff context and no error clusters found — showing raw terminal buffer; check if logs are flowing via the VS Code terminal bridge",
            "terminal_buffer": buf_data
        });
    }

    json!({
        "diff_hunks": [],
        "files_cross_referenced": [],
        "truncated": false,
        "fallback_source": "none",
        "fallback_reason": "no diff context, no error clusters, and terminal buffer is empty — no data available yet"
    })
}

async fn get_container_logs(state: &DaemonState, args: &Value) -> Value {
    let container_id = args["container_id"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;

    let docker_available = state.docker_reachable.load(std::sync::atomic::Ordering::Relaxed);
    let (containers, events) = {
        let store = state.error_store.read().unwrap();
        let containers = store.container_ids();
        let events = store.get_events(container_id, limit);
        (containers, events)
    };

    if docker_available && !events.is_empty() {
        return json!({
            "containers": containers,
            "events": events,
            "docker_available": true,
            "fallback_source": "none"
        });
    }

    // Docker running but no errors stored yet — still report correctly.
    if docker_available {
        return json!({
            "containers": containers,
            "events": [],
            "docker_available": true,
            "fallback_source": "none",
            "fallback_reason": "Docker is connected and containers are tracked, but no ERROR/WARN/FATAL events have been captured yet"
        });
    }

    // Docker not reachable — fall back to compressed errors then terminal buffer.
    let err_data = collect_compressed_errors_data(state, 50, container_id).await;
    let has_clusters = err_data["clusters"].as_array().is_some_and(|a| !a.is_empty());
    let has_traces = err_data["stack_traces"].as_array().is_some_and(|a| !a.is_empty());

    if has_clusters || has_traces {
        return json!({
            "containers": [],
            "events": [],
            "docker_available": false,
            "fallback_source": "compressed_errors",
            "fallback_reason": "Docker is not reachable — showing terminal error clusters instead; start Docker Desktop if container monitoring is needed",
            "clusters": err_data["clusters"],
            "stack_traces": err_data["stack_traces"],
            "total_error_lines": err_data["total_error_lines"]
        });
    }

    let buf_data = collect_terminal_buffer_data(state, 100, None);
    let lines_returned = buf_data["lines_returned"].as_u64().unwrap_or(0);

    if lines_returned > 0 {
        return json!({
            "containers": [],
            "events": [],
            "docker_available": false,
            "fallback_source": "terminal_buffer",
            "fallback_reason": "Docker is not reachable and no error clusters found — showing raw terminal buffer",
            "terminal_buffer": buf_data
        });
    }

    json!({
        "containers": [],
        "events": [],
        "docker_available": false,
        "fallback_source": "none",
        "fallback_reason": "Docker is not reachable and terminal buffer is empty — no data available"
    })
}

// ── Phase 3 tools ─────────────────────────────────────────────────────────────

pub async fn get_postmortem(state: &DaemonState, args: &Value) -> Value {
    let minutes = args["minutes"].as_u64().unwrap_or(30).clamp(1, 1440);
    let now = crate::buffer::now_ms();
    let cutoff_ms = now.saturating_sub(minutes * 60 * 1_000);

    let lines = {
        let guard = state.buf.ring.read().unwrap();
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

    json!({
        "window_minutes": minutes,
        "total_lines": lines.len(),
        "timeline": timeline,
        "docker_events_in_window": docker_in_window,
        "stack_traces": traces,
        "fallback_source": "none"
    })
}

pub async fn get_correlated_errors(state: &DaemonState, args: &Value) -> Value {
    let window_secs = args["window_secs"].as_u64().unwrap_or(5);
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    // Collect recent terminal error lines
    let terminal_errors: Vec<_> = {
        let guard = state.buf.ring.read().unwrap();
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
                    let diff = de.timestamp_ms.abs_diff(tl.timestamp_ms);
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

    let has_docker_correlations = correlations.iter().any(|c| {
        c["correlated_docker_events"]
            .as_array()
            .is_some_and(|a: &Vec<Value>| !a.is_empty())
    });

    // Also correlate with HTTP errors (third source)
    let all_http = crate::http_store::get_http_events(&state.http_store, 500);
    let correlations: Vec<Value> = correlations
        .into_iter()
        .map(|mut c| {
            let ts = c["timestamp_ms"].as_u64().unwrap_or(0);
            let nearby_http: Vec<Value> = all_http
                .iter()
                .filter(|he| {
                    let diff = he.timestamp_ms.abs_diff(ts);
                    diff <= window_secs * 1_000
                })
                .map(|he| json!({"method": he.method, "url": he.url, "status": he.status, "latency_ms": he.latency_ms}))
                .collect();
            if !nearby_http.is_empty() {
                c["correlated_http_errors"] = json!(nearby_http);
            }
            c
        })
        .collect();

    let has_http_correlations = correlations.iter().any(|c| {
        c["correlated_http_errors"]
            .as_array()
            .is_some_and(|a: &Vec<Value>| !a.is_empty())
    });
    let has_correlations = has_docker_correlations || has_http_correlations;

    json!({
        "correlations": correlations,
        "has_cross_source_correlations": has_correlations,
        "window_secs": window_secs,
        "fallback_source": if has_correlations { "none" } else { "terminal_only" }
    })
}

fn watch_log_file(state: &DaemonState, args: &Value) -> Value {
    let path_str = match args["path"].as_str() {
        Some(p) => p,
        None => return json!({ "error": "missing 'path' argument" }),
    };

    let abs_path = if std::path::Path::new(path_str).is_absolute() {
        std::path::PathBuf::from(path_str)
    } else {
        state.cwd.join(path_str)
    };

    if !abs_path.exists() {
        return json!({ "error": format!("file not found: {path_str}") });
    }

    let mut list = state.watch_list.write().unwrap();
    if list.contains(&abs_path) {
        return json!({ "status": "already_watching", "path": path_str });
    }
    list.push(abs_path);
    json!({ "status": "watching", "path": path_str })
}

fn get_watched_files(state: &DaemonState) -> Value {
    let list = state.watch_list.read().unwrap();
    let paths: Vec<String> = list
        .iter()
        .filter_map(|p| p.strip_prefix(&state.cwd).ok().map(|r| r.to_string_lossy().replace('\\', "/")))
        .collect();
    json!({ "watched_files": paths, "count": paths.len() })
}

fn get_http_errors(state: &DaemonState, args: &Value) -> Value {
    let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as usize;
    let events = crate::http_store::get_http_events(&state.http_store, limit);

    if events.is_empty() {
        return json!({
            "events": [],
            "total": 0,
            "proxy_port": 8769,
            "usage": "Set HTTP_PROXY=http://127.0.0.1:8769 (or X-Proxy-Target header) to route HTTP requests through BlackBox. Only 4xx/5xx responses are logged.",
            "fallback_source": "none"
        });
    }

    json!({
        "events": events,
        "total": events.len(),
        "proxy_port": 8769
    })
}

pub async fn get_recent_commits(state: &DaemonState, args: &Value) -> Value {
    let limit = args["limit"].as_u64().unwrap_or(20).clamp(1, 100) as usize;
    let path_filter = args["path_filter"].as_str();
    let cwd = state.cwd.clone();
    let path_owned = path_filter.map(|s| s.to_string());

    let commits = tokio::task::spawn_blocking(move || {
        crate::scanners::git::get_recent_commits(&cwd, limit, path_owned.as_deref())
    })
    .await
    .unwrap_or_default();

    if commits.is_empty() {
        return json!({
            "commits": [],
            "total": 0,
            "fallback_source": "none",
            "fallback_reason": "No commits found — either not a git repo or path_filter matched nothing"
        });
    }

    json!({
        "commits": commits,
        "total": commits.len()
    })
}

pub fn get_process_logs(state: &DaemonState, args: &Value) -> Value {
    let pid = args["pid"].as_u64();
    let limit = args["limit"].as_u64().unwrap_or(200).clamp(1, 500) as usize;

    let terminal_filter = pid.map(|p| format!("process:{p}"));
    let filter_ref = terminal_filter.as_deref();

    // List known process terminals in the buffer
    let all_terminals = crate::buffer::list_terminals(&state.buf);
    let process_terminals: Vec<&str> = all_terminals
        .iter()
        .filter(|t| t.starts_with("process:"))
        .map(|t| t.as_str())
        .collect();

    if process_terminals.is_empty() && terminal_filter.is_none() {
        return json!({
            "lines": [],
            "total": 0,
            "known_processes": [],
            "hint": "No process logs captured. Run: blackbox-run <command> [args] to capture a process."
        });
    }

    let lines = crate::buffer::get_last_n(&state.buf, limit, filter_ref);
    let content: String = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    let source = filter_ref.unwrap_or("all_processes");
    let output = crate::typed_context::wrap_untrusted(&content, source);

    json!({
        "content": output,
        "lines_returned": lines.len(),
        "pid_filter": pid,
        "known_processes": process_terminals
    })
}

pub fn get_structured_context(state: &DaemonState, args: &Value) -> Value {
    let span_id = args["span_id"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as usize;

    let events = if let Some(sid) = span_id {
        crate::structured_store::get_by_span_id(&state.structured, sid)
    } else {
        crate::structured_store::get_recent(&state.structured, limit, None)
    };

    let total_parsed = crate::structured_store::store_len(&state.structured);

    if events.is_empty() {
        if total_parsed == 0 {
            return json!({
                "events": [],
                "total_parsed": 0,
                "fallback_source": "none",
                "hint": "No structured JSON logs detected. Emit logs in JSON format (tracing, pino, logrus, structlog) to enable span correlation."
            });
        }
        return json!({
            "events": [],
            "total_parsed": total_parsed,
            "span_id": span_id,
            "fallback_source": "none",
            "hint": if span_id.is_some() {
                "No events found for this span_id. Check spelling or use get_structured_context without span_id to list recent events."
            } else {
                "No structured events found."
            }
        });
    }

    json!({
        "events": events,
        "count": events.len(),
        "total_parsed": total_parsed,
        "span_id_filter": span_id
    })
}
