use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Cargo,
    Npm,
    Go,
    Unknown,
}

// ── Phase 2: Log compression ──────────────────────────────────────────────────

/// A deduplicated log cluster produced by the Drain algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCluster {
    /// Template with `*` wildcards replacing variable tokens, e.g. "Connection refused to *"
    pub pattern: String,
    pub count: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    /// Verbatim first occurrence kept for human readability.
    pub example: String,
    /// Detected log level ("error", "warn", "fatal"), None for unclassified.
    pub level: Option<String>,
}

/// A parsed stack frame extracted from terminal output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    pub raw: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub is_user_code: bool,
}

/// A full parsed stack trace from terminal output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStackTrace {
    /// "rust" | "python" | "nodejs" | "java"
    pub language: String,
    pub error_message: String,
    pub frames: Vec<StackFrame>,
    /// Deduplicated list of source file paths mentioned in user-code frames.
    pub source_files: Vec<String>,
    pub captured_at_ms: u64,
}

// ── Phase 2: Docker monitoring ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ErrorSource {
    Terminal,
    Docker { container_id: String },
}

/// An error event from any source (terminal or Docker container).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub source: ErrorSource,
    pub text: String,
    pub timestamp_ms: u64,
    /// "error" | "warn" | "fatal", None when source is plain stderr.
    pub level: Option<String>,
}

// ── Phase 2: Smart git diffs ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HunkLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunkLine {
    pub kind: HunkLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub file: String,
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<HunkLine>,
}

/// A single line captured from a terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub text: String,
    /// Milliseconds since the Unix epoch (UTC).
    pub timestamp_ms: u64,
    /// VS Code terminal name (e.g. "bash", "Python Debug"). None for injected/shell-hook lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_terminal: Option<String>,
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

/// A single git commit with lightweight metadata (no diffs — token-efficient).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub timestamp_iso: String,
    pub changed_files: Vec<String>,
    pub insertions: u32,
    pub deletions: u32,
}