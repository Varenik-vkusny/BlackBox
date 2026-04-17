use std::path::Path;
use blackbox_core::types::{ManifestInfo, ProjectKind};

// Priority order: Cargo > Go > npm
const MANIFEST_PRIORITY: &[(&str, ProjectKind)] = &[
    ("Cargo.toml", ProjectKind::Cargo),
    ("go.mod", ProjectKind::Go),
    ("package.json", ProjectKind::Npm),
];

pub fn scan_manifests(cwd: &Path) -> Vec<ManifestInfo> {
    let mut results = Vec::new();

    for (filename, kind) in MANIFEST_PRIORITY {
        let path = cwd.join(filename);
        if !path.exists() {
            continue;
        }
        let info = parse_manifest(&path, kind.clone());
        results.push(info);
    }

    results
}

fn parse_manifest(path: &std::path::PathBuf, kind: ProjectKind) -> ManifestInfo {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let (name, version) = match kind {
        ProjectKind::Cargo => parse_cargo(&content),
        ProjectKind::Go => parse_go(&content),
        ProjectKind::Npm => parse_npm(&content),
        ProjectKind::Unknown => (String::new(), String::new()),
    };
    ManifestInfo { manifest_type: kind, name, version }
}

fn parse_cargo(content: &str) -> (String, String) {
    // Simple line-by-line parse — no toml dep needed for MVP
    let name = extract_toml_string(content, "name");
    let version = extract_toml_string(content, "version");
    (name, version)
}

fn parse_go(content: &str) -> (String, String) {
    // go.mod: first line is "module <name>", second line is "go <version>"
    let mut name = String::new();
    let mut version = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("module ") {
            name = line.strip_prefix("module ").unwrap_or("").trim().to_string();
        } else if line.starts_with("go ") {
            version = line.strip_prefix("go ").unwrap_or("").trim().to_string();
        }
    }
    (name, version)
}

fn parse_npm(content: &str) -> (String, String) {
    // Use serde_json to parse package.json
    serde_json::from_str::<serde_json::Value>(content)
        .map(|v| {
            let name = v["name"].as_str().unwrap_or("").to_string();
            let version = v["version"].as_str().unwrap_or("").to_string();
            (name, version)
        })
        .unwrap_or_default()
}

fn extract_toml_string(content: &str, key: &str) -> String {
    // Matches: key = "value"
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(&format!("{key} =")) || line.starts_with(&format!("{key}=")) {
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    return line[start + 1..start + 1 + end].to_string();
                }
            }
        }
    }
    String::new()
}
