# BlackBox daemon startup script
# Run this once before using Antigravity or blackbox-lab.
# The daemon listens on:
#   8765 - VS Code terminal bridge (TCP)
#   8766 - status server (TUI)
#   8768 - HTTP API + MCP endpoint (Antigravity: http://127.0.0.1:8768/mcp)
#   8769 - HTTP proxy logger (set HTTP_PROXY=http://127.0.0.1:8769)

param(
    [string]$Cwd = $PSScriptRoot,
    [switch]$Release
)

$build = if ($Release) { "release" } else { "debug" }
$exe = Join-Path $PSScriptRoot "target\$build\blackbox-daemon.exe"

# Build if binary doesn't exist
if (-not (Test-Path $exe)) {
    Write-Host "Building blackbox-daemon ($build)..."
    $buildArgs = if ($Release) { @("build", "-p", "blackbox-daemon", "--release") } else { @("build", "-p", "blackbox-daemon") }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { Write-Error "Build failed"; exit 1 }
}

# Kill existing instance on port 8768 if already running
$existing = Get-NetTCPConnection -LocalPort 8768 -State Listen -ErrorAction SilentlyContinue
if ($existing) {
    $daemonPid = $existing.OwningProcess
    Write-Host "Stopping existing daemon (PID $daemonPid)..."
    Stop-Process -Id $daemonPid -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
}

Write-Host "Starting blackbox-daemon..."
Write-Host "  cwd    : $Cwd"
Write-Host "  binary : $exe"
Write-Host "  MCP    : http://127.0.0.1:8768/mcp"

Start-Process -FilePath $exe -ArgumentList "--cwd", $Cwd -WindowStyle Hidden

# Wait for port 8768 to become available
$attempts = 0
do {
    Start-Sleep -Milliseconds 300
    $ready = Get-NetTCPConnection -LocalPort 8768 -State Listen -ErrorAction SilentlyContinue
    $attempts++
} while (-not $ready -and $attempts -lt 20)

if ($ready) {
    Write-Host "Daemon is ready on port 8768." -ForegroundColor Green
} else {
    Write-Warning "Daemon did not start within 6 seconds. Check for errors."
}
