use std::io;
use std::time::Duration;

use blackbox_core::types::StatusResponse;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let status_port: u16 = args.windows(2)
        .find(|w| w[0] == "--status-port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(8766);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut status: Option<StatusResponse> = None;

    loop {
        // Poll status from daemon
        if let Ok(new_status) = fetch_status(status_port).await {
            status = Some(new_status);
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),   // status panel
                    Constraint::Min(5),      // buffer hint panel
                    Constraint::Length(3),   // help bar
                ])
                .split(f.area());

            // Panel 1: Status
            let status_text = if let Some(ref s) = status {
                let error_style = if s.has_recent_errors {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                let branch = s.git_branch.as_deref().unwrap_or("detached HEAD");
                vec![
                    Line::from(vec![
                        Span::styled("  Status: ", Style::default().fg(Color::Gray)),
                        Span::styled("running", Style::default().fg(Color::Green)),
                        Span::raw("   "),
                        Span::styled("Uptime: ", Style::default().fg(Color::Gray)),
                        Span::raw(format!("{}s", s.uptime_secs)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Project: ", Style::default().fg(Color::Gray)),
                        Span::styled(format!("{:?}", s.project_type).to_lowercase(), Style::default().fg(Color::Cyan)),
                        Span::raw("   "),
                        Span::styled("Branch: ", Style::default().fg(Color::Gray)),
                        Span::raw(branch.to_string()),
                        Span::raw(format!("  ({} dirty)", s.git_dirty_files)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Buffer: ", Style::default().fg(Color::Gray)),
                        Span::raw(format!("{} lines", s.buffer_lines)),
                        Span::raw("   "),
                        Span::styled("Errors: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            if s.has_recent_errors { "YES" } else { "no" },
                            error_style,
                        ),
                    ]),
                ]
            } else {
                vec![Line::from(Span::styled(
                    "  Waiting for daemon on port 8766...",
                    Style::default().fg(Color::Yellow),
                ))]
            };

            let status_block = Paragraph::new(status_text)
                .block(Block::default().borders(Borders::ALL).title(" \u{25c9} BlackBox "));
            f.render_widget(status_block, chunks[0]);

            // Panel 2: Buffer hint
            let hint_text = vec![
                Line::from(Span::styled(
                    "  Ask your AI agent to call get_terminal_buffer() to see recent output.",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(Span::styled(
                    "  Or call get_snapshot() for a full context overview.",
                    Style::default().fg(Color::Gray),
                )),
            ];
            let hint_block = Paragraph::new(hint_text)
                .block(Block::default().borders(Borders::ALL).title(" MCP Tools "));
            f.render_widget(hint_block, chunks[1]);

            // Panel 3: Help bar
            let help = Paragraph::new(Line::from(vec![
                Span::styled(" [q] ", Style::default().fg(Color::Yellow)),
                Span::raw("quit"),
            ]))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        })?;

        // Check for keypress (non-blocking, 100ms timeout)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

async fn fetch_status(port: u16) -> Result<StatusResponse, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).await?;
    stream.shutdown().await?; // signal we're done writing
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    let line = lines.next_line().await?.ok_or("no data")?;
    let status: StatusResponse = serde_json::from_str(&line)?;
    Ok(status)
}
