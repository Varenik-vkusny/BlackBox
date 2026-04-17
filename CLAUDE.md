# BlackBox — Phase 1 Workspace Guide

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
| **XML injection guard** | Escape `</terminal_output>` in wrapped output | ✅ |
| **Status TUI (blackbox-tui)** | ratatui dashboard polling status_server port 8766 | ✅ |
| **Interactive sandbox** | 5-tab TUI for manual testing without AI agent (bonus) | ✅ |

## Phase Status
- **Phase 1**: ✅ Complete (MVP daemon + VS Code bridge + sandbox)
- **Phase 2**: Docker, smart git diffs, compression (not started)
- **Phase 3**: OS-level PTY interception, PII masking (not started)

## Cargo Workspace Pattern
- Workspace root: `[workspace.package]` with version, edition = "2021", rust-version = "1.77"
- All crates: use `{ workspace = true }` for serde, tokio, etc.
- Three crates: blackbox-core (types lib), blackbox-daemon (MCP binary), blackbox-tui (ratatui binary), blackbox-sandbox (interactive testing TUI)

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

## Interactive Sandbox
- `blackbox-sandbox`: 5-tab TUI (Logs, Snapshot, Metadata, File, Inject)
- Manual tool calling without AI agent, reusable across Phase 2/3
- Spawn daemon internally, own stdin/stdout pipes

## Sandbox UI Patterns
- **Scroll preservation**: Check `scroll >= content_len - 1` before auto-refresh to preserve user scroll; only pin to bottom if already there
- **Multi-line input**: Accept `\n` literals in text fields for blocks (split on `\n` before sending)
- **File range syntax**: `path:N` = 20-line window around line N; `path:N-M` = explicit range (use `rsplitn(2, ':')` to avoid Windows drive letter issues)
- **Empty manifest name**: Workspace `Cargo.toml` has no `[package]` — display `(workspace root — no [package])` instead of blank

## Permissions Best Practices
- **Broad wildcards over specific rules**: `Bash(git *)` instead of listing 8 specific git commands; `Bash(cargo *)` instead of 3 specific cargo subcommands
- **Consolidation goal**: Reduce `.claude/settings.json` maintenance burden; audit allow list annually

## Dependencies
- `gix` over `git2-rs`: pure Rust, no libgit2 C bindings, simpler MSRV
- `ratatui 0.29+` for TUI, `crossterm 0.28+` for terminal control
