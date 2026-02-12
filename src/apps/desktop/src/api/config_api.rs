//! Configuration API

use crate::api::app_state::AppState;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct GetConfigRequest {
    pub path: Option<String>,
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
            error!("Failed to get config: path={:?}, error={}", request.path, e);
            Err(format!("Failed to get config: {}", e))
        }
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
pub async fn get_mode_configs(state: State<'_, AppState>) -> Result<Value, String> {
    use bitfun_core::service::config::types::ModeConfig;
    use std::collections::HashMap;

    let config_service = &state.config_service;
    let mut mode_configs: HashMap<String, ModeConfig> = config_service
        .get_config(Some("ai.mode_configs"))
        .await
        .unwrap_or_default();

    let all_modes = state.agent_registry.get_modes_info().await;
    let mut needs_save = false;

    for mode in all_modes {
        let mode_id = mode.id;
        let default_tools = mode.default_tools;

        if !mode_configs.contains_key(&mode_id) {
            let new_config = ModeConfig {
                mode_id: mode_id.clone(),
                available_tools: default_tools.clone(),
                enabled: true,
                default_tools: default_tools,
            };
            mode_configs.insert(mode_id.clone(), new_config);
            needs_save = true;
        } else if let Some(config) = mode_configs.get_mut(&mode_id) {
            config.default_tools = default_tools.clone();
        }
    }

    if needs_save {
        match to_json_value(&mode_configs, "mode configs") {
            Ok(mode_configs_value) => {
                if let Err(e) = config_service
                    .set_config("ai.mode_configs", mode_configs_value)
                    .await
                {
                    warn!("Failed to save initialized mode configs: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to serialize initialized mode configs: {}", e);
            }
        }
    }

    Ok(to_json_value(mode_configs, "mode configs")?)
}

#[tauri::command]
pub async fn get_mode_config(state: State<'_, AppState>, mode_id: String) -> Result<Value, String> {
    use bitfun_core::service::config::types::ModeConfig;

    let config_service = &state.config_service;
    let agent_registry = &state.agent_registry;
    let path = format!("ai.mode_configs.{}", mode_id);
    let config_result = config_service.get_config::<ModeConfig>(Some(&path)).await;

    let config = match config_result {
        Ok(existing_config) => {
            let mut cfg = existing_config;
            if let Some(mode) = agent_registry.get_mode_agent(&mode_id) {
                cfg.default_tools = mode.default_tools();
            }
            cfg
        }
        Err(_) => {
            if let Some(mode) = agent_registry.get_mode_agent(&mode_id) {
                let default_tools = mode.default_tools();
                let new_config = ModeConfig {
                    mode_id: mode_id.clone(),
                    available_tools: default_tools.clone(),
                    enabled: true,
                    default_tools: default_tools,
                };
                match to_json_value(&new_config, "initial mode config") {
                    Ok(new_config_value) => {
                        if let Err(e) = config_service.set_config(&path, new_config_value).await {
                            warn!(
                                "Failed to save initial mode config: mode_id={}, error={}",
                                mode_id, e
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to serialize initial mode config: mode_id={}, error={}",
                            mode_id, e
                        );
                    }
                }
                new_config
            } else {
                ModeConfig {
                    mode_id: mode_id.clone(),
                    available_tools: vec![],
                    enabled: true,
                    default_tools: vec![],
                }
            }
        }
    };

    Ok(to_json_value(config, "mode config")?)
}

#[tauri::command]
pub async fn set_mode_config(
    state: State<'_, AppState>,
    mode_id: String,
    config: Value,
) -> Result<String, String> {
    let config_service = &state.config_service;
    let path = format!("ai.mode_configs.{}", mode_id);

    match config_service.set_config(&path, config).await {
        Ok(_) => {
            if let Err(e) = bitfun_core::service::config::reload_global_config().await {
                warn!(
                    "Failed to reload global config after mode config change: mode_id={}, error={}",
                    mode_id, e
                );
            } else {
                info!(
                    "Global config reloaded after mode config change: mode_id={}",
                    mode_id
                );
            }

            Ok(format!("Mode '{}' configuration set successfully", mode_id))
        }
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
    state: State<'_, AppState>,
    mode_id: String,
) -> Result<String, String> {
    use bitfun_core::service::config::types::ModeConfig;

    let agent_registry = &state.agent_registry;
    let default_tools = if let Some(mode) = agent_registry.get_mode_agent(&mode_id) {
        mode.default_tools()
    } else {
        return Err(format!("Mode does not exist: {}", mode_id));
    };

    let default_config = ModeConfig {
        mode_id: mode_id.clone(),
        available_tools: default_tools.clone(),
        enabled: true,
        default_tools: default_tools,
    };

    let config_service = &state.config_service;
    let path = format!("ai.mode_configs.{}", mode_id);
    let default_config_value = to_json_value(&default_config, "default mode config")?;

    match config_service.set_config(&path, default_config_value).await {
        Ok(_) => {
            if let Err(e) = bitfun_core::service::config::reload_global_config().await {
                warn!(
                    "Failed to reload global config after mode config reset: mode_id={}, error={}",
                    mode_id, e
                );
            } else {
                info!(
                    "Global config reloaded after mode config reset: mode_id={}",
                    mode_id
                );
            }

            Ok(format!(
                "Mode '{}' configuration reset successfully",
                mode_id
            ))
        }
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
pub async fn get_subagent_configs(state: State<'_, AppState>) -> Result<Value, String> {
    use bitfun_core::service::config::types::SubAgentConfig;
    use std::collections::HashMap;

    let config_service = &state.config_service;
    let mut subagent_configs: HashMap<String, SubAgentConfig> = config_service
        .get_config(Some("ai.subagent_configs"))
        .await
        .unwrap_or_default();

    let all_subagents = state.agent_registry.get_subagents_info().await;
    let mut needs_save = false;

    for subagent in all_subagents {
        let subagent_id = subagent.id;
        if !subagent_configs.contains_key(&subagent_id) {
            subagent_configs.insert(subagent_id, SubAgentConfig { enabled: true });
            needs_save = true;
        }
    }

    if needs_save {
        match to_json_value(&subagent_configs, "subagent configs") {
            Ok(subagent_configs_value) => {
                if let Err(e) = config_service
                    .set_config("ai.subagent_configs", subagent_configs_value)
                    .await
                {
                    warn!("Failed to save initialized subagent configs: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to serialize initialized subagent configs: {}", e);
            }
        }
    }

    Ok(to_json_value(subagent_configs, "subagent configs")?)
}

#[tauri::command]
pub async fn set_subagent_config(
    state: State<'_, AppState>,
    subagent_id: String,
    enabled: bool,
) -> Result<String, String> {
    use bitfun_core::service::config::types::SubAgentConfig;

    let config_service = &state.config_service;
    let config = SubAgentConfig { enabled };
    let path = format!("ai.subagent_configs.{}", subagent_id);
    let config_value = to_json_value(&config, "subagent config")?;

    match config_service.set_config(&path, config_value).await {
        Ok(_) => {
            if let Err(e) = bitfun_core::service::config::reload_global_config().await {
                warn!("Failed to reload global config after subagent config change: subagent_id={}, error={}", subagent_id, e);
            } else {
                info!("Global config reloaded after subagent config change: subagent_id={}, enabled={}", subagent_id, enabled);
            }

            Ok(format!(
                "SubAgent '{}' configuration set successfully",
                subagent_id
            ))
        }
        Err(e) => {
            error!(
                "Failed to set subagent config: subagent_id={}, enabled={}, error={}",
                subagent_id, enabled, e
            );
            Err(format!("Failed to set SubAgent config: {}", e))
        }
    }
}

#[tauri::command]
pub async fn sync_tool_configs(_state: State<'_, AppState>) -> Result<Value, String> {
    match bitfun_core::service::config::tool_config_sync::sync_tool_configs().await {
        Ok(report) => {
            info!(
                "Tool configs synced: new_tools={}, deleted_tools={}, updated_modes={}",
                report.new_tools.len(),
                report.deleted_tools.len(),
                report.updated_modes.len()
            );
            Ok(to_json_value(report, "tool config sync report")?)
        }
        Err(e) => {
            error!("Failed to sync tool configs: {}", e);
            Err(format!("Failed to sync tool configs: {}", e))
        }
    }
}
