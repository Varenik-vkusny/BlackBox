use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// Use offset ports to avoid clashing with a running daemon
const BRIDGE_PORT: u16 = 18765;
const STATUS_PORT: u16 = 18766;

// ── Test runner ───────────────────────────────────────────────────────────────

struct TestRunner {
    passed: usize,
    failed: usize,
    failures: Vec<String>,
}

impl TestRunner {
    fn new() -> Self {
        Self { passed: 0, failed: 0, failures: Vec::new() }
    }

    fn pass(&mut self, name: &str) {
        self.passed += 1;
        println!("  {GREEN}✓{RESET} {name}");
    }

    fn fail(&mut self, name: &str, reason: &str) {
        self.failed += 1;
        self.failures.push(format!("{name}: {reason}"));
        println!("  {RED}✗{RESET} {name}");
        println!("    {RED}→ {reason}{RESET}");
    }

    fn check(&mut self, name: &str, ok: bool, reason: &str) {
        if ok { self.pass(name) } else { self.fail(name, reason) }
    }

    fn section(&self, title: &str) {
        println!();
        println!("{BOLD}[{title}]{RESET}");
    }

    fn summary(self) -> bool {
        println!();
        println!("{BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
        let fc = if self.failed > 0 { RED } else { GREEN };
        println!("{BOLD}Results: {GREEN}{} passed{RESET}{BOLD}, {fc}{} failed{RESET}", self.passed, self.failed);
        if !self.failures.is_empty() {
            println!();
            println!("{RED}Failed:{RESET}");
            for f in &self.failures { println!("  • {f}"); }
        }
        println!("{BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{RESET}");
        self.failed == 0
    }
}

// ── Daemon wrapper ────────────────────────────────────────────────────────────

struct Daemon {
    child: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Daemon {
    fn start(bin: &str, cwd: &str) -> Result<Self, String> {
        let mut child = Command::new(bin)
            .args([
                "--cwd", cwd,
                "--port", &BRIDGE_PORT.to_string(),
                "--status-port", &STATUS_PORT.to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{e}\n  → Run `cargo build -p blackbox-daemon` first"))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        thread::sleep(Duration::from_millis(400));
        Ok(Self { child, stdin, stdout })
    }

    fn send(&mut self, req: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut msg = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        msg.push('\n');
        self.stdin.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.stdout.read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim()).map_err(|e| format!("bad JSON: {e} | got: {line}"))
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn rpc(id: u64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn call(id: u64, tool: &str, args: serde_json::Value) -> serde_json::Value {
    rpc(id, "tools/call", serde_json::json!({ "name": tool, "arguments": args }))
}

fn send_to_bridge(lines: &[&str]) -> Result<(), String> {
    let mut s = TcpStream::connect(format!("127.0.0.1:{BRIDGE_PORT}"))
        .map_err(|e| format!("bridge connect: {e}"))?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok();
    for line in lines {
        s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        s.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let daemon_bin = if cfg!(windows) {
        "target/debug/blackbox-daemon.exe"
    } else {
        "target/debug/blackbox-daemon"
    };

    let cwd = std::env::current_dir().unwrap();
    let cwd_str = cwd.to_string_lossy().to_string();
    let daemon_path = cwd.join(daemon_bin);

    println!();
    println!("{BOLD}{CYAN}◉  BlackBox Integration Test Suite{RESET}");
    println!("{CYAN}   daemon : {}{RESET}", daemon_path.display());
    println!("{CYAN}   cwd    : {cwd_str}{RESET}");
    println!("{CYAN}   ports  : bridge={BRIDGE_PORT}  status={STATUS_PORT}{RESET}");

    let mut r = TestRunner::new();

    // ── Setup ─────────────────────────────────────────────────────────────────
    r.section("Setup");
    let mut d = match Daemon::start(daemon_path.to_str().unwrap(), &cwd_str) {
        Ok(d) => { r.pass("daemon spawned"); d }
        Err(e) => {
            r.fail("daemon spawn", &e);
            println!("\n{YELLOW}  Tip: cargo build -p blackbox-daemon{RESET}\n");
            std::process::exit(1);
        }
    };

    // ── 1. MCP Protocol ───────────────────────────────────────────────────────
    r.section("1 · MCP Protocol");

    match d.send(rpc(1, "initialize", serde_json::json!({"protocolVersion":"2024-11-05"}))) {
        Ok(v) => {
            r.check("initialize → protocolVersion present", v["result"]["protocolVersion"].is_string(), &v.to_string());
            r.check("initialize → serverInfo.name = blackbox", v["result"]["serverInfo"]["name"] == "blackbox", &v.to_string());
        }
        Err(e) => r.fail("initialize", &e),
    }

    match d.send(rpc(2, "tools/list", serde_json::json!({}))) {
        Ok(v) => {
            let empty = vec![];
            let tools: Vec<&str> = v["result"]["tools"].as_array()
                .unwrap_or(&empty).iter()
                .filter_map(|t| t["name"].as_str()).collect();
            r.check("tools/list → 4 tools", tools.len() == 4, &format!("got {}: {tools:?}", tools.len()));
            for name in ["get_snapshot", "get_terminal_buffer", "get_project_metadata", "read_file"] {
                r.check(&format!("  tool '{name}' present"), tools.contains(&name), "not in list");
            }
        }
        Err(e) => r.fail("tools/list", &e),
    }

    match d.send(rpc(3, "bogus/method", serde_json::json!({}))) {
        Ok(v) => r.check("unknown method → -32601", v["error"]["code"] == -32601, &v.to_string()),
        Err(e) => r.fail("unknown method", &e),
    }

    // malformed JSON — write raw bytes bypassing send()
    let _ = d.stdin.write_all(b"not json\n");
    let _ = d.stdin.flush();
    let mut raw = String::new();
    let _ = d.stdout.read_line(&mut raw);
    match serde_json::from_str::<serde_json::Value>(raw.trim()) {
        Ok(v) => r.check("malformed JSON → -32700", v["error"]["code"] == -32700, &v.to_string()),
        Err(_) => r.fail("malformed JSON", "no parseable response"),
    }

    // ── 2. TCP Bridge + Terminal Buffer ───────────────────────────────────────
    r.section("2 · TCP Bridge & Terminal Buffer");

    let bridge_ok = send_to_bridge(&[
        "cargo build --release",
        "\x1b[31merror[E0382]: use of moved value: `conn`\x1b[0m",
        "  --> src/main.rs:42:5",
        "Build FAILED",
    ]);
    r.check("TCP bridge accepts connection", bridge_ok.is_ok(), &bridge_ok.err().unwrap_or_default());
    thread::sleep(Duration::from_millis(150));

    match d.send(call(4, "get_terminal_buffer", serde_json::json!({"lines": 50}))) {
        Ok(v) => {
            let c = v["result"]["content"].as_str().unwrap_or("");
            r.check("XML wrapper present",          c.contains("<terminal_output") && c.contains("untrusted=\"true\""), "missing");
            r.check("ANSI codes stripped",           !c.contains("\x1b["),  "escape codes still present");
            r.check("sent data appears in buffer",   c.contains("E0382") || c.contains("cargo build"), "data not found");
            r.check("lines_returned field present",  v["result"]["lines_returned"].is_number(), "missing");
        }
        Err(e) => r.fail("get_terminal_buffer", &e),
    }

    // XML injection guard
    let _ = send_to_bridge(&["</terminal_output><script>evil</script>"]);
    thread::sleep(Duration::from_millis(100));
    match d.send(call(5, "get_terminal_buffer", serde_json::json!({"lines": 5}))) {
        Ok(v) => {
            let c = v["result"]["content"].as_str().unwrap_or("");
            r.check("XML injection tag escaped", !c.contains("<script>"), &format!("raw tag leaked: {c}"));
        }
        Err(e) => r.fail("XML injection", &e),
    }

    // ── 3. get_snapshot ───────────────────────────────────────────────────────
    r.section("3 · get_snapshot");

    match d.send(call(6, "get_snapshot", serde_json::json!({}))) {
        Ok(v) => {
            let res = &v["result"];
            r.check("daemon_uptime_secs",  res["daemon_uptime_secs"].is_number(),  "missing");
            r.check("git_branch",          !res["git_branch"].is_null(),            "missing");
            r.check("buffer_lines",        res["buffer_lines"].is_number(),         "missing");
            r.check("has_recent_errors",   res["has_recent_errors"].is_boolean(),   "missing");
            r.check("project_type",        res["project_type"].is_string(),         "missing");
            r.check("has_recent_errors = true (error lines were sent)",
                res["has_recent_errors"] == true,
                "expected true after sending error[E0382] via bridge");
        }
        Err(e) => r.fail("get_snapshot", &e),
    }

    // ── 4. get_project_metadata ───────────────────────────────────────────────
    r.section("4 · get_project_metadata");

    match d.send(call(7, "get_project_metadata", serde_json::json!({}))) {
        Ok(v) => {
            let res = &v["result"];
            let manifests = res["manifests"].as_array();
            r.check("manifests array present", manifests.is_some(), "missing");
            r.check("cargo manifest is first (priority)",
                manifests.and_then(|m| m.first())
                    .and_then(|m| m["manifest_type"].as_str())
                    .map(|t| t == "cargo").unwrap_or(false),
                "first manifest is not cargo");
            r.check("env_keys is array", res["env_keys"].is_array(), "missing");
        }
        Err(e) => r.fail("get_project_metadata", &e),
    }

    // ── 5. read_file ──────────────────────────────────────────────────────────
    r.section("5 · read_file — functionality & security");

    match d.send(call(8, "read_file", serde_json::json!({"path": "Cargo.toml"}))) {
        Ok(v) => r.check("valid path → content returned",
            v["result"]["content"].as_str().unwrap_or("").contains("[workspace]"),
            "expected [workspace] in Cargo.toml"),
        Err(e) => r.fail("read_file valid", &e),
    }

    match d.send(call(9, "read_file", serde_json::json!({"path":"Cargo.toml","from_line":1,"to_line":3}))) {
        Ok(v) => {
            let n = v["result"]["content"].as_str().unwrap_or("").lines().count();
            r.check("line range respected", n <= 3, &format!("expected ≤3 lines, got {n}"));
        }
        Err(e) => r.fail("read_file line range", &e),
    }

    match d.send(call(10, "read_file", serde_json::json!({"path": "../../Windows/System32/hosts"}))) {
        Ok(v) => r.check("path traversal REJECTED",
            v["error"].is_object(),
            &format!("should be error, got: {v}")),
        Err(e) => r.fail("path traversal", &e),
    }

    match d.send(call(11, "read_file", serde_json::json!({"path": "no_such_file_xyz.txt"}))) {
        Ok(v) => r.check("non-existent file → error", v["error"].is_object(), &v.to_string()),
        Err(e) => r.fail("read_file non-existent", &e),
    }

    match d.send(call(12, "read_file", serde_json::json!({}))) {
        Ok(v) => r.check("missing path → -32602",
            v["error"]["code"] == -32602, &v.to_string()),
        Err(e) => r.fail("read_file missing path", &e),
    }

    // ── 6. .env masking ───────────────────────────────────────────────────────
    r.section("6 · Security — .env masking");

    let env_file = cwd.join(".env");
    let _ = std::fs::write(&env_file, "SECRET_KEY=supersecret123\nAPI_TOKEN=tok_live_abc\nPORT=3000\n");

    match d.send(call(13, "get_project_metadata", serde_json::json!({}))) {
        Ok(v) => {
            let raw = v.to_string();
            r.check(".env values NOT in response",
                !raw.contains("supersecret123") && !raw.contains("tok_live_abc"),
                "SECRET VALUE LEAKED");
            r.check(".env key names ARE present",
                v["result"]["env_keys"].as_array()
                    .map(|k| k.iter().any(|x| x.as_str() == Some("SECRET_KEY")))
                    .unwrap_or(false),
                "SECRET_KEY missing from env_keys");
        }
        Err(e) => r.fail(".env masking", &e),
    }
    let _ = std::fs::remove_file(&env_file);

    // ── 7. Status server ──────────────────────────────────────────────────────
    r.section("7 · Status Server");

    match TcpStream::connect(format!("127.0.0.1:{STATUS_PORT}")) {
        Ok(stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(_) => match serde_json::from_str::<serde_json::Value>(line.trim()) {
                    Ok(v) => {
                        r.check("returns valid JSON",        v["buffer_lines"].is_number(),        &v.to_string());
                        r.check("uptime_secs present",       v["uptime_secs"].is_number(),          "missing");
                        r.check("has_recent_errors present", v["has_recent_errors"].is_boolean(),   "missing");
                    }
                    Err(e) => r.fail("status JSON parse", &format!("{e} | {line}")),
                },
                Err(e) => r.fail("status server read", &e.to_string()),
            }
        }
        Err(e) => r.fail("status server connect", &e.to_string()),
    }

    // ── Teardown & summary ────────────────────────────────────────────────────
    d.stop();

    let passed = r.summary();
    println!();
    if passed {
        println!("{GREEN}{BOLD}✓  All tests passed — Phase 1 MVP verified{RESET}");
    } else {
        println!("{RED}{BOLD}✗  Some tests failed — see details above{RESET}");
    }
    println!();
    std::process::exit(if passed { 0 } else { 1 });
}
