use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use blackbox_core::types::{LogCluster, LogLine};
use regex::Regex;

const SIMILARITY_THRESHOLD: f32 = 0.5;
const CLUSTER_CAP: usize = 1000;
const TRIE_TOKEN_DEPTH: usize = 4;

pub type SharedDrainState = Arc<RwLock<DrainState>>;

// ── PreMasker: compile-once regexes ─────────────────────────────────────────────

static TIMESTAMP_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();
static EMAIL_RE: OnceLock<Regex> = OnceLock::new();
static UUID_RE: OnceLock<Regex> = OnceLock::new();
static IP_RE: OnceLock<Regex> = OnceLock::new();
static GIT_SHA_RE: OnceLock<Regex> = OnceLock::new();
static PATH_RE: OnceLock<Regex> = OnceLock::new();
static HEX_RE: OnceLock<Regex> = OnceLock::new();
static NUM_RE: OnceLock<Regex> = OnceLock::new();

/// Replace dynamic values with static tokens before any tokenization or similarity.
/// Order matters: more specific patterns must run first so they are not corrupted
/// by broader replacements (e.g. IPs contain digits, URLs contain paths).
fn pre_mask(line: &str) -> String {
    // 1. Timestamps — very specific, unlikely to false-positive.
    let s = TIMESTAMP_RE
        .get_or_init(|| {
            Regex::new(r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)?\b|\b\d{2}:\d{2}:\d{2}(?:\.\d+)?\b|\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\b|\b\d{2}/[A-Za-z]{3}/\d{4}:\d{2}:\d{2}:\d{2}\s+[+-]\d{4}\b").unwrap()
        })
        .replace_all(line, "<TIMESTAMP>");

    // 2. URLs — must run before PATH_RE so the whole URL is preserved as one token.
    let s = URL_RE
        .get_or_init(|| Regex::new(r"https?://[^\s]+").unwrap())
        .replace_all(&s, "<URL>");

    // 3. Emails
    let s = EMAIL_RE
        .get_or_init(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap())
        .replace_all(&s, "<EMAIL>");

    // 4. UUIDs
    let s = UUID_RE
        .get_or_init(|| {
            Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
                .unwrap()
        })
        .replace_all(&s, "<UUID>");

    // 5. IPs
    let s = IP_RE
        .get_or_init(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap())
        .replace_all(&s, "<IP>");

    // 6. Git SHAs — before generic HEX so short hex strings get semantic label.
    let s = GIT_SHA_RE
        .get_or_init(|| Regex::new(r"(?i)\b[a-f0-9]{7,40}\b").unwrap())
        .replace_all(&s, "<GIT_SHA>");

    // 7. File paths — after URL_RE to avoid breaking URLs.
    let s = PATH_RE
        .get_or_init(|| {
            Regex::new(r"(?:/[a-zA-Z0-9_.-]+(?:/[a-zA-Z0-9_.-]+)*|(?:[A-Za-z]:\\)[a-zA-Z0-9_.\\-]+|\.{1,2}/[a-zA-Z0-9_.-]+(?:/[a-zA-Z0-9_.-]+)*)").unwrap()
        })
        .replace_all(&s, "<PATH>");

    // 8. Generic hex literals.
    let s = HEX_RE
        .get_or_init(|| Regex::new(r"\b0x[0-9a-fA-F]+\b|\b[0-9a-fA-F]{6,}\b").unwrap())
        .replace_all(&s, "<HEX>");

    // 9. Standalone numbers — broadest, must be last.
    NUM_RE
        .get_or_init(|| Regex::new(r"\b\d+\b").unwrap())
        .replace_all(&s, "<NUM>")
        .into_owned()
}

// ── Trie Structures ────────────────────────────────────────────────────────────

/// A node in the fixed-depth prefix tree.
/// `children` maps token → deeper node. `clusters` holds templates at leaf level.
struct TreeNode {
    children: HashMap<String, Box<TreeNode>>,
    clusters: Vec<LogCluster>,
}

impl TreeNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            clusters: Vec::new(),
        }
    }
}

/// Drain3-style state: token-count roots → token-prefix trie → leaf clusters.
pub struct DrainState {
    /// First layer: token count → TreeNode.
    roots: HashMap<usize, TreeNode>,
    /// Total clusters across all leaves (for cap enforcement).
    total: usize,
}

impl DrainState {
    fn new() -> Self {
        Self {
            roots: HashMap::new(),
            total: 0,
        }
    }
}

pub fn new_drain_state() -> SharedDrainState {
    Arc::new(RwLock::new(DrainState::new()))
}

// ── Public API (unchanged signatures) ───────────────────────────────────────────

/// Feed a new log line into the Drain state.
pub fn ingest_line(state: &SharedDrainState, line: &LogLine) {
    let masked = pre_mask(&line.text);
    let tokens = tokenize(&masked);
    if tokens.is_empty() {
        return;
    }

    let level = detect_level(&line.text);
    let mut guard = state.write().unwrap();

    // Evict oldest cluster when at cap BEFORE taking a leaf mutable reference.
    if guard.total >= CLUSTER_CAP {
        evict_oldest(&mut guard.roots);
        guard.total = guard.total.saturating_sub(1);
    }

    let token_count = tokens.len();
    let root = guard.roots.entry(token_count).or_insert_with(TreeNode::new);

    // Walk the trie using the first TRIE_TOKEN_DEPTH tokens.
    let depth = token_count.min(TRIE_TOKEN_DEPTH);
    let mut node = root;
    for token in tokens.iter().take(depth) {
        node = node
            .children
            .entry(token.clone())
            .or_insert_with(|| Box::new(TreeNode::new()));
    }

    // Pre-compute pattern tokens for every cluster in this leaf to avoid
    // re-tokenizing inside the comparison loop.
    let candidates: Vec<(usize, Vec<String>)> = node
        .clusters
        .iter()
        .enumerate()
        .map(|(i, c)| (i, tokenize(&c.pattern)))
        .collect();

    let best = candidates
        .iter()
        .max_by(|(_, ta), (_, tb)| {
            let sim_a = similarity(ta, &tokens);
            let sim_b = similarity(tb, &tokens);
            sim_a.partial_cmp(&sim_b).unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some((idx, pattern_tokens)) = best {
        if similarity(pattern_tokens, &tokens) >= SIMILARITY_THRESHOLD {
            let cluster = &mut node.clusters[*idx];
            cluster.pattern = merge_pattern(pattern_tokens, &tokens);
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

    // No match → create a new cluster in this leaf.
    let sources = line
        .source_terminal
        .clone()
        .map(|s| vec![s])
        .unwrap_or_default();
    node.clusters.push(LogCluster {
        pattern: masked,
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
    let mut result: Vec<LogCluster> = collect_all_clusters(&guard.roots)
        .into_iter()
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
    collect_all_clusters(&guard.roots)
        .iter()
        .filter(|c| c.level.is_some())
        .map(|c| c.count)
        .sum()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace().map(String::from).collect()
}

/// Fraction of matching positions. Token lengths are guaranteed equal by trie
/// routing, but we keep the guard for robustness.
pub fn similarity(a: &[String], b: &[String]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let matches = a
        .iter()
        .zip(b.iter())
        .filter(|(x, y)| x == y || x.as_str() == "*")
        .count();
    matches as f32 / a.len().max(1) as f32
}

/// Produce a merged pattern: keep equal tokens, replace differing with `*`.
fn merge_pattern(template: &[String], new: &[String]) -> String {
    template
        .iter()
        .zip(new.iter())
        .map(|(t, n)| {
            if t == n || t.as_str() == "*" {
                t.clone()
            } else {
                "*".to_string()
            }
        })
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

// ── Trie traversal helpers ──────────────────────────────────────────────────────

/// Collect references to every cluster in the trie.
fn collect_all_clusters(roots: &HashMap<usize, TreeNode>) -> Vec<&LogCluster> {
    let mut out = Vec::with_capacity(roots.len() * 4);
    for root in roots.values() {
        collect_from_node(root, &mut out);
    }
    out
}

fn collect_from_node<'a>(node: &'a TreeNode, out: &mut Vec<&'a LogCluster>) {
    out.extend(node.clusters.iter());
    for child in node.children.values() {
        collect_from_node(child, out);
    }
}

/// Evict the single least-recently-seen cluster across the entire trie.
/// At cap=1000 a full DFS scan is negligible (~1–2 µs).
fn evict_oldest(roots: &mut HashMap<usize, TreeNode>) {
    let mut best: Option<(usize, Vec<String>, usize, u64)> = None;
    for (&token_count, root) in roots.iter() {
        find_oldest(root, token_count, &[], &mut best);
    }

    if let Some((token_count, path, idx, _)) = best {
        if let Some(root) = roots.get_mut(&token_count) {
            if let Some(leaf) = navigate_mut(root, &path) {
                if idx < leaf.clusters.len() {
                    leaf.clusters.remove(idx);
                }
            }
        }
        // Prune empty roots to keep the map compact.
        roots.retain(|_, r| has_any_clusters(r));
    }
}

/// DFS to locate the cluster with the smallest `last_seen_ms`.
fn find_oldest(
    node: &TreeNode,
    token_count: usize,
    path: &[String],
    best: &mut Option<(usize, Vec<String>, usize, u64)>,
) {
    for (i, c) in node.clusters.iter().enumerate() {
        let is_better = best
            .as_ref()
            .map(|(_, _, _, ts)| c.last_seen_ms < *ts)
            .unwrap_or(true);
        if is_better {
            *best = Some((token_count, path.to_vec(), i, c.last_seen_ms));
        }
    }
    for (token, child) in &node.children {
        let mut child_path = path.to_vec();
        child_path.push(token.clone());
        find_oldest(child, token_count, &child_path, best);
    }
}

/// Navigate from a root to a leaf using `path` tokens.
fn navigate_mut<'a>(node: &'a mut TreeNode, path: &[String]) -> Option<&'a mut TreeNode> {
    let mut current = node;
    for token in path {
        current = current.children.get_mut(token)?.as_mut();
    }
    Some(current)
}

fn has_any_clusters(node: &TreeNode) -> bool {
    if !node.clusters.is_empty() {
        return true;
    }
    node.children.values().any(|c| has_any_clusters(c))
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(text: &str) -> LogLine {
        LogLine {
            text: text.into(),
            timestamp_ms: 1000,
            source_terminal: None,
        }
    }

    // ── PreMasker tests ─────────────────────────────────────────────────────────

    #[test]
    fn pre_mask_replaces_ip() {
        assert_eq!(
            pre_mask("Connection refused to 192.168.1.1"),
            "Connection refused to <IP>"
        );
    }

    #[test]
    fn pre_mask_replaces_uuid() {
        assert_eq!(
            pre_mask("Request 5f4dcc3b-2c00-4e2a-b5e0-123456789abc failed"),
            "Request <UUID> failed"
        );
    }

    #[test]
    fn pre_mask_replaces_hex() {
        // Short hex with 0x prefix.
        assert_eq!(pre_mask("Hash 0xcafe is valid"), "Hash <HEX> is valid");
        assert_eq!(pre_mask("Pointer 0x7ffe1234"), "Pointer <HEX>");
    }

    #[test]
    fn pre_mask_replaces_number() {
        assert_eq!(pre_mask("Port 8080 open"), "Port <NUM> open");
        assert_eq!(pre_mask("Exit code 1"), "Exit code <NUM>");
    }

    #[test]
    fn pre_mask_replaces_timestamp() {
        assert_eq!(
            pre_mask("2024-01-15T10:30:00.123Z error: timeout"),
            "<TIMESTAMP> error: timeout"
        );
        assert_eq!(
            pre_mask("Jan 15 10:30:00 connection refused"),
            "<TIMESTAMP> connection refused"
        );
        assert_eq!(
            pre_mask("10:30:00.123 request completed"),
            "<TIMESTAMP> request completed"
        );
    }

    #[test]
    fn pre_mask_replaces_url() {
        assert_eq!(
            pre_mask("GET https://api.example.com/v1/users/123"),
            "GET <URL>"
        );
        assert_eq!(
            pre_mask("Redirect to http://localhost:8080/login"),
            "Redirect to <URL>"
        );
    }

    #[test]
    fn pre_mask_replaces_email() {
        assert_eq!(
            pre_mask("Login failed for admin@example.com"),
            "Login failed for <EMAIL>"
        );
    }

    #[test]
    fn pre_mask_replaces_git_sha() {
        assert_eq!(
            pre_mask("Built from commit a1b2c3d"),
            "Built from commit <GIT_SHA>"
        );
        // 40-char SHA
        assert_eq!(
            pre_mask("Deploy abcdef1234567890abcdef1234567890abcdef12"),
            "Deploy <GIT_SHA>"
        );
    }

    #[test]
    fn pre_mask_replaces_path() {
        assert_eq!(pre_mask("File not found: /tmp/data.txt"), "File not found: <PATH>");
        assert_eq!(pre_mask("Config at ./src/main.rs"), "Config at <PATH>");
        assert_eq!(
            pre_mask("Cache C:\\Users\\user\\AppData"),
            "Cache <PATH>"
        );
    }

    #[test]
    fn pre_mask_url_before_path() {
        // URL_RE must run before PATH_RE so the entire URL is preserved,
        // not split into scheme + path tokens.
        let masked = pre_mask("GET https://example.com/api/v1");
        assert_eq!(masked, "GET <URL>");
        assert!(
            !masked.contains("<PATH>"),
            "URL should not be partially masked as path: {masked}"
        );
    }

    #[test]
    fn pre_mask_ordering_is_correct() {
        // IP must be masked before NUM so that 192.168.1.1 becomes <IP>, not <NUM>.<NUM>...
        let masked = pre_mask("Host 192.168.1.1:8080");
        assert_eq!(masked, "Host <IP>:<NUM>");
    }

    // ── Core Drain tests ──────────────────────────────────────────────────────

    #[test]
    fn identical_lines_collapse_to_one_cluster() {
        let state = new_drain_state();
        for _ in 0..10 {
            ingest_line(&state, &make_line("error: connection refused to 127.0.0.1"));
        }
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, 10);
        // Because of pre-masking the IP becomes <IP>, so the template is semantic.
        assert!(clusters[0].pattern.contains("<IP>"));
    }

    #[test]
    fn source_filtering_works() {
        let state = new_drain_state();
        ingest_line(
            &state,
            &LogLine {
                text: "error from redis".into(),
                timestamp_ms: 100,
                source_terminal: Some("bb-redis".into()),
            },
        );
        ingest_line(
            &state,
            &LogLine {
                text: "error from postgres".into(),
                timestamp_ms: 101,
                source_terminal: Some("bb-postgres".into()),
            },
        );

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
    fn wildcard_merging() {
        // These two lines differ at a hostname which pre-masking does NOT catch,
        // so they should merge with a literal * wildcard.
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: timeout connecting to hostA"));
        ingest_line(&state, &make_line("error: timeout connecting to hostB"));
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 1);
        assert!(
            clusters[0].pattern.contains('*'),
            "pattern should have wildcard: {}",
            clusters[0].pattern
        );
    }

    #[test]
    fn ip_pre_masking_avoids_unnecessary_wildcard() {
        // IPs are masked before clustering, so two lines with different IPs
        // should produce a clean <IP> template instead of *.
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: timeout connecting to 10.0.0.1"));
        ingest_line(&state, &make_line("error: timeout connecting to 10.0.0.2"));
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 1);
        assert!(
            clusters[0].pattern.contains("<IP>"),
            "pattern should contain <IP>: {}",
            clusters[0].pattern
        );
        assert!(
            !clusters[0].pattern.contains('*'),
            "pattern should NOT need * when pre-mask handles the variable: {}",
            clusters[0].pattern
        );
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
    fn total_error_line_count_sums_all_levels() {
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: something broke"));
        ingest_line(&state, &make_line("error: something broke"));
        ingest_line(&state, &make_line("warn: low disk"));
        // Both error and warn have level.is_some(), so total = 2 + 1 = 3.
        assert_eq!(total_error_line_count(&state), 3);
    }

    #[test]
    fn trie_routing_different_token_counts() {
        // 4-token line and 5-token line must land in different roots.
        let state = new_drain_state();
        ingest_line(&state, &make_line("error: timeout"));
        ingest_line(&state, &make_line("error: timeout now"));
        let clusters = get_error_clusters(&state, 100, None);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn eviction_keeps_cap() {
        let state = new_drain_state();
        for i in 0..(CLUSTER_CAP + 50) {
            // Use suffix like "msgA0", "msgA1" — digits are not standalone,
            // so pre-masking leaves them intact. Each line is unique and
            // has similarity 1/3 < 0.5, so no merging occurs.
            ingest_line(
                &state,
                &make_line(&format!("error: msgA{i} happened")),
            );
        }
        // Count total clusters by iterating the trie.
        let guard = state.read().unwrap();
        let all = collect_all_clusters(&guard.roots);
        assert_eq!(all.len(), CLUSTER_CAP);
    }
}
