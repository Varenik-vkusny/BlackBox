use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use blackbox_core::types::ErrorEvent;

#[allow(dead_code)]
const PER_CONTAINER_CAP: usize = 500;

pub type SharedErrorStore = Arc<RwLock<ErrorStore>>;

pub struct ErrorStore {
    pub by_container: HashMap<String, VecDeque<ErrorEvent>>,
}

impl ErrorStore {
    fn new() -> Self {
        Self { by_container: HashMap::new() }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, container_id: &str, event: ErrorEvent) {
        let queue = self.by_container.entry(container_id.to_string()).or_default();
        if queue.len() >= PER_CONTAINER_CAP {
            queue.pop_front();
        }
        queue.push_back(event);
    }

    pub fn get_events(&self, container_id: Option<&str>, limit: usize) -> Vec<ErrorEvent> {
        match container_id {
            Some(id) => self
                .by_container
                .get(id)
                .map(|q| q.iter().rev().take(limit).cloned().collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .rev()
                .collect(),
            None => {
                let mut all: Vec<&ErrorEvent> = self
                    .by_container
                    .values()
                    .flat_map(|q| q.iter())
                    .collect();
                all.sort_by_key(|e| e.timestamp_ms);
                all.into_iter().rev().take(limit).cloned().collect::<Vec<_>>()
                    .into_iter().rev().collect()
            }
        }
    }

    pub fn container_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.by_container.keys().cloned().collect();
        ids.sort();
        ids
    }
}

pub fn new_error_store() -> SharedErrorStore {
    Arc::new(RwLock::new(ErrorStore::new()))
}
