use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::time::Instant;
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";

tokio::task_local! {
    static REQUEST_ID: String;
}

pub(crate) async fn middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let started = Instant::now();
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_request_id(value))
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let response_id = request_id.clone();
    let mut response = REQUEST_ID
        .scope(request_id, async move { next.run(request).await })
        .await;
    if let Ok(value) = HeaderValue::from_str(&response_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    tracing::info!(
        request_id = %response_id,
        method = %method,
        path = %path,
        status = response.status().as_u16(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "MiniApp market request completed"
    );
    response
}

pub(crate) fn current() -> String {
    REQUEST_ID
        .try_with(Clone::clone)
        .unwrap_or_else(|_| Uuid::new_v4().to_string())
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_proxy_ids_but_rejects_log_injection() {
        assert!(valid_request_id("2f84a7f1-420d-4b93"));
        assert!(valid_request_id("nginx_request_42"));
        assert!(!valid_request_id(""));
        assert!(!valid_request_id("line\nbreak"));
        assert!(!valid_request_id(&"a".repeat(65)));
    }
}
