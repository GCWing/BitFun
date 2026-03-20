//! AI Relay - Lightweight HTTP proxy for AI API requests
//!
//! When running BitFun Server on a remote machine that cannot directly access
//! AI APIs (due to network restrictions), AI Relay acts as a local proxy:
//!
//! ```text
//! Remote Server                    Local Machine
//! ┌─────────────┐    SSH Tunnel    ┌─────────────┐
//! │ BitFun      │ ───────────────► │ AI Relay    │ ──► AI API
//! │ Server      │   ssh -R 9090:   │ :9090       │     (OpenAI, etc.)
//! └─────────────┘                  └─────────────┘
//! ```
//!
//! Usage:
//! 1. Start AI Relay on local machine: `bitfun-server --ai-relay --port 9090`
//! 2. SSH to remote with reverse tunnel: `ssh -R 9090:localhost:9090 user@remote`
//! 3. Configure remote BitFun to use proxy: `proxy_url = "http://localhost:9090"`

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    response::{IntoResponse, Response},
    Router,
};
use reqwest::Client;
use std::sync::Arc;

/// AI Relay state
#[derive(Clone)]
pub struct RelayState {
    pub client: Client,
}

/// Create the AI Relay router
pub fn create_relay_router() -> Router {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("Failed to create HTTP client");

    let state = Arc::new(RelayState { client });

    Router::new()
        .fallback(handle_proxy)
        .with_state(state)
}

/// Handle all incoming requests and proxy them
async fn handle_proxy(
    State(state): State<Arc<RelayState>>,
    req: Request,
) -> Result<Response, ProxyError> {
    // Get the target URL from the request
    let uri = req.uri().clone();

    // Reconstruct the target URL
    let target_url = reconstruct_target_url(&uri)?;

    log::info!(
        "AI Relay: proxying {} {}",
        req.method(),
        target_url
    );

    // Remove hop-by-hop headers
    let mut headers = req.headers().clone();
    remove_hop_by_hop_headers(&mut headers);

    // Build the proxied request
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);
    
    let mut builder = state.client.request(method, &target_url);

    // Add headers
    for (name, value) in headers.iter() {
        if let Ok(header_value) = HeaderValue::from_bytes(value.as_bytes()) {
            builder = builder.header(name.as_str(), header_value);
        }
    }

    // Add body if present
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| ProxyError::RequestError(format!("Failed to read body: {}", e)))?;

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes);
    }

    // Send request
    let response = builder
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("Failed to connect to upstream: {}", e)))?;

    log::info!(
        "AI Relay: received response status {}",
        response.status()
    );

    // Build response
    let status = response.status();
    let mut resp_builder = axum::http::Response::builder()
        .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));

    for (name, value) in response.headers() {
        if let Ok(header_name) = HeaderName::try_from(name.as_str()) {
            if let Ok(header_value) = HeaderValue::from_bytes(value.as_bytes()) {
                resp_builder = resp_builder.header(header_name, header_value);
            }
        }
    }

    let body_bytes = response
        .bytes()
        .await
        .map_err(|e| ProxyError::UpstreamError(format!("Failed to read response: {}", e)))?;

    resp_builder
        .body(Body::from(body_bytes))
        .map_err(|e| ProxyError::ResponseError(format!("Failed to build response: {}", e)))
}

/// Reconstruct the target URL from the request
fn reconstruct_target_url(uri: &Uri) -> Result<String, ProxyError> {
    // The request should contain the actual target in the path
    // Format: /{scheme}/{host}/{path}
    // Example: /https/api.openai.com/v1/chat/completions

    let path = uri.path();

    // Skip the leading slash and extract scheme
    let path = path.strip_prefix('/').ok_or_else(|| {
        ProxyError::InvalidRequest("Invalid path format".to_string())
    })?;

    let parts: Vec<&str> = path.splitn(3, '/').collect();
    if parts.is_empty() {
        return Err(ProxyError::InvalidRequest(
            "Missing scheme and host in path".to_string(),
        ));
    }

    let scheme = *parts.get(0).unwrap_or(&"https");
    let host = parts.get(1).ok_or_else(|| {
        ProxyError::InvalidRequest("Missing host in path".to_string())
    })?;
    let rest = parts.get(2).unwrap_or(&"");

    // Build the target URL
    let target_url = if rest.is_empty() {
        format!("{}://{}", scheme, host)
    } else {
        format!("{}://{}/{}", scheme, host, rest)
    };

    // Add query string if present
    if let Some(query) = uri.query() {
        Ok(format!("{}?{}", target_url, query))
    } else {
        Ok(target_url)
    }
}

/// Remove hop-by-hop headers that should not be forwarded
fn remove_hop_by_hop_headers(headers: &mut HeaderMap) {
    const HOP_BY_HOP: &[&str] = &[
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        "host",
    ];

    for header in HOP_BY_HOP {
        headers.remove(*header);
    }
}

/// Proxy errors
#[derive(Debug)]
pub enum ProxyError {
    InvalidRequest(String),
    RequestError(String),
    UpstreamError(String),
    ResponseError(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ProxyError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ProxyError::RequestError(msg) => (StatusCode::BAD_REQUEST, msg),
            ProxyError::UpstreamError(msg) => (StatusCode::BAD_GATEWAY, msg),
            ProxyError::ResponseError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
        };

        let body = serde_json::json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconstruct_target_url() {
        // Test with full path
        let uri: Uri = "/https/api.openai.com/v1/chat/completions".parse().unwrap();
        let result = reconstruct_target_url(&uri).unwrap();
        assert_eq!(result, "https://api.openai.com/v1/chat/completions");

        // Test with query string
        let uri: Uri = "/https/api.openai.com/v1/models?limit=10"
            .parse()
            .unwrap();
        let result = reconstruct_target_url(&uri).unwrap();
        assert_eq!(result, "https://api.openai.com/v1/models?limit=10");

        // Test with host only
        let uri: Uri = "/https/api.openai.com".parse().unwrap();
        let result = reconstruct_target_url(&uri).unwrap();
        assert_eq!(result, "https://api.openai.com");
    }
}
