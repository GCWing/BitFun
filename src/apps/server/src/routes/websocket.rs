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
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// WebSocket message protocol (JSON RPC 2.0 style)
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum WsMessage {
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
pub struct ErrorInfo {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// WebSocket connection handler
pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    tracing::info!("New WebSocket connection");
    ws.on_upgrade(|socket| handle_socket(socket, state))
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
        let _ = sender.send(Message::Text(json)).await;
    }

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                tracing::debug!("Received text message: {}", text);
                if let Err(e) = handle_text_message(&mut sender, &text, &state).await {
                    tracing::error!("Failed to handle message: {:?}", e);
                }
            }
            Ok(Message::Binary(data)) => {
                tracing::debug!("Received binary message: {} bytes", data.len());
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
            Err(e) => {
                tracing::error!("WebSocket error: {:?}", e);
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
            tracing::info!("Handling request: method={}, id={}", method, id);

            let result = handle_command(&method, params, state).await;

            let response = match result {
                Ok(data) => WsMessage::Response {
                    id,
                    result: Some(data),
                    error: None,
                },
                Err(e) => WsMessage::Response {
                    id,
                    result: None,
                    error: Some(ErrorInfo {
                        code: -1,
                        message: e.to_string(),
                        data: None,
                    }),
                },
            };

            let json = serde_json::to_string(&response)?;
            sender.send(Message::Text(json)).await?;
        }
        WsMessage::Event { event, .. } => {
            tracing::debug!("Received event: {}", event);
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
    _params: serde_json::Value,
    _state: &AppState,
) -> Result<serde_json::Value> {
    match method {
        "ping" => Ok(serde_json::json!({
            "pong": true,
            "timestamp": chrono::Utc::now().timestamp(),
        })),
        _ => {
            tracing::warn!("Unknown command: {}", method);
            Err(anyhow::anyhow!("Unknown command: {}", method))
        }
    }
}
