use std::sync::Arc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_foundation::{NSDictionary, NSError, NSString};
use objc2_web_kit::{WKContentWorld, WKWebView};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::runtime::script;
use crate::runtime::{BridgeError, BridgeResponse};
use crate::server::response::WebDriverErrorResponse;

pub(super) async fn evaluate_script<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    timeout_ms: u64,
    script: &str,
    args: &[Value],
    async_mode: bool,
    frame_context: &Value,
) -> Result<Value, WebDriverErrorResponse> {
    let wrapped = script::build_native_eval_script(script, args, async_mode, frame_context);
    let (sender, receiver) = oneshot::channel::<Result<String, String>>();

    let result = webview.with_webview(move |platform_webview| unsafe {
        let wk_webview: &WKWebView = &*platform_webview.inner().cast();
        let ns_script = NSString::from_str(&wrapped);
        let mtm = MainThreadMarker::new_unchecked();
        let empty_dict: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
        let content_world = WKContentWorld::pageWorld(mtm);

        let sender = Arc::new(std::sync::Mutex::new(Some(sender)));
        let block = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
            let response = if !error.is_null() {
                Err((&*error).localizedDescription().to_string())
            } else if result.is_null() {
                Ok("null".to_string())
            } else {
                ns_object_to_string(&*result)
                    .ok_or_else(|| "Script returned a non-string payload".to_string())
            };

            if let Ok(mut guard) = sender.lock() {
                if let Some(sender) = guard.take() {
                    let _ = sender.send(response);
                }
            }
        });

        wk_webview.callAsyncJavaScript_arguments_inFrame_inContentWorld_completionHandler(
            &ns_script,
            Some(&empty_dict),
            None,
            &content_world,
            Some(&block),
        );
    });

    if let Err(error) = result {
        return Err(WebDriverErrorResponse::javascript_error(
            format!("Failed to evaluate script: {error}"),
            None,
        ));
    }

    let response_payload = tokio::time::timeout(Duration::from_millis(timeout_ms), receiver)
        .await
        .map_err(|_| {
            WebDriverErrorResponse::timeout(format!("Script timed out after {timeout_ms}ms"))
        })?
        .map_err(|_| {
            WebDriverErrorResponse::unknown_error("Script response channel closed unexpectedly")
        })?
        .map_err(|error| WebDriverErrorResponse::javascript_error(error, None))?;

    let response: BridgeResponse = serde_json::from_str(&response_payload).map_err(|error| {
        WebDriverErrorResponse::unknown_error(format!("Invalid native script response: {error}"))
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

unsafe fn ns_object_to_string(obj: &AnyObject) -> Option<String> {
    let class_name = obj.class().name().to_str().unwrap_or("");
    if !class_name.contains("String") {
        return None;
    }

    let ns_string: &NSString = &*std::ptr::from_ref::<AnyObject>(obj).cast::<NSString>();
    Some(ns_string.to_string())
}
