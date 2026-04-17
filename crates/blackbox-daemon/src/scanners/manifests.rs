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

#[cfg(test)]
mod tests {
    use super::*;
    use blackbox_core::types::ProjectKind;

    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn make_dir(id: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bbtest_manifests_{id}_{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_cargo_parsed_correctly() {
        let dir = make_dir("cargo");
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"myapp\"\nversion = \"1.2.3\"\n").unwrap();
        let manifests = scan_manifests(&dir);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].manifest_type, ProjectKind::Cargo);
        assert_eq!(manifests[0].name, "myapp");
        assert_eq!(manifests[0].version, "1.2.3");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_cargo_first_when_multiple_manifests() {
        let dir = make_dir("multi");
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"rust-app\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(dir.join("package.json"), r#"{"name":"frontend","version":"2.0.0"}"#).unwrap();
        std::fs::write(dir.join("go.mod"), "module mymodule\ngo 1.21\n").unwrap();
        let manifests = scan_manifests(&dir);
        assert_eq!(manifests.len(), 3, "should detect all 3 manifests");
        assert_eq!(manifests[0].manifest_type, ProjectKind::Cargo, "cargo must be first");
        assert_eq!(manifests[1].manifest_type, ProjectKind::Go,    "go must be second");
        assert_eq!(manifests[2].manifest_type, ProjectKind::Npm,   "npm must be third");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_npm_parsed_correctly() {
        let dir = make_dir("npm");
        std::fs::write(dir.join("package.json"), r#"{"name":"my-pkg","version":"3.0.1"}"#).unwrap();
        let manifests = scan_manifests(&dir);
        assert_eq!(manifests[0].manifest_type, ProjectKind::Npm);
        assert_eq!(manifests[0].name, "my-pkg");
        assert_eq!(manifests[0].version, "3.0.1");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_go_mod_parsed_correctly() {
        let dir = make_dir("go");
        std::fs::write(dir.join("go.mod"), "module github.com/user/repo\ngo 1.22\n").unwrap();
        let manifests = scan_manifests(&dir);
        assert_eq!(manifests[0].manifest_type, ProjectKind::Go);
        assert_eq!(manifests[0].name, "github.com/user/repo");
        assert_eq!(manifests[0].version, "1.22");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_empty_dir_returns_empty() {
        let dir = make_dir("empty");
        assert!(scan_manifests(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
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
