use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tauri::{Runtime, Webview};
use tokio::sync::oneshot;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2WebMessageReceivedEventArgs,
    ICoreWebView2WebMessageReceivedEventHandler, ICoreWebView2WebMessageReceivedEventHandler_Impl,
};
use windows::core::implement;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

use super::remove_pending_request;
use crate::runtime::{script, BridgeError};
use crate::server::response::WebDriverErrorResponse;
use crate::server::AppState;

static REGISTERED_WEBVIEWS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub(super) async fn evaluate_script<R: Runtime>(
    state: std::sync::Arc<AppState>,
    webview: Webview<R>,
    timeout_ms: u64,
    script_source: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> Result<Value, WebDriverErrorResponse> {
    ensure_message_handler(&webview)?;

    let request_id = state.next_request_id();
    let (sender, receiver) = oneshot::channel();

    state
        .pending_requests
        .lock()
        .map_err(|_| WebDriverErrorResponse::unknown_error("Failed to lock pending request map"))?
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

    Err(WebDriverErrorResponse::javascript_error(
        error
            .message
            .unwrap_or_else(|| "Unknown JavaScript error".into()),
        error.stack,
    ))
}

fn ensure_message_handler<R: Runtime>(webview: &Webview<R>) -> Result<(), WebDriverErrorResponse> {
    let label = webview.label().to_string();
    let registry = REGISTERED_WEBVIEWS.get_or_init(|| Mutex::new(HashSet::new()));

    {
        let registered = registry.lock().map_err(|_| {
            WebDriverErrorResponse::unknown_error("Failed to lock WebDriver message registry")
        })?;
        if registered.contains(&label) {
            return Ok(());
        }
    }

    let registration_result = std::sync::Arc::new(std::sync::Mutex::new(Ok::<(), String>(())));
    let registration_result_slot = registration_result.clone();
    let result = webview.with_webview(move |platform_webview| unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let outcome = match platform_webview.controller().CoreWebView2() {
            Ok(webview2) => register_message_handler(&webview2).map_err(|error| error.message),
            Err(error) => Err(format!("Failed to access CoreWebView2: {error:?}")),
        };

        if let Ok(mut guard) = registration_result_slot.lock() {
            *guard = outcome;
        }
    });

    match result {
        Ok(()) => {
            let outcome = registration_result.lock().map_err(|_| {
                WebDriverErrorResponse::unknown_error("Failed to read WebView2 registration result")
            })?;
            if let Err(error) = &*outcome {
                return Err(WebDriverErrorResponse::unknown_error(error.clone()));
            }
            registry
                .lock()
                .map_err(|_| {
                    WebDriverErrorResponse::unknown_error(
                        "Failed to update WebDriver message registry",
                    )
                })?
                .insert(label);
            Ok(())
        }
        Err(error) => Err(WebDriverErrorResponse::unknown_error(format!(
            "Failed to register WebView2 message handler: {error}"
        ))),
    }
}

#[implement(ICoreWebView2WebMessageReceivedEventHandler)]
struct WebMessageReceivedHandler;

impl ICoreWebView2WebMessageReceivedEventHandler_Impl for WebMessageReceivedHandler_Impl {
    fn Invoke(
        &self,
        _sender: windows::core::Ref<'_, ICoreWebView2>,
        args: windows::core::Ref<'_, ICoreWebView2WebMessageReceivedEventArgs>,
    ) -> windows::core::Result<()> {
        let Some(args) = args.clone() else {
            return Ok(());
        };

        let mut msg_ptr = windows::core::PWSTR::null();
        if unsafe { args.WebMessageAsJson(&raw mut msg_ptr) }.is_err() {
            log::warn!("Failed to read WebView2 WebMessage JSON");
            return Ok(());
        }

        let msg_text = unsafe { msg_ptr.to_string().unwrap_or_default() };
        let payload = parse_message_payload(&msg_text);

        match payload {
            Some(payload) => {
                if let Err(error) = crate::handle_bridge_result(payload) {
                    log::warn!("Failed to dispatch WebView2 bridge payload: {}", error);
                }
            }
            None => {
                log::warn!("Ignoring invalid WebView2 bridge payload: {}", msg_text);
            }
        }

        Ok(())
    }
}

unsafe fn register_message_handler(webview: &ICoreWebView2) -> Result<(), WebDriverErrorResponse> {
    let handler: ICoreWebView2WebMessageReceivedEventHandler = WebMessageReceivedHandler.into();
    let mut token = std::mem::zeroed();
    webview
        .add_WebMessageReceived(&handler, &raw mut token)
        .map_err(|error| {
            WebDriverErrorResponse::unknown_error(format!(
                "Failed to register WebView2 message handler: {error:?}"
            ))
        })?;

    std::mem::forget(handler);
    Ok(())
}

fn parse_message_payload(message: &str) -> Option<Value> {
    if let Ok(inner) = serde_json::from_str::<String>(message) {
        serde_json::from_str(&inner).ok()
    } else {
        serde_json::from_str::<Value>(message).ok()
    }
}
