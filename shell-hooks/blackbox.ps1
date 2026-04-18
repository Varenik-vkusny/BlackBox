# BlackBox shell hook for PowerShell
#
# Intercepts each command entered at the prompt and sends it to the BlackBox
# daemon over TCP. Uses PSReadLine key binding — no external tools needed.
#
# Usage (add to your PowerShell profile):
#   . "C:\path\to\blackbox.ps1"
#
# To find your profile path: echo $PROFILE
#
# Environment variables:
#   BLACKBOX_HOST  (default: 127.0.0.1)
#   BLACKBOX_PORT  (default: 8765)

$BlackBoxHost = if ($env:BLACKBOX_HOST) { $env:BLACKBOX_HOST } else { '127.0.0.1' }
$BlackBoxPort = if ($env:BLACKBOX_PORT) { [int]$env:BLACKBOX_PORT } else { 8765 }

# Keep a persistent TCP client to avoid per-command connection overhead
$script:BlackBoxClient = $null

function BlackBox-Send {
    param([string]$Line)
    if ([string]::IsNullOrWhiteSpace($Line)) { return }
    try {
        if ($null -eq $script:BlackBoxClient -or -not $script:BlackBoxClient.Connected) {
            $script:BlackBoxClient = [System.Net.Sockets.TcpClient]::new($BlackBoxHost, $BlackBoxPort)
        }
        $bytes = [System.Text.Encoding]::UTF8.GetBytes("PS> $Line`n")
        $script:BlackBoxClient.GetStream().Write($bytes, 0, $bytes.Length)
    } catch {
        # Daemon not running — silently discard and reset client so next call retries
        $script:BlackBoxClient = $null
    }
}

# Intercept the Enter key: capture line, send it, then submit normally
Set-PSReadLineKeyHandler -Chord Enter -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    BlackBox-Send $line
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
}

Write-Host "BlackBox hook active — sending commands to ${BlackBoxHost}:${BlackBoxPort}" -ForegroundColor DarkGray
