use std::path::PathBuf;
use std::time::Instant;

use blackbox_core::types::StatusResponse;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use crate::buffer::{buffer_len, has_recent_errors, SharedBuffer};
use crate::scanners::{git::scan_git, manifests::scan_manifests};

pub async fn run_status_server(buf: SharedBuffer, cwd: PathBuf, start_time: Instant, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Status server failed to bind {addr}: {e}. Internal API will be unavailable.");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let status = build_status(&buf, &cwd, start_time);
                let mut json = serde_json::to_string(&status).unwrap_or_default();
                json.push('\n');
                tokio::spawn(async move {
                    let _ = stream.write_all(json.as_bytes()).await;
                });
            }
            Err(_) => break,
        }
    }
}

fn build_status(buf: &SharedBuffer, cwd: &PathBuf, start_time: Instant) -> StatusResponse {
    let (branch_str, dirty) = scan_git(cwd);
    // Convert the "unknown" sentinel into None, and detached HEAD into None
    let git_branch = match branch_str.as_str() {
        "unknown" | "HEAD (detached)" => None,
        other => Some(other.to_string()),
    };

    let manifests = scan_manifests(cwd);
    let project_type = manifests
        .first()
        .map(|m| m.manifest_type.clone())
        .unwrap_or(blackbox_core::types::ProjectKind::Unknown);

    StatusResponse {
        uptime_secs: start_time.elapsed().as_secs(),
        buffer_lines: buffer_len(buf),
        git_branch,
        git_dirty_files: dirty,
        project_type,
        has_recent_errors: has_recent_errors(buf),
    }
}
