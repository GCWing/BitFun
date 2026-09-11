//! Resource admission for the shared GitHub identity authority.
use crate::error::MarketError;
use axum::{
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;

const MAX_AUTH_BODY: usize = 16 * 1024;
const MAX_AUTH_REQUESTS: usize = 128;
const STARTS_PER_MINUTE: u32 = 300;

struct AuthorizationRate {
    window: Instant,
    starts: u32,
    callbacks: u32,
}

impl AuthorizationRate {
    fn new() -> Self {
        Self {
            window: Instant::now(),
            starts: 0,
            callbacks: 0,
        }
    }

    fn allow(&mut self, callback: bool) -> bool {
        if self.window.elapsed() >= Duration::from_secs(60) {
            *self = Self::new();
        }
        let count = if callback {
            &mut self.callbacks
        } else {
            &mut self.starts
        };
        if *count >= STARTS_PER_MINUTE {
            return false;
        }
        *count += 1;
        true
    }
}

pub(crate) async fn admit(mut request: Request, next: Next) -> Response {
    let path = request
        .uri()
        .path()
        .strip_prefix("/miniapp/api/v1")
        .unwrap_or(request.uri().path());
    if !path.starts_with("/auth/") {
        return next.run(request).await;
    }
    if matches!(
        path,
        "/auth/github/start" | "/auth/desktop/start" | "/auth/github/callback"
    ) {
        static RATE: OnceLock<Mutex<AuthorizationRate>> = OnceLock::new();
        let allowed = RATE
            .get_or_init(|| Mutex::new(AuthorizationRate::new()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .allow(path.ends_with("/callback"));
        if !allowed {
            let mut response = MarketError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "auth_rate_limit",
                "Sign-in is busy. Please try again shortly.",
            )
            .into_response();
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
            return response;
        }
    }
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let Ok(_permit) = Arc::clone(SLOTS.get_or_init(|| Arc::new(Semaphore::new(MAX_AUTH_REQUESTS))))
        .try_acquire_owned()
    else {
        return MarketError::service_unavailable(
            "auth_capacity",
            "Sign-in is busy. Please try again shortly.",
        )
        .into_response();
    };
    if request.method() == Method::POST {
        let body = std::mem::replace(request.body_mut(), axum::body::Body::empty());
        let bytes = match tokio::time::timeout(
            Duration::from_secs(10),
            axum::body::to_bytes(body, MAX_AUTH_BODY),
        )
        .await
        {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) => {
                return MarketError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "payload_too_large",
                    "The authentication request exceeds its size limit.",
                )
                .into_response()
            }
            Err(_) => {
                return MarketError::new(
                    StatusCode::REQUEST_TIMEOUT,
                    "auth_timeout",
                    "The authentication request timed out.",
                )
                .into_response()
            }
        };
        *request.body_mut() = axum::body::Body::from(bytes);
    }
    tokio::time::timeout(Duration::from_secs(45), next.run(request))
        .await
        .unwrap_or_else(|_| {
            MarketError::new(
                StatusCode::GATEWAY_TIMEOUT,
                "auth_timeout",
                "The identity service timed out.",
            )
            .into_response()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::post, Json, Router};
    use tower::ServiceExt;

    #[test]
    fn sign_in_start_flood_does_not_consume_callback_capacity() {
        let mut rate = AuthorizationRate::new();
        for _ in 0..STARTS_PER_MINUTE {
            assert!(rate.allow(false));
        }
        assert!(!rate.allow(false));
        assert!(rate.allow(true));
        rate.window = Instant::now() - Duration::from_secs(61);
        assert!(rate.allow(false));
    }

    #[tokio::test]
    async fn auth_body_limit_is_enforced_without_changing_other_api_limits() {
        let app = Router::new()
            .route(
                "/auth/desktop/poll",
                post(|Json(_body): Json<serde_json::Value>| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/submissions",
                post(|Json(_body): Json<serde_json::Value>| async { StatusCode::NO_CONTENT }),
            )
            .layer(axum::middleware::from_fn(admit));
        for (path, expected) in [
            ("/auth/desktop/poll", StatusCode::PAYLOAD_TOO_LARGE),
            ("/submissions", StatusCode::NO_CONTENT),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::json!({"value": "a".repeat(MAX_AUTH_BODY)}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }
}
