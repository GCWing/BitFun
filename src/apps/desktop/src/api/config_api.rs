//! Configuration API

use crate::api::app_state::AppState;
use bitfun_core::util::errors::BitFunError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigRequest {
    pub path: Option<String>,
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

#[derive(Debug, Deserialize, Default)]
pub struct GetRuntimeLoggingInfoRequest {}

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
    request: GetConfigRequest,
) -> Result<Value, String> {
    let config_service = &state.config_service;

    match config_service
        .get_config::<Value>(request.path.as_deref())
        .await
    {
        Ok(config) => Ok(config),
        Err(e) => {
            if request.skip_retry_on_not_found
                && is_expected_config_path_not_found(&e, request.path.as_deref())
            {
                return Err(format!("Failed to get config: {}", e));
            }
            error!("Failed to get config: path={:?}, error={}", request.path, e);
            Err(format!("Failed to get config: {}", e))
        }
    }
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
            Some("ai.review_team_project_strategy_overrides"),
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
    request: SetConfigRequest,
) -> Result<String, String> {
    let config_service = &state.config_service;

    match config_service
        .set_config(&request.path, request.value)
        .await
    {
        Ok(_) => {
            if request.path.starts_with("ai.models")
                || request.path.starts_with("ai.default_models")
                || request.path.starts_with("ai.agent_models")
                || request.path.starts_with("ai.stream_idle_timeout_secs")
                || request.path.starts_with("ai.proxy")
            {
                state.ai_client_factory.invalidate_cache();
                info!(
                    "AI config changed, cache invalidated: path={}",
                    request.path
                );
            }

            Ok("Configuration set successfully".to_string())
        }
        Err(e) => {
            error!("Failed to set config: path={}, error={}", request.path, e);
            Err(format!("Failed to set config: {}", e))
        }
    }
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
pub async fn import_config(state: State<'_, AppState>, config: Value) -> Result<Value, String> {
    let config_service = &state.config_service;

    let export_data: bitfun_core::service::config::ConfigExport =
        serde_json::from_value(config).map_err(|e| format!("Invalid config format: {}", e))?;

    match config_service.import_config(export_data).await {
        Ok(result) => {
            state.ai_client_factory.invalidate_cache();
            info!("Config imported, AI client cache invalidated");
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
    _state: State<'_, AppState>,
    _request: GetRuntimeLoggingInfoRequest,
) -> Result<Value, String> {
    let logging_info = crate::logging::get_runtime_logging_info();
    to_json_value(logging_info, "runtime logging info")
}

#[tauri::command]
pub async fn get_mode_configs(_state: State<'_, AppState>) -> Result<Value, String> {
    let mode_configs =
        bitfun_core::service::config::mode_config_canonicalizer::get_mode_config_views()
            .await
            .map_err(|e| format!("Failed to get mode configs: {}", e))?;

    to_json_value(mode_configs, "mode configs")
}

#[tauri::command]
pub async fn get_mode_config(
    _state: State<'_, AppState>,
    mode_id: String,
) -> Result<Value, String> {
    let config =
        bitfun_core::service::config::mode_config_canonicalizer::get_mode_config_view(&mode_id)
            .await
            .map_err(|e| format!("Failed to get mode config: {}", e))?;

    to_json_value(config, "mode config")
}

#[tauri::command]
pub async fn set_mode_config(
    state: State<'_, AppState>,
    mode_id: String,
    config: Value,
) -> Result<String, String> {
    let _ = state;

    match bitfun_core::service::config::mode_config_canonicalizer::persist_mode_config_from_value(
        &mode_id, config,
    )
    .await
    {
        Ok(_) => Ok(format!("Mode '{}' configuration set successfully", mode_id)),
        Err(e) => {
            error!(
                "Failed to set mode config: mode_id={}, error={}",
                mode_id, e
            );
            Err(format!("Failed to set mode config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn reset_mode_config(
    _state: State<'_, AppState>,
    mode_id: String,
) -> Result<String, String> {
    match bitfun_core::service::config::mode_config_canonicalizer::reset_mode_config_to_default(
        &mode_id,
    )
    .await
    {
        Ok(_) => Ok(format!(
            "Mode '{}' configuration reset successfully",
            mode_id
        )),
        Err(e) => {
            error!(
                "Failed to reset mode config: mode_id={}, error={}",
                mode_id, e
            );
            Err(format!("Failed to reset mode config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn canonicalize_mode_configs(_state: State<'_, AppState>) -> Result<Value, String> {
    match bitfun_core::service::config::mode_config_canonicalizer::canonicalize_mode_configs().await
    {
        Ok(report) => {
            info!(
                "Mode configs canonicalized: removed_modes={}, updated_modes={}",
                report.removed_mode_configs.len(),
                report.updated_modes.len()
            );
            Ok(to_json_value(
                report,
                "mode config canonicalization report",
            )?)
        }
        Err(e) => {
            error!("Failed to canonicalize mode configs: {}", e);
            Err(format!("Failed to canonicalize mode configs: {}", e))
        }
    }
}
