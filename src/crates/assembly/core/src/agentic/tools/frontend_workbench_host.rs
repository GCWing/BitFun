//! Host boundary for the packaged-frontend workbench.
//!
//! Core owns the tool contract; the desktop adapter owns revision storage,
//! webview navigation, confirmation UI, and rollback timing.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendWorkbenchHostRequest {
    pub action: String,
    pub draft_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_workbench_request_round_trips_without_new_fields() {
        let old = serde_json::json!({ "action": "prepare", "draftId": null });
        let request: FrontendWorkbenchHostRequest = serde_json::from_value(old.clone()).unwrap();
        assert!(request.command_id.is_none());
        assert!(request.arguments.is_none());
        assert_eq!(serde_json::to_value(request).unwrap(), old);
    }
}

pub type FrontendWorkbenchFuture =
    Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'static>>;
pub type FrontendWorkbenchHandler =
    Arc<dyn Fn(FrontendWorkbenchHostRequest) -> FrontendWorkbenchFuture + Send + Sync>;

static FRONTEND_WORKBENCH_HANDLER: OnceLock<FrontendWorkbenchHandler> = OnceLock::new();

pub fn set_frontend_workbench_handler(handler: FrontendWorkbenchHandler) {
    let _ = FRONTEND_WORKBENCH_HANDLER.set(handler);
}

pub fn frontend_workbench_host_available() -> bool {
    FRONTEND_WORKBENCH_HANDLER.get().is_some()
}

pub async fn invoke_frontend_workbench(
    request: FrontendWorkbenchHostRequest,
) -> Result<Value, String> {
    let Some(handler) = FRONTEND_WORKBENCH_HANDLER.get() else {
        return Err(
            "FrontendWorkbench is available only in the OpenBitFun desktop app".to_string(),
        );
    };
    handler(request).await
}
