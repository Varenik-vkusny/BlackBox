# BlackBox — Phase 1 & 2 Workspace Guide

> [!IMPORTANT]
> **🤖 AI Operational Meta-Rules:**
> 1. **Context Initialization**: Every agent session MUST begin by reading this `CLAUDE.md` file.
> 2. **Knowledge Base (KB)**: The `docs/` folder is the primary source of project truth.
> 3. **Usage Pattern**: Do not read all documents. Inspect filenames and headers (Metadata) to decide what to read based on the task (exactly like identifying a Skill).
> 4. **Doc Maintenance**: When updating core logic or architecture, ensure corresponding files in `docs/` are synchronized.

## Phase 1 Requirements (All Complete ✅)

| Requirement | Implementation | Status |
|---|---|---|
| **Terminal bridge plugin** | VS Code extension subscribes to `onDidWriteTerminalData`, sends to TCP 127.0.0.1:8765 | ✅ |
| **Ring buffer + memory bound** | `Arc<RwLock<VecDeque<LogLine>>>`, 5000-line capacity, FIFO eviction | ✅ |
| **ANSI stripping** | OnceLock regex pattern covers CSI/OSC/charset/BS/CR | ✅ |
| **Git scanner** | `gix` library: branch name + dirty file count | ✅ |
| **Manifest detection** | Priority order: Cargo > Go > npm; returns all; ProjectKind enum | ✅ |
| **.env masking** | Regex captures key names only; values never exposed | ✅ |
| **MCP tools (4)** | get_snapshot, get_terminal_buffer, get_project_metadata, read_file | ✅ |
| **Path traversal protection** | canonicalize + starts_with validation | ✅ |
| **XML injection guard** | Escape `</terminal_output>` + common HTML tags in wrapped output | ✅ |
| **XML injection guard** | Escape `</terminal_output>` + common HTML tags in wrapped output | ✅ |

## Phase 2 Requirements (All Complete ✅)

| Requirement | Implementation | Status |
|---|---|---|
| **Log deduplication (Drain)** | `scanners/drain.rs`: prefix-tree clustering, 1000-cluster cap, wildcard merging | ✅ |
| **Stack trace parsing** | `scanners/stacktrace.rs`: Rust/Python/Node.js/Java state-machine parsers, stdlib filtering | ✅ |
| **Smart git diffs** | `scanners/git.rs`: `get_changed_files` + `get_diff_hunks` via `git diff` subprocess | ✅ |
| **Docker monitoring** | `docker/mod.rs` with bollard: stream logs, demux stdout/stderr, filter ERROR/WARN/FATAL | ✅ |
| **DaemonState refactor** | `daemon_state.rs`: single struct threading buf + drain + error_store through tasks | ✅ |
| **Blocking I/O fix** | `scan_git`/`scan_manifests`/`scan_env_keys` wrapped in `spawn_blocking` | ✅ |
| **MCP tools (3 new = 7 total)** | get_compressed_errors, get_contextual_diff, get_container_logs | ✅ |

## Phase Status
- **Phase 1**: ✅ Complete (MVP daemon + VS Code bridge)
- **Phase 2**: ✅ Complete (compression, Docker monitoring, smart diffs — 7 MCP tools total)
- **Phase 3**: ✅ Complete (PII masker + entropy scanner, ANSI state machine, typed context, 2 new MCP tools, shell hooks, React lab UI)

## Cargo Workspace Pattern
- Workspace root: `[workspace.package]` with version, edition = "2021", rust-version = "1.77"
- All crates: use `{ workspace = true }` for serde, tokio, etc.
- Three crates: blackbox-core (types lib), blackbox-daemon (MCP binary), blackbox-run (cli wrapper)

## Ring Buffer Design
- `Arc<RwLock<VecDeque<LogLine>>>` with FIFO eviction: `pop_front()` when at 5000-line capacity
- Bounded memory, oldest logs discarded first

## ANSI Stripping
- Compile regex once via `static ANSI_RE: OnceLock<Regex>` (not lazy_static)
- Pattern covers CSI, OSC, designate charset, backspace, CR
- Critical for AI consumption: newlines preserved, color codes removed

## Manifest Detection
- Priority const array: `[("Cargo.toml", Cargo), ("go.mod", Go), ("package.json", Npm)]`
- Return ALL manifests in priority order (not just first match)
- Use ProjectKind enum (Cargo, Go, Npm, Unknown) with `serde(rename_all = "lowercase")`

## Security Patterns
- **Path traversal**: `canonicalize()` both paths, verify `requested.starts_with(cwd)`
- **XML injection**: escape `</terminal_output>` → `&lt;/terminal_output&gt;` in wrapped output
- **.env masking**: regex capture group 1 (key name) only; values never returned

## Tokio Patterns
- `spawn_blocking` for !Send ops (stdin.lock())
- `tokio::select!` for shutdown signal + task spawns
- `Handle::current().block_on()` to enter async from blocking pool

## Testing Strategy
- Spawn daemon as subprocess, JSON-RPC over stdin/stdout pipes (true integration tests, no AI calls)
- Test isolation: `static COUNTER: AtomicUsize` with `fetch_add()` for unique temp dir per test
- Zero-cost: subprocess startup ~100ms, subprocess teardown deletes temp dir

## MCP Protocol
- Newline-delimited JSON-RPC 2.0 over stdio
- Tools return structured JSON (e.g., `{ "name": "...", "version": "..." }`), not formatted strings
- Error codes: -32600 (invalid req), -32700 (parse), -32601 (method not found), -32602 (invalid params), -32603 (internal)

## Permissions Best Practices
- **Broad wildcards over specific rules**: `Bash(git *)` instead of listing 8 specific git commands; `Bash(cargo *)` instead of 3 specific cargo subcommands
- **Consolidation goal**: Reduce `.claude/settings.json` maintenance burden; audit allow list annually

## Phase 2 Architecture Patterns

### DaemonState
- `daemon_state.rs`: single `DaemonState` struct (`buf`, `drain`, `error_store`, `cwd`, `start_time`) — `Clone` is cheap, all fields are `Arc` or `Copy`
- Thread `DaemonState` through tasks by cloning; avoids growing function-argument lists

### Drain Algorithm (`scanners/drain.rs`)
- `DrainState { prefix_tree: HashMap<usize, Vec<LogCluster>> }` — keyed by token count
- Similarity = `matching_tokens / token_count`; threshold 0.5; wildcards stored as `*`
- `push_line_and_drain(buf, drain, line)` in `buffer.rs` keeps both in sync
- Cluster cap: 1000; evict oldest by `last_seen_ms`

### Stack Trace Parser (`scanners/stacktrace.rs`)
- State-machine per language: Rust panic / Python Traceback / Node.js Error / Java Exception
- Minimum 2 frames to avoid false positives
- `extract_source_files(traces)` → deduped list used for git diff cross-reference
- Python: check `lines[j].text.starts_with(' ')` (original, not trimmed) to detect indented code lines vs exception message

### Smart Git Diffs (`scanners/git.rs`)
- `get_changed_files`: runs `git diff --name-status HEAD` + `git diff --name-status --cached`
- `get_diff_hunks`: runs `git diff HEAD -U3 -- <files>`, parses unified diff output
- Cap: 50 hunks total, 30 lines/hunk; returns `truncated: bool`
- Intersection pattern: `error_files ∩ changed_files` = only relevant hunks

### Docker Monitoring (`docker/`)
- `bollard::Docker::connect_with_local_defaults()` — handles Windows named pipe automatically
- `docker/demux.rs`: 8-byte header `[stream_type(1), 0, 0, 0, size(4 BE)]`
- `docker/log_filter.rs`: JSON level detection → keep ERROR/WARN/FATAL; plain text stderr always kept
- `docker/error_store.rs`: `HashMap<String, VecDeque<ErrorEvent>>` per container, 500-entry cap
- Retry loop: 10s wait when Docker unavailable; `get_container_logs` returns `docker_available: false` (not error)

### Log Analysis & Graceful Fallbacks (`mcp/tools.rs`)
- **4 supported languages**: Rust panic / Python Traceback / Node.js Error / Java Exception (state-machine parsers, not regex)
- **Filtering**: ERROR/WARN/FATAL only; minimum 2 frames per trace to avoid single-line false positives; stdlib frames filtered per language
- **Drain deduplication**: Groups identical-length log lines by similarity (≥0.5 threshold); wildcard merging (`*` replaces differing tokens); 1000-cluster cap, FIFO eviction
- **Graceful fallback chain** (every tool guarantees non-empty response):
  - `get_contextual_diff` → diff hunks (if match) OR compressed_errors (if no match) OR terminal_buffer (if no clusters) OR `fallback_source: "none"`
  - `get_compressed_errors` → clusters + traces (if any) OR terminal_buffer (if empty) OR `fallback_source: "none"`
  - `get_container_logs` → events (if Docker running) OR compressed_errors (if Docker unavailable) OR terminal_buffer OR `fallback_source: "none"`
  - **Each response includes `fallback_source` field** so AI learns which source is most useful for specific parts of project
- **Intersection pattern for diffs**: Extract files from stack traces, cross-reference with dirty git files (not all project files) for surgical precision
- **Docker availability signal**: `docker_available: true` + empty events = connected but no errors; `docker_available: false` = not reachable (triggers fallback)

## Phase 3 Diagnostic Fixes (Session 2026-04-18)

### Windows Path Normalization Bug (CRITICAL)
- **Pattern**: Path::canonicalize() returns `\\?\C:\...` on Windows; cwd may be plain `C:\...`
- **Impact**: strip_prefix(cwd) fails on Windows when comparing diff files to git changed files
- **Fix**: Canonicalize both paths: `let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());` before strip_prefix
- **Location**: `scanners/git.rs` normalize_path() function; also in `mcp/tools.rs` get_contextual_diff intersection

### Untracked Files in git diff (CRITICAL for Phase 3)
- **Pattern**: `git diff HEAD` doesn't show untracked files; diff shows "No changed lines intersected"
- **Fix**: Add `git ls-files --others --exclude-standard` to get_changed_files(); in get_diff_hunks, fallback to `git diff --no-index -- /dev/null <file>` for files not in HEAD diff
- **Location**: `scanners/git.rs` get_changed_files() and get_diff_hunks(); results are all green (added lines)

### Stack Trace Parser Ordering (Node.js catches Java/Python)
- **Pattern**: Node.js parser contains("Error: ") matches Python ConnectionError and Java exception messages; consumes wrong frames
- **Fix**: Replace .contains() with is_nodejs_error_type() exhaustive enum (Error, TypeError, ReferenceError, SyntaxError, RangeError, InternalError, AggregateError)
- **Also fix**: Python parser must return (trace, next_i) tuple; callers use returned next_i not frames.len() magic
- **Location**: `scanners/stacktrace.rs` try_parse_nodejs, is_nodejs_error_type(), try_parse_python signature

### Python Single-Frame Stack Traces
- **Pattern**: Python errors with 1 frame (e.g., NameError: x not defined) rejected by min_frames < 2 check
- **Fix**: Change `if frames.len() < 2` to `if frames.is_empty()` in try_parse_python
- **Location**: `scanners/stacktrace.rs` try_parse_python, line ~120

### inject_log Multiline Support
- **Pattern**: Custom payload with newlines sent as single LogLine; stack trace parser can't detect multi-line traces
- **Fix**: Split on '\n', call push_line_and_drain for each (not just push_line); updates both buffer and Drain state
- **Location**: `admin_api.rs` inject_log handler, `buffer.rs` push_line_and_drain function

## Phase 3 Architecture Patterns

### ANSI State Machine (`scanners/ansi.rs`)
- Stateful parser: Normal → Esc → Csi/Osc states; handles split sequences across buffer boundaries
- Use for line-by-line input (stateless version ok); state persistence needed for streaming input
- Covers: CSI color codes (\x1b[31m), OSC terminal titles (\x1b]...\x07), charset designate (\x1b(B), BS/CR discard

### PII Masker with Entropy (`pii_masker.rs`)
- Regex patterns (OnceLock for compile-once): email, JWT (eyJ...), Bearer, CC (Luhn), password=value, AKIA* keys
- Entropy scanner: Shannon entropy > 4.5 bits/char AND len ≥ 20 AND !http && !:// → mask as <SECRET_MASKED>
- Applied in push_line AFTER ANSI strip (order matters: strip first, then mask)

### Typed Context System (`typed_context.rs`)
- Wraps untrusted terminal output: `<terminal_output source="..." untrusted="true" pii_masked="true">...</terminal_output>`
- Sanitizes XML: escapes </terminal_output>, <script, <iframe, <object to prevent injection
- Used in mcp/tools.rs for all output responses (signals to Claude that content is passive data, not instructions)

### New MCP Tools
- `get_postmortem(minutes: 1-1440)` - timeline analysis with error spikes by minute, stack traces, Docker events within window
- `get_correlated_errors(window_secs: u64)` - cross-references terminal errors with Docker events within time window; returns correlation list + fallback_source field
- Both tools follow graceful fallback pattern: primary data → compressed_errors → terminal_buffer → "none"

### Shell Hooks Integration
- **bash**: `trap '__blackbox_preexec' DEBUG` sending `$ <command>` via `/dev/tcp/$BLACKBOX_HOST:$BLACKBOX_PORT`
- **zsh**: `preexec()` hook, same mechanism
- **PowerShell**: PSReadLine key handler with persistent TcpClient for Enter key capture
- Requires: cwd contains shell-hooks/, users source in ~/.bashrc/~/.zshrc/$PROFILE with BLACKBOX_HOST/PORT env vars

## BlackBox Lab UI (Phase 3 React Frontend)

### Design System
- Theme: OLED dark (#0F172A bg, #22C55E CTA, accent tokens: --accent-cyan/red/orange/green)
- Typography: JetBrains Mono body + Inter headings; 16px base, line-height 1.5
- Token pattern: CSS custom properties (--text-primary, --border, --accent-*) in index.css

### Component Architecture
- `LogExplorer`: 4 tabs (Raw, Compressed, Docker, Postmortem) with tab-btn/.tab-row style classes
- Tab state preserved via activeTab useState; scroll position preserved if scroll ≥ content_len - 1
- `InjectionStation`: Textarea (not input) for multiline; Ctrl+Enter to inject; line counter display
- `Header`: uptime display (formatUptime helper), dirty file count, status indicator (offline/nominal/errors)

### Integration with Daemon API
- Polling interval: 3s (balance between responsiveness and load)
- Endpoints: /api/status (uptime/git), /api/terminal, /api/compressed, /api/docker, /api/postmortem, /api/correlated, /api/inject, /api/clear
- Error handling: daemonOnline state; offline UI gracefully degrades (show "Daemon offline" message, buttons disabled)

## Phase 3 Testing & Verification

### Cargo Build
- Full workspace: `cargo build --workspace`; check for `error[` lines only (warnings acceptable for this phase)
- Run daemon: `cargo run -p blackbox-daemon -- --cwd .` (verify starts without panic)

### Manual Test Scenarios (blackbox-lab)
- Rust panic injection → stack trace appears in Analyzed tab with rust badge
- Python traceback (single frame) → appears in Analyzed (previously missed due to min_frames < 2)
- Java exception → correct java badge (not nodejs; requires parser ordering fix)
- PII payload → email/JWT/card/token all masked in Raw tab
- Untracked file error → Contextual Diff shows diff hunks (requires Windows path fix + git ls-files)
- Postmortem request → timeline shows error spikes by minute_offset
- Large log block paste → textarea handles multiline; Ctrl+Enter injects as separate lines

### Known Test Blockers
- Docker monitoring requires docker daemon running; without it, /api/docker returns `docker_available: false` (not error)
- Windows: Path canonicalization critical; always test git diffs with untracked files on Windows
- Java vs Node.js detection fragile; test with real exception types not generic "Error:" strings

## Diagnostics & Gotchas

### Windows File Corruption
- **Symptom**: `git diff` shows `Binary files differ` but file looks normal in editor
- **Cause**: VS Code or system appends UTF-16 encoded bytes (e.g., `// test\n` becomes `2F 00 2F 00 20 00 74 00 65 00 73 00 74 00 0A 00 0A 00`)
- **Fix**: Use PowerShell `[System.IO.File]::ReadAllBytes("path")` to find where clean bytes end (look for last `0A 0A` = `\n\n`), truncate to that point, write back as UTF-8
- **Prevention**: Don't manually append test code to files; use proper test injection via `blackbox-lab` inject station

### Windows .exe File Lock on Rebuild
- **Symptom**: `cargo build` fails with "Access is denied" when daemon/sandbox is running
- **Fix**: Run `Stop-Process -Name "blackbox-daemon","blackbox-sandbox" -Force -ErrorAction SilentlyContinue` via PowerShell before rebuild
- **Note**: Bash shell (`/usr/bin/bash`) cannot run PowerShell commands; use PowerShell tool directly

### git diff Validation
- Always verify `git diff HEAD -- <file>` returns text output (contains `+++ b/` and `@@ ` hunk headers), not `Binary files differ`
- If binary, file is either actually binary or has corruption; check with PowerShell `ReadAllBytes` before parsing

### Drain Bucketing Constraint
- Similarity calculation only meaningful within same token-count bucket (e.g., 4-token lines grouped separately from 5-token lines)
- Different-length lines always spawn separate clusters (not a bug; inherent to Drain algorithm)
- Example: `"error: timeout to 10.0.0.1"` (4 tokens) never merges with `"error: timeout connecting to 10.0.0.1"` (5 tokens)

## Dependencies
- `gix` over `git2-rs`: pure Rust, no libgit2 C bindings, simpler MSRV
- `ratatui 0.29+` for TUI, `crossterm 0.28+` for terminal control
- `bollard 0.17` for Docker Engine API (Unix socket + Windows named pipe)
- `futures-util 0.3` for `StreamExt` trait on bollard log streams
