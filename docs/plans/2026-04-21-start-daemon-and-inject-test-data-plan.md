# Start BlackBox Environment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Start the BlackBox daemon and populate the frontend dashboard with test data.

**Architecture:** Background Rust daemon for log processing and a PowerShell-based injector for sample data.

**Tech Stack:** Rust (via PowerShell startup script), PowerShell (for data injection).

---

### Task 1: Start BlackBox Daemon

**Files:**
- Run: `.\start-daemon.ps1`

**Step 1: Execute startup script**
Run: `powershell -ExecutionPolicy Bypass -File .\start-daemon.ps1`
Expected: Output showing "Building blackbox-daemon" (if needed) and "Daemon is ready on port 8768."

**Step 2: Verify port 8768 is listening**
Run: `Get-NetTCPConnection -LocalPort 8768 -State Listen`
Expected: Process found listening on port 8768.

---

### Task 2: Inject Test Data

**Files:**
- Run: `blackbox-lab\test-data-inject.ps1`

**Step 1: Execute injection script**
Run: `powershell -ExecutionPolicy Bypass -File .\blackbox-lab\test-data-inject.ps1`
Expected: Console output showing "Injecting..." followed by "OK" for various log types, ending with "Test data injection complete!".
