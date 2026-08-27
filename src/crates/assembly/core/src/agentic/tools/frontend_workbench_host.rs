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
        return Err("FrontendWorkbench is available only in the BitFun desktop app".to_string());
    };
    handler(request).await
}
