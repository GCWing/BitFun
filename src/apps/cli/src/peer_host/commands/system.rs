//! System HostInvoke handlers.

use serde_json::{json, Value};

use openbitfun_core::service::token_usage::TokenUsageStatisticsRequest;

use crate::peer_host::state::PeerHostState;

pub(crate) async fn get_system_info() -> Result<Value, String> {
    let info = openbitfun_core::service::system::get_system_info();
    Ok(json!({
        "platform": info.platform,
        "arch": info.arch,
        "osVersion": info.os_version,
    }))
}

pub(crate) async fn get_token_usage_statistics(
    state: &PeerHostState,
    args: &Value,
) -> Result<Value, String> {
    let request = args
        .get("request")
        .cloned()
        .ok_or_else(|| "Missing 'request' field in args".to_string())?;
    let request: TokenUsageStatisticsRequest =
        serde_json::from_value(request).map_err(|error| error.to_string())?;
    let statistics = state
        .token_usage_service
        .get_statistics_for_request(request)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::to_value(statistics).map_err(|error| error.to_string())
}
