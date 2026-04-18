use std::path::PathBuf;
use std::time::Instant;

use crate::buffer::SharedBuffer;
use crate::docker::error_store::SharedErrorStore;
use crate::scanners::drain::SharedDrainState;

/// All shared daemon state threaded through task boundaries.
/// Clone is cheap — every field is an Arc or Copy type.
#[derive(Clone)]
pub struct DaemonState {
    pub buf: SharedBuffer,
    pub drain: SharedDrainState,
    pub error_store: SharedErrorStore,
    pub cwd: PathBuf,
    pub start_time: Instant,
}
