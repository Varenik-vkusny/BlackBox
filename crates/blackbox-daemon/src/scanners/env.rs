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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_dir(prefix: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("bbtest_{prefix}_{n}"))
    }

    fn make_temp_env(content: &str) -> std::path::PathBuf {
        let dir = unique_dir("env");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), content).unwrap();
        dir
    }

    #[test]
    fn test_keys_returned_values_never_are() {
        let dir = make_temp_env("SECRET=password123\nAPI_KEY=tok_abc\nDEBUG=true\n");
        let keys = scan_env_keys(&dir);
        assert!(keys.contains(&"SECRET".to_string()));
        assert!(keys.contains(&"API_KEY".to_string()));
        assert!(keys.contains(&"DEBUG".to_string()));
        // Critical: values must never appear
        let joined = keys.join(",");
        assert!(!joined.contains("password123"), "value leaked");
        assert!(!joined.contains("tok_abc"), "value leaked");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_comments_and_blanks_ignored() {
        let dir = make_temp_env("# this is a comment\n\nVALID=yes\n   \n#COMMENTED_OUT=x\n");
        let keys = scan_env_keys(&dir);
        assert_eq!(keys, vec!["VALID"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_deduplication_across_env_files() {
        let dir = unique_dir("dedup");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "FOO=1\nBAR=2\n").unwrap();
        std::fs::write(dir.join(".env.local"), "FOO=override\nBAZ=3\n").unwrap();
        let keys = scan_env_keys(&dir);
        // FOO should appear only once
        assert_eq!(keys.iter().filter(|k| k.as_str() == "FOO").count(), 1);
        assert!(keys.contains(&"BAR".to_string()));
        assert!(keys.contains(&"BAZ".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_empty_value_key_included() {
        let dir = make_temp_env("EMPTY_VAR=\n");
        let keys = scan_env_keys(&dir);
        assert!(keys.contains(&"EMPTY_VAR".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
