# BlackBox Dashboard Test Data Injection (PowerShell)
# Run with: powershell -ExecutionPolicy Bypass -File test-data-inject.ps1

$DAEMON_URL = "http://127.0.0.1:8768/api"
$count = 0

function Inject-Log {
    param([string]$Message)

    $script:count++
    Write-Host "[$script:count] Injecting: $Message ... " -NoNewline -ForegroundColor Cyan

    try {
        $payload = @{ text = $Message } | ConvertTo-Json -Compress
        $response = Invoke-RestMethod -Uri "$DAEMON_URL/inject" -Method Post `
            -Headers @{ "Content-Type" = "application/json" } `
            -Body $payload -ErrorAction Stop
        Write-Host "OK" -ForegroundColor Green
    } catch {
        Write-Host "FAIL" -ForegroundColor Red
        Write-Host "Error: Check if daemon is running on 127.0.0.1:8768" -ForegroundColor Yellow
        exit 1
    }
}

Write-Host ""
Write-Host "BlackBox Test Data Injection" -ForegroundColor Cyan
Write-Host "============================" -ForegroundColor Cyan
Write-Host ""

# 1. Normal startup logs
Write-Host "Injecting normal startup logs..." -ForegroundColor Yellow
Inject-Log "2024-04-20 10:15:23.456 [INFO] BlackBox daemon started"
Inject-Log "2024-04-20 10:15:23.789 [INFO] Listening on 127.0.0.1:8768"
Inject-Log "2024-04-20 10:15:24.012 [INFO] Loaded 3 MCP tools"
Inject-Log "2024-04-20 10:15:24.345 [DEBUG] Terminal bridge connected from vscode_bridge"

# 2. Rust panic
Write-Host ""
Write-Host "Injecting Rust panic..." -ForegroundColor Yellow
Inject-Log "thread 'main' panicked at 'Database connection failed: Connection refused (os error 111)', src/test-fixtures/database.rs:28:20"
Inject-Log "stack backtrace:"
Inject-Log "   0: rust_begin_unwind"
Inject-Log "             at /rustc/07dbd53c5/library/std/src/panicking.rs:647:5"
Inject-Log "   1: core::panicking::panic_fmt"
Inject-Log "             at /rustc/07dbd53c5/library/core/src/panicking.rs:72:14"
Inject-Log "   2: blackbox_daemon::database::Database::connect"
Inject-Log "             at ./src/test-fixtures/database.rs:28:20"
Inject-Log "   3: blackbox_daemon::main"
Inject-Log "             at ./src/main.rs:42:15"

# 3. Python traceback
Write-Host ""
Write-Host "Injecting Python traceback..." -ForegroundColor Yellow
Inject-Log "Traceback (most recent call last):"
Inject-Log "  File src/test-fixtures/processor.py, line 19, in process_batch"
Inject-Log "    cursor.execute()"
Inject-Log "  File env/lib/python3.11/site-packages/psycopg2/__init__.py, line 130, in connect"
Inject-Log "    conn = _connect(dsn, connection_factory=connection_factory, **kwds)"
Inject-Log "psycopg2.OperationalError: could not connect to server: Connection refused"
Inject-Log "  Is the server running on host localhost (127.0.0.1) port 5432?"

# 4. Node.js error
Write-Host ""
Write-Host "Injecting Node.js error..." -ForegroundColor Yellow
Inject-Log "Error: connect ECONNREFUSED 127.0.0.1:8080"
Inject-Log "    at TCPConnectWrap.afterConnect [as oncomplete] (net.js:1148:10)"
Inject-Log "    at async apiClient.post (src/test-fixtures/server.js:16:8)"
Inject-Log "    at async /app/routes/webhook.js:42:15"

# 5. Repeated connection refused (for dedup testing)
Write-Host ""
Write-Host "Injecting repeated connection errors..." -ForegroundColor Yellow
for ($i = 1; $i -le 8; $i++) {
    Inject-Log "error: connection refused to 127.0.0.1:5432 - attempt $i/10"
}

# 6. Mixed workload
Write-Host ""
Write-Host "Injecting mixed workload..." -ForegroundColor Yellow
Inject-Log "[2024-04-20 10:15:45.123] Processing batch #1 (125 items)"
Inject-Log "[2024-04-20 10:15:45.456] Batch #1 committed successfully"
Inject-Log "[2024-04-20 10:15:46.789] Processing batch #2 (98 items)"
Inject-Log "[2024-04-20 10:15:47.012] warn: slow query detected (1523ms)"
Inject-Log "[2024-04-20 10:15:47.345] error: batch #2 commit failed - retry 1"
Inject-Log "[2024-04-20 10:15:48.678] error: batch #2 commit failed - retry 2"
Inject-Log "[2024-04-20 10:15:49.901] error: batch #2 commit failed - giving up"

# 7. Docker errors
Write-Host ""
Write-Host "Injecting container error logs..." -ForegroundColor Yellow
Inject-Log "nginx-prod-001 | 2024/04/20 10:15:50 [error] 1024#1024: *1 connect() failed (111: Connection refused)"
Inject-Log "postgres-db-001 | ERROR: could not open file pg_xlog/000000010000000000000001: No such file or directory"
Inject-Log "redis-cache-001 | # oO0OoO0OoO0Oo Redis is starting oO0OoO0OoO0Oo"
Inject-Log "redis-cache-001 | WARNING: no config file specified, using the default config"

# 8. HTTP patterns
Write-Host ""
Write-Host "Injecting HTTP error patterns..." -ForegroundColor Yellow
Inject-Log "GET /api/users/123 - 404 Not Found (23ms)"
Inject-Log "POST /api/events - 500 Internal Server Error (1234ms)"
Inject-Log "GET /api/health - 200 OK (2ms)"
Inject-Log "PUT /api/config - 401 Unauthorized (45ms)"

Write-Host ""
Write-Host "Test data injection complete! ($count logs injected)" -ForegroundColor Green
Write-Host ""
Write-Host "Dashboard should now show:" -ForegroundColor Cyan
Write-Host "  - 1 Rust panic (stack trace)" -ForegroundColor Gray
Write-Host "  - 1 Python error (traceback)" -ForegroundColor Gray
Write-Host "  - 1 Node.js error" -ForegroundColor Gray
Write-Host "  - [x8] Deduplicated connection refused" -ForegroundColor Gray
Write-Host "  - Mixed normal logs" -ForegroundColor Gray
Write-Host "  - Container errors" -ForegroundColor Gray
Write-Host "  - HTTP patterns" -ForegroundColor Gray
Write-Host ""
Write-Host "Open http://localhost:5173 to see the dashboard" -ForegroundColor Cyan
