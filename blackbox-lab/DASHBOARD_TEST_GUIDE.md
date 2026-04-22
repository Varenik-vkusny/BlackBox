# BlackBox Dashboard — Complete Test Guide

## 🚀 Quick Start

### 1. Start the BlackBox Daemon

```bash
cd c:\Users\user\Desktop\BlackBox
cargo run -p blackbox-daemon -- --cwd .
```

Expected output:
```
[INFO] BlackBox daemon listening on http://127.0.0.1:8768
```

### 2. Start the React Dashboard

In a new terminal:
```bash
cd c:\Users\user\Desktop\BlackBox\blackbox-lab
npm run dev
```

Expected output:
```
VITE v8.0.8  ready in 234 ms

➜  Local:   http://localhost:5173/
➜  press h to show help
```

### 3. Inject Test Data

In a third terminal (PowerShell):

**Option A: PowerShell (Recommended for Windows)**
```powershell
cd c:\Users\user\Desktop\BlackBox\blackbox-lab
powershell -ExecutionPolicy Bypass -File test-data-inject.ps1
```

**Option B: Bash (if using WSL/Git Bash)**
```bash
cd /mnt/c/Users/user/Desktop/BlackBox/blackbox-lab
chmod +x test-data-inject.sh
./test-data-inject.sh
```

---

## ✅ What Gets Injected

The test script injects **38 log lines** covering:

| Type | Count | Purpose |
|------|-------|---------|
| **Startup logs** | 4 | Normal operation context |
| **Rust panic** | 8 | Full stack trace (tests OverviewDashboard + TriageView) |
| **Python traceback** | 6 | Error with file references (tests diff) |
| **Node.js error** | 4 | Different language (tests severity) |
| **Repeated messages** | 8 | [×8] dedup badge (tests UnifiedStream dedup) |
| **Mixed workload** | 8 | Normal + error logs mixed |
| **Docker logs** | 4 | Container error simulation |
| **HTTP patterns** | 4 | Status codes and latencies |

---

## 🧪 Complete Test Checklist

After running the script, open **http://localhost:5173** and verify:

### ✓ Level 1: Overview Dashboard (Default View)

- [ ] **Health banner shows "Panic detected"** — Rust panic = critical
- [ ] **Metric pills display:**
  - Stack Traces: **1**
  - Log Clusters: **8** (the repeated "connection refused")
  - Containers: **4**
  - HTTP Errors: **2** (404, 500)
- [ ] **Service health cards:**
  - **vscode_bridge**: RED (1 panic) — click shows **"1 error lines"**
  - **Docker**: RED/ORANGE (4 container errors)
  - **http-proxy**: ORANGE (2 errors = 4xx+5xx)
- [ ] **30-min histogram**: Shows error spike
- [ ] **Recent commits**: Last 3-4 commits from `git log`
- [ ] **Watched Files**: Appears if any .log files in blackbox-lab/

### ✓ Level 2: Triage View

**Click on the red "vscode_bridge" card** → Triage view opens

- [ ] **Header shows:**
  - `← Overview` button
  - "vscode_bridge" badge
  - "1 trace · 8 clusters" stats
- [ ] **Stack traces section shows:**
  - Rust panic card with message: *"Database connection failed: Connection refused"*
  - Expand to see 5 stack frames
  - Each frame references `src/test-fixtures/database.rs:28`
- [ ] **Repeated errors section shows:**
  - 8 collapsed log entries with pattern: *"error: connection refused to 127.0.0.1:5432"*
  - Each shows timestamp and "attempt X/10"
- [ ] **"Inspect Changes" button** works (loads diff)

### ✓ Level 3: Split View (Diff)

**Click "Inspect Changes" in any stack trace card**

- [ ] **Left side shows stack trace details:**
  - Error message
  - Language badge (Rust)
  - File pills: `database.rs`, `main.rs`
- [ ] **Right side shows unified diff:**
  - Lines from `src/test-fixtures/database.rs` (around panic line)
  - Green/red diff coloring
  - File path references

### ✓ Level 4: Raw Logs

**Click "Raw Logs" button or switch to "raw" tab**

- [ ] **Stream shows all 38 log entries in reverse chronological order**
- [ ] **Dedup badges appear:**
  - One [×8] badge for repeated "connection refused"
  - Other single lines appear once each
- [ ] **Tabs work:**
  - All / Terminal / Docker / Network / Analyzed
- [ ] **Search box filters:**
  - Type "panic" → shows only Rust panic lines
  - Type "5432" → shows connection errors
  - Type "span_id:..." → shows structured logs (if any)
- [ ] **Smart Collapse toggle** groups identical-pattern lines

### ✓ Sidebar (Source Matrix)

Throughout all views:

- [ ] **Views section shows:**
  - Overview (radio selected when on overview)
  - Raw Logs (radio selected when on raw)
- [ ] **Terminal section:**
  - vscode_bridge: Green dot (nominal) OR Red (has errors)
  - Count shows "1 error lines" when panicked
- [ ] **Docker section:**
  - 4 containers listed (if Docker running)
  - Errors labeled "× error" not bare numbers
- [ ] **Network section:**
  - proxy :8769
  - "2 errors" label
  - Breakdown: "2× 4xx · 2× 5xx"
- [ ] **Git info at bottom:**
  - Current branch
  - Dirty file count (if applicable)

### ✓ Status Bar (Top 48px)

- [ ] **Shows:**
  - BlackBox logo
  - Daemon online pill (green) or offline (gray)
  - Uptime (e.g., "15m 23s")
  - Git branch (e.g., "main")
  - Docker status (e.g., "docker · 4" containers)
  - HTTP errors warning (orange pill if 2+ errors)
- [ ] **Action buttons work:**
  - Pause/Resume toggle (changes icon)
  - Clear all logs (resets counts)
  - Raw Logs button (when on overview/triage)

---

## 🔍 Advanced Testing

### Test Dedup Behavior

In Raw Logs tab:

1. Inject more repeated messages:
   ```powershell
   for ($i = 1; $i -le 15; $i++) {
       Invoke-RestMethod -Uri "http://127.0.0.1:8768/api/inject" -Method Post `
           -Headers @{"Content-Type"="application/json"} `
           -Body (@{text="error: socket timeout to db.internal:3306"} | ConvertTo-Json -Compress)
   }
   ```

2. Verify **[×15] socket timeout** badge appears

3. Click to expand → shows 15 identical entries

### Test Daemon Offline

1. Kill daemon: `Ctrl+C` in daemon terminal
2. Refresh dashboard
3. Overview shows "Daemon Offline" message
4. Status bar shows offline pill (gray)
5. Service cards are disabled/grayed

Restart daemon and refresh.

### Test Severity Colors

Compare:
- **Critical (Red)**: Rust panic, ≥10 errors, 5xx HTTP
- **Warning (Orange)**: <10 errors, 4xx HTTP, slow queries
- **Nominal (Green)**: No errors

---

## 📁 Test Files Created

These real files are referenced by stack traces (for diff testing):

```
blackbox-lab/src/test-fixtures/
├── database.rs        (Rust, line 28 = panic point)
├── processor.py       (Python, line 19 = traceback)
└── server.js          (Node.js, line 16 = async error)
```

To see diffs working:
1. Modify one of these files (add a comment, change a line)
2. Inject a new error that references that file
3. Click "Inspect Changes" → diff shows your change highlighted

---

## 🐛 Troubleshooting

### Daemon Not Responding (Scripts Fail)

```
Error: Check if daemon is running on 127.0.0.1:8768
```

**Fix:**
1. Ensure daemon is running on correct port
2. Check `cargo run -p blackbox-daemon -- --cwd .`
3. Verify port 8768 is not blocked by firewall

### Dashboard Shows No Data

1. Clear browser cache: `Ctrl+Shift+Delete` → Empty all time
2. Refresh: `Ctrl+Shift+R` (hard refresh)
3. Check console for errors: `F12` → Console tab
4. Verify polling is active (pause button should show resume icon)

### Diff Not Loading

1. Verify files exist: `dir src\test-fixtures\*`
2. Try "Inspect Changes" again
3. Check daemon logs for errors
4. Daemon needs git diff working: run from git repo root

### Dedup Badges Not Appearing

1. Ensure 3+ consecutive identical-prefix lines were injected
2. Check stream sorting (most recent first)
3. Try injecting 10x same message in one batch

---

## 🎬 Demo Flow (5 Minutes)

1. **Start everything** (daemon + React + inject test data) — 2 min
2. **Show Overview** → point out health banner, metric pills, service cards — 1 min
3. **Click vscode_bridge** → Triage view with Rust panic card + dedup clusters — 1 min
4. **Click "Inspect Changes"** → diff overlay showing line 28 of database.rs — 30s
5. **Back to Overview → Raw Logs** → point out [×8] dedup badge, smart collapse — 1 min
6. **Stop** or continue free exploration

---

## 🚦 Success Criteria

Dashboard is **fully working** if:

- ✅ Overview shows service cards with correct error counts
- ✅ Clicking a service card opens Triage with grouped errors
- ✅ [×8] dedup badge appears for repeated messages
- ✅ "Inspect Changes" opens diff showing real file content
- ✅ All 4 views accessible (Overview / Triage / Raw / Split)
- ✅ Sidebar updates based on current view + selected service
- ✅ Status bar shows daemon health + git info
- ✅ No TypeScript errors in browser console

**If all ✅ → Dashboard is production-ready! 🎉**
