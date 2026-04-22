use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use blackbox_core::types::{LogCluster, LogLine};

const SIMILARITY_THRESHOLD: f32 = 0.5;
const CLUSTER_CAP: usize = 1000;

pub type SharedDrainState = Arc<RwLock<DrainState>>;

pub struct DrainState {
    /// Prefix tree keyed by token count → list of clusters at that depth.
    prefix_tree: HashMap<usize, Vec<LogCluster>>,
    /// Total cluster count for cap enforcement.
    total: usize,
}

impl DrainState {
    fn new() -> Self {
        Self { prefix_tree: HashMap::new(), total: 0 }
    }
}

pub fn new_drain_state() -> SharedDrainState {
    Arc::new(RwLock::new(DrainState::new()))
}

/// Feed a new log line into the Drain state.
pub fn ingest_line(state: &SharedDrainState, line: &LogLine) {
    let tokens = tokenize(&line.text);
    if tokens.is_empty() {
        return;
    }
    println!("Drain: ingesting line: {} (source: {:?})", line.text, line.source_terminal);
    let level = detect_level(&line.text);
    // println!("Drain: detected level: {:?}", level);
    let mut guard = state.write().unwrap();
    // Evict oldest cluster by last_seen_ms when at cap BEFORE taking bucket mutable reference.
    if guard.total >= CLUSTER_CAP {
        evict_oldest(&mut guard.prefix_tree);
        guard.total -= 1;
    }

    let token_len = tokens.len();
    let bucket = guard.prefix_tree.entry(token_len).or_default();

    // Find best matching cluster in this token-count bucket.
    let best = bucket
        .iter_mut()
        .max_by(|a, b| {
            let sim_a = similarity(&tokenize(&a.pattern), &tokens);
            let sim_b = similarity(&tokenize(&b.pattern), &tokens);
            sim_a.partial_cmp(&sim_b).unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some(cluster) = best {
        let cluster_tokens = tokenize(&cluster.pattern);
        if similarity(&cluster_tokens, &tokens) >= SIMILARITY_THRESHOLD {
            // println!("Drain: matched existing cluster: {}", cluster.pattern);
            // Update existing cluster: merge variable tokens with wildcard.
            cluster.pattern = merge_pattern(&cluster_tokens, &tokens);
            cluster.count += 1;
            cluster.last_seen_ms = line.timestamp_ms;
            if level.is_some() && cluster.level.is_none() {
                cluster.level = level;
            }
            if let Some(source) = &line.source_terminal {
                if !cluster.sources.contains(source) {
                    cluster.sources.push(source.clone());
                }
            }
            return;
        }
    }

    let sources = line.source_terminal.clone().map(|s| vec![s]).unwrap_or_default();
    // println!("Drain: creating NEW cluster for: {} (level: {:?}, sources: {:?})", line.text, level, sources);
    bucket.push(LogCluster {
        pattern: line.text.clone(),
        count: 1,
        first_seen_ms: line.timestamp_ms,
        last_seen_ms: line.timestamp_ms,
        example: line.text.clone(),
        level,
        sources,
    });
    guard.total += 1;
}

/// Return clusters where level is set, sorted by count descending.
pub fn get_error_clusters(
    state: &SharedDrainState,
    limit: usize,
    source_filter: Option<&str>,
) -> Vec<LogCluster> {
    let guard = state.read().unwrap();
    let mut result: Vec<LogCluster> = guard
        .prefix_tree
        .values()
        .flat_map(|bucket| bucket.iter())
        .filter(|c| c.level.is_some())
        .filter(|c| {
            if let Some(f) = source_filter {
                c.sources.iter().any(|s| s == f)
            } else {
                true
            }
        })
        .cloned()
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result.truncate(limit);
    result
}

/// All clusters (for the get_compressed_errors total count).
pub fn total_error_line_count(state: &SharedDrainState) -> u64 {
    let guard = state.read().unwrap();
    guard
        .prefix_tree
        .values()
        .flat_map(|b| b.iter())
        .filter(|c| c.level.is_some())
        .map(|c| c.count)
        .sum()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}

/// Fraction of matching positions: identical_count / max(len_a, len_b).
pub fn similarity(a: &[String], b: &[String]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let matches = a.iter().zip(b.iter()).filter(|(x, y)| x == y || x.as_str() == "*").count();
    matches as f32 / a.len().max(1) as f32
}

/// Produce a merged pattern: keep equal tokens, replace differing with `*`.
fn merge_pattern(template: &[String], new: &[String]) -> String {
    template
        .iter()
        .zip(new.iter())
        .map(|(t, n)| if t == n || t.as_str() == "*" { t.clone() } else { "*".to_string() })
        .collect::<Vec<_>>()
        .join(" ")
}

fn detect_level(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if lower.contains("fatal") {
        Some("fatal".into())
    } else if lower.contains("error") || lower.contains("panic") || lower.contains("exception") {
        Some("error".into())
    } else if lower.contains("warn") {
        Some("warn".into())
    } else {
        None
    }
}

fn evict_oldest(tree: &mut HashMap<usize, Vec<LogCluster>>) {
    let oldest_key = tree
        .iter()
        .flat_map(|(k, bucket)| bucket.iter().map(move |c| (*k, c.last_seen_ms)))
        .min_by_key(|(_, ts)| *ts)
        .map(|(k, _)| k);

    if let Some(key) = oldest_key {
        if let Some(bucket) = tree.get_mut(&key) {
            if let Some(pos) = bucket
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.last_seen_ms)
                .map(|(i, _)| i)
            {
                bucket.remove(pos);
            }
            if bucket.is_empty() {
                tree.remove(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(text: &str) -> LogLine {
        LogLine { text: text.into(), timestamp_ms: 1000, source_terminal: None }
    }

    #[test]
    fn identical_lines_collapse_to_one_cluster() {
        let state = new_drain_state();
        for _ in 0..10 {
            ingest_line(&state, &make_line("error: connection refused to 127.0.0.1"));
        }
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, 10);
    }

    #[test]
    fn source_filtering_works() {
        let state = new_drain_state();
        ingest_line(&state, &LogLine { text: "error from redis".into(), timestamp_ms: 100, source_terminal: Some("bb-redis".into()) });
        ingest_line(&state, &LogLine { text: "error from postgres".into(), timestamp_ms: 101, source_terminal: Some("bb-postgres".into()) });

        let all = get_error_clusters(&state, 10, None);
        assert_eq!(all.len(), 2);

        let redis = get_error_clusters(&state, 10, Some("bb-redis"));
        assert_eq!(redis.len(), 1);
        assert!(redis[0].pattern.contains("redis"));
    }

    #[test]
    fn different_lines_produce_separate_clusters() {
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: connection refused to 127.0.0.1"));
        ingest_line(&state, &make_line("warn: disk space low on /dev/sda1"));
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn similarity_identical() {
        let a = vec!["foo".into(), "bar".into()];
        assert!((similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn similarity_unrelated() {
        let a = vec!["foo".into(), "bar".into()];
        let b = vec!["baz".into(), "qux".into()];
        assert!(similarity(&a, &b) < 0.5);
    }

    #[test]
    fn wildcard_merging() {
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: timeout connecting to 10.0.0.1"));
        ingest_line(&state, &make_line("error: timeout connecting to 10.0.0.2"));
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 1);
        assert!(clusters[0].pattern.contains('*'), "pattern should have wildcard: {}", clusters[0].pattern);
    }
}
