use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use crossbeam_queue::ArrayQueue;
use tokio::sync::Notify;

use blackbox_core::types::LogLine;

const BUFFER_CAPACITY: usize = 5000;
const INGEST_QUEUE_CAPACITY: usize = 8192;

pub struct BufferState {
    pub ring: RwLock<VecDeque<LogLine>>,
    pub ingest_queue: ArrayQueue<LogLine>,
    pub notify: Notify,
}

pub type SharedBuffer = Arc<BufferState>;

pub fn new_buffer() -> SharedBuffer {
    let buf = Arc::new(BufferState {
        ring: RwLock::new(VecDeque::with_capacity(BUFFER_CAPACITY)),
        ingest_queue: ArrayQueue::new(INGEST_QUEUE_CAPACITY),
        notify: Notify::new(),
    });

    // Start background ingestion task
    let buf_clone = buf.clone();
    tokio::spawn(async move {
        loop {
            // Drain the queue first to handle items pushed before we started waiting
            while let Some(line) = buf_clone.ingest_queue.pop() {
                let mut guard = buf_clone.ring.write().unwrap();
                if guard.len() >= BUFFER_CAPACITY {
                    guard.pop_front();
                }
                guard.push_back(line);
            }
            // Wait for next burst
            buf_clone.notify.notified().await;
        }
    });

    buf
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
    let timestamp_ms = now_ms();
    let line = LogLine { text, timestamp_ms, source_terminal: terminal };
    
    // Lock-free push to ingestion queue
    if buf.ingest_queue.push(line).is_ok() {
        buf.notify.notify_one();
    }
}

pub fn get_last_n(buf: &SharedBuffer, n: usize, terminal: Option<&str>) -> Vec<LogLine> {
    let guard = buf.ring.read().unwrap();
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

pub fn list_terminals(buf: &SharedBuffer) -> Vec<String> {
    let guard = buf.ring.read().unwrap();
    let mut names: Vec<String> = guard
        .iter()
        .filter_map(|l| l.source_terminal.clone())
        .collect::<std::collections::HashSet<String>>()
        .into_iter()
        .collect();
    names.sort();
    names
}

pub fn buffer_len(buf: &SharedBuffer) -> usize {
    buf.ring.read().unwrap().len()
}

/// Push a line to the ring buffer, Drain compression state, and structured log store.
/// Structured JSON parsing happens BEFORE PII masking so JSON is intact.
/// Individual field values are masked inside the structured parser.
pub fn push_line_and_drain(
    buf: &SharedBuffer,
    drain: &crate::scanners::drain::SharedDrainState,
    structured: &crate::structured_store::SharedStructuredStore,
    text: String,
    terminal: Option<String>,
) {
    // Step 1: ANSI strip
    let stripped = crate::scanners::ansi::strip_ansi_stateless(&text);

    // Step 2: Try structured parse BEFORE PII masking (JSON structure is intact here)
    let timestamp_ms = now_ms();
    let structured_event = crate::structured_store::try_parse(&stripped, timestamp_ms);

    // Step 3: PII mask for ring buffer
    let masked = crate::pii_masker::mask_pii(&stripped);
    if masked.trim().is_empty() {
        return;
    }

    // Step 4: Push masked text to ring buffer via lock-free queue
    let line = LogLine { text: masked, timestamp_ms, source_terminal: terminal };
    if buf.ingest_queue.push(line.clone()).is_ok() {
        buf.notify.notify_one();
    }

    // Step 5: Drain ingest
    crate::scanners::drain::ingest_line(drain, &line);

    // Step 6: Structured store ingest (if parsed)
    if let Some(event) = structured_event {
        crate::structured_store::ingest_event(structured, event);
    }
}

pub fn has_recent_errors(buf: &SharedBuffer) -> bool {
    let guard = buf.ring.read().unwrap();
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

    #[tokio::test]
    async fn test_push_and_get() {
        let buf = new_buffer();
        push_line(&buf, "line one".into(), None);
        push_line(&buf, "line two".into(), None);
        
        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let lines = get_last_n(&buf, 10, None);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "line one");
        assert_eq!(lines[1].text, "line two");
    }

    #[tokio::test]
    async fn test_capacity_eviction() {
        let buf = new_buffer();
        for i in 0..=BUFFER_CAPACITY {
            push_line(&buf, format!("line {i}"), None);
        }
        
        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        assert_eq!(buffer_len(&buf), BUFFER_CAPACITY);
        let lines = get_last_n(&buf, 1, None);
        assert_eq!(lines[0].text, format!("line {BUFFER_CAPACITY}"));
    }

    #[tokio::test]
    async fn test_ansi_stripping() {
        let buf = new_buffer();
        push_line(&buf, "\x1b[31merror: something failed\x1b[0m".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let lines = get_last_n(&buf, 1, None);
        assert_eq!(lines[0].text, "error: something failed");
    }

    #[tokio::test]
    async fn test_empty_lines_skipped() {
        let buf = new_buffer();
        push_line(&buf, "   ".into(), None);
        push_line(&buf, "\x1b[0m".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert_eq!(buffer_len(&buf), 0);
    }

    #[tokio::test]
    async fn test_has_recent_errors() {
        let buf = new_buffer();
        push_line(&buf, "build succeeded".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert!(!has_recent_errors(&buf));
        push_line(&buf, "error[E0382]: use of moved value".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert!(has_recent_errors(&buf));
    }

    #[tokio::test]
    async fn test_terminal_filter() {
        let buf = new_buffer();
        push_line(&buf, "rust error".into(), Some("cargo".into()));
        push_line(&buf, "python error".into(), Some("python".into()));
        push_line(&buf, "no terminal".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let cargo_lines = get_last_n(&buf, 10, Some("cargo"));
        assert_eq!(cargo_lines.len(), 1);
        assert_eq!(cargo_lines[0].text, "rust error");
        let all_lines = get_last_n(&buf, 10, None);
        assert_eq!(all_lines.len(), 3);
    }

    #[tokio::test]
    async fn test_list_terminals() {
        let buf = new_buffer();
        push_line(&buf, "a".into(), Some("bash".into()));
        push_line(&buf, "b".into(), Some("Python Debug".into()));
        push_line(&buf, "c".into(), None);
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let terminals = list_terminals(&buf);
        assert_eq!(terminals, vec!["Python Debug", "bash"]);
    }
}
