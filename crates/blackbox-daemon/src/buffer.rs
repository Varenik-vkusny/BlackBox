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

pub fn push_line(buf: &SharedBuffer, text: String) {
    let text = crate::scanners::ansi::strip_ansi_stateless(&text);
    let text = crate::pii_masker::mask_pii(&text);
    if text.trim().is_empty() {
        return;
    }
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let line = LogLine { text, timestamp_ms };
    let mut guard = buf.write().unwrap();
    if guard.len() >= BUFFER_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(line);
}

pub fn get_last_n(buf: &SharedBuffer, n: usize) -> Vec<LogLine> {
    let guard = buf.read().unwrap();
    let skip = guard.len().saturating_sub(n);
    guard.iter().skip(skip).cloned().collect()
}

pub fn buffer_len(buf: &SharedBuffer) -> usize {
    buf.read().unwrap().len()
}

/// Push a line to both the ring buffer and the Drain compression state.
pub fn push_line_and_drain(buf: &SharedBuffer, drain: &crate::scanners::drain::SharedDrainState, text: String) {
    push_line(buf, text);
    // Drain ingests the already-stripped line from the buffer.
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
        push_line(&buf, "line one".into());
        push_line(&buf, "line two".into());
        let lines = get_last_n(&buf, 10);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line one");
        assert_eq!(lines[1].text, "line two");
    }

    #[test]
    fn test_capacity_eviction() {
        let buf = new_buffer();
        for i in 0..=BUFFER_CAPACITY {
            push_line(&buf, format!("line {i}"));
        }
        assert_eq!(buffer_len(&buf), BUFFER_CAPACITY);
        let lines = get_last_n(&buf, 1);
        assert_eq!(lines[0].text, format!("line {BUFFER_CAPACITY}"));
    }

    #[test]
    fn test_ansi_stripping() {
        let buf = new_buffer();
        push_line(&buf, "\x1b[31merror: something failed\x1b[0m".into());
        let lines = get_last_n(&buf, 1);
        assert_eq!(lines[0].text, "error: something failed");
    }

    #[test]
    fn test_empty_lines_skipped() {
        let buf = new_buffer();
        push_line(&buf, "   ".into());
        push_line(&buf, "\x1b[0m".into()); // ANSI-only becomes empty after strip
        assert_eq!(buffer_len(&buf), 0);
    }

    #[test]
    fn test_has_recent_errors() {
        let buf = new_buffer();
        push_line(&buf, "build succeeded".into());
        assert!(!has_recent_errors(&buf));
        push_line(&buf, "error[E0382]: use of moved value".into());
        assert!(has_recent_errors(&buf));
    }
}
