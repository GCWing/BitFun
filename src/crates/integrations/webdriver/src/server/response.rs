use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct WebDriverResponse {
    pub value: Value,
}

impl WebDriverResponse {
    pub fn success<T: Serialize>(value: T) -> Self {
        Self {
            value: serde_json::to_value(value).unwrap_or(Value::Null),
        }
    }

    pub fn null() -> Self {
        Self { value: Value::Null }
    }
}

impl IntoResponse for WebDriverResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [("Content-Type", "application/json; charset=utf-8")],
            Json(self),
        )
            .into_response()
    }
}

#[derive(Debug)]
pub struct WebDriverErrorResponse {
    pub status: StatusCode,
    pub error: String,
    pub message: String,
    pub stacktrace: Option<String>,
}

impl WebDriverErrorResponse {
    pub fn new(
        status: StatusCode,
        error: impl Into<String>,
        message: impl Into<String>,
        stacktrace: Option<String>,
    ) -> Self {
        Self {
            status,
            error: error.into(),
            message: message.into(),
            stacktrace,
        }
    }

    pub fn invalid_session_id(session_id: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "invalid session id",
            format!("Unknown session: {session_id}"),
            None,
        )
    }

    pub fn no_such_window(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such window", message, None)
    }

    pub fn no_such_element(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such element", message, None)
    }

    pub fn stale_element_reference(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "stale element reference",
            message,
            None,
        )
    }

    pub fn no_such_frame(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such frame", message, None)
    }

    pub fn session_not_created(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "session not created",
            message,
            None,
        )
    }

    pub fn javascript_error(message: impl Into<String>, stacktrace: Option<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "javascript error",
            message,
            stacktrace,
        )
    }

    pub fn unknown_error(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unknown error",
            message,
            None,
        )
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid argument", message, None)
    }

    pub fn invalid_selector(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid selector", message, None)
    }

    pub fn no_such_cookie(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such cookie", message, None)
    }

    pub fn no_such_alert(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such alert", message, None)
    }

    pub fn no_such_shadow_root(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "no such shadow root", message, None)
    }

    pub fn unsupported_operation(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unsupported operation",
            message,
            None,
        )
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(StatusCode::REQUEST_TIMEOUT, "timeout", message, None)
    }
}

impl IntoResponse for WebDriverErrorResponse {
    fn into_response(self) -> Response {
        (
            self.status,
            [("Content-Type", "application/json; charset=utf-8")],
            Json(json!({
                "value": {
                    "error": self.error,
                    "message": self.message,
                    "stacktrace": self.stacktrace.unwrap_or_default()
                }
            })),
        )
            .into_response()
    }
}

pub type WebDriverResult = Result<WebDriverResponse, WebDriverErrorResponse>;
