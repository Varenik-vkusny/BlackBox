use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Cargo,
    Npm,
    Go,
    Unknown,
}

/// A single line captured from a terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    /// Milliseconds since the Unix epoch (UTC).
    pub timestamp_ms: u64,
}

/// Status snapshot returned by the daemon's status server (port 8766).
/// Consumed by blackbox-tui for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub uptime_secs: u64,
    pub buffer_lines: usize,
    pub git_branch: Option<String>,  // None when in detached HEAD state
    pub git_dirty_files: usize,
    pub project_type: ProjectKind,
    pub has_recent_errors: bool,
}

/// Metadata about a detected project manifest (Cargo.toml, package.json, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestInfo {
    pub manifest_type: ProjectKind,
    pub name: String,
    pub version: String,
}
