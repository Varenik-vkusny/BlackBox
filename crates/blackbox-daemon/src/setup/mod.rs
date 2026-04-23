pub mod client;

use client::ClientRegistry;
use serde_json::Value;
use std::io::Write;

const REGISTRY_JSON: &str = include_str!("paths.json");

pub fn run_setup(auto: bool) {
    let registry: ClientRegistry = serde_json::from_str(REGISTRY_JSON)
        .expect("Failed to parse embedded client registry");

    let exe_path = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            eprintln!("Failed to determine binary path: {e}");
            std::process::exit(1);
        }
    };

    let mut configured = 0;
    let mut skipped = 0;

    for client in &registry.clients {
        let path = match client.config_path() {
            Some(p) => p,
            None => continue,
        };

        if !path.exists() {
            skipped += 1;
            if !auto {
                println!("{} — not detected", client.name);
            }
            continue;
        }

        if !auto {
            print!("Configure {}? [Y/n] ", client.name);
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            if input.trim().eq_ignore_ascii_case("n") {
                skipped += 1;
                continue;
            }
        }

        match configure_client(&path, &client.id, &exe_path) {
            Ok(()) => {
                println!("{} — configured", client.name);
                configured += 1;
            }
            Err(e) => {
                eprintln!("{} — failed: {}", client.name, e);
                skipped += 1;
            }
        }
    }

    println!("\nDone: {} configured, {} skipped", configured, skipped);
}

fn configure_client(path: &std::path::Path, _client_id: &str, exe_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut config: Value = serde_json::from_str(&content).unwrap_or_else(|_| {
        serde_json::json!({})
    });

    if !config.is_object() {
        config = serde_json::json!({});
    }

    let servers = config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(obj) = servers.as_object_mut() {
        obj.insert(
            "blackbox".to_string(),
            serde_json::json!({
                "command": exe_path,
                "args": []
            }),
        );
    }

    let out = serde_json::to_string_pretty(&config)?;
    std::fs::write(path, out)?;
    Ok(())
}
