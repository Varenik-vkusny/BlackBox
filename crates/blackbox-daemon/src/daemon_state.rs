use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::buffer::SharedBuffer;
use crate::docker::error_store::SharedErrorStore;
use crate::file_watcher::SharedWatchList;
use crate::http_store::SharedHttpStore;
use crate::scanners::drain::SharedDrainState;
use crate::structured_store::SharedStructuredStore;

/// All shared daemon state threaded through task boundaries.
/// Clone is cheap — every field is an Arc or Copy type.
#[derive(Clone)]
pub struct DaemonState {
    pub buf: SharedBuffer,
    pub drain: SharedDrainState,
    pub error_store: SharedErrorStore,
    pub http_store: SharedHttpStore,
    pub structured: SharedStructuredStore,
    pub cwd: PathBuf,
    pub start_time: Instant,
    pub watch_list: SharedWatchList,
    /// True while the Docker monitor has a live, ping-verified connection.
    /// Separate from error_store so get_container_logs reports correctly
    /// even when no ERROR events have occurred yet.
    pub docker_reachable: Arc<AtomicBool>,
}
