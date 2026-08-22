//! Platform-neutral host port for controlling the active BitFun product surface.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitFunControlHostRequest {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
    #[serde(default)]
    pub arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

pub type BitFunControlFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
pub type BitFunControlHandler =
    Arc<dyn Fn(BitFunControlHostRequest) -> BitFunControlFuture + Send + Sync>;

static BITFUN_CONTROL_HANDLER: OnceLock<BitFunControlHandler> = OnceLock::new();

/// Register the product-surface adapter. The first registered host owns the process.
pub fn set_bitfun_control_handler(handler: BitFunControlHandler) {
    let _ = BITFUN_CONTROL_HANDLER.set(handler);
}

pub fn bitfun_control_host_available() -> bool {
    BITFUN_CONTROL_HANDLER.get().is_some()
}

pub async fn invoke_bitfun_control(request: BitFunControlHostRequest) -> Result<Value, String> {
    let Some(handler) = BITFUN_CONTROL_HANDLER.get() else {
        return Err("BitFunControl host is not available on this product surface".to_string());
    };
    handler(request).await
}
