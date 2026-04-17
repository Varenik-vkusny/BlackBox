use std::path::Path;
use regex::Regex;

/// Finds .env files in cwd and returns masked key names.
/// Returns list of "KEY_NAME" strings (values are never returned).
pub fn scan_env_keys(cwd: &Path) -> Vec<String> {
    static ENV_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = ENV_RE.get_or_init(|| {
        Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)=").expect("valid env regex")
    });

    let mut keys = Vec::new();

    // Check .env, .env.local, .env.development, .env.production
    for filename in &[".env", ".env.local", ".env.development", ".env.production"] {
        let path = cwd.join(filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }
                if let Some(cap) = re.captures(line) {
                    keys.push(cap[1].to_string());
                }
            }
        }
    }

    keys.sort();
    keys.dedup();
    keys
}
