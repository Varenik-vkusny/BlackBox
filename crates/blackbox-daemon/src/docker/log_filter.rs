use crate::docker::demux::StreamKind;

#[allow(dead_code)]
/// Decide whether a log line from a Docker container should be kept.
/// Returns the filtered text to store, or None to discard.
///
/// Rules:
/// - All stderr lines are kept unconditionally.
/// - For stdout: try JSON parse, check level field; keep ERROR/WARN/FATAL, discard rest.
/// - Plain-text stdout lines without a parseable level are discarded (likely INFO chatter).
pub fn should_keep(line: &str, stream: StreamKind) -> Option<String> {
    if stream == StreamKind::Stderr {
        return Some(line.to_string());
    }

    // Try to parse as JSON and extract level field.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
        let level = v["level"]
            .as_str()
            .or_else(|| v["severity"].as_str())
            .or_else(|| v["lvl"].as_str())
            .unwrap_or("")
            .to_lowercase();

        let keep = matches!(level.as_str(), "error" | "err" | "fatal" | "warn" | "warning");
        if keep {
            // Return the message field if present, otherwise the full line.
            let msg = v["msg"]
                .as_str()
                .or_else(|| v["message"].as_str())
                .unwrap_or(line);
            return Some(msg.to_string());
        }
        return None;
    }

    // Plain-text stdout: apply keyword heuristic.
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("fatal") || lower.contains("panic")
        || lower.contains("exception") || lower.contains("warn")
    {
        return Some(line.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_info_discarded() {
        let line = r#"{"level":"INFO","msg":"server started"}"#;
        assert!(should_keep(line, StreamKind::Stdout).is_none());
    }

    #[test]
    fn json_error_kept() {
        let line = r#"{"level":"ERROR","msg":"db connection failed"}"#;
        assert_eq!(should_keep(line, StreamKind::Stdout).unwrap(), "db connection failed");
    }

    #[test]
    fn json_warn_kept() {
        let line = r#"{"level":"warn","msg":"retrying request"}"#;
        assert_eq!(should_keep(line, StreamKind::Stdout).unwrap(), "retrying request");
    }

    #[test]
    fn stderr_always_kept() {
        let line = "some random stderr output";
        assert!(should_keep(line, StreamKind::Stderr).is_some());
    }

    #[test]
    fn plain_text_error_kept() {
        assert!(should_keep("error: cannot connect", StreamKind::Stdout).is_some());
    }

    #[test]
    fn plain_text_info_discarded() {
        assert!(should_keep("server listening on :8080", StreamKind::Stdout).is_none());
    }
}
