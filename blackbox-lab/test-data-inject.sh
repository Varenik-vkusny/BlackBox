#!/bin/bash

# BlackBox Dashboard Test Data Injection Script
# This script injects various log types to test the dashboard

DAEMON_URL="http://127.0.0.1:8768/api"

echo "🔧 BlackBox Test Data Injection"
echo "================================"
echo ""

# Helper function to inject logs
inject() {
    local msg="$1"
    echo -n "Injecting: $msg ... "
    local resp=$(curl -s -X POST "$DAEMON_URL/inject" \
        -H "Content-Type: application/json" \
        -d "{\"text\": \"$msg\"}" \
        2>/dev/null)
    if [ $? -eq 0 ]; then
        echo "✓"
    else
        echo "✗ (check if daemon is running on 8768)"
        exit 1
    fi
}

# 1. Normal startup logs
echo ""
echo "📝 Injecting normal startup logs..."
inject "2024-04-20 10:15:23.456 [INFO] BlackBox daemon started"
inject "2024-04-20 10:15:23.789 [INFO] Listening on 127.0.0.1:8768"
inject "2024-04-20 10:15:24.012 [INFO] Loaded 3 MCP tools"
inject "2024-04-20 10:15:24.345 [DEBUG] Terminal bridge connected from vscode_bridge"

# 2. Rust panic (with fake stack trace)
echo ""
echo "🚨 Injecting Rust panic..."
inject "thread 'main' panicked at 'Database connection failed: Connection refused (os error 111)', src/test-fixtures/database.rs:28:20"
inject "stack backtrace:"
inject "   0: rust_begin_unwind"
inject "             at /rustc/07dbd53c5/library/std/src/panicking.rs:647:5"
inject "   1: core::panicking::panic_fmt"
inject "             at /rustc/07dbd53c5/library/core/src/panicking.rs:72:14"
inject "   2: blackbox_daemon::database::Database::connect"
inject "             at ./src/test-fixtures/database.rs:28:20"
inject "   3: blackbox_daemon::main"
inject "             at ./src/main.rs:42:15"

# 3. Python traceback
echo ""
echo "🐍 Injecting Python traceback..."
inject "Traceback (most recent call last):"
inject "  File \"src/test-fixtures/processor.py\", line 19, in process_batch"
inject "    cursor.execute("
inject "  File \"env/lib/python3.11/site-packages/psycopg2/__init__.py\", line 130, in connect"
inject "    conn = _connect(dsn, connection_factory=connection_factory, **kwds)"
inject "psycopg2.OperationalError: could not connect to server: Connection refused"
inject "\tIs the server running on host \"localhost\" (127.0.0.1) port 5432?"

# 4. Node.js error
echo ""
echo "🟨 Injecting Node.js error..."
inject "Error: connect ECONNREFUSED 127.0.0.1:8080"
inject "    at TCPConnectWrap.afterConnect [as oncomplete] (net.js:1148:10)"
inject "    at async apiClient.post (src/test-fixtures/server.js:16:8)"
inject "    at async /app/routes/webhook.js:42:15"

# 5. Repeated connection refused (for dedup badge testing)
echo ""
echo "🔁 Injecting repeated connection errors (testing [×N] dedup)..."
for i in {1..8}; do
    inject "error: connection refused to 127.0.0.1:5432 - attempt $i/10"
done

# 6. Normal processing logs mixed with errors
echo ""
echo "📊 Injecting mixed workload..."
inject "[2024-04-20 10:15:45.123] Processing batch #1 (125 items)"
inject "[2024-04-20 10:15:45.456] Batch #1 committed successfully"
inject "[2024-04-20 10:15:46.789] Processing batch #2 (98 items)"
inject "[2024-04-20 10:15:47.012] warn: slow query detected (1523ms)"
inject "[2024-04-20 10:15:47.345] error: batch #2 commit failed - retry 1"
inject "[2024-04-20 10:15:48.678] error: batch #2 commit failed - retry 2"
inject "[2024-04-20 10:15:49.901] error: batch #2 commit failed - giving up"

# 7. Docker-like stderr simulation
echo ""
echo "🐳 Injecting container error logs..."
inject "nginx-prod-001 | 2024/04/20 10:15:50 [error] 1024#1024: *1 connect() failed (111: Connection refused) while connecting to upstream"
inject "postgres-db-001 | ERROR: could not open file \"pg_xlog/000000010000000000000001\": No such file or directory"
inject "redis-cache-001 | # oO0OoO0OoO0Oo Redis is starting oO0OoO0OoO0Oo"
inject "redis-cache-001 | WARNING: no config file specified, using the default config"

# 8. Some HTTP-like patterns
echo ""
echo "🌐 Injecting HTTP error patterns..."
inject "GET /api/users/123 - 404 Not Found (23ms)"
inject "POST /api/events - 500 Internal Server Error (1234ms)"
inject "GET /api/health - 200 OK (2ms)"
inject "PUT /api/config - 401 Unauthorized (45ms)"

# 9. Final summary
echo ""
echo "✅ Test data injection complete!"
echo ""
echo "📊 Dashboard should now show:"
echo "   • 1 Rust panic (stack trace)"
echo "   • 1 Python error (traceback)"
echo "   • 1 Node.js error"
echo "   • [×8] Deduplicated connection refused"
echo "   • Mixed normal logs"
echo "   • Container errors"
echo "   • HTTP patterns"
echo ""
echo "🔍 Open the dashboard at http://localhost:5173"
echo "   and test:"
echo "   - Overview dashboard with service cards"
echo "   - Click on 'vscode_bridge' → Triage view"
echo "   - Click a stack trace → Inspect Changes (diff)"
echo "   - [×8] dedup badge for repeated messages"
echo "   - Switched to Raw Logs to see all entries"
echo ""
