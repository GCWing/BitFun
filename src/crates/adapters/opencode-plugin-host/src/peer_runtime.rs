use crate::peer::{PeerState, RpcHandlerError};
use crate::{read_frame, write_frame, PluginHostError};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, OwnedSemaphorePermit};

pub(super) async fn run_reader(mut reader: OwnedReadHalf, state: Arc<PeerState>) {
    let mut closed = state.close_signal.subscribe();
    if *closed.borrow() {
        return;
    }
    loop {
        let message = tokio::select! {
            biased;
            change = closed.changed() => {
                let _ = change;
                return;
            }
            result = read_frame(&mut reader, state.max_frame_bytes) => result,
        };
        let result = match message {
            Ok(message) => route_message(message, state.clone()).await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            state.close(error.to_string()).await;
            return;
        }
    }
}

pub(super) async fn run_writer(
    mut writer: OwnedWriteHalf,
    mut receiver: mpsc::Receiver<Value>,
    state: Arc<PeerState>,
) {
    let mut closed = state.close_signal.subscribe();
    if *closed.borrow() {
        return;
    }
    loop {
        let message = tokio::select! {
            biased;
            change = closed.changed() => {
                let _ = change;
                receiver.close();
                return;
            }
            message = receiver.recv() => message,
        };
        let Some(message) = message else {
            state
                .close("JSON-RPC outbound channel is closed".to_string())
                .await;
            receiver.close();
            return;
        };
        if let Err(error) = write_frame(&mut writer, &message, state.max_frame_bytes).await {
            state.close(error.to_string()).await;
            receiver.close();
            return;
        }
    }
}

async fn route_message(message: Value, state: Arc<PeerState>) -> Result<(), PluginHostError> {
    let object = message.as_object().ok_or_else(|| {
        PluginHostError::Protocol("JSON-RPC message must be an object".to_string())
    })?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(protocol_error("JSON-RPC version must be 2.0"));
    }
    if object.contains_key("method") {
        return route_request(object, state);
    }
    route_response(object, &state).await
}

fn route_request(
    object: &Map<String, Value>,
    state: Arc<PeerState>,
) -> Result<(), PluginHostError> {
    if object.contains_key("result") || object.contains_key("error") {
        return Err(protocol_error(
            "JSON-RPC request must not contain result or error",
        ));
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .filter(|method| !method.is_empty())
        .ok_or_else(|| protocol_error("JSON-RPC request has no non-empty string method"))?;
    let request_id = match object.get("id") {
        Some(Value::String(request_id)) if !request_id.is_empty() => Some(request_id.clone()),
        Some(_) => {
            return Err(protocol_error(
                "JSON-RPC request id must be a non-empty string",
            ))
        }
        None => None,
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    match state.handler_limit.clone().try_acquire_owned() {
        Ok(permit) => {
            tokio::spawn(dispatch_request(
                state,
                permit,
                request_id,
                method.to_string(),
                params,
            ));
        }
        Err(_) => reject_overloaded_request(&state, request_id)?,
    }
    Ok(())
}

fn reject_overloaded_request(
    state: &PeerState,
    request_id: Option<String>,
) -> Result<(), PluginHostError> {
    let Some(request_id) = request_id else {
        return Ok(());
    };
    state
        .outbound
        .try_send(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32000,
                "message": "JSON-RPC handler concurrency limit reached"
            }
        }))
        .map_err(|_| protocol_error("JSON-RPC outbound queue is full"))
}

async fn route_response(
    object: &Map<String, Value>,
    state: &PeerState,
) -> Result<(), PluginHostError> {
    let request_id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|request_id| !request_id.is_empty())
        .ok_or_else(|| protocol_error("JSON-RPC response has no non-empty string id"))?;
    let result = match (object.get("result"), object.get("error")) {
        (Some(result), None) => Ok(result.clone()),
        (None, Some(error)) => parse_rpc_error(error),
        (Some(_), Some(_)) => Err(protocol_error(
            "JSON-RPC response must not contain both result and error",
        )),
        (None, None) => Err(protocol_error(
            "JSON-RPC response has neither result nor error",
        )),
    };
    let protocol_failure = result.as_ref().err().and_then(|error| match error {
        PluginHostError::Protocol(message) => Some(message.clone()),
        _ => None,
    });
    if let Some(sender) = state.remove_pending(request_id).await {
        log::debug!(
            "Plugin host RPC response received: generation={}, request_id={}, outcome={}",
            state.generation,
            request_id,
            if result.is_ok() { "success" } else { "error" }
        );
        let _ = sender.send(result);
    }
    match protocol_failure {
        Some(message) => Err(PluginHostError::Protocol(message)),
        None => Ok(()),
    }
}

async fn dispatch_request(
    state: Arc<PeerState>,
    _permit: OwnedSemaphorePermit,
    request_id: Option<String>,
    method: String,
    params: Value,
) {
    let handler = state.handlers.read().await.get(&method).cloned();
    let result = match handler {
        Some(handler) => handler(params).await,
        None => Err(RpcHandlerError::new(
            -32601,
            format!("Method not found: {method}"),
        )),
    };
    let Some(request_id) = request_id else {
        return;
    };
    let response = match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": request_id, "result": value}),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": error.code, "message": error.message, "data": error.data},
        }),
    };
    let _ = state.outbound.send(response).await;
}

fn parse_rpc_error(value: &Value) -> Result<Value, PluginHostError> {
    let Some(object) = value.as_object() else {
        return Err(protocol_error("JSON-RPC error must be an object"));
    };
    let code = object
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| protocol_error("JSON-RPC error has no integer code"))?;
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("JSON-RPC error has no string message"))?;
    Err(PluginHostError::Rpc {
        code,
        message: message.to_string(),
        data: object.get("data").cloned(),
    })
}

fn protocol_error(message: &str) -> PluginHostError {
    PluginHostError::Protocol(message.to_string())
}
