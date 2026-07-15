//! Configuration API

use crate::api::app_state::AppState;
use crate::startup_trace::DesktopStartupTrace;
use bitfun_core::util::errors::BitFunError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigRequest {
    pub path: Option<String>,
    #[serde(default)]
    pub skip_retry_on_not_found: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigsRequest {
    pub paths: Vec<String>,
    #[serde(default)]
    pub skip_retry_on_not_found: bool,
}

#[derive(Debug, Deserialize)]
pub struct SetConfigRequest {
    pub path: String,
    pub value: Value,
}

#[derive(Debug, Deserialize)]
pub struct ResetConfigRequest {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigRequest {
    pub config_data: Value,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetRuntimeLoggingInfoRequest {}

#[derive(Debug, Deserialize, Default)]
pub struct ExportDiagnosticsBundleRequest {}

fn to_json_value<T: Serialize>(value: T, context: &str) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("Failed to serialize {}: {}", context, e))
}

fn is_expected_config_path_not_found(error: &BitFunError, path: Option<&str>) -> bool {
    match (error, path) {
        (BitFunError::NotFound(message), Some(path)) => {
            message == &format!("Config path '{}' not found", path)
        }
        _ => false,
    }
}

#[tauri::command]
pub async fn get_config(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: GetConfigRequest,
) -> Result<Value, String> {
    let config_service = &state.config_service;
    let trace_started = Instant::now();
    let trace_target = request.path.clone();

    let result = match config_service
        .get_config::<Value>(request.path.as_deref())
        .await
    {
        Ok(config) => Ok(config),
        Err(e) => {
            if request.skip_retry_on_not_found
                && is_expected_config_path_not_found(&e, request.path.as_deref())
            {
                Err(format!("Failed to get config: {}", e))
            } else {
                error!("Failed to get config: path={:?}, error={}", request.path, e);
                Err(format!("Failed to get config: {}", e))
            }
        }
    };
    startup_trace.record_tauri_command_elapsed(
        "get_config",
        trace_target.as_deref(),
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn get_configs(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: GetConfigsRequest,
) -> Result<BTreeMap<String, Value>, String> {
    let config_service = &state.config_service;
    let mut configs = BTreeMap::new();
    let trace_started = Instant::now();
    let trace_target = request.paths.join(",");

    for path in request.paths {
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
                if request.skip_retry_on_not_found
                    && is_expected_config_path_not_found(&e, Some(path.as_str()))
                {
                    startup_trace.record_tauri_command_elapsed(
                        "get_configs",
                        Some(&trace_target),
                        trace_started,
                    );
                    return Err(format!("Failed to get config: {}", e));
                }
                error!("Failed to get config: path={}, error={}", path, e);
                startup_trace.record_tauri_command_elapsed(
                    "get_configs",
                    Some(&trace_target),
                    trace_started,
                );
                return Err(format!("Failed to get config: {}", e));
            }
        }
    }

    startup_trace.record_tauri_command_elapsed("get_configs", Some(&trace_target), trace_started);
    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_expected_config_path_not_found_errors() {
        let error = BitFunError::NotFound(
            "Config path 'ai.review_team_rate_limit_status' not found".to_string(),
        );

        assert!(is_expected_config_path_not_found(
            &error,
            Some("ai.review_team_rate_limit_status"),
        ));
        assert!(!is_expected_config_path_not_found(
            &error,
            Some("ai.review_teams.default"),
        ));
        assert!(!is_expected_config_path_not_found(&error, None));
        assert!(!is_expected_config_path_not_found(
            &BitFunError::config("Config path 'ai.review_team_rate_limit_status' not found"),
            Some("ai.review_team_rate_limit_status"),
        ));
    }
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
    request: SetConfigRequest,
) -> Result<String, String> {
    let config_service = &state.config_service;
    let trace_started = Instant::now();
    let trace_target = request.path.clone();

    let result = match config_service
        .set_config(&request.path, request.value)
        .await
    {
        Ok(_) => {
            if request.path.starts_with("ai.models")
                || request.path.starts_with("ai.default_models")
                || request.path.starts_with("ai.agent_models")
                || request.path.starts_with("ai.stream_idle_timeout_secs")
                || request.path.starts_with("ai.stream_ttft_timeout_secs")
                || request.path.starts_with("ai.proxy")
            {
                state.ai_client_factory.invalidate_cache();
                info!(
                    "AI config changed, cache invalidated: path={}",
                    request.path
                );
            }

            // Notify auto-sync to upload the updated config to the relay
            crate::api::remote_connect_api::notify_settings_changed();

            Ok("Configuration set successfully".to_string())
        }
        Err(e) => {
            error!("Failed to set config: path={}, error={}", request.path, e);
            Err(format!("Failed to set config: {}", e))
        }
    };
    startup_trace.record_tauri_command_elapsed(
        "set_config",
        Some(trace_target.as_str()),
        trace_started,
    );
    result
}

#[tauri::command]
pub async fn reset_config(
    state: State<'_, AppState>,
    request: ResetConfigRequest,
) -> Result<String, String> {
    let config_service = &state.config_service;

    match config_service.reset_config(request.path.as_deref()).await {
        Ok(_) => {
            let message = if let Some(path) = &request.path {
                format!("Configuration '{}' reset successfully", path)
            } else {
                "All configurations reset successfully".to_string()
            };

            let should_invalidate = match &request.path {
                Some(path) => path.starts_with("ai"),
                None => true,
            };
            if should_invalidate {
                state.ai_client_factory.invalidate_cache();
                info!(
                    "AI config reset, cache invalidated: path={:?}",
                    request.path
                );
            }

            // Notify auto-sync: config reset, upload to relay
            crate::api::remote_connect_api::notify_settings_changed();

            Ok(message)
        }
        Err(e) => {
            error!(
                "Failed to reset config: path={:?}, error={}",
                request.path, e
            );
            Err(format!("Failed to reset config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn export_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config_service = &state.config_service;

    match config_service.export_config().await {
        Ok(export_data) => Ok(to_json_value(export_data, "export config data")?),
        Err(e) => {
            error!("Failed to export config: {}", e);
            Err(format!("Failed to export config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn import_config(
    state: State<'_, AppState>,
    request: ImportConfigRequest,
) -> Result<Value, String> {
    let config_service = &state.config_service;
    let config_data = request
        .config_data
        .get("config")
        .cloned()
        .unwrap_or(request.config_data);

    match config_service.import_config_data(config_data).await {
        Ok(result) => {
            state.ai_client_factory.invalidate_cache();
            info!("Config imported, AI client cache invalidated");
            // Notify auto-sync: config changed, upload to relay
            crate::api::remote_connect_api::notify_settings_changed();
            Ok(to_json_value(result, "import config result")?)
        }
        Err(e) => {
            error!("Failed to import config: {}", e);
            Err(format!("Failed to import config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn validate_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config_service = &state.config_service;

    match config_service.validate_config().await {
        Ok(validation_result) => Ok(to_json_value(
            validation_result,
            "config validation result",
        )?),
        Err(e) => {
            error!("Failed to validate config: {}", e);
            Err(format!("Failed to validate config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn reload_config(state: State<'_, AppState>) -> Result<String, String> {
    let config_service = &state.config_service;

    match config_service.reload().await {
        Ok(_) => {
            info!("Config reloaded");
            Ok("Configuration reloaded successfully".to_string())
        }
        Err(e) => {
            error!("Failed to reload config: {}", e);
            Err(format!("Failed to reload config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn sync_config_to_global(_state: State<'_, AppState>) -> Result<String, String> {
    match bitfun_core::service::config::reload_global_config().await {
        Ok(_) => {
            info!("Config synced to global service");
            Ok("Configuration synced to global service".to_string())
        }
        Err(e) => {
            error!("Failed to sync config to global service: {}", e);
            Err(format!("Failed to sync config to global service: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_global_config_health() -> Result<bool, String> {
    Ok(bitfun_core::service::config::GlobalConfigManager::is_initialized())
}

#[tauri::command]
pub async fn get_runtime_logging_info(
    startup_trace: State<'_, DesktopStartupTrace>,
    _state: State<'_, AppState>,
    _request: GetRuntimeLoggingInfoRequest,
) -> Result<Value, String> {
    let trace_started = Instant::now();
    let logging_info = crate::logging::get_runtime_logging_info();
    let result = to_json_value(logging_info, "runtime logging info");
    startup_trace.record_tauri_command_elapsed("get_runtime_logging_info", None, trace_started);
    result
}

#[tauri::command]
pub async fn export_diagnostics_bundle(
    _state: State<'_, AppState>,
    _request: ExportDiagnosticsBundleRequest,
) -> Result<Value, String> {
    let bundle_info = crate::crash_diagnostics::export_diagnostics_bundle()?;
    to_json_value(bundle_info, "diagnostics bundle info")
}

#[tauri::command]
pub async fn get_agent_profile_configs(_state: State<'_, AppState>) -> Result<Value, String> {
    let agent_profiles =
        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_views()
            .await
            .map_err(|e| format!("Failed to get agent profile configs: {}", e))?;

    to_json_value(agent_profiles, "agent profile configs")
}

#[tauri::command]
pub async fn get_agent_profile_config(
    _state: State<'_, AppState>,
    agent_id: String,
) -> Result<Value, String> {
    let config =
        bitfun_core::service::config::mode_config_canonicalizer::get_agent_profile_view(&agent_id)
            .await
            .map_err(|e| format!("Failed to get agent profile config: {}", e))?;

    to_json_value(config, "agent profile config")
}

#[tauri::command]
pub async fn set_agent_profile_config(
    state: State<'_, AppState>,
    agent_id: String,
    config: Value,
) -> Result<String, String> {
    let _ = state;

    match bitfun_core::service::config::mode_config_canonicalizer::persist_agent_profile_from_value(
        &agent_id, config,
    )
    .await
    {
        Ok(_) => Ok(format!("Agent profile for '{}' set successfully", agent_id)),
        Err(e) => {
            error!(
                "Failed to set agent profile config: agent_id={}, error={}",
                agent_id, e
            );
            Err(format!("Failed to set agent profile config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn reset_agent_profile_config(
    _state: State<'_, AppState>,
    agent_id: String,
) -> Result<String, String> {
    match bitfun_core::service::config::mode_config_canonicalizer::reset_agent_profile_to_default(
        &agent_id,
    )
    .await
    {
        Ok(_) => Ok(format!(
            "Agent profile for '{}' reset successfully",
            agent_id
        )),
        Err(e) => {
            error!(
                "Failed to reset agent profile config: agent_id={}, error={}",
                agent_id, e
            );
            Err(format!("Failed to reset agent profile config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn canonicalize_agent_profile_configs(
    _state: State<'_, AppState>,
) -> Result<Value, String> {
    match bitfun_core::service::config::mode_config_canonicalizer::canonicalize_agent_profile_configs(
    )
    .await {
        Ok(report) => {
            info!(
                "Agent profile configs canonicalized: removed_profiles={}, updated_profiles={}",
                report.removed_profile_configs.len(),
                report.updated_profiles.len()
            );
            Ok(to_json_value(
                report,
                "agent profile config canonicalization report",
            )?)
        }
        Err(e) => {
            error!("Failed to canonicalize agent profile configs: {}", e);
            Err(format!("Failed to canonicalize agent profile configs: {}", e))
        }
    }
}
