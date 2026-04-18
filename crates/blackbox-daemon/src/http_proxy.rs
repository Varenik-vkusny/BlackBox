use std::net::SocketAddr;
use std::time::Instant;

use axum::{
    body::Bytes,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::http_store::{push_http_event, HttpEvent, SharedHttpStore};

const BODY_CAP: usize = 512;

pub async fn run_http_proxy(store: SharedHttpStore, port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .fallback(proxy_handler)
        .layer(cors)
        .with_state(store);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("BlackBox http_proxy: failed to bind {addr}: {e}");
            return;
        }
    };
    eprintln!("BlackBox http_proxy: listening on {addr}");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("BlackBox http_proxy: server error: {e}");
    }
}

async fn proxy_handler(
    State(store): State<SharedHttpStore>,
    req: Request,
) -> impl IntoResponse {
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    // Determine target URL:
    // 1. Absolute URI (standard HTTP proxy format): GET http://host/path HTTP/1.1
    // 2. X-Proxy-Target header: X-Proxy-Target: http://localhost:3000
    let target = build_target_url(&uri, &headers);
    let target = match target {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                "BlackBox proxy: cannot determine target URL. Use absolute URI or X-Proxy-Target header.\n".to_string(),
            ).into_response();
        }
    };

    // Read request body (capped for storage)
    let body_bytes = match axum::body::to_bytes(req.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => Bytes::new(),
    };
    let req_body_preview = body_preview(&body_bytes);

    // Forward the request
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("BlackBox proxy: client error: {e}\n")).into_response();
        }
    };

    let start = Instant::now();
    let fwd_method = reqwest::Method::from_bytes(method.as_bytes())
        .unwrap_or(reqwest::Method::GET);

    let mut fwd_req = client.request(fwd_method, &target).body(body_bytes.to_vec());
    // Forward select headers (skip hop-by-hop)
    for (k, v) in &headers {
        let name = k.as_str().to_lowercase();
        if name == "host" || name == "connection" || name == "proxy-connection" || name == "x-proxy-target" {
            continue;
        }
        fwd_req = fwd_req.header(k, v);
    }

    let resp = match fwd_req.send().await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("BlackBox proxy: upstream error: {e}\n")).into_response();
        }
    };

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();

    // Read response body
    let resp_headers = resp.headers().clone();
    let resp_status = resp.status();
    let resp_bytes = resp.bytes().await.unwrap_or_default();
    let resp_body_preview = body_preview(&resp_bytes);

    // Log only 4xx/5xx (apply PII masking to bodies before storing)
    if status >= 400 {
        let now = crate::buffer::now_ms();
        let masked_req = req_body_preview.map(|b| crate::pii_masker::mask_pii(&b));
        let masked_resp = resp_body_preview.map(|b| crate::pii_masker::mask_pii(&b));
        push_http_event(
            &store,
            HttpEvent {
                method: method.clone(),
                url: target.clone(),
                status,
                latency_ms,
                request_body: masked_req,
                response_body: masked_resp,
                timestamp_ms: now,
            },
        );
    }

    // Build response back to caller
    let mut builder = axum::http::Response::builder().status(resp_status);
    for (k, v) in &resp_headers {
        let name = k.as_str().to_lowercase();
        if name == "transfer-encoding" || name == "connection" {
            continue;
        }
        builder = builder.header(k, v);
    }
    builder
        .body(axum::body::Body::from(resp_bytes))
        .unwrap_or_else(|_| axum::http::Response::new(axum::body::Body::empty()))
        .into_response()
}

fn build_target_url(uri: &Uri, headers: &HeaderMap) -> Option<String> {
    // Check X-Proxy-Target header first
    if let Some(t) = headers.get("x-proxy-target").and_then(|v| v.to_str().ok()) {
        let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        // If target already ends at a path, just use it; otherwise append request path
        if t.contains("://") {
            let base = t.trim_end_matches('/');
            return Some(format!("{}{}", base, path));
        }
    }

    // Absolute URI form: http://host/path
    if uri.scheme().is_some() {
        return Some(uri.to_string());
    }

    None
}

fn body_preview(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let s = String::from_utf8_lossy(&bytes[..bytes.len().min(BODY_CAP)]).to_string();
    Some(s)
}
