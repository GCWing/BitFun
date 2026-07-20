//! Config HostInvoke handlers for CLI Peer Host.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use bitfun_core::service::config::get_global_config_service;
use bitfun_core::util::errors::BitFunError;

use crate::peer_host::args::{optional_bool, request_value};

fn is_expected_config_path_not_found(error: &BitFunError, path: Option<&str>) -> bool {
    match (error, path) {
        (BitFunError::NotFound(message), Some(path)) => {
            message == &format!("Config path '{path}' not found")
        }
        _ => false,
    }
}

pub(crate) async fn get_config(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let path = request
        .get("path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let skip_retry_on_not_found = optional_bool(request, "skipRetryOnNotFound").unwrap_or(false);

    let config_service = get_global_config_service()
        .await
        .map_err(|e| format!("Failed to get config service: {e}"))?;

    match config_service.get_config::<Value>(path.as_deref()).await {
        Ok(config) => Ok(config),
        Err(e) => {
            if skip_retry_on_not_found && is_expected_config_path_not_found(&e, path.as_deref()) {
                return Err(format!("Failed to get config: {e}"));
            }
            tracing::error!("Failed to get config: path={path:?}, error={e}");
            Err(format!("Failed to get config: {e}"))
        }
    }
}

pub(crate) async fn get_configs(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let paths = request
        .get("paths")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing or invalid 'paths' field".to_string())?;
    let skip_retry_on_not_found = optional_bool(request, "skipRetryOnNotFound").unwrap_or(false);

    let config_service = get_global_config_service()
        .await
        .map_err(|e| format!("Failed to get config service: {e}"))?;

    let mut configs = BTreeMap::new();
    for path_value in paths {
        let path = path_value
            .as_str()
            .ok_or_else(|| "Invalid path entry in 'paths'".to_string())?
            .to_string();
        if configs.contains_key(&path) {
            continue;
        }
        match config_service
            .get_config::<Value>(Some(path.as_str()))
            .await
        {
            Ok(config) => {
                configs.insert(path, config);
            }
            Err(e) => {
                if skip_retry_on_not_found
                    && is_expected_config_path_not_found(&e, Some(path.as_str()))
                {
                    return Err(format!("Failed to get config: {e}"));
                }
                tracing::error!("Failed to get config: path={path}, error={e}");
                return Err(format!("Failed to get config: {e}"));
            }
        }
    }

    Ok(json!(configs))
}

pub(crate) async fn set_config(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let path = request
        .get("path")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing or invalid 'path' field".to_string())?;
    let value = request
        .get("value")
        .cloned()
        .ok_or_else(|| "Missing 'value' field".to_string())?;

    let config_service = get_global_config_service()
        .await
        .map_err(|e| format!("Failed to get config service: {e}"))?;

    config_service.set_config(&path, value).await.map_err(|e| {
        tracing::error!("Failed to set config: path={path}, error={e}");
        format!("Failed to set config: {e}")
    })?;

    // Config changed on this host via a peer controller — schedule the cloud
    // push so other same-account devices converge.
    crate::account_sync::notify_local_settings_changed();

    Ok(json!("Configuration set successfully"))
}

pub(crate) async fn get_agent_profile_config(args: &Value) -> Result<Value, String> {
    // Desktop Tauri takes top-level `agentId` (not always under `request`).
    let agent_id = crate::peer_host::args::get_string(args, "agentId")
        .or_else(|_| crate::peer_host::args::get_string(request_value(args), "agentId"))?;

    let config =
        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(&agent_id)
            .await
            .map_err(|e| format!("Failed to get agent profile config: {e}"))?;
    serde_json::to_value(config).map_err(|e| format!("Failed to serialize agent profile: {e}"))
}

pub(crate) async fn get_agent_profile_configs() -> Result<Value, String> {
    let configs =
        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_views()
            .await
            .map_err(|e| format!("Failed to get agent profile configs: {e}"))?;
    serde_json::to_value(configs)
        .map_err(|e| format!("Failed to serialize agent profile configs: {e}"))
}
