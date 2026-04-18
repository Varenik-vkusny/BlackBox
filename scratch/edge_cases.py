"""
BlackBox MCP Edge Case Test Runner
Tests: Rust panic, Python traceback, Node.js error, Java exception,
       PII masking, ANSI stripping, Drain deduplication, empty buffer fallback
"""
import json
import urllib.request
import time

BASE = "http://127.0.0.1:8768"

def inject(lines: list[str]):
    # API expects {"text": "..."} — splits on \n internally
    combined = "\n".join(lines)
    data = json.dumps({"text": combined}).encode()
    req = urllib.request.Request(
        f"{BASE}/api/inject",
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        assert r.status == 200, f"inject failed: {r.status}"

def clear():
    req = urllib.request.Request(f"{BASE}/api/clear", data=b"", method="POST")
    with urllib.request.urlopen(req, timeout=5) as r:
        assert r.status == 200

def section(title: str):
    print(f"\n{'='*60}")
    print(f"  {title}")
    print('='*60)

# ── 1. Rust Panic ─────────────────────────────────────────────
section("EDGE CASE 1: Rust panic (multi-frame)")
clear()
inject([
    "thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 5', src/main.rs:42:5",
    "stack backtrace:",
    "   0: rust_begin_unwind",
    "             at /rustc/abc/library/std/src/panicking.rs:617:5",
    "   1: core::panicking::panic_fmt",
    "             at /rustc/abc/library/core/src/panicking.rs:67:14",
    "   2: myapp::process_data",
    "             at src/main.rs:42:5",
    "   3: myapp::main",
    "             at src/main.rs:10:5",
    "note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.",
])
time.sleep(0.3)
print("injected Rust panic")

# ── 2. Python Traceback (single frame — edge case for min_frames fix) ─
section("EDGE CASE 2: Python single-frame traceback (min_frames fix)")
inject([
    "Traceback (most recent call last):",
    '  File "app.py", line 7, in <module>',
    "    result = undefined_var",
    "NameError: name 'undefined_var' is not defined",
])
time.sleep(0.3)
print("injected Python single-frame trace")

# ── 3. Python multi-frame ─────────────────────────────────────
section("EDGE CASE 3: Python multi-frame traceback")
inject([
    "Traceback (most recent call last):",
    '  File "server.py", line 45, in handle_request',
    "    response = process(data)",
    '  File "processor.py", line 12, in process',
    "    conn = db.connect()",
    '  File "db.py", line 8, in connect',
    "    raise ConnectionError('DB unreachable')",
    "ConnectionError: DB unreachable",
])
time.sleep(0.3)
print("injected Python multi-frame trace")

# ── 4. Node.js Error (NOT Python/Java — parser ordering edge case) ─
section("EDGE CASE 4: Node.js TypeError (parser ordering check)")
inject([
    "TypeError: Cannot read properties of undefined (reading 'map')",
    "    at processItems (/app/src/utils.js:23:18)",
    "    at handleRequest (/app/src/server.js:87:5)",
    "    at Layer.handle [as handle_request] (/app/node_modules/express/lib/router/layer.js:95:5)",
    "    at next (/app/node_modules/express/lib/router/route.js:144:13)",
])
time.sleep(0.3)
print("injected Node.js TypeError")

# ── 5. Java Exception (should NOT be caught by Node.js parser) ─
section("EDGE CASE 5: Java NullPointerException (parser ordering)")
inject([
    "java.lang.NullPointerException: Cannot invoke method toString() on null",
    "\tat com.example.app.UserService.getUser(UserService.java:42)",
    "\tat com.example.app.UserController.handleGet(UserController.java:18)",
    "\tat sun.reflect.NativeMethodAccessorImpl.invoke0(Native Method)",
    "\tat java.lang.reflect.Method.invoke(Method.java:498)",
])
time.sleep(0.3)
print("injected Java NullPointerException")

# ── 6. PII Masking edge cases ─────────────────────────────────
section("EDGE CASE 6: PII masking (email, JWT, Bearer, CC, password)")
inject([
    "User login attempt: john.doe@example.com from 192.168.1.1",
    "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    "DB password=supersecretpassword123 used for connection",
    "AWS key: AKIAIOSFODNN7EXAMPLE detected in config",
    "Card number: 4532015112830366 charged $99.99",
])
time.sleep(0.3)
print("injected PII data")

# ── 7. ANSI color codes (should be stripped) ─────────────────
section("EDGE CASE 7: ANSI escape codes in logs")
inject([
    "\x1b[31mERROR\x1b[0m: Failed to connect to \x1b[33m10.0.0.1:5432\x1b[0m",
    "\x1b[1;32mSUCCESS\x1b[0m: Build completed in \x1b[36m2.4s\x1b[0m",
    "\x1b]0;user@host: ~/project\x07terminal title set",
])
time.sleep(0.3)
print("injected ANSI-colored logs")

# ── 8. Drain deduplication (same template, different values) ──
section("EDGE CASE 8: Drain deduplication — repeated similar errors")
inject([f"ERROR: connection timeout to 10.0.{i}.1 after 30s" for i in range(10)])
inject([f"WARN: retry {i+1}/3 for endpoint /api/users/{1000+i}" for i in range(8)])
time.sleep(0.3)
print("injected 10 timeout errors + 8 retry warnings for deduplication")

# ── 9. High-entropy secret (entropy scanner) ───────────────────
section("EDGE CASE 9: High-entropy string (should be masked)")
inject([
    "Config loaded: db_secret=xK9mP2nQvR5sT8wZ3yA6bC1dE4fG7hI0jL",
    "api_key=aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789abcd set in env",
])
time.sleep(0.3)
print("injected high-entropy secrets")

# ── 10. Mixed ERROR/WARN/INFO (filter check) ──────────────────
section("EDGE CASE 10: Mixed levels — only ERROR/WARN/FATAL should cluster")
inject([
    "INFO: Server started on port 3000",
    "DEBUG: Processing request GET /health",
    "INFO: Database connected successfully",
    "WARN: Memory usage at 78%, consider scaling",
    "ERROR: Request handler threw unhandled exception",
    "FATAL: OOM killer activated, process terminating",
    "INFO: Graceful shutdown initiated",
    "DEBUG: Cleanup complete",
])
time.sleep(0.3)
print("injected mixed log levels")

print("\n" + "="*60)
print("  All edge cases injected! Check MCP tools now.")
print("="*60)
