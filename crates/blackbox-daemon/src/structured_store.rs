use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use blackbox_core::types::{LogLine, StructuredEvent, StructuredLogFormat};
use serde_json::Value;

const STORE_CAPACITY: usize = 2_000;

pub type SharedStructuredStore = Arc<RwLock<VecDeque<StructuredEvent>>>;

pub fn new_structured_store() -> SharedStructuredStore {
    Arc::new(RwLock::new(VecDeque::with_capacity(STORE_CAPACITY)))
}

/// Parse raw (pre-PII-mask) text as a structured log event.
/// Must be called BEFORE PII masking so JSON structure is intact.
/// Field values are individually PII-masked inside build_event.
pub fn try_parse(text: &str, timestamp_ms: u64) -> Option<StructuredEvent> {
    parse_structured(text, timestamp_ms)
}

pub fn ingest_event(store: &SharedStructuredStore, event: StructuredEvent) {
    let mut guard = store.write().unwrap();
    if guard.len() >= STORE_CAPACITY {
        guard.pop_front();
    }
    guard.push_back(event);
}

/// Return all events matching the given span_id (exact match).
pub fn get_by_span_id(store: &SharedStructuredStore, span_id: &str) -> Vec<StructuredEvent> {
    let guard = store.read().unwrap();
    guard
        .iter()
        .filter(|e| e.span_id.as_deref() == Some(span_id))
        .cloned()
        .collect()
}

/// Return the most recent `limit` structured events, optionally filtered by span_id.
pub fn get_recent(store: &SharedStructuredStore, limit: usize, span_id: Option<&str>) -> Vec<StructuredEvent> {
    let guard = store.read().unwrap();
    guard
        .iter()
        .rev()
        .filter(|e| span_id.map_or(true, |s| e.span_id.as_deref() == Some(s)))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn store_len(store: &SharedStructuredStore) -> usize {
    store.read().unwrap().len()
}

// ── Parsers ──────────────────────────────────────────────────────────────────

fn parse_structured(text: &str, timestamp_ms: u64) -> Option<StructuredEvent> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let obj: Value = serde_json::from_str(trimmed).ok()?;
    let map = obj.as_object()?;

    // ── pino / bunyan: level is a number ────────────────────────────────────
    if let Some(level_num) = map.get("level").and_then(|v| v.as_u64()) {
        let level = pino_level(level_num);
        let message = str_field(map, &["msg", "message"]).unwrap_or_default();
        if message.is_empty() {
            return None; // not a log line
        }
        return Some(build_event(
            timestamp_ms,
            StructuredLogFormat::Pino,
            Some(level),
            message,
            map,
            &[],
        ));
    }

    // ── tracing (Rust): has "fields" object ─────────────────────────────────
    if let Some(fields_val) = map.get("fields").and_then(|v| v.as_object()) {
        let level = str_field(map, &["level"]);
        let message = str_field(fields_val, &["message", "msg"])
            .or_else(|| str_field(map, &["message", "msg"]))
            .unwrap_or_default();
        if message.is_empty() {
            return None;
        }

        // Correlation IDs from fields (promoted to top-level event fields)
        let span_id = str_field(fields_val, &["span_id", "spanId"])
            .or_else(|| str_field(map, &["span_id", "spanId"]));
        let trace_id = str_field(fields_val, &["trace_id", "traceId"])
            .or_else(|| str_field(map, &["trace_id", "traceId"]));
        let request_id = str_field(fields_val, &["request_id", "requestId", "req_id"])
            .or_else(|| str_field(map, &["request_id", "requestId", "req_id"]));

        // Remaining fields → extra (skip correlation IDs already promoted)
        let promoted_keys = ["message", "msg", "span_id", "spanId", "trace_id", "traceId",
                              "request_id", "requestId", "req_id"];
        let mut extra: HashMap<String, String> = HashMap::new();
        for (k, v) in fields_val {
            if promoted_keys.contains(&k.as_str()) {
                continue;
            }
            if let Some(s) = v.as_str() {
                extra.insert(k.clone(), crate::pii_masker::mask_pii(s));
            } else if matches!(v, Value::Number(_) | Value::Bool(_)) {
                extra.insert(k.clone(), v.to_string());
            }
        }

        let target = str_field(map, &["target"]);
        // span from "span" object (fallback for span_id)
        let span_id = span_id.or_else(|| {
            map.get("span").and_then(|v| v.as_object())
                .and_then(|s| str_field(s, &["id", "span_id"]))
        });

        return Some(StructuredEvent {
            timestamp_ms,
            format: StructuredLogFormat::Tracing,
            level,
            message: crate::pii_masker::mask_pii(&message),
            span_id,
            trace_id,
            request_id,
            target,
            extra,
        });
    }

    // ── structlog (Python): has "event" key ─────────────────────────────────
    if let Some(message) = map.get("event").and_then(|v| v.as_str()) {
        if message.is_empty() {
            return None;
        }
        let level = str_field(map, &["level", "log_level"]);
        return Some(build_event(
            timestamp_ms,
            StructuredLogFormat::Structlog,
            level,
            message.to_string(),
            map,
            &["event"],
        ));
    }

    // ── Java Logback / Log4j2 / Spring Boot JSON ────────────────────────────
    // Logback (logstash-logback-encoder): "@timestamp" + "message" + "logger_name"
    // Log4j2 JSON layout: "instant" object + "loggerName" + "message"
    // Spring Boot structured logging: "@timestamp" + "log.level" + "message"
    let is_java_logback = map.contains_key("logger_name") || map.contains_key("loggerName")
        || map.get("instant").and_then(|v| v.as_object()).is_some()
        || (map.contains_key("@timestamp") && map.contains_key("message"));
    if is_java_logback {
        let message = str_field(map, &["message", "msg"]).unwrap_or_default();
        if !message.is_empty() {
            // Spring Boot uses "log.level" with a dot in the key name
            let level = str_field(map, &["level"])
                .or_else(|| map.get("log.level").and_then(|v| v.as_str()).map(|s| s.to_string()));
            // Correlation IDs from common Java observability libraries (Micrometer, OpenTelemetry)
            let span_id = str_field(map, &["spanId", "span_id", "X-B3-SpanId"]);
            let trace_id = str_field(map, &["traceId", "trace_id", "X-B3-TraceId"]);
            let request_id = str_field(map, &["requestId", "request_id", "X-Request-ID"]);
            let target = str_field(map, &["logger_name", "loggerName"]);
            let standard_java_keys = ["message", "level", "log.level", "@timestamp", "@version",
                "logger_name", "loggerName", "thread_name", "threadName", "thread",
                "spanId", "span_id", "traceId", "trace_id", "requestId", "request_id",
                "X-B3-SpanId", "X-B3-TraceId", "X-Request-ID", "endOfBatch",
                "loggerFqcn", "instant", "contextMap"];
            let mut extra = HashMap::new();
            for (k, v) in map {
                if standard_java_keys.contains(&k.as_str()) { continue; }
                if let Some(s) = v.as_str() {
                    extra.insert(k.clone(), crate::pii_masker::mask_pii(s));
                } else if matches!(v, Value::Number(_) | Value::Bool(_)) {
                    extra.insert(k.clone(), v.to_string());
                }
            }
            return Some(StructuredEvent {
                timestamp_ms,
                format: StructuredLogFormat::JavaLogback,
                level,
                message: crate::pii_masker::mask_pii(&message),
                span_id,
                trace_id,
                request_id,
                target,
                extra,
            });
        }
    }

    // ── logrus / generic: has "msg" or "message" + "level" as string ────────
    let message = str_field(map, &["msg", "message", "log"])?;
    if message.is_empty() {
        return None;
    }
    let level = str_field(map, &["level", "severity"]);

    // Require at least a level or a time field to avoid treating arbitrary JSON as logs
    let has_time = map.contains_key("time") || map.contains_key("timestamp") || map.contains_key("ts");
    if level.is_none() && !has_time {
        return None;
    }

    let format = if map.contains_key("time") && level.is_some() {
        StructuredLogFormat::Logrus
    } else {
        StructuredLogFormat::Generic
    };

    Some(build_event(timestamp_ms, format, level, message, map, &[]))
}

/// Build a StructuredEvent from the parsed JSON map, extracting common correlation fields.
fn build_event(
    timestamp_ms: u64,
    format: StructuredLogFormat,
    level: Option<String>,
    message: String,
    map: &serde_json::Map<String, Value>,
    skip_extra_keys: &[&str],
) -> StructuredEvent {
    let span_id = str_field(map, &["span_id", "spanId", "span.id"]);
    let trace_id = str_field(map, &["trace_id", "traceId", "dd.trace_id"]);
    let request_id = str_field(map, &["request_id", "requestId", "req_id", "x_request_id"]);
    let target = str_field(map, &["target", "logger", "name"]);

    // Collect remaining string fields as extra (PII-masked), skip internal fields
    let standard_keys = [
        "level", "msg", "message", "log", "time", "timestamp", "ts", "pid", "hostname",
        "v", "name", "span_id", "spanId", "trace_id", "traceId", "request_id", "requestId",
        "req_id", "x_request_id", "target", "logger", "dd.trace_id", "dd.span_id",
    ];
    let mut extra = HashMap::new();
    for (k, v) in map {
        if standard_keys.contains(&k.as_str()) || skip_extra_keys.contains(&k.as_str()) {
            continue;
        }
        if let Some(s) = v.as_str() {
            extra.insert(k.clone(), crate::pii_masker::mask_pii(s));
        } else if matches!(v, Value::Number(_) | Value::Bool(_)) {
            extra.insert(k.clone(), v.to_string());
        }
    }

    StructuredEvent {
        timestamp_ms,
        format,
        level,
        message: crate::pii_masker::mask_pii(&message),
        span_id,
        trace_id,
        request_id,
        target,
        extra,
    }
}

fn str_field(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = map.get(*k).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

fn pino_level(n: u64) -> String {
    match n {
        10 => "trace",
        20 => "debug",
        30 => "info",
        40 => "warn",
        50 => "error",
        60 => "fatal",
        _ => "unknown",
    }
    .to_string()
}
