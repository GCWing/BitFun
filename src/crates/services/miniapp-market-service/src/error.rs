use crate::request_id;
use axum::http::{header::HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use openbitfun_product_domains::miniapp::market::{MarketApiErrorBody, MarketApiErrorEnvelope};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct MarketError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl MarketError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn service_unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "MiniApp market request failed");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The marketplace could not complete this request.",
        )
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl IntoResponse for MarketError {
    fn into_response(self) -> Response {
        let request_id = request_id::current();
        tracing::warn!(
            request_id = %request_id,
            status = self.status.as_u16(),
            error_code = self.code,
            "MiniApp market request rejected"
        );
        let body = MarketApiErrorEnvelope {
            error: MarketApiErrorBody {
                code: self.code.to_string(),
                message: self.message,
                request_id: request_id.clone(),
                details: self.details,
            },
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-request-id"), value);
        }
        response
    }
}

pub type MarketResult<T> = Result<T, MarketError>;
