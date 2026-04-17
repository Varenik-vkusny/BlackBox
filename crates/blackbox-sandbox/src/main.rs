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
const REFRESH_MS:  u64 = 1500; // auto-refresh logs every 1.5 s

// ── Tabs ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tab { Logs, Snapshot, Metadata, File, Inject }

impl Tab {
    const ALL: &'static [Tab] = &[Tab::Logs, Tab::Snapshot, Tab::Metadata, Tab::File, Tab::Inject];
    fn label(self) -> &'static str {
        match self {
            Tab::Logs     => "1 Logs",
            Tab::Snapshot => "2 Snapshot",
            Tab::Metadata => "3 Metadata",
            Tab::File     => "4 File",
            Tab::Inject   => "5 Inject",
        }
    }
    fn index(self) -> usize {
        match self { Tab::Logs=>0, Tab::Snapshot=>1, Tab::Metadata=>2, Tab::File=>3, Tab::Inject=>4 }
    }
}

// ── Log entry (richer than raw string) ───────────────────────────────────────

#[derive(Clone)]
struct LogEntry {
    text:      String,
    source:    String, // "vscode_bridge" | injected
    is_error:  bool,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct App {
    tab:          Tab,
    logs:         Vec<LogEntry>,
    snapshot:     String,
    metadata:     String,
    file_path:    String,       // editable in File tab
    file_content: String,
    inject_input: String,       // editable in Inject tab
    scroll:       usize,
    status_bar:   String,
    last_refresh: Instant,
    editing:      bool,         // true when typing in File/Inject input
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
            scroll: 0,
            status_bar: " Ready — press [r] to refresh, [1-5] to switch tabs".into(),
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
}

// ── Daemon wrapper (owns child process + JSON-RPC pipes) ──────────────────────

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
                   "--port", &BRIDGE_PORT.to_string(),
                   "--status-port", &STATUS_PORT.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;

        let stdin  = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        std::thread::sleep(Duration::from_millis(350));

        let mut d = Self { _child: child, stdin, stdout, seq: 0 };
        // handshake
        let _ = d.rpc("initialize", serde_json::json!({"protocolVersion":"2024-11-05"}));
        Ok(d)
    }

    fn rpc(&mut self, method: &str, params: serde_json::Value)
        -> Result<serde_json::Value, String>
    {
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

    fn tool(&mut self, name: &str, args: serde_json::Value)
        -> Result<serde_json::Value, String>
    {
        self.rpc("tools/call", serde_json::json!({"name": name, "arguments": args}))
    }
}

impl Drop for Daemon {
    fn drop(&mut self) { let _ = self._child.kill(); }
}

// ── Bridge helper (sends test lines to TCP port) ──────────────────────────────

fn inject_line(text: &str) -> Result<(), String> {
    let mut s = TcpStream::connect(format!("127.0.0.1:{BRIDGE_PORT}"))
        .map_err(|e| format!("bridge: {e}"))?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    if !text.ends_with('\n') { s.write_all(b"\n").map_err(|e| e.to_string())?; }
    Ok(())
}

// ── Refresh helpers ───────────────────────────────────────────────────────────

fn refresh_logs(daemon: &mut Daemon, app: &mut App) {
    match daemon.tool("get_terminal_buffer", serde_json::json!({"lines": 200})) {
        Ok(v) => {
            let raw = v["result"]["content"].as_str().unwrap_or("");
            // strip XML wrapper for display
            let inner = raw
                .trim_start_matches(|c: char| c != '\n').trim_start_matches('\n');
            let inner = if let Some(pos) = inner.rfind("</terminal_output>") {
                &inner[..pos]
            } else { inner };

            let returned = v["result"]["lines_returned"].as_u64().unwrap_or(0);
            app.logs = inner.lines().map(|l| {
                let is_error = {
                    let lo = l.to_lowercase();
                    lo.contains("error") || lo.contains("panic")
                        || lo.contains("failed") || lo.contains("exception")
                };
                LogEntry {
                    text: l.to_string(),
                    source: "vscode_bridge".into(),
                    is_error,
                }
            }).collect();

            // auto-scroll to bottom on refresh
            app.scroll = app.logs.len().saturating_sub(1);
            app.set_status(format!(
                "Logs refreshed — {} lines in buffer  (source: vscode_bridge | untrusted)",
                returned
            ));
        }
        Err(e) => app.set_status(format!("Error fetching logs: {e}")),
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
                for (i, m) in arr.iter().enumerate() {
                    out.push_str(&format!(
                        "  [{i}] type={:<8} name={:<20} version={}\n",
                        m["manifest_type"].as_str().unwrap_or("?"),
                        m["name"].as_str().unwrap_or("?"),
                        m["version"].as_str().unwrap_or("?"),
                    ));
                }
            }
            out.push_str("\n── .env Keys (values masked) ────────────────────────\n");
            if let Some(arr) = v["result"]["env_keys"].as_array() {
                if arr.is_empty() {
                    out.push_str("  (no .env files found)\n");
                }
                for key in arr {
                    out.push_str(&format!("  {}=<MASKED>\n", key.as_str().unwrap_or("?")));
                }
            }
            app.metadata = out;
            app.set_status("Metadata refreshed");
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

fn refresh_file(daemon: &mut Daemon, app: &mut App) {
    let path = app.file_path.clone();
    match daemon.tool("read_file", serde_json::json!({"path": path})) {
        Ok(v) => {
            if v["error"].is_object() {
                let msg = v["error"]["message"].as_str().unwrap_or("error");
                app.file_content = format!("[ERROR] {msg}");
                app.set_status(format!("read_file error: {msg}"));
            } else {
                let content = v["result"]["content"].as_str().unwrap_or("").to_string();
                let from = v["result"]["from_line"].as_u64().unwrap_or(1);
                let to   = v["result"]["to_line"].as_u64().unwrap_or(0);
                let lines = content.lines().count();
                app.file_content = content;
                app.reset_scroll();
                app.set_status(format!("File: {path}  |  lines {from}–{to}  |  {lines} shown"));
            }
        }
        Err(e) => app.set_status(format!("Error: {e}")),
    }
}

// ── Drawing ───────────────────────────────────────────────────────────────────

fn draw(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &App) -> io::Result<()> {
    terminal.draw(|f| {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // tab bar
                Constraint::Min(5),     // content
                Constraint::Length(3),  // status bar
            ])
            .split(area);

        // ── Tab bar ───────────────────────────────────────────────────────────
        let tab_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, 5); 5])
            .split(chunks[0]);

        for tab in Tab::ALL {
            let active = *tab == app.tab;
            let style = if active {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let label = format!(" {} ", tab.label());
            let block = Paragraph::new(label)
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
        }

        // ── Status bar ────────────────────────────────────────────────────────
        let help = match app.tab {
            Tab::File   => " [e] edit path  [Enter] load  [↑↓/jk] scroll  [r] refresh  [q] quit",
            Tab::Inject => " [e] edit input  [Enter] send  [r] clear  [q] quit",
            _           => " [1-5] tabs  [↑↓/jk] scroll  [r] refresh  [q] quit",
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
        let style = if e.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = Span::styled(
            format!("[{}] ", &e.source[..4.min(e.source.len())]),
            Style::default().fg(Color::DarkGray),
        );
        ListItem::new(Line::from(vec![prefix, Span::styled(&e.text, style)]))
    }).collect();

    let visible = area.height.saturating_sub(2) as usize;
    let start = app.scroll.min(app.logs.len().saturating_sub(1));
    let slice: Vec<ListItem> = items.into_iter().skip(start).take(visible).collect();

    let list = List::new(slice)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}

fn draw_text(f: &mut ratatui::Frame, app: &App, area: Rect, title: &str, content: &str) {
    let visible = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = content.lines()
        .skip(app.scroll)
        .take(visible)
        .map(|l| Line::from(Span::raw(l)))
        .collect();

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

    // path input
    let path_style = if app.editing {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let path_bar = Paragraph::new(Line::from(vec![
        Span::styled(" path: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.file_path, path_style),
        if app.editing { Span::styled("█", Style::default().fg(Color::Yellow)) }
        else { Span::raw("") },
    ]))
    .block(Block::default().borders(Borders::ALL)
        .title(if app.editing { " Edit path — Esc to cancel, Enter to load " }
               else { " [e] to edit path " }));
    f.render_widget(path_bar, chunks[0]);

    // file content
    let visible = chunks[1].height.saturating_sub(2) as usize;
    let lines: Vec<Line> = app.file_content.lines()
        .enumerate()
        .skip(app.scroll)
        .take(visible)
        .map(|(i, l)| {
            let num = Span::styled(
                format!("{:>4} │ ", app.scroll + i + 1),
                Style::default().fg(Color::DarkGray),
            );
            Line::from(vec![num, Span::raw(l)])
        })
        .collect();

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

    // input
    let input_style = if app.editing {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.inject_input, input_style),
        if app.editing { Span::styled("█", Style::default().fg(Color::Yellow)) }
        else { Span::raw("") },
    ]))
    .block(Block::default().borders(Borders::ALL)
        .title(if app.editing { " Type log line — Enter to inject, Esc to cancel " }
               else { " [e] to type a log line, [Enter] to send to bridge " }));
    f.render_widget(input, chunks[0]);

    // hints
    let hint_lines = vec![
        Line::from(Span::styled(
            "  Inject any text into the terminal buffer — the daemon receives it on port 28765.",
            Style::default().fg(Color::Gray),
        )),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  Try: ", Style::default().fg(Color::DarkGray)),
            Span::styled("error[E0382]: use of moved value", Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("  Try: ", Style::default().fg(Color::DarkGray)),
            Span::styled("\x1b[31mpanic: index out of bounds\x1b[0m", Style::default().fg(Color::Yellow)),
            Span::styled("  (ANSI will be stripped)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  Try: ", Style::default().fg(Color::DarkGray)),
            Span::styled("BUILD SUCCESS", Style::default().fg(Color::Green)),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "  After injecting, switch to tab [1] and press [r] to see the line appear.",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let hint = Paragraph::new(hint_lines)
        .block(Block::default().borders(Borders::ALL).title(" Inject Guide "));
    f.render_widget(hint, chunks[1]);
}

// ── Main event loop ───────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let daemon_bin = if cfg!(windows) {
        "target/debug/blackbox-daemon.exe"
    } else {
        "target/debug/blackbox-daemon"
    };

    let cwd = std::env::current_dir()?;
    let daemon_path = cwd.join(daemon_bin);

    // Check binary exists before entering raw mode
    if !daemon_path.exists() {
        eprintln!("ERROR: daemon binary not found at {}", daemon_path.display());
        eprintln!("Run:  cargo build -p blackbox-daemon");
        std::process::exit(1);
    }

    // ── TUI setup ─────────────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    // ── Spawn daemon ──────────────────────────────────────────────────────────
    let mut daemon = match Daemon::spawn(daemon_path.to_str().unwrap(), &cwd.to_string_lossy()) {
        Ok(d) => { app.set_status("Daemon started — use [r] to refresh, [1-5] to switch tabs"); d }
        Err(e) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            eprintln!("Failed to start daemon: {e}");
            std::process::exit(1);
        }
    };

    // ── Event loop ────────────────────────────────────────────────────────────
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

            // ── Editing mode (File path / Inject input) ───────────────────────
            if app.editing {
                match key.code {
                    KeyCode::Esc => {
                        app.editing = false;
                        app.set_status("Cancelled");
                    }
                    KeyCode::Enter => {
                        app.editing = false;
                        match app.tab {
                            Tab::File   => { refresh_file(&mut daemon, &mut app); }
                            Tab::Inject => {
                                let line = app.inject_input.clone();
                                if !line.is_empty() {
                                    match inject_line(&line) {
                                        Ok(_)  => {
                                            app.set_status(format!("Injected: \"{line}\"  → switch to [1] Logs and press [r]"));
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
                // quit
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,

                // tab switching
                KeyCode::Char('1') => { app.tab = Tab::Logs;     app.reset_scroll(); }
                KeyCode::Char('2') => { app.tab = Tab::Snapshot; app.reset_scroll(); refresh_snapshot(&mut daemon, &mut app); }
                KeyCode::Char('3') => { app.tab = Tab::Metadata; app.reset_scroll(); refresh_metadata(&mut daemon, &mut app); }
                KeyCode::Char('4') => { app.tab = Tab::File;     app.reset_scroll(); }
                KeyCode::Char('5') => { app.tab = Tab::Inject;   app.reset_scroll(); }

                // refresh
                KeyCode::Char('r') => {
                    app.reset_scroll();
                    match app.tab {
                        Tab::Logs     => { refresh_logs(&mut daemon, &mut app);     app.last_refresh = Instant::now(); }
                        Tab::Snapshot => refresh_snapshot(&mut daemon, &mut app),
                        Tab::Metadata => refresh_metadata(&mut daemon, &mut app),
                        Tab::File     => refresh_file(&mut daemon, &mut app),
                        Tab::Inject   => { app.inject_input.clear(); app.set_status("Input cleared"); }
                    }
                }

                // edit mode
                KeyCode::Char('e') if matches!(app.tab, Tab::File | Tab::Inject) => {
                    app.editing = true;
                    let tab = app.tab;
                    app.set_status(match tab {
                        Tab::File   => "Editing path — type path, Enter to load, Esc to cancel",
                        Tab::Inject => "Type log line — Enter to inject, Esc to cancel",
                        _           => "",
                    });
                }
                KeyCode::Enter if app.tab == Tab::File => {
                    refresh_file(&mut daemon, &mut app);
                }
                KeyCode::Enter if app.tab == Tab::Inject && !app.inject_input.is_empty() => {
                    let line = app.inject_input.clone();
                    match inject_line(&line) {
                        Ok(_)  => {
                            app.set_status(format!("Injected: \"{line}\"  → switch to [1] and [r]"));
                            app.inject_input.clear();
                        }
                        Err(e) => app.set_status(format!("Inject error: {e}")),
                    }
                }

                // scroll
                KeyCode::Up   | KeyCode::Char('k') => app.scroll_up(),
                KeyCode::Down | KeyCode::Char('j')  => {
                    let lines = match app.tab {
                        Tab::Logs     => app.logs.len(),
                        Tab::Snapshot => app.snapshot.lines().count(),
                        Tab::Metadata => app.metadata.lines().count(),
                        Tab::File     => app.file_content.lines().count(),
                        Tab::Inject   => 0,
                    };
                    let viewport = terminal.size()?.height.saturating_sub(8) as usize;
                    app.scroll_down(lines, viewport);
                }
                KeyCode::Home  => app.reset_scroll(),
                KeyCode::End   => {
                    let lines = match app.tab {
                        Tab::Logs => app.logs.len(),
                        _         => 0,
                    };
                    app.scroll = lines.saturating_sub(1);
                }

                _ => {}
            }
        }
    }

    // ── Cleanup ───────────────────────────────────────────────────────────────
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
