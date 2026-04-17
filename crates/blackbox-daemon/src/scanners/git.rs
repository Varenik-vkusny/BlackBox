use std::path::Path;

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

    let dirty = count_dirty_files(&repo);
    (branch, dirty)
}

fn count_dirty_files(repo: &gix::Repository) -> usize {
    // Use gix index to count conflicted (non-Unconflicted stage) entries
    // as a proxy for dirty/conflicted files. Returns 0 on any error — best-effort.
    repo.index()
        .ok()
        .map(|idx| {
            idx.entries()
                .iter()
                .filter(|e| {
                    use gix::index::entry::Stage;
                    e.stage() != Stage::Unconflicted
                })
                .count()
        })
        .unwrap_or(0)
}
