//! Desktop adapter for the platform-neutral BitFunControl tool host port.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bitfun_core::agentic::tools::bitfun_control_host::{
    set_bitfun_control_handler, BitFunControlHostRequest,
};
use bitfun_core::infrastructure::events::{emit_global_event, BackendEvent};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::oneshot;

const BITFUN_CONTROL_REQUEST_EVENT: &str = "agentic://bitfun-control-request";
const BITFUN_CONTROL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

type PendingResponse = oneshot::Sender<Result<Value, String>>;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static PENDING_RESPONSES: OnceLock<Mutex<HashMap<String, PendingResponse>>> = OnceLock::new();

fn pending_responses() -> &'static Mutex<HashMap<String, PendingResponse>> {
    PENDING_RESPONSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_pending_responses() -> std::sync::MutexGuard<'static, HashMap<String, PendingResponse>> {
    pending_responses()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn dispatch_request(request: BitFunControlHostRequest) -> Result<Value, String> {
    let request_id = format!(
        "bitfun-control-{}-{}",
        std::process::id(),
        NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    );
    let (sender, receiver) = oneshot::channel();
    lock_pending_responses().insert(request_id.clone(), sender);

    let mut payload = serde_json::to_value(request).map_err(|error| error.to_string())?;
    let Some(payload) = payload.as_object_mut() else {
        lock_pending_responses().remove(&request_id);
        return Err("BitFunControl request serialization produced an invalid payload".to_string());
    };
    payload.insert("requestId".to_string(), Value::String(request_id.clone()));

    if let Err(error) = emit_global_event(BackendEvent::Custom {
        event_name: BITFUN_CONTROL_REQUEST_EVENT.to_string(),
        payload: Value::Object(payload.clone()),
    })
    .await
    {
        lock_pending_responses().remove(&request_id);
        return Err(format!("Failed to send BitFunControl request: {error}"));
    }

    match tokio::time::timeout(BITFUN_CONTROL_RESPONSE_TIMEOUT, receiver).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("BitFunControl response channel closed".to_string()),
        Err(_) => {
            lock_pending_responses().remove(&request_id);
            Err("BitFunControl timed out waiting for the active product surface".to_string())
        }
    }
}

pub(crate) fn install() {
    set_bitfun_control_handler(Arc::new(|request| {
        Box::pin(async move { dispatch_request(request).await })
    }));
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportBitFunControlResultRequest {
    request_id: String,
    success: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

/// Return a product-surface result to the waiting BitFunControl tool call.
#[tauri::command]
pub(crate) async fn report_bitfun_control_result(
    request: ReportBitFunControlResultRequest,
) -> Result<(), String> {
    let sender = lock_pending_responses()
        .remove(&request.request_id)
        .ok_or_else(|| "BitFunControl request is no longer pending".to_string())?;
    let result = if request.success {
        Ok(request.result.unwrap_or(Value::Null))
    } else {
        Err(request
            .error
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| "BitFunControl request failed".to_string()))
    };
    sender
        .send(result)
        .map_err(|_| "BitFunControl request receiver is no longer available".to_string())
}
