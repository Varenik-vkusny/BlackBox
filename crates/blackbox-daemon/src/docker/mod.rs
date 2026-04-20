pub mod demux;
pub mod error_store;
pub mod log_filter;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use bollard::container::{ListContainersOptions, LogOutput, LogsOptions};
use bollard::Docker;
use futures_util::StreamExt;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use blackbox_core::types::{ErrorEvent, ErrorSource};

use demux::StreamKind;
use error_store::SharedErrorStore;
use log_filter::should_keep;

const DISCOVERY_INTERVAL_SECS: u64 = 30;
const POLL_INTERVAL_SECS: u64 = 5;
const BACKOFF_MIN_SECS: u64 = 10;
const BACKOFF_MAX_SECS: u64 = 300;

/// Background task: connect to Docker Engine and stream filtered error events.
/// Sets `reachable` flag to reflect live connectivity so callers report correctly.
pub async fn run_docker_monitor(store: SharedErrorStore, reachable: Arc<AtomicBool>) {
    let mut backoff = BACKOFF_MIN_SECS;
    let mut ever_connected = false;

    loop {
        match try_connect_verified().await {
            Some(docker) => {
                if !ever_connected {
                    eprintln!("BlackBox: Docker connected, monitoring containers");
                    ever_connected = true;
                }
                reachable.store(true, Ordering::Relaxed);
                backoff = BACKOFF_MIN_SECS;
                if let Err(e) = monitor_all_containers(&store).await {
                    eprintln!("BlackBox: Docker monitor error: {e}, retrying in {backoff}s");
                    let _ = docker; // keep alive until here so drop is explicit
                }
                reachable.store(false, Ordering::Relaxed);
            }
            None => {
                // Docker not available — wait silently.
                reachable.store(false, Ordering::Relaxed);
            }
        }
        sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(BACKOFF_MAX_SECS);
    }
}

/// Try connecting to Docker and verify with a ping (3-second timeout).
///
/// bollard's connect_* functions create a lazy client that only opens the pipe
/// on the first API call — so we must actually ping to know if Docker is reachable.
/// Falls back from named pipe to TCP (localhost:2375) for Docker Desktop
/// configurations where the WSL2 backend doesn't respond on the default pipe.
async fn try_connect_verified() -> Option<Docker> {
    let candidates = [
        // Named pipe / DOCKER_HOST — default on Windows/Linux/Mac
        Docker::connect_with_local_defaults(),
        // TCP localhost:2375 — Docker Desktop "Expose daemon on TCP" setting
        Docker::connect_with_http_defaults(),
    ];

    for result in candidates {
        let docker = match result {
            Ok(d) => d,
            Err(_) => continue,
        };
        let ping = tokio::time::timeout(Duration::from_secs(3), docker.ping()).await;
        if matches!(ping, Ok(Ok(_))) {
            return Some(docker);
        }
    }
    None
}

/// Discovers running containers and spawns a polling task per container.
/// Returns only when Docker itself becomes unreachable (list_containers fails).
async fn monitor_all_containers(
    store: &SharedErrorStore,
) -> Result<(), bollard::errors::Error> {
    let mut tasks: HashMap<String, JoinHandle<()>> = HashMap::new();

    loop {
        // Re-create a fresh client each discovery cycle — avoids stale named-pipe
        // connection pool state that causes "hyper legacy client: Connect" errors.
        let docker = match Docker::connect_with_local_defaults() {
            Ok(d) => d,
            Err(e) => {
                for (_, h) in tasks.drain() {
                    h.abort();
                }
                return Err(e);
            }
        };

        let containers = match docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: false,
                ..Default::default()
            }))
            .await
        {
            Ok(c) => c,
            Err(e) => {
                for (_, h) in tasks.drain() {
                    h.abort();
                }
                return Err(e);
            }
        };

        let running: std::collections::HashSet<String> =
            containers.iter().filter_map(|c| c.id.clone()).collect();

        tasks.retain(|id, handle| {
            if running.contains(id) {
                true
            } else {
                handle.abort();
                false
            }
        });

        for container in containers {
            let id = match container.id {
                Some(id) => id,
                None => continue,
            };
            if tasks.contains_key(&id) {
                continue;
            }
            let name = container
                .names
                .and_then(|names| names.into_iter().next())
                .unwrap_or_else(|| id.clone());
            let display_name = name.trim_start_matches('/').to_string();

            let store_clone = store.clone();
            let id_clone = id.clone();

            let handle = tokio::spawn(async move {
                poll_container_logs(&id_clone, &display_name, &store_clone).await;
            });
            tasks.insert(id, handle);
        }

        sleep(Duration::from_secs(DISCOVERY_INTERVAL_SECS)).await;
    }
}

/// Polls a single container for new log lines every POLL_INTERVAL_SECS.
///
/// Uses `follow: false` + `since` timestamp instead of a persistent streaming
/// connection. Each poll is a short-lived named-pipe request, which is reliable
/// on Windows where long-lived hyper streams through named pipes tend to fail
/// with "client error (Connect)".
async fn poll_container_logs(container_id: &str, display_name: &str, store: &SharedErrorStore) {
    // since == 0 → first poll fetches the last 100 historical lines via `tail`.
    let mut since: i64 = 0;
    let mut first_poll = true;

    loop {
        let docker = match Docker::connect_with_local_defaults() {
            Ok(d) => d,
            Err(_) => {
                sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                continue;
            }
        };

        let poll_start = now_secs();

        let options = if first_poll {
            LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                tail: "100".to_string(),
                ..Default::default()
            }
        } else {
            LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                since,
                tail: "all".to_string(),
                ..Default::default()
            }
        };

        let mut stream = docker.logs(container_id, Some(options));

        while let Some(Ok(chunk)) = stream.next().await {
            let (stream_kind, text) = match &chunk {
                LogOutput::StdOut { message } => {
                    (StreamKind::Stdout, String::from_utf8_lossy(message).into_owned())
                }
                LogOutput::StdErr { message } => {
                    (StreamKind::Stderr, String::from_utf8_lossy(message).into_owned())
                }
                _ => continue,
            };

            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(filtered) = should_keep(trimmed, stream_kind) {
                    let event = ErrorEvent {
                        source: ErrorSource::Docker {
                            container_id: display_name.to_string(),
                        },
                        text: filtered,
                        timestamp_ms: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        level: extract_level(trimmed),
                    };
                    store.write().unwrap().push(display_name, event);
                }
            }
        }

        since = poll_start;
        first_poll = false;
        sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn extract_level(line: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
        let level = v["level"]
            .as_str()
            .or_else(|| v["severity"].as_str())
            .or_else(|| v["lvl"].as_str())
            .unwrap_or("")
            .to_lowercase();
        if !level.is_empty() {
            return Some(level);
        }
    }
    let lower = line.to_lowercase();
    if lower.contains("fatal") {
        Some("fatal".into())
    } else if lower.contains("error") || lower.contains("panic") {
        Some("error".into())
    } else if lower.contains("warn") {
        Some("warn".into())
    } else {
        None
    }
}
