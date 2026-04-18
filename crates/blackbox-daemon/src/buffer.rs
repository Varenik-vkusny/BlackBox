use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use blackbox_core::types::LogLine;

const BUFFER_CAPACITY: usize = 5000;

pub type SharedBuffer = Arc<RwLock<VecDeque<LogLine>>>;

pub fn new_buffer() -> SharedBuffer {
    Arc::new(RwLock::new(VecDeque::with_capacity(BUFFER_CAPACITY)))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn push_line(buf: &SharedBuffer, text: String, terminal: Option<String>) {
    let text = crate::scanners::ansi::strip_ansi_stateless(&text);
    let text = crate::pii_masker::mask_pii(&text);
    if text.trim().is_empty() {
        return;
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let line = LogLine { text, timestamp_ms, source_terminal: terminal };
    let mut guard = buf.write().unwrap();
    if guard.len() >= BUFFER_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(line);
}

/// Returns the last `n` lines, optionally filtered to a specific terminal name.
pub fn get_last_n(buf: &SharedBuffer, n: usize, terminal: Option<&str>) -> Vec<LogLine> {
    let guard = buf.read().unwrap();
    match terminal {
        None => {
            let skip = guard.len().saturating_sub(n);
            guard.iter().skip(skip).cloned().collect()
        }
        Some(name) => guard
            .iter()
            .filter(|l| l.source_terminal.as_deref() == Some(name))
            .rev()
            .take(n)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
    }
}

/// Returns sorted unique terminal names present in the buffer.
pub fn list_terminals(buf: &SharedBuffer) -> Vec<String> {
    let guard = buf.read().unwrap();
    let mut names: Vec<String> = guard
        .iter()
        .filter_map(|l| l.source_terminal.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

pub fn buffer_len(buf: &SharedBuffer) -> usize {
    buf.read().unwrap().len()
}

/// Push a line to both the ring buffer and the Drain compression state.
pub fn push_line_and_drain(
    buf: &SharedBuffer,
    drain: &crate::scanners::drain::SharedDrainState,
    text: String,
    terminal: Option<String>,
) {
    push_line(buf, text, terminal);
    let line = buf.read().unwrap().back().cloned();
    if let Some(l) = line {
        crate::scanners::drain::ingest_line(drain, &l);
    }
}

pub fn has_recent_errors(buf: &SharedBuffer) -> bool {
    let guard = buf.read().unwrap();
    // Check last 200 lines for error indicators
    let skip = guard.len().saturating_sub(200);
    guard.iter().skip(skip).any(|line| {
        let lower = line.text.to_lowercase();
        lower.contains("error") || lower.contains("panic") || lower.contains("exception")
            || lower.contains("fatal") || lower.contains("failed")
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_get() {
        let buf = new_buffer();
        push_line(&buf, "line one".into(), None);
        push_line(&buf, "line two".into(), None);
        let lines = get_last_n(&buf, 10, None);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line one");
        assert_eq!(lines[1].text, "line two");
    }

    #[test]
    fn test_capacity_eviction() {
        let buf = new_buffer();
        for i in 0..=BUFFER_CAPACITY {
            push_line(&buf, format!("line {i}"), None);
        }
        assert_eq!(buffer_len(&buf), BUFFER_CAPACITY);
        let lines = get_last_n(&buf, 1, None);
        assert_eq!(lines[0].text, format!("line {BUFFER_CAPACITY}"));
    }

    #[test]
    fn test_ansi_stripping() {
        let buf = new_buffer();
        push_line(&buf, "\x1b[31merror: something failed\x1b[0m".into(), None);
        let lines = get_last_n(&buf, 1, None);
        assert_eq!(lines[0].text, "error: something failed");
    }

    #[test]
    fn test_empty_lines_skipped() {
        let buf = new_buffer();
        push_line(&buf, "   ".into(), None);
        push_line(&buf, "\x1b[0m".into(), None);
        assert_eq!(buffer_len(&buf), 0);
    }

    #[test]
    fn test_has_recent_errors() {
        let buf = new_buffer();
        push_line(&buf, "build succeeded".into(), None);
        assert!(!has_recent_errors(&buf));
        push_line(&buf, "error[E0382]: use of moved value".into(), None);
        assert!(has_recent_errors(&buf));
    }

    #[test]
    fn test_terminal_filter() {
        let buf = new_buffer();
        push_line(&buf, "rust error".into(), Some("cargo".into()));
        push_line(&buf, "python error".into(), Some("python".into()));
        push_line(&buf, "no terminal".into(), None);
        let cargo_lines = get_last_n(&buf, 10, Some("cargo"));
        assert_eq!(cargo_lines.len(), 1);
        assert_eq!(cargo_lines[0].text, "rust error");
        let all_lines = get_last_n(&buf, 10, None);
        assert_eq!(all_lines.len(), 3);
    }

    #[test]
    fn test_list_terminals() {
        let buf = new_buffer();
        push_line(&buf, "a".into(), Some("bash".into()));
        push_line(&buf, "b".into(), Some("Python Debug".into()));
        push_line(&buf, "c".into(), None);
        let terminals = list_terminals(&buf);
        assert_eq!(terminals, vec!["Python Debug", "bash"]);
    }
}
