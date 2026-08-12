//! Configured web-tool timeouts (阈值参数配置化：`ai.thresholds.tool_timeout.*`).

use crate::service::config::get_global_config_service;

/// Resolve the configured WebFetch timeout (`ai.thresholds.tool_timeout.web_fetch_secs`),
/// falling back to `WEB_FETCH_TIMEOUT_SECS = 30` when unset or invalid.
pub(crate) async fn configured_web_fetch_timeout_secs() -> u64 {
    let Ok(config_service) = get_global_config_service().await else {
        return 30;
    };
    let Ok(thresholds) = config_service
        .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return 30;
    };
    let secs = thresholds.tool_timeout.web_fetch_secs;
    if secs == 0 {
        return 30;
    }
    secs
}

/// Resolve the configured Exa web-search timeout (`ai.thresholds.tool_timeout.exa_secs`),
/// falling back to `EXA_TIMEOUT_SECS = 25` when unset or invalid.
pub(crate) async fn configured_exa_timeout_secs() -> u64 {
    let Ok(config_service) = get_global_config_service().await else {
        return 25;
    };
    let Ok(thresholds) = config_service
        .get_config::<crate::service::config::types::AiThresholdsConfig>(Some("ai.thresholds"))
        .await
    else {
        return 25;
    };
    let secs = thresholds.tool_timeout.exa_secs;
    if secs == 0 {
        return 25;
    }
    secs
}
