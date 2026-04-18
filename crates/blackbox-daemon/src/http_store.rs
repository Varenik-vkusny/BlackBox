use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

const HTTP_STORE_CAP: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpEvent {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub latency_ms: u64,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub timestamp_ms: u64,
}

pub type SharedHttpStore = Arc<RwLock<VecDeque<HttpEvent>>>;

pub fn new_http_store() -> SharedHttpStore {
    Arc::new(RwLock::new(VecDeque::new()))
}

pub fn push_http_event(store: &SharedHttpStore, event: HttpEvent) {
    let mut guard = store.write().unwrap();
    if guard.len() >= HTTP_STORE_CAP {
        guard.pop_front();
    }
    guard.push_back(event);
}

pub fn get_http_events(store: &SharedHttpStore, limit: usize) -> Vec<HttpEvent> {
    let guard = store.read().unwrap();
    guard.iter().rev().take(limit).cloned().collect()
}
