use anyhow::Result;
/// WebSocket handler
///
/// Implements real-time bidirectional communication with frontend:
/// - Command request/response (JSON RPC format)
/// - Event push (streaming output, tool calls, etc.)
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header::ORIGIN, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::AppState;

const MAX_WS_TEXT_BYTES: usize = 256 * 1024;

/// WebSocket message protocol (JSON RPC 2.0 style)
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum WsMessage {
    /// Request message
    #[serde(rename = "request")]
    Request {
        id: String,
        method: String,
        params: serde_json::Value,
    },
    /// Response message
    #[serde(rename = "response")]
    Response {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ErrorInfo>,
    },
    /// Event message (no response required)
    #[serde(rename = "event")]
    Event {
        event: String,
        payload: serde_json::Value,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct ErrorInfo {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// WebSocket connection handler
pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if !browser_origin_allowed(&headers, &state) {
        tracing::warn!("Rejected WebSocket upgrade from untrusted browser origin");
        return StatusCode::FORBIDDEN.into_response();
    }
    tracing::info!("New WebSocket connection");
    ws.max_message_size(MAX_WS_TEXT_BYTES)
        .max_frame_size(MAX_WS_TEXT_BYTES)
        .on_upgrade(|socket| handle_socket(socket, state))
}

fn browser_origin_allowed(headers: &HeaderMap, state: &AppState) -> bool {
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    crate::normalize_browser_origin(origin)
        .is_ok_and(|origin| state.allowed_browser_origins.contains(&origin))
}

/// Handle a single WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("WebSocket connection established");

    let welcome_msg = WsMessage::Event {
        event: "connection_established".to_string(),
        payload: serde_json::json!({
            "server": "BitFun Server",
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": chrono::Utc::now().timestamp(),
        }),
    };

    if let Ok(json) = serde_json::to_string(&welcome_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if text.len() > MAX_WS_TEXT_BYTES {
                    tracing::warn!(
                        message_bytes = text.len(),
                        "Rejected oversized WebSocket message"
                    );
                    break;
                }
                if handle_text_message(&mut sender, &text, &state)
                    .await
                    .is_err()
                {
                    tracing::warn!(
                        error_category = "message_processing",
                        "Failed to handle WebSocket message"
                    );
                }
            }
            Ok(Message::Binary(data)) => {
                tracing::warn!(
                    message_bytes = data.len(),
                    "Rejected unsupported binary WebSocket message"
                );
                break;
            }
            Ok(Message::Ping(data)) => {
                tracing::trace!("Received Ping");
                let _ = sender.send(Message::Pong(data)).await;
            }
            Ok(Message::Pong(_)) => {
                tracing::trace!("Received Pong");
            }
            Ok(Message::Close(_)) => {
                tracing::info!("Client closed connection");
                break;
            }
            Err(_) => {
                tracing::warn!(error_category = "transport", "WebSocket connection failed");
                break;
            }
        }
    }

    tracing::info!("WebSocket connection closed");
}

/// Handle text message
async fn handle_text_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    text: &str,
    state: &AppState,
) -> Result<()> {
    let ws_msg: WsMessage = serde_json::from_str(text)?;

    match ws_msg {
        WsMessage::Request { id, method, params } => {
            tracing::info!(
                method = safe_protocol_token(&method),
                id_kind = "string",
                "Handling WebSocket request"
            );

            let result = handle_command(&method, params, state).await;

            let response = match result {
                Ok(data) => WsMessage::Response {
                    id,
                    result: Some(data),
                    error: None,
                },
                Err(error) => WsMessage::Response {
                    id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: json_rpc_error_code(error.code),
                        message: error.detail.clone(),
                        data: serde_json::to_value(error).ok(),
                    }),
                },
            };

            let json = serde_json::to_string(&response)?;
            sender.send(Message::Text(json.into())).await?;
        }
        WsMessage::Event { event, .. } => {
            tracing::debug!(
                event = safe_protocol_token(&event),
                "Received WebSocket event"
            );
        }
        WsMessage::Response { .. } => {
            tracing::warn!("Received response message (client should not send responses)");
        }
    }

    Ok(())
}

/// Handle specific commands
async fn handle_command(
    method: &str,
    params: serde_json::Value,
    state: &AppState,
) -> bitfun_core::external_sources::ExternalSourceOperationResult<serde_json::Value> {
    if super::external_sources::supports(method) {
        return super::external_sources::dispatch(method, params, state).await;
    }
    match method {
        "ping" => Ok(serde_json::json!({
            "pong": true,
            "timestamp": chrono::Utc::now().timestamp(),
        })),
        _ => {
            tracing::warn!(
                method = safe_protocol_token(method),
                "Unknown Server Host command"
            );
            Err(bitfun_core::external_sources::ExternalSourceOperationError::host_capability_unavailable(
                "Unknown Server Host operation",
            ))
        }
    }
}

fn safe_protocol_token(value: &str) -> &str {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        value
    } else {
        "<invalid>"
    }
}

fn json_rpc_error_code(
    code: bitfun_core::external_sources::ExternalSourceOperationErrorCode,
) -> i32 {
    use bitfun_core::external_sources::ExternalSourceOperationErrorCode;
    match code {
        ExternalSourceOperationErrorCode::InvalidRequest => -32602,
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable => -32601,
        ExternalSourceOperationErrorCode::StaleRevision
        | ExternalSourceOperationErrorCode::Conflict => -32009,
        ExternalSourceOperationErrorCode::HostUnavailable
        | ExternalSourceOperationErrorCode::Unavailable
        | ExternalSourceOperationErrorCode::RuntimeUnavailable
        | ExternalSourceOperationErrorCode::DependencyFailed
        | ExternalSourceOperationErrorCode::Timeout
        | ExternalSourceOperationErrorCode::Overloaded
        | ExternalSourceOperationErrorCode::ProcessLost
        | ExternalSourceOperationErrorCode::TemporarilyUnavailable => -32003,
        ExternalSourceOperationErrorCode::Cancelled => -32800,
        ExternalSourceOperationErrorCode::TrustRequired
        | ExternalSourceOperationErrorCode::PolicyIncompatible
        | ExternalSourceOperationErrorCode::PolicyLimited
        | ExternalSourceOperationErrorCode::Unsupported
        | ExternalSourceOperationErrorCode::IncompatibleVersion => -32010,
        ExternalSourceOperationErrorCode::NotFound => -32004,
        ExternalSourceOperationErrorCode::InvalidResponse
        | ExternalSourceOperationErrorCode::Internal => -32603,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_allowed_origins(origins: &[&str]) -> AppState {
        AppState {
            external_workspace_root: None,
            allowed_browser_origins: std::sync::Arc::new(
                origins.iter().map(|origin| (*origin).to_string()).collect(),
            ),
        }
    }

    #[test]
    fn browser_origin_requires_an_exact_allowlist_match() {
        let state = state_with_allowed_origins(&["http://localhost:1422"]);
        let mut allowed_headers = HeaderMap::new();
        allowed_headers.insert(ORIGIN, "http://localhost:1422".parse().unwrap());
        assert!(browser_origin_allowed(&allowed_headers, &state));

        let mut unknown_headers = HeaderMap::new();
        unknown_headers.insert(ORIGIN, "https://example.test".parse().unwrap());
        assert!(!browser_origin_allowed(&unknown_headers, &state));
        assert!(browser_origin_allowed(&HeaderMap::new(), &state));
    }

    #[test]
    fn typed_errors_keep_stable_json_rpc_categories() {
        use bitfun_core::external_sources::ExternalSourceOperationErrorCode;

        assert_eq!(
            json_rpc_error_code(ExternalSourceOperationErrorCode::InvalidRequest),
            -32602
        );
        assert_eq!(
            json_rpc_error_code(ExternalSourceOperationErrorCode::HostCapabilityUnavailable),
            -32601
        );
        assert_eq!(
            json_rpc_error_code(ExternalSourceOperationErrorCode::PolicyLimited),
            -32010
        );
    }

    #[test]
    fn client_protocol_tokens_are_bounded_before_logging() {
        assert_eq!(
            safe_protocol_token("get_external_source_snapshot"),
            "get_external_source_snapshot"
        );
        assert_eq!(safe_protocol_token("method\nsecret"), "<invalid>");
        assert_eq!(safe_protocol_token(&"x".repeat(65)), "<invalid>");
    }
}
