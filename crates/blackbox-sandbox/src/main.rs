use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;

const BRIDGE_PORT: u16 = 28765;
const STATUS_PORT: u16 = 28766;
const REFRESH_MS:  u64 = 1500;

// ── Tabs ───────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tab { Logs, Snapshot, Metadata, File, Inject, Errors, Diff, Docker }

impl Tab {
    const ALL: &'static [Tab] = &[
        Tab::Logs, Tab::Snapshot, Tab::Metadata, Tab::File,
        Tab::Inject, Tab::Errors, Tab::Diff, Tab::Docker,
    ];
    fn label(self) -> &'static str {
        match self {
            Tab::Logs     => "1 Logs",
            Tab::Snapshot => "2 Snap",
            Tab::Metadata => "3 Meta",
            Tab::File     => "4 File",
            Tab::Inject   => "5 Inject",
            Tab::Errors   => "6 Errors",
            Tab::Diff     => "7 Diff",
            Tab::Docker   => "8 Docker",
        }
    }
    fn index(self) -> usize {
        match self {
            Tab::Logs=>0, Tab::Snapshot=>1, Tab::Metadata=>2, Tab::File=>3,
            Tab::Inject=>4, Tab::Errors=>5, Tab::Diff=>6, Tab::Docker=>7,
        }
    }
}

// ── Log entry ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct LogEntry {
    text:     String,
    is_error: bool,
}

// ── App state ──────────────────────────────────────────────────────────────────

struct App {
    tab:            Tab,
    logs:           Vec<LogEntry>,
    snapshot:       String,
    metadata:       String,
    file_path:      String,
    file_content:   String,
    inject_input:   String,
    errors_content: String,   // rendered output of get_compressed_errors
    diff_content:   String,   // rendered output of get_contextual_diff
    docker_content: String,   // rendered output of get_container_logs
    scroll:         usize,
    status_bar:     String,
    last_refresh:   Instant,
    editing:        bool,
}

impl App {
    fn new() -> Self {
        Self {
            tab: Tab::Logs,
            logs: Vec::new(),
            snapshot: String::new(),
            metadata: String::new(),
            file_path: "Cargo.toml".into(),
            file_content: String::new(),
            inject_input: String::new(),
            errors_content: String::new(),
            diff_content: String::new(),
            docker_content: String::new(),
            scroll: 0,
            status_bar: " Ready — [r] refresh  [1-8] tabs  [q] quit".into(),
            last_refresh: Instant::now() - Duration::from_secs(10),
            editing: false,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_bar = format!(" {}", msg.into());
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self, content_lines: usize, viewport: usize) {
        let max = content_lines.saturating_sub(viewport);
        if self.scroll < max { self.scroll += 1; }
    }

    fn reset_scroll(&mut self) { self.scroll = 0; }

    fn scroll_to_bottom(&mut self, content_lines: usize) {
        self.scroll = content_lines.saturating_sub(1);
    }

    fn current_content_lines(&self) -> usize {
        match self.tab {
            Tab::Logs     => self.logs.len(),
            Tab::Snapshot => self.snapshot.lines().count(),
            Tab::Metadata => self.metadata.lines().count(),
            Tab::File     => self.file_content.lines().count(),
            Tab::Errors   => self.errors_content.lines().count(),
            Tab::Diff     => self.diff_content.lines().count(),
            Tab::Docker   => self.docker_content.lines().count(),
            Tab::Inject   => 0,
        }
    }
}

// ── Daemon wrapper ─────────────────────────────────────────────────────────────

struct Daemon {
    _child: Child,
    stdin:  std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    seq:    u64,
}

impl Daemon {
    fn spawn(bin: &str, cwd: &str) -> Result<Self, String> {
        let mut child = Command::new(bin)
            .args(["--cwd", cwd,
                   "--port",        &BRIDGE_PORT.to_string(),
                   "--status-port", &STATUS_PORT.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;

        let stdin  = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        std::thread::sleep(Duration::from_millis(400));

        let mut d = Self { _child: child, stdin, stdout, seq: 0 };
        let _ = d.rpc("initialize", serde_json::json!({"protocolVersion":"2024-11-05"}));
        Ok(d)
    }

    fn rpc(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        self.seq += 1;
        let req = serde_json::json!({
            "jsonrpc": "2.0", "id": self.seq,
            "method": method, "params": params
        });
        let mut msg = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        msg.push('\n');
        self.stdin.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        self.stdout.read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim()).map_err(|e| format!("parse: {e}"))
    }

    fn tool(&mut self, name: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
        self.rpc("tools/call", serde_json::json!({"name": name, "arguments": args}))
    }
}

impl Drop for Daemon {
    fn drop(&mut self) { let _ = self._child.kill(); }
}

// ── TCP bridge helpers ─────────────────────────────────────────────────────────

fn inject_one_line(text: &str) -> Result<(), String> {
    let mut s = TcpStream::connect(format!("127.0.0.1:{BRIDGE_PORT}"))
        .map_err(|e| format!("bridge: {e}"))?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    if !text.ends_with('\n') { s.write_all(b"\n").map_err(|e| e.to_string())?; }
    Ok(())
}

fn inject_text(text: &str) -> Result<usize, String> {
    let lines: Vec<&str> = text.split("\\n").collect();
    for line in &lines { inject_one_line(line)?; }
    Ok(lines.len())
}

// ── File path parser ───────────────────────────────────────────────────────────

struct FileRequest { path: String, from_line: Option<u64>, to_line: Option<u64> }

fn parse_file_input(input: &str) -> FileRequest {
    let parts: Vec<&str> = input.rsplitn(2, ':').collect();
    if parts.len() == 2 {
        let range_part = parts[0];
        let path = parts[1].to_string();
        if let Some(dash) = range_part.find('-') {
            let from = range_part[..dash].parse::<u64>().ok();
            let to   = range_part[dash + 1..].parse::<u64>().ok();
            if from.is_some() || to.is_some() {
                return FileRequest { path, from_line: from, to_line: to };
            }
        }
        if let Ok(n) = range_part.parse::<u64>() {
            let from = n.saturating_sub(20).max(1);
            return FileRequest { path, from_line: Some(from), to_line: Some(n + 20) };
        }
    }
    FileRequest { path: input.to_string(), from_line: None, to_line: None }
}

// ── Refresh functions ──────────────────────────────────────────────────────────

fn refresh_logs(daemon: &mut Daemon, app: &mut App) {
    let at_bottom = app.logs.is_empty() || app.scroll >= app.logs.len().saturating_sub(1);
    match daemon.tool("get_terminal_buffer", serde_json::json!({"lines": 200})) {
        Ok(v) => {
            let raw = v["result"]["content"].as_str().unwrap_or("");
            let inner = raw.trim_start_matches(|c: char| c != '\n').trim_start_matches('\n');
            let inner = if let Some(pos) = inner.rfind("</terminal_output>") { &inner[..pos] }
                        else { inner };
            let returned = v["result"]["lines_returned"].as_u64().unwrap_or(0);
            app.logs = inner.lines().map(|l| {
                let lo = l.to_lowercase();
                let is_error = lo.contains("error") || lo.contains("panic")
                    || lo.contains("failed") || lo.contains("exception") || lo.contains("fatal");
                LogEntry { text: l.to_string(), is_error }
            }).collect();
            if at_bottom { app.scroll_to_bottom(app.logs.len()); }
            app.set_status(format!("Logs refreshed — {returned} lines (source: vscode_bridge | untrusted)"));
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_snapshot(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_snapshot", serde_json::json!({})) {
        Ok(v) => {
            let r = &v["result"];
            app.snapshot = format!(
                "daemon_uptime_secs : {}\nproject_type       : {}\ngit_branch         : {}\ngit_dirty_files    : {}\nbuffer_lines       : {}\nhas_recent_errors  : {}",
                r["daemon_uptime_secs"], r["project_type"],
                r["git_branch"], r["git_dirty_files"],
                r["buffer_lines"], r["has_recent_errors"],
            );
            app.set_status("Snapshot refreshed");
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_metadata(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_project_metadata", serde_json::json!({})) {
        Ok(v) => {
            let mut out = String::new();
            out.push_str("── Manifests (priority order) ──────────────────────\n");
            if let Some(arr) = v["result"]["manifests"].as_array() {
                if arr.is_empty() { out.push_str("  (no manifests found)\n"); }
                for (i, m) in arr.iter().enumerate() {
                    let kind = m["manifest_type"].as_str().unwrap_or("?");
                    let name = m["name"].as_str().unwrap_or("").trim().to_string();
                    let name_display = if name.is_empty() {
                        match kind { "cargo" => "(workspace root)".into(), _ => "(unnamed)".into() }
                    } else { name };
                    out.push_str(&format!(
                        "  [{i}] {kind:<8}  {name_display:<36}  v{}\n",
                        m["version"].as_str().unwrap_or("?")
                    ));
                }
            }
            out.push_str("\n── .env Keys (values masked) ────────────────────────\n");
            if let Some(arr) = v["result"]["env_keys"].as_array() {
                if arr.is_empty() { out.push_str("  (no .env files found)\n"); }
                else { for k in arr { out.push_str(&format!("  {}=<MASKED>\n", k.as_str().unwrap_or("?"))); } }
            }
            app.metadata = out;
            app.set_status("Metadata refreshed");
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_file(daemon: &mut Daemon, app: &mut App) {
    let req = parse_file_input(&app.file_path);
    let mut args = serde_json::json!({"path": req.path});
    if let Some(f) = req.from_line { args["from_line"] = f.into(); }
    if let Some(t) = req.to_line   { args["to_line"]   = t.into(); }
    match daemon.tool("read_file", args) {
        Ok(v) => {
            if v["error"].is_object() {
                let msg = v["error"]["message"].as_str().unwrap_or("error");
                app.file_content = format!("[ERROR] {msg}");
                app.set_status(format!("read_file error: {msg}"));
            } else {
                let content = v["result"]["content"].as_str().unwrap_or("").to_string();
                let from = v["result"]["from_line"].as_u64().unwrap_or(1);
                let to   = v["result"]["to_line"].as_u64().unwrap_or(0);
                app.set_status(format!("{}  lines {from}–{to}  (tip: path:N or path:N-M)", req.path));
                app.file_content = content;
                app.reset_scroll();
            }
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_errors(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_compressed_errors", serde_json::json!({"limit": 50})) {
        Ok(v) => {
            let res = &v["result"];
            let mut out = String::new();

            // ── Drain clusters ──────────────────────────────────────────────────
            let total = res["total_error_lines"].as_u64().unwrap_or(0);
            out.push_str(&format!("── Error Clusters (Drain) — {total} total error lines ──────────\n"));
            if let Some(clusters) = res["clusters"].as_array() {
                if clusters.is_empty() {
                    out.push_str("  (no error clusters yet — inject some error lines via tab 5)\n");
                }
                for (i, c) in clusters.iter().enumerate() {
                    let pattern = c["pattern"].as_str().unwrap_or("?");
                    let count   = c["count"].as_u64().unwrap_or(0);
                    let level   = c["level"].as_str().unwrap_or("?");
                    let example = c["example"].as_str().unwrap_or("");
                    out.push_str(&format!("\n  [{i}] count={count:<5}  level={level}\n"));
                    out.push_str(&format!("       pattern : {pattern}\n"));
                    if pattern != example {
                        let ex = if example.len() > 80 { &example[..80] } else { example };
                        out.push_str(&format!("       example : {ex}\n"));
                    }
                }
            }

            // ── Stack traces ────────────────────────────────────────────────────
            out.push_str("\n\n── Parsed Stack Traces ─────────────────────────────────────────\n");
            if let Some(traces) = res["stack_traces"].as_array() {
                if traces.is_empty() {
                    out.push_str("  (no stack traces detected)\n");
                    out.push_str("  Inject a Rust panic:\n");
                    out.push_str("  thread 'main' panicked at 'err', src/main.rs:1\\n");
                    out.push_str("     0: myapp::run\\n      at src/main.rs:1\\n");
                    out.push_str("     1: std::rt::lang_start\n");
                }
                for (i, t) in traces.iter().enumerate() {
                    let lang    = t["language"].as_str().unwrap_or("?");
                    let msg     = t["error_message"].as_str().unwrap_or("?");
                    let n_user  = t["frames"].as_array()
                        .map(|f| f.iter().filter(|fr| fr["is_user_code"] == true).count())
                        .unwrap_or(0);
                    let n_total = t["frames"].as_array().map(|f| f.len()).unwrap_or(0);
                    let files   = t["source_files"].as_array()
                        .map(|f| f.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();

                    out.push_str(&format!("\n  [{i}] {lang}  —  {n_user}/{n_total} user frames\n"));
                    let msg_short = if msg.len() > 90 { &msg[..90] } else { msg };
                    out.push_str(&format!("       error : {msg_short}\n"));
                    if !files.is_empty() {
                        out.push_str(&format!("       files : {files}\n"));
                    }
                    if let Some(frames) = t["frames"].as_array() {
                        for fr in frames.iter().take(5) {
                            let is_user = fr["is_user_code"] == true;
                            let raw     = fr["raw"].as_str().unwrap_or("");
                            let prefix  = if is_user { "  ●" } else { "  ○" };
                            let raw_short = if raw.len() > 70 { &raw[..70] } else { raw };
                            out.push_str(&format!("       {prefix} {raw_short}\n"));
                        }
                        if frames.len() > 5 {
                            out.push_str(&format!("       ... {} more frames\n", frames.len() - 5));
                        }
                    }
                }
            }

            app.errors_content = out;
            app.reset_scroll();
            app.set_status("Errors refreshed — ● user code  ○ stdlib (filtered)");
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_diff(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_contextual_diff", serde_json::json!({})) {
        Ok(v) => {
            let res = &v["result"];
            let mut out = String::new();

            let cross = res["files_cross_referenced"].as_array()
                .map(|f| f.iter().filter_map(|s| s.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            let truncated = res["truncated"].as_bool().unwrap_or(false);

            out.push_str("── Contextual Diff — cross-referenced with stack trace errors ──\n");
            out.push_str("\nHow it works:\n");
            out.push_str("  1. Extracts source files from recent stack traces\n");
            out.push_str("  2. Intersects with dirty git files\n");
            out.push_str("  3. Returns hunks only for that intersection\n\n");

            if cross.is_empty() {
                out.push_str("── Files Cross-Referenced ──────────────────────────────────────\n");
                out.push_str("  (empty — no overlap between stack trace files and git changes)\n\n");
                out.push_str("  To test:\n");
                out.push_str("  1. Modify any source file (don't commit)\n");
                out.push_str("  2. Inject a stack trace mentioning that file in tab 5\n");
                out.push_str("     Example: thread 'main' panicked at 'err', src/main.rs:1\\n");
                out.push_str("              0: myapp::main\\n   at src/main.rs:1\\n");
                out.push_str("              1: std::rt::lang_start\n");
                out.push_str("  3. Press [r] here\n");
            } else {
                out.push_str(&format!("── Files Cross-Referenced ({}) ──────────────────────────────\n", cross.len()));
                for f in &cross { out.push_str(&format!("  ✓ {f}\n")); }
                if truncated { out.push_str("\n  ⚠ output truncated (50 hunk / 30 line caps)\n"); }
            }

            if let Some(hunks) = res["diff_hunks"].as_array() {
                if hunks.is_empty() && !cross.is_empty() {
                    out.push_str("\n  (no diff hunks — files may be unmodified)\n");
                }
                let mut current_file = String::new();
                for hunk in hunks {
                    let file     = hunk["file"].as_str().unwrap_or("?");
                    let old_start = hunk["old_start"].as_u64().unwrap_or(0);
                    let new_start = hunk["new_start"].as_u64().unwrap_or(0);
                    if file != current_file {
                        out.push_str(&format!("\n── {file} ──\n"));
                        current_file = file.to_string();
                    }
                    out.push_str(&format!("@@ -{old_start} +{new_start} @@\n"));
                    if let Some(lines) = hunk["lines"].as_array() {
                        for l in lines {
                            let kind = l["kind"].as_str().unwrap_or("context");
                            let text = l["text"].as_str().unwrap_or("");
                            let prefix = match kind { "added" => "+", "removed" => "-", _ => " " };
                            out.push_str(&format!("{prefix}{text}\n"));
                        }
                    }
                }
            }

            app.diff_content = out;
            app.reset_scroll();
            let hunk_count = res["diff_hunks"].as_array().map(|h| h.len()).unwrap_or(0);
            app.set_status(format!(
                "Diff refreshed — {} files cross-referenced, {} hunks{}",
                cross.len(), hunk_count,
                if truncated { " (truncated)" } else { "" }
            ));
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_docker(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_container_logs", serde_json::json!({"limit": 100})) {
        Ok(v) => {
            let res = &v["result"];
            let mut out = String::new();
            let docker_available = res["docker_available"].as_bool().unwrap_or(false);

            out.push_str("── Docker Container Error Monitor ───────────────────────────────\n");
            out.push_str(&format!("\n  docker_available : {docker_available}\n\n"));

            if !docker_available {
                out.push_str("  Docker is not running or has no active containers.\n");
                out.push_str("  Start Docker Desktop and run a container, then press [r].\n\n");
                out.push_str("  Quick test:\n");
                out.push_str("    docker run -d --name test-bb nginx\n");
                out.push_str("    # then come back and press [r]\n\n");
                out.push_str("  Filter rules:\n");
                out.push_str("    stderr         → always kept (unconditional)\n");
                out.push_str("    stdout JSON    → kept if level = ERROR|WARN|FATAL\n");
                out.push_str("    stdout JSON    → dropped if level = INFO|DEBUG|TRACE\n");
                out.push_str("    stdout plain   → kept if contains error/warn/fatal keyword\n");
                out.push_str("    stdout plain   → dropped otherwise\n");
            } else {
                if let Some(containers) = res["containers"].as_array() {
                    out.push_str(&format!("── Monitored Containers ({}) ─────────────────────────────────\n", containers.len()));
                    for c in containers {
                        out.push_str(&format!("  • {}\n", c.as_str().unwrap_or("?")));
                    }
                }

                if let Some(events) = res["events"].as_array() {
                    out.push_str(&format!("\n── Error Events ({}) ────────────────────────────────────────\n", events.len()));
                    if events.is_empty() {
                        out.push_str("  (no error events yet — containers may be healthy)\n");
                    }
                    for (i, e) in events.iter().enumerate() {
                        let source = match e["source"]["type"].as_str().unwrap_or("?") {
                            "docker" => e["source"]["container_id"].as_str().unwrap_or("?"),
                            other    => other,
                        };
                        let level = e["level"].as_str().unwrap_or("?");
                        let text  = e["text"].as_str().unwrap_or("");
                        let ts    = e["timestamp_ms"].as_u64().unwrap_or(0);
                        let text_short = if text.len() > 90 { &text[..90] } else { text };
                        out.push_str(&format!("\n  [{i}] [{level}] {source}  ts={ts}\n"));
                        out.push_str(&format!("       {text_short}\n"));
                    }
                }
            }

            app.docker_content = out;
            app.reset_scroll();
            let event_count = res["events"].as_array().map(|e| e.len()).unwrap_or(0);
            app.set_status(format!(
                "Docker refreshed — available={docker_available}  {event_count} events stored"
            ));
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

// ── Drawing ────────────────────────────────────────────────────────────────────

fn draw(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &App) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
            .split(area);

        // ── Tab bar (8 equal columns) ─────────────────────────────────────────
        let tab_constraints = vec![Constraint::Ratio(1, 8); 8];
        let tab_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(tab_constraints)
            .split(chunks[0]);

        for tab in Tab::ALL {
            let active = *tab == app.tab;
            let style = if active {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let block = Paragraph::new(format!(" {} ", tab.label()))
                .style(style)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(block, tab_chunks[tab.index()]);
        }

        // ── Content ───────────────────────────────────────────────────────────
        let content_area = chunks[1];
        match app.tab {
            Tab::Logs     => draw_logs(f, app, content_area),
            Tab::Snapshot => draw_text(f, app, content_area, "Snapshot", &app.snapshot.clone()),
            Tab::Metadata => draw_text(f, app, content_area, "Project Metadata", &app.metadata.clone()),
            Tab::File     => draw_file(f, app, content_area),
            Tab::Inject   => draw_inject(f, app, content_area),
            Tab::Errors   => draw_errors(f, app, content_area),
            Tab::Diff     => draw_diff(f, app, content_area),
            Tab::Docker   => draw_docker(f, app, content_area),
        }

        // ── Status bar ────────────────────────────────────────────────────────
        let help = match app.tab {
            Tab::File   => " [e] edit path (path:N or path:N-M)  [Enter] load  [↑↓] scroll  [q] quit",
            Tab::Inject => " [e] edit  [Enter] send  [r] clear input  — \\n for multi-line  [q] quit",
            _           => " [1-8] tabs  [↑↓/jk] scroll  [r] refresh  [G] bottom  [g] top  [q] quit",
        };
        let status = Paragraph::new(vec![
            Line::from(Span::styled(&app.status_bar, Style::default().fg(Color::Green))),
            Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
        ])
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    })?;
    Ok(())
}

fn draw_logs(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let title = format!(" Terminal Logs — {} lines ", app.logs.len());
    let items: Vec<ListItem> = app.logs.iter().map(|e| {
        let style = if e.is_error { Style::default().fg(Color::Red) }
                    else { Style::default().fg(Color::White) };
        ListItem::new(Line::from(Span::styled(&e.text, style)))
    }).collect();

    let visible = area.height.saturating_sub(2) as usize;
    let start = if app.logs.is_empty() { 0 } else { app.scroll.min(app.logs.len() - 1) };
    let slice: Vec<ListItem> = items.into_iter().skip(start).take(visible).collect();
    let list = List::new(slice)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}

fn draw_text(f: &mut ratatui::Frame, app: &App, area: Rect, title: &str, content: &str) {
    let visible = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = content.lines().skip(app.scroll).take(visible)
        .map(|l| Line::from(Span::raw(l))).collect();
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(format!(" {title} ")))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_file(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let path_style = if app.editing {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let cursor = if app.editing { Span::styled("█", Style::default().fg(Color::Yellow)) }
                 else { Span::raw("") };
    let path_bar = Paragraph::new(Line::from(vec![
        Span::styled(" path: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.file_path, path_style),
        cursor,
    ]))
    .block(Block::default().borders(Borders::ALL)
        .title(if app.editing { " Edit path — Enter to load, Esc to cancel " }
               else { " [e] edit  —  path  or  path:N  or  path:N-M " }));
    f.render_widget(path_bar, chunks[0]);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.file_content.lines()
        .enumerate().skip(app.scroll).take(visible)
        .map(|(i, l)| {
            let num = Span::styled(
                format!("{:>4} │ ", app.scroll + i + 1),
                Style::default().fg(Color::DarkGray),
            );
            Line::from(vec![num, Span::raw(l)])
        }).collect();
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" File Content "))
        .wrap(Wrap { trim: false });
    f.render_widget(para, chunks[1]);
}

fn draw_inject(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    let input_style = if app.editing {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let cursor = if app.editing { Span::styled("█", Style::default().fg(Color::Yellow)) }
                 else { Span::raw("") };
    let input = Paragraph::new(Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.inject_input, input_style),
        cursor,
    ]))
    .block(Block::default().borders(Borders::ALL)
        .title(if app.editing { " Type log text — \\n for newlines, Enter to inject " }
               else { " [e] type log text  [Enter] send to bridge " }));
    f.render_widget(input, chunks[0]);

    let hints = vec![
        Line::from(Span::styled("  Inject text into the terminal buffer via TCP bridge.", Style::default().fg(Color::Gray))),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  Single error:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("error: connection refused to 127.0.0.1:5432", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("  Rust panic:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("thread 'main' panicked at 'err', src/main.rs:1\\n   0: myapp::run\\n      at src/main.rs:1\\n   1: std::rt::lang_start",
                Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  Python:        ", Style::default().fg(Color::DarkGray)),
            Span::styled("Traceback (most recent call last):\\n  File \"src/app.py\", line 10, in run\\n    process()\\nValueError: bad input",
                Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  50× same err:  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Use [r] to clear, inject the same line 50 times → test Drain clustering in tab 6",
                Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled("  After injecting → [6] Errors to see clusters, [7] Diff for git hunks.", Style::default().fg(Color::DarkGray))),
    ];
    let hint = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL).title(" Inject Guide "));
    f.render_widget(hint, chunks[1]);
}

fn draw_errors(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let all_lines: Vec<&str> = app.errors_content.lines().collect();
    let lines: Vec<Line> = all_lines.iter().skip(app.scroll).take(visible)
        .map(|l| {
            // Colour coding: clusters cyan, user frames red, stdlib frames gray
            let style = if l.contains("count=") {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if l.contains("  ● ") {
                Style::default().fg(Color::Red)
            } else if l.contains("  ○ ") {
                Style::default().fg(Color::DarkGray)
            } else if l.starts_with("──") || l.starts_with('\n') {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(*l, style))
        }).collect();
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL)
            .title(" Compressed Errors — Drain clusters + stack traces (get_compressed_errors) "))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_diff(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.diff_content.lines().skip(app.scroll).take(visible)
        .map(|l| {
            let style = if l.starts_with('+') {
                Style::default().fg(Color::Green)
            } else if l.starts_with('-') {
                Style::default().fg(Color::Red)
            } else if l.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else if l.starts_with("──") || l.contains("cross-referenced") {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if l.contains('✓') {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(l, style))
        }).collect();
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL)
            .title(" Contextual Diff — git hunks × stack trace files (get_contextual_diff) "))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_docker(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.docker_content.lines().skip(app.scroll).take(visible)
        .map(|l| {
            let style = if l.contains("[error]") || l.contains("[fatal]") {
                Style::default().fg(Color::Red)
            } else if l.contains("[warn]") {
                Style::default().fg(Color::Yellow)
            } else if l.starts_with("──") {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if l.contains("docker_available : false") {
                Style::default().fg(Color::DarkGray)
            } else if l.contains("docker_available : true") {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(l, style))
        }).collect();
    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL)
            .title(" Docker Container Logs — ERROR/WARN/FATAL only (get_container_logs) "))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

// ── Main event loop ────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let daemon_bin = if cfg!(windows) { "target/debug/blackbox-daemon.exe" }
                    else { "target/debug/blackbox-daemon" };

    let cwd = std::env::current_dir()?;
    let daemon_path = cwd.join(daemon_bin);

    if !daemon_path.exists() {
        eprintln!("ERROR: daemon not found at {}", daemon_path.display());
        eprintln!("Run:  cargo build -p blackbox-daemon");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut daemon = match Daemon::spawn(daemon_path.to_str().unwrap(), &cwd.to_string_lossy()) {
        Ok(d) => { app.set_status("Daemon started — [r] refresh  [1-8] tabs  [q] quit"); d }
        Err(e) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            eprintln!("Failed to start daemon: {e}");
            std::process::exit(1);
        }
    };

    loop {
        draw(&mut terminal, &app)?;

        // Auto-refresh logs tab
        if app.tab == Tab::Logs
            && !app.editing
            && app.last_refresh.elapsed() > Duration::from_millis(REFRESH_MS)
        {
            refresh_logs(&mut daemon, &mut app);
            app.last_refresh = Instant::now();
        }

        if !event::poll(Duration::from_millis(200))? { continue; }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press { continue; }

            // ── Editing mode ──────────────────────────────────────────────────
            if app.editing {
                match key.code {
                    KeyCode::Esc => { app.editing = false; app.set_status("Cancelled"); }
                    KeyCode::Enter => {
                        app.editing = false;
                        match app.tab {
                            Tab::File => refresh_file(&mut daemon, &mut app),
                            Tab::Inject => {
                                let text = app.inject_input.clone();
                                if !text.is_empty() {
                                    match inject_text(&text) {
                                        Ok(n) => {
                                            app.set_status(format!(
                                                "Injected {n} line(s) — [6] Errors or [7] Diff to analyse"
                                            ));
                                            app.inject_input.clear();
                                        }
                                        Err(e) => app.set_status(format!("Inject error: {e}")),
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Backspace => {
                        let s = if app.tab == Tab::File { &mut app.file_path }
                                else { &mut app.inject_input };
                        s.pop();
                    }
                    KeyCode::Char(c) => {
                        let s = if app.tab == Tab::File { &mut app.file_path }
                                else { &mut app.inject_input };
                        s.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // ── Normal mode ───────────────────────────────────────────────────
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,

                // Tab switching
                KeyCode::Char('1') => { app.tab = Tab::Logs;     app.reset_scroll(); }
                KeyCode::Char('2') => { app.tab = Tab::Snapshot; app.reset_scroll(); refresh_snapshot(&mut daemon, &mut app); }
                KeyCode::Char('3') => { app.tab = Tab::Metadata; app.reset_scroll(); refresh_metadata(&mut daemon, &mut app); }
                KeyCode::Char('4') => { app.tab = Tab::File;     app.reset_scroll(); }
                KeyCode::Char('5') => { app.tab = Tab::Inject;   app.reset_scroll(); }
                KeyCode::Char('6') => { app.tab = Tab::Errors;   app.reset_scroll(); refresh_errors(&mut daemon, &mut app); }
                KeyCode::Char('7') => { app.tab = Tab::Diff;     app.reset_scroll(); refresh_diff(&mut daemon, &mut app); }
                KeyCode::Char('8') => { app.tab = Tab::Docker;   app.reset_scroll(); refresh_docker(&mut daemon, &mut app); }

                // Refresh
                KeyCode::Char('r') => {
                    match app.tab {
                        Tab::Logs     => { refresh_logs(&mut daemon, &mut app); app.last_refresh = Instant::now(); }
                        Tab::Snapshot => { app.reset_scroll(); refresh_snapshot(&mut daemon, &mut app); }
                        Tab::Metadata => { app.reset_scroll(); refresh_metadata(&mut daemon, &mut app); }
                        Tab::File     => { app.reset_scroll(); refresh_file(&mut daemon, &mut app); }
                        Tab::Inject   => { app.inject_input.clear(); app.set_status("Input cleared"); }
                        Tab::Errors   => { app.reset_scroll(); refresh_errors(&mut daemon, &mut app); }
                        Tab::Diff     => { app.reset_scroll(); refresh_diff(&mut daemon, &mut app); }
                        Tab::Docker   => { app.reset_scroll(); refresh_docker(&mut daemon, &mut app); }
                    }
                }

                // Edit mode
                KeyCode::Char('e') if matches!(app.tab, Tab::File | Tab::Inject) => {
                    app.editing = true;
                    app.set_status(match app.tab {
                        Tab::File   => "Edit path (path:N or path:N-M), Enter to load, Esc to cancel",
                        Tab::Inject => "Type log text — \\n for newlines, Enter to inject, Esc to cancel",
                        _           => "",
                    });
                }
                KeyCode::Enter if app.tab == Tab::File => refresh_file(&mut daemon, &mut app),
                KeyCode::Enter if app.tab == Tab::Inject && !app.inject_input.is_empty() => {
                    let text = app.inject_input.clone();
                    match inject_text(&text) {
                        Ok(n) => {
                            app.set_status(format!("Injected {n} line(s) — [6] Errors or [7] Diff"));
                            app.inject_input.clear();
                        }
                        Err(e) => app.set_status(format!("Inject error: {e}")),
                    }
                }

                // Scroll
                KeyCode::Up   | KeyCode::Char('k') => app.scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => {
                    let lines = app.current_content_lines();
                    let viewport = terminal.size()?.height.saturating_sub(8) as usize;
                    app.scroll_down(lines, viewport);
                }
                KeyCode::Home | KeyCode::Char('g') => app.reset_scroll(),
                KeyCode::End  | KeyCode::Char('G') => {
                    let lines = app.current_content_lines();
                    app.scroll_to_bottom(lines);
                }

                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
