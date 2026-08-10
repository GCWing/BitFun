use axum::extract::Request;
use axum::http::{header::HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

tokio::task_local! {
    static CURRENT_REQUEST_ID: String;
}

pub(crate) async fn middleware(request: Request, next: Next) -> Response {
    let header = HeaderName::from_static("x-request-id");
    let request_id = request
        .headers()
        .get(&header)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_request_id(value))
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let span = tracing::info_span!("skin_market_request", request_id = %request_id);
    let mut response = CURRENT_REQUEST_ID
        .scope(request_id.clone(), next.run(request).instrument(span))
        .await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(header, value);
    }
    response
}

pub(crate) fn current_or_new() -> String {
    CURRENT_REQUEST_ID
        .try_with(Clone::clone)
        .unwrap_or_else(|_| Uuid::new_v4().to_string())
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
