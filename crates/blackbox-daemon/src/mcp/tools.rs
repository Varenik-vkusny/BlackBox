use std::path::PathBuf;
use std::time::Instant;

use blackbox_core::protocol::{error_codes, JsonRpcResponse};
use serde_json::{json, Value};

use crate::buffer::{buffer_len, get_last_n, has_recent_errors, SharedBuffer};
use crate::scanners::{env::scan_env_keys, git::scan_git, manifests::scan_manifests};

pub async fn handle_tools_call(
    id: Option<Value>,
    params: Option<Value>,
    buf: &SharedBuffer,
    cwd: &PathBuf,
    start_time: Instant,
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
        "get_snapshot" => get_snapshot(id, buf, cwd, start_time),
        "get_terminal_buffer" => get_terminal_buffer(id, buf, &args),
        "get_project_metadata" => get_project_metadata(id, cwd),
        "read_file" => read_file(id, cwd, &args),
        _ => JsonRpcResponse::error(id, error_codes::METHOD_NOT_FOUND, format!("Unknown tool: {tool_name}")),
    }
}

fn get_snapshot(id: Option<Value>, buf: &SharedBuffer, cwd: &PathBuf, start_time: Instant) -> JsonRpcResponse {
    let uptime_secs = start_time.elapsed().as_secs();
    let (branch, dirty_files) = scan_git(cwd);
    let manifests = scan_manifests(cwd);
    let project_type = manifests.first()
        .map(|m| format!("{:?}", m.manifest_type).to_lowercase())
        .unwrap_or_else(|| "unknown".into());

    JsonRpcResponse::success(id, json!({
        "daemon_uptime_secs": uptime_secs,
        "project_type": project_type,
        "git_branch": branch,
        "git_dirty_files": dirty_files,
        "buffer_lines": buffer_len(buf),
        "has_recent_errors": has_recent_errors(buf)
    }))
}

fn get_terminal_buffer(id: Option<Value>, buf: &SharedBuffer, args: &Value) -> JsonRpcResponse {
    let n = args["lines"].as_u64().unwrap_or(100).min(500) as usize;
    let lines = get_last_n(buf, n);

    let content: String = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");

    // Strip any closing tags to prevent injection breaking the XML wrapper
    let safe_content = content.replace("</terminal_output>", "&lt;/terminal_output&gt;");

    let output = format!(
        "<terminal_output source=\"vscode_bridge\" untrusted=\"true\">\n{safe_content}\n</terminal_output>"
    );

    JsonRpcResponse::success(id, json!({ "content": output, "lines_returned": lines.len() }))
}

fn get_project_metadata(id: Option<Value>, cwd: &PathBuf) -> JsonRpcResponse {
    let manifests = scan_manifests(cwd);
    let env_keys = scan_env_keys(cwd);

    JsonRpcResponse::success(id, json!({
        "manifests": manifests,
        "env_keys": env_keys
    }))
}

fn read_file(id: Option<Value>, cwd: &PathBuf, args: &Value) -> JsonRpcResponse {
    let path_str = match args["path"].as_str() {
        Some(p) => p,
        None => return JsonRpcResponse::error(id, error_codes::INVALID_PARAMS, "Missing path".into()),
    };

    // Security: resolve and verify path stays within cwd
    let requested = cwd.join(path_str);
    let canonical_cwd = match std::fs::canonicalize(cwd) {
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
