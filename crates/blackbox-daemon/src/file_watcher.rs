use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::buffer::{push_line_and_drain, SharedBuffer};
use crate::scanners::drain::SharedDrainState;
use crate::structured_store::SharedStructuredStore;

/// Per-file state tracked by the watcher.
struct WatchedFile {
    path: PathBuf,
    offset: u64,
}

pub type SharedWatchList = Arc<RwLock<Vec<PathBuf>>>;

pub fn new_watch_list() -> SharedWatchList {
    Arc::new(RwLock::new(Vec::new()))
}

/// Spawn the file-watching background task.
/// Returns a channel sender for adding new paths at runtime.
pub async fn run_file_watcher(
    buf: SharedBuffer,
    drain: SharedDrainState,
    structured: SharedStructuredStore,
    cwd: PathBuf,
    watch_list: SharedWatchList,
) {
    // Channel: notify callback → tokio async task
    let (tx, _rx) = mpsc::channel::<PathBuf>(256);

    // Auto-detect *.log / logs/ / log/ in cwd at startup
    let auto_paths = auto_detect_log_files(&cwd);
    {
        let mut list = watch_list.write().unwrap();
        for p in &auto_paths {
            if !list.contains(p) {
                list.push(p.clone());
            }
        }
    }
    for p in &auto_paths {
        eprintln!("BlackBox file_watcher: auto-detected {}", p.display());
    }

    // tx is used only to keep the bridge_tx alive; watch_list is the real API.
    let _tx = tx;

    // Notify watcher lives in a blocking thread (it uses sync callbacks)
    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
        let _ = notify_tx.send(res);
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("BlackBox file_watcher: failed to create watcher: {e}");
            return;
        }
    };

    // Register all auto-detected paths
    let initial_paths: Vec<PathBuf> = watch_list.read().unwrap().clone();
    for path in &initial_paths {
        let _ = watcher.watch(path, RecursiveMode::NonRecursive);
    }

    // State per watched file; rejected_paths avoids re-checking binary files every 100ms
    let mut file_states: HashMap<PathBuf, WatchedFile> = HashMap::new();
    let mut rejected_paths: HashSet<PathBuf> = HashSet::new();
    for path in &initial_paths {
        match open_tail(path) {
            Some(state) => { file_states.insert(path.clone(), state); }
            None => { rejected_paths.insert(path.clone()); }
        }
    }

    let buf_clone = buf.clone();
    let drain_clone = drain.clone();
    let structured_clone = structured.clone();
    let watch_list_clone = watch_list.clone();

    // Bridge: sync notify events → async channel
    let (bridge_tx, mut bridge_rx) = mpsc::channel::<PathBuf>(256);
    std::thread::spawn(move || {
        for ev in notify_rx.into_iter().flatten() {
            if matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                for path in ev.paths {
                    let _ = bridge_tx.blocking_send(path);
                }
            }
        }
    });

    // Main async loop: process file change events
    tokio::spawn(async move {
        let mut watcher = watcher;
        let mut file_states = file_states;
        let mut rejected_paths = rejected_paths;

        loop {
            // Check if new paths were added to watch_list since last iteration
            {
                let list = watch_list_clone.read().unwrap();
                for path in list.iter() {
                    if file_states.contains_key(path) || rejected_paths.contains(path) {
                        continue;
                    }
                    let _ = watcher.watch(path, RecursiveMode::NonRecursive);
                    match open_tail(path) {
                        Some(state) => {
                            eprintln!("BlackBox file_watcher: watching {}", path.display());
                            file_states.insert(path.clone(), state);
                        }
                        None => {
                            rejected_paths.insert(path.clone());
                        }
                    }
                }
            }

            // Wait for a change event (100ms timeout to also check new paths)
            let changed_path = tokio::time::timeout(
                std::time::Duration::from_millis(100),
                bridge_rx.recv(),
            )
            .await
            .ok()
            .flatten();

            if let Some(path) = changed_path {
                if let Some(state) = file_states.get_mut(&path) {
                    read_new_lines(state, &buf_clone, &drain_clone, &structured_clone, &path);
                }
            }
        }
    });
}

/// Open a file and seek to EOF (tail mode — only new writes are processed).
fn open_tail(path: &Path) -> Option<WatchedFile> {
    if !path.exists() {
        return None;
    }
    // Validate: first 512 bytes must be valid UTF-8 (skip binary files)
    if !is_text_file(path) {
        eprintln!("BlackBox file_watcher: skipping binary file {}", path.display());
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    Some(WatchedFile {
        path: path.to_path_buf(),
        offset: meta.len(), // start at EOF — only new writes matter
    })
}

/// Read lines added since last offset, push each through the pipeline.
fn read_new_lines(
    state: &mut WatchedFile,
    buf: &SharedBuffer,
    drain: &SharedDrainState,
    structured: &SharedStructuredStore,
    canonical_path: &Path,
) {
    let mut file = match std::fs::File::open(&state.path) {
        Ok(f) => f,
        Err(_) => return,
    };

    // Detect log rotation: file is shorter than our offset → file was recreated
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len < state.offset {
        state.offset = 0;
    }

    if file.seek(SeekFrom::Start(state.offset)).is_err() {
        return;
    }

    let mut new_bytes = Vec::new();
    if file.read_to_end(&mut new_bytes).is_err() {
        return;
    }

    if new_bytes.is_empty() {
        return;
    }

    state.offset += new_bytes.len() as u64;

    let text = match String::from_utf8(new_bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).to_string(),
    };

    // Rate limit: max 1000 lines per event
    let source_tag = format!(
        "file:{}",
        canonical_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    );
    let mut count = 0usize;
    for line in text.lines() {
        if count >= 1_000 {
            break;
        }
        if !line.trim().is_empty() {
            push_line_and_drain(buf, drain, structured, line.to_string(), Some(source_tag.clone()));
            count += 1;
        }
    }
}

/// Check if a file looks like text by reading first 512 bytes and checking UTF-8 validity.
fn is_text_file(path: &Path) -> bool {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 512];
    let n = f.read(&mut buf).unwrap_or(0);
    if n == 0 {
        return true; // empty file is fine
    }
    std::str::from_utf8(&buf[..n]).is_ok()
}

/// Auto-detect log files in cwd: *.log at root level + logs/ and log/ directories.
fn auto_detect_log_files(cwd: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();

    // Root-level *.log files
    if let Ok(entries) = std::fs::read_dir(cwd) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "log" {
                        found.push(path);
                    }
                }
            }
        }
    }

    // logs/ and log/ directories
    for dir_name in &["logs", "log"] {
        let dir = cwd.join(dir_name);
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "log" || ext == "txt" {
                                found.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    found
}
