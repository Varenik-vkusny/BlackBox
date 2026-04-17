use serde::{Deserialize, Serialize};

/// A single line captured from a terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    pub timestamp_ms: u64,
}

/// Status snapshot returned by the daemon's status server (port 8766).
/// Consumed by blackbox-tui for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub uptime_secs: u64,
    pub buffer_lines: usize,
    pub git_branch: String,
    pub git_dirty_files: usize,
    pub project_type: String,
    pub has_recent_errors: bool,
}

/// Metadata about a detected project manifest (Cargo.toml, package.json, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestInfo {
    pub manifest_type: String, // "cargo", "npm", "go"
    pub name: String,
    pub version: String,
}
