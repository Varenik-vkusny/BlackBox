# BlackBox — Technical Improvements & Debt Log

## Phase 3 targets

- [ ] Ring buffer uses `Arc<RwLock<VecDeque>>` — upgrade to lock-free Disruptor pattern
- [ ] ANSI stripping via regex — replace with proper state-machine parser
- [ ] XML injection guard in `get_terminal_buffer` escapes a fixed list of tags — enforce full data isolation via a typed context system (see Phase 3 spec)

## Known issues to fix before Phase 3

- [ ] VS Code extension TCP reconnect uses simple backoff — needs exponential backoff with jitter
- [ ] Docker monitor reconnects all containers on every 10s retry cycle — should maintain per-container state and only reconnect dropped streams
- [ ] `get_contextual_diff` cross-reference uses simple string equality on file paths — may miss matches if stack trace reports relative vs. absolute paths; normalise before comparison
- [ ] Stack trace parser minimum frame threshold of 2 may produce false positives on some Node.js single-frame errors — consider lowering to 1 for `nodejs` language

## Architecture improvements (lower priority)

- [ ] `get_contextual_diff` fallback chain returns compressed errors when diff is empty — but if logs have no file paths at all (e.g. plain `console.log` output), even the compressed errors path won't find relevant files; consider adding an explicit `get_all_diffs` mode that skips the intersection filter

- [ ] `get_terminal_buffer` and `get_compressed_errors` both read the same ring buffer — consider merging into one tool with `mode: "raw" | "compressed"` parameter
- [ ] Drain algorithm uses linear scan per token-count bucket — for high-throughput, replace with trie or inverted index
- [ ] `get_diff_hunks` runs `git diff` via subprocess — could be replaced with pure-Rust gix diff API once `gix-diff` stabilises hunk-level output
