use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct ClientRegistry {
    pub clients: Vec<ClientEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ClientEntry {
    pub name: String,
    pub id: String,
    pub paths: HashMap<String, String>,
}

impl ClientEntry {
    pub fn config_path(&self) -> Option<PathBuf> {
        let os_key = if cfg!(target_os = "windows") { "windows" } else { "unix" };
        self.paths.get(os_key).map(|p| expand_path(p))
    }
}

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(&path[2..])
        } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
            PathBuf::from(userprofile).join(&path[2..])
        } else {
            PathBuf::from(path)
        }
    } else if cfg!(target_os = "windows") {
        let mut expanded = path.to_string();
        for (key, val) in std::env::vars() {
            let pattern = format!("%{}%", key);
            expanded = expanded.replace(&pattern, &val);
        }
        PathBuf::from(expanded)
    } else {
        PathBuf::from(path)
    }
}
