pub mod demux;
pub mod error_store;
pub mod log_filter;

use std::collections::HashMap;
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

const RETRY_INTERVAL_SECS: u64 = 10;
const DISCOVERY_INTERVAL_SECS: u64 = 30;

/// Background task: connect to Docker Engine and stream filtered error events.
/// Silently retries every 10 seconds when Docker is unavailable.
pub async fn run_docker_monitor(store: SharedErrorStore) {
    loop {
        match connect_docker() {
            Ok(docker) => {
                eprintln!("BlackBox: Docker connected, monitoring containers");
                if let Err(e) = monitor_all_containers(&docker, &store).await {
                    eprintln!("BlackBox: Docker monitor error: {e}, retrying in {RETRY_INTERVAL_SECS}s");
                }
            }
            Err(_) => {
                // Docker not available — wait silently.
            }
        }
        sleep(Duration::from_secs(RETRY_INTERVAL_SECS)).await;
    }
}

fn connect_docker() -> Result<Docker, bollard::errors::Error> {
    Docker::connect_with_local_defaults()
}

async fn monitor_all_containers(
    docker: &Docker,
    store: &SharedErrorStore,
) -> Result<(), bollard::errors::Error> {
    // Maps container_id → streaming task handle.
    // Tasks are aborted when a container stops or Docker disconnects.
    let mut tasks: HashMap<String, JoinHandle<()>> = HashMap::new();

    loop {
        let containers = match docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: false,
                ..Default::default()
            }))
            .await
        {
            Ok(c) => c,
            Err(e) => {
                for (_, handle) in tasks.drain() {
                    handle.abort();
                }
                return Err(e);
            }
        };

        // Build set of currently running IDs.
        let running: std::collections::HashSet<String> = containers
            .iter()
            .filter_map(|c| c.id.clone())
            .collect();

        // Abort tasks for containers that stopped.
        tasks.retain(|id, handle| {
            if running.contains(id) {
                true
            } else {
                handle.abort();
                false
            }
        });

        // Spawn tasks for newly discovered containers.
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

            let docker_clone = docker.clone();
            let store_clone = store.clone();
            let id_clone = id.clone();

            let handle = tokio::spawn(async move {
                loop {
                    stream_container_logs(&docker_clone, &id_clone, &display_name, &store_clone)
                        .await;
                    sleep(Duration::from_secs(5)).await;
                }
            });
            tasks.insert(id, handle);
        }

        sleep(Duration::from_secs(DISCOVERY_INTERVAL_SECS)).await;
    }
}

async fn stream_container_logs(
    docker: &Docker,
    container_id: &str,
    display_name: &str,
    store: &SharedErrorStore,
) {
    let options = LogsOptions::<String> {
        follow: true,
        stdout: true,
        stderr: true,
        tail: "100".to_string(), // start with last 100 lines of history
        ..Default::default()
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
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let level = extract_level(trimmed);
                let event = ErrorEvent {
                    source: ErrorSource::Docker { container_id: display_name.to_string() },
                    text: filtered,
                    timestamp_ms,
                    level,
                };
                store.write().unwrap().push(display_name, event);
            }
        }
    }
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
