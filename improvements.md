# BlackBox — Technical Improvements & Debt Log

## Phase 1

- [ ] Ring buffer uses `Arc<RwLock<VecDeque>>` — upgrade to lock-free Disruptor pattern in Phase 3
- [ ] ANSI stripping via regex is a `@TODO [Future Upgrade]` — replace with proper state-machine parser in Phase 3
- [ ] Stack trace detection in terminal buffer uses basic regex — replace with tree-sitter in Phase 2 (`@TODO [Future Upgrade]`)
- [ ] VS Code extension TCP reconnect uses simple backoff — consider exponential backoff with jitter
- [ ] `get_project_metadata` scanner is synchronous blocking I/O inside async context — wrap with `tokio::task::spawn_blocking` in future
