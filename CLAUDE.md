# BlackBox — Phase 1 Workspace Guide

## Phase Status
- **Phase 1 Complete**: MVP daemon (MCP server, ring buffer, TCP bridge, TUI, interactive sandbox)
- **Phase 2 Not Started**: Docker + smart compression + git diffs
- **Phase 3 Not Started**: PTY interception + PII masking

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

## Dependencies
- `gix` over `git2-rs`: pure Rust, no libgit2 C bindings, simpler MSRV
- `ratatui 0.29+` for TUI, `crossterm 0.28+` for terminal control
