use std::path::Path;

use blackbox_core::types::{ChangedFile, ChangeType, CommitInfo, DiffHunk, HunkLine, HunkLineKind};

/// Normalize a file path for cross-reference comparison.
/// Strips leading `./`, converts `\` to `/`, makes absolute paths relative to cwd.
pub fn normalize_path(path: &str, cwd: &Path) -> String {
    let p = path.replace('\\', "/");
    let p = p.trim_start_matches("./");
    if std::path::Path::new(p).is_absolute() {
        if let Ok(abs) = std::path::Path::new(p).canonicalize() {
            // On Windows canonicalize() returns \\?\ UNC paths.
            // cwd may be a plain C:\... path — strip_prefix fails unless both are canonical.
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
            if let Ok(rel) = abs.strip_prefix(&cwd_canon) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
    }
    p.to_string()
}

/// Returns (branch_name, dirty_file_count).
/// Returns ("unknown", 0) if not a git repo or on error.
pub fn scan_git(cwd: &Path) -> (String, usize) {
    let repo = match gix::open(cwd) {
        Ok(r) => r,
        Err(_) => return ("unknown".into(), 0),
    };

    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|r| r.shorten().to_string())
        .unwrap_or_else(|| "HEAD (detached)".into());

    let cwd = repo.work_dir().map(|p| p.to_path_buf()).unwrap_or_else(|| cwd.to_path_buf());
    let dirty = count_dirty_files(&cwd);
    (branch, dirty)
}

fn count_dirty_files(cwd: &Path) -> usize {
    // Collect unique filenames from staged + unstaged diffs to deduplicate.
    let mut files = std::collections::HashSet::new();
    for args in [["diff", "--name-only", "HEAD"], ["diff", "--cached", "--name-only"]] {
        if let Ok(out) = std::process::Command::new("git").args(args).current_dir(cwd).output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if !line.trim().is_empty() {
                    files.insert(line.to_string());
                }
            }
        }
    }
    files.len()
}

/// List all uncommitted changed files (staged + unstaged) relative to HEAD.
/// Returns empty vec if not a git repo or on error.
pub fn get_changed_files(cwd: &Path) -> Vec<ChangedFile> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-status", "HEAD"])
        .current_dir(cwd)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files: Vec<ChangedFile> = stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let status = parts.next()?.trim();
            let path = parts.next()?.trim().to_string();
            let change_type = match status.chars().next()? {
                'A' => ChangeType::Added,
                'D' => ChangeType::Deleted,
                _ => ChangeType::Modified,
            };
            Some(ChangedFile { path, change_type })
        })
        .collect();

    // Also include untracked files that are staged (git diff --cached)
    if let Ok(staged) = std::process::Command::new("git")
        .args(["diff", "--name-status", "--cached"])
        .current_dir(cwd)
        .output()
    {
        let staged_str = String::from_utf8_lossy(&staged.stdout);
        for line in staged_str.lines() {
            let mut parts = line.splitn(2, '\t');
            let status = parts.next().map(str::trim).unwrap_or("");
            let path = match parts.next() {
                Some(p) => p.trim().to_string(),
                None => continue,
            };
            if !files.iter().any(|f| f.path == path) {
                let change_type = match status.chars().next().unwrap_or('M') {
                    'A' => ChangeType::Added,
                    'D' => ChangeType::Deleted,
                    _ => ChangeType::Modified,
                };
                files.push(ChangedFile { path, change_type });
            }
        }
    }

    // Also include untracked files — git diff HEAD won't show them, but they
    // can still be referenced by stack traces and we want diff coverage.
    if let Ok(untracked) = std::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(cwd)
        .output()
    {
        let s = String::from_utf8_lossy(&untracked.stdout);
        for line in s.lines() {
            let path = line.trim().to_string();
            if !path.is_empty() && !files.iter().any(|f| f.path == path) {
                files.push(ChangedFile { path, change_type: ChangeType::Added });
            }
        }
    }

    files
}

const MAX_HUNKS: usize = 50;
const MAX_HUNK_LINES: usize = 30;

/// Generate unified diff hunks for the given files vs HEAD.
/// Falls back to --no-index diff for untracked files.
/// Returns (hunks, truncated). Caps at MAX_HUNKS / MAX_HUNK_LINES.
pub fn get_diff_hunks(cwd: &Path, files: &[String]) -> (Vec<DiffHunk>, bool) {
    if files.is_empty() {
        return (vec![], false);
    }

    let mut all_hunks: Vec<DiffHunk> = Vec::new();
    let mut truncated = false;

    // Batch diff for tracked files vs HEAD
    let mut args = vec!["diff", "HEAD", "-U3", "--"];
    for f in files {
        args.push(f.as_str());
    }
    if let Ok(o) = std::process::Command::new("git").args(&args).current_dir(cwd).output() {
        let text = String::from_utf8_lossy(&o.stdout);
        if !text.trim().is_empty() {
            let (h, t) = parse_unified_diff(&text);
            all_hunks.extend(h);
            if t { truncated = true; }
        }
    }

    // For files that produced no output (untracked), use --no-index against /dev/null
    // git diff --no-index exits 1 when files differ (normal), so don't check success().
    let seen: std::collections::HashSet<String> = all_hunks.iter().map(|h| h.file.clone()).collect();
    for file in files {
        let norm = file.replace('\\', "/");
        if seen.iter().any(|f| f.ends_with(&norm) || norm.ends_with(f.as_str())) {
            continue;
        }
        let abs = cwd.join(file);
        if !abs.exists() { continue; }
        if let Ok(o) = std::process::Command::new("git")
            .args(["diff", "--no-index", "-U3", "--", "/dev/null", file.as_str()])
            .current_dir(cwd)
            .output()
        {
            let text = String::from_utf8_lossy(&o.stdout);
            if !text.trim().is_empty() {
                let (h, t) = parse_unified_diff(&text);
                all_hunks.extend(h);
                if t { truncated = true; }
            }
        }
    }

    if all_hunks.len() > MAX_HUNKS {
        all_hunks.truncate(MAX_HUNKS);
        truncated = true;
    }

    (all_hunks, truncated)
}

fn parse_unified_diff(diff: &str) -> (Vec<DiffHunk>, bool) {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current_file = String::new();
    let mut current_hunk: Option<DiffHunk> = None;
    let mut truncated = false;

    for line in diff.lines() {
        if let Some(stripped) = line.strip_prefix("+++ b/") {
            current_file = stripped.to_string();
        } else if line.starts_with("@@ ") {
            // Flush previous hunk
            if let Some(h) = current_hunk.take() {
                if hunks.len() < MAX_HUNKS {
                    hunks.push(h);
                } else {
                    truncated = true;
                }
            }
            // Parse "@@ -old_start,... +new_start,... @@"
            let (old_start, new_start) = parse_hunk_header(line);
            current_hunk = Some(DiffHunk {
                file: current_file.clone(),
                old_start,
                new_start,
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            if hunk.lines.len() >= MAX_HUNK_LINES {
                continue; // silently drop excess lines (truncated per-hunk)
            }
            let (kind, text) = if let Some(stripped) = line.strip_prefix('+') {
                (HunkLineKind::Added, stripped.to_string())
            } else if let Some(stripped) = line.strip_prefix('-') {
                (HunkLineKind::Removed, stripped.to_string())
            } else if let Some(stripped) = line.strip_prefix(' ') {
                (HunkLineKind::Context, stripped.to_string())
            } else {
                continue;
            };
            hunk.lines.push(HunkLine { kind, text });
        }
    }

    if let Some(h) = current_hunk {
        if hunks.len() < MAX_HUNKS {
            hunks.push(h);
        } else {
            truncated = true;
        }
    }

    (hunks, truncated)
}

fn parse_hunk_header(line: &str) -> (u32, u32) {
    // "@@ -42,6 +44,8 @@" → (42, 44)
    let mut parts = line.splitn(4, ' ');
    parts.next(); // "@@"
    let old = parts
        .next()
        .and_then(|s| s.trim_start_matches('-').split(',').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let new = parts
        .next()
        .and_then(|s| s.trim_start_matches('+').split(',').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (old, new)
}

/// Fetch recent git commits with lightweight metadata (no diffs).
/// Optionally filter to commits that touched a specific file path.
/// Uses `git log --format=... --stat` to get per-commit stats in one pass.
pub fn get_recent_commits(cwd: &Path, limit: usize, path_filter: Option<&str>) -> Vec<CommitInfo> {
    // Custom format: each commit header is on one line, separated by a delimiter
    // %x1f = ASCII unit separator (won't appear in normal text)
    // Format: hash\x1fauthor\x1ftimestamp\x1fsubject
    let format = "--format=%x00%H%x1f%an%x1f%aI%x1f%s";
    let limit_str = limit.to_string();

    let mut args = vec!["log", format, "-n", &limit_str, "--stat", "--stat-width=200"];
    if let Some(path) = path_filter {
        args.push("--");
        args.push(path);
    }

    let output = match std::process::Command::new("git")
        .args(&args)
        .current_dir(cwd)
        .output()
    {
        Ok(o) if !o.stdout.is_empty() => o,
        _ => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    parse_git_log_stat(&text)
}

/// Parse `git log --format=%x00%H%x1f%an%x1f%aI%x1f%s --stat` output.
/// Commits are delimited by NUL (\x00) at the start of each header line.
fn parse_git_log_stat(text: &str) -> Vec<CommitInfo> {
    let mut commits: Vec<CommitInfo> = Vec::new();

    // Split into commit blocks on \x00 (NUL byte before each header)
    for block in text.split('\x00') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();
        let header = match lines.next() {
            Some(h) => h,
            None => continue,
        };

        // Parse header: hash\x1fauthor\x1ftimestamp\x1fsubject
        let parts: Vec<&str> = header.splitn(4, '\x1f').collect();
        if parts.len() < 4 {
            continue;
        }
        let hash = parts[0].trim().to_string();
        let author = parts[1].to_string();
        let timestamp_iso = parts[2].to_string();
        let message = parts[3].to_string();

        if hash.len() < 7 {
            continue;
        }

        let mut changed_files: Vec<String> = Vec::new();
        let mut insertions: u32 = 0;
        let mut deletions: u32 = 0;

        // Remaining lines are the --stat output:
        // " src/foo.rs | 12 +++---"
        // " 2 files changed, 8 insertions(+), 4 deletions(-)"
        for stat_line in lines {
            let trimmed = stat_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains("changed,") || trimmed.ends_with("changed") {
                // Summary line: "N files changed, X insertions(+), Y deletions(-)"
                if let Some(ins) = extract_stat_number(trimmed, "insertion") {
                    insertions = ins;
                }
                if let Some(del) = extract_stat_number(trimmed, "deletion") {
                    deletions = del;
                }
            } else if let Some(pipe_pos) = trimmed.rfind(" | ") {
                // File stat line: "path/to/file.rs | 12 ++--"
                let file_path = trimmed[..pipe_pos].trim().to_string();
                if !file_path.is_empty() {
                    changed_files.push(file_path);
                }
            }
        }

        commits.push(CommitInfo {
            hash: hash[..7.min(hash.len())].to_string(),
            message,
            author,
            timestamp_iso,
            changed_files,
            insertions,
            deletions,
        });
    }

    commits
}

fn extract_stat_number(line: &str, keyword: &str) -> Option<u32> {
    // "8 insertions(+)" → Some(8)
    let pos = line.find(keyword)?;
    let before = line[..pos].trim();
    // Walk back to find the number
    let num_str: String = before.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    let num_str: String = num_str.chars().rev().collect();
    num_str.parse().ok()
}
