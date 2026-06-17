use std::sync::Arc;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use std::time::Duration;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::runtime::script;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::runtime::BridgeError;
use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;
use serde_json::Value;
use tauri::Webview;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use tokio::sync::oneshot;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub(crate) async fn evaluate_script<R: tauri::Runtime>(
    state: Arc<AppState>,
    webview: Webview<R>,
    timeout_ms: u64,
    script_source: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> Result<Value, WebDriverErrorResponse> {
    #[cfg(target_os = "macos")]
    {
        let _ = state;
        return macos::evaluate_script(
            webview,
            timeout_ms,
            script_source,
            args,
            async_mode,
            frame_context,
        )
        .await;
    }

    #[cfg(target_os = "windows")]
    {
        return windows::evaluate_script(
            state,
            webview,
            timeout_ms,
            script_source,
            args,
            async_mode,
            frame_context,
        )
        .await;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let request_id = state.next_request_id();
        let (sender, receiver) = oneshot::channel();

        state
            .pending_requests
            .lock()
            .map_err(|_| {
                WebDriverErrorResponse::unknown_error("Failed to lock pending request map")
            })?
            .insert(request_id.clone(), sender);

        let injected = script::build_bridge_eval_script(
            &request_id,
            script_source,
            args,
            async_mode,
            frame_context,
        );
        webview.eval(&injected).map_err(|error| {
            remove_pending_request(&state, &request_id);
            WebDriverErrorResponse::javascript_error(
                format!("Failed to evaluate script: {error}"),
                None,
            )
        })?;

        let response = tokio::time::timeout(Duration::from_millis(timeout_ms), receiver)
            .await
            .map_err(|_| {
                remove_pending_request(&state, &request_id);
                WebDriverErrorResponse::timeout(format!("Script timed out after {timeout_ms}ms"))
            })?
            .map_err(|_| {
                WebDriverErrorResponse::unknown_error("Bridge response channel closed unexpectedly")
            })?;

        if response.ok {
            return Ok(response.value.unwrap_or(Value::Null));
        }

        let error = response.error.unwrap_or(BridgeError {
            message: Some("Unknown JavaScript error".into()),
            stack: None,
        });
        return Err(WebDriverErrorResponse::javascript_error(
            error
                .message
                .unwrap_or_else(|| "Unknown JavaScript error".into()),
            error.stack,
        ));
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn remove_pending_request(state: &AppState, request_id: &str) {
    if let Ok(mut pending) = state.pending_requests.lock() {
        pending.remove(request_id);
    }
}
