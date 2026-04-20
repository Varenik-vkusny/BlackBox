use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PRIMARY_URL: &str = "http://127.0.0.1:8768/mcp";

/// Check if a primary daemon instance is already running.
/// Uses a 500ms timeout so startup is fast even when no primary exists.
pub async fn primary_is_running() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get("http://127.0.0.1:8768/api/status")
        .send()
        .await
        .is_ok()
}

/// Run as a lightweight MCP proxy: read JSON-RPC from stdin, POST to primary daemon,
/// write responses back to stdout. No ports bound, no state. Exits on stdin EOF.
pub async fn run_mcp_proxy() {
    // eprintln!("BlackBox: primary detected on :8768 — running as MCP proxy");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Ok(Some(line)) = reader.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let result = client
            .post(PRIMARY_URL)
            .header("Content-Type", "application/json")
            .body(line)
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().as_u16() == 202 => {
                // Notification — server correctly returns 202 with no body; nothing to write
            }
            Ok(resp) => {
                match resp.bytes().await {
                    Ok(body) if !body.is_empty() => {
                        let _ = stdout.write_all(&body).await;
                        let _ = stdout.write_all(b"\n").await;
                        let _ = stdout.flush().await;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                // Forward error as JSON-RPC parse error so the client knows something went wrong
                eprintln!("BlackBox proxy: forward error: {e}");
                let err = format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32603,\"message\":\"proxy error: {}\"}}}}\n",
                    e.to_string().replace('"', "'")
                );
                let _ = stdout.write_all(err.as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
    }

    eprintln!("BlackBox proxy: stdin EOF, exiting");
}
