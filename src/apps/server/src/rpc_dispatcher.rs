//! WebSocket RPC command dispatcher.
//!
//! Maps Tauri command names (used by the frontend `api.invoke()`) to
//! server-side handler functions. Each handler receives the raw JSON
//! `params` and returns a JSON `result`.

use crate::bootstrap::ServerAppState;
use anyhow::{anyhow, Result};
use bitfun_core::agentic::core::SessionConfig;
use bitfun_core::agentic::coordination::{DialogSubmissionPolicy, DialogTriggerSource};
use std::path::PathBuf;
use std::sync::Arc;

/// Dispatch a WebSocket RPC method call to the appropriate handler.
///
/// The `method` string matches the Tauri command name exactly (e.g.
/// `"open_workspace"`, `"terminal_create"`), so the frontend's
/// `api.invoke(name, args)` works identically over both Tauri IPC and
/// WebSocket.
pub async fn dispatch(
    method: &str,
    params: serde_json::Value,
    state: &Arc<ServerAppState>,
) -> Result<serde_json::Value> {
    match method {
        // ── Ping ──────────────────────────────────────────────
        "ping" => Ok(serde_json::json!({
            "pong": true,
            "timestamp": chrono::Utc::now().timestamp(),
        })),

        // ── Health / Status ──────────────────────────────────
        "get_health_status" => {
            let uptime = state.start_time.elapsed().as_secs();
            Ok(serde_json::json!({
                "status": "healthy",
                "message": "All services are running normally",
                "services": {
                    "workspace_service": true,
                    "config_service": true,
                    "filesystem_service": true,
                },
                "uptime_seconds": uptime,
            }))
        }

        // ── Workspace ────────────────────────────────────────
        "open_workspace" => {
            let request = extract_request(&params)?;
            let path: String = serde_json::from_value(
                request.get("path").cloned().ok_or_else(|| anyhow!("Missing path"))?,
            )?;
            let info = state.workspace_service.open_workspace(path.into()).await
                .map_err(|e| anyhow!("{}", e))?;
            *state.workspace_path.write().await = Some(info.root_path.clone());
            Ok(serde_json::to_value(&info).unwrap_or_default())
        }
        "get_current_workspace" => {
            let ws = state.workspace_service.get_current_workspace().await;
            Ok(serde_json::to_value(&ws).unwrap_or(serde_json::Value::Null))
        }
        "get_recent_workspaces" => {
            let list = state.workspace_service.get_recent_workspaces().await;
            Ok(serde_json::to_value(&list).unwrap_or_default())
        }
        "get_opened_workspaces" => {
            let list = state.workspace_service.get_opened_workspaces().await;
            Ok(serde_json::to_value(&list).unwrap_or_default())
        }

        // ── File System ──────────────────────────────────────
        "read_file_content" => {
            let request = extract_request(&params)?;
            let file_path = get_string(&request, "filePath")?;
            let result = state.filesystem_service.read_file(&file_path).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!(result.content))
        }
        "write_file_content" => {
            let request = extract_request(&params)?;
            let file_path = get_string(&request, "filePath")?;
            let content = get_string(&request, "content")?;
            state.filesystem_service.write_file(&file_path, &content).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::Value::Null)
        }
        "check_path_exists" => {
            let path_str = if let Some(req) = params.get("request") {
                get_string(req, "path")?
            } else {
                get_string(&params, "path")?
            };
            let exists = std::path::Path::new(&path_str).exists();
            Ok(serde_json::json!(exists))
        }
        "get_file_tree" => {
            let request = extract_request(&params)?;
            let path = get_string(&request, "path")?;
            let nodes = state.filesystem_service.build_file_tree(&path).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::to_value(&nodes).unwrap_or_default())
        }
        "fs_exists" => {
            let path_str = get_string(&params, "path")?;
            let exists = std::path::Path::new(&path_str).exists();
            Ok(serde_json::json!(exists))
        }

        // ── Config ───────────────────────────────────────────
        "get_config" => {
            let request = extract_request(&params)?;
            let key = request.get("key").and_then(|v| v.as_str());
            let config: serde_json::Value = state.config_service
                .get_config(key).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(config)
        }
        "set_config" => {
            let request = extract_request(&params)?;
            let key = get_string(&request, "key")?;
            let value = request.get("value").cloned().ok_or_else(|| anyhow!("Missing value"))?;
            state.config_service.set_config(&key, value).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!("ok"))
        }
        "get_model_configs" => {
            let models = state.config_service.get_ai_models().await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::to_value(&models).unwrap_or_default())
        }

        // ── Agentic (Session / Dialog) ───────────────────────
        "create_session" => {
            let request = extract_request(&params)?;
            let session_name = get_string(&request, "sessionName")?;
            let agent_type = get_string(&request, "agentType")?;
            let workspace_path = get_string(&request, "workspacePath")?;
            let session_id = request.get("sessionId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let config = SessionConfig {
                workspace_path: Some(workspace_path.clone()),
                ..Default::default()
            };

            let session = state.coordinator
                .create_session_with_workspace(
                    session_id,
                    session_name,
                    agent_type,
                    config,
                    workspace_path,
                )
                .await
                .map_err(|e| anyhow!("{}", e))?;

            Ok(serde_json::json!({
                "sessionId": session.session_id,
                "sessionName": session.session_name,
                "agentType": session.agent_type,
            }))
        }
        "list_sessions" => {
            let request = extract_request(&params)?;
            let workspace_path = get_string(&request, "workspacePath")?;
            let sessions = state.coordinator
                .list_sessions(&PathBuf::from(workspace_path))
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::to_value(&sessions).unwrap_or_default())
        }
        "delete_session" => {
            let request = extract_request(&params)?;
            let session_id = get_string(&request, "sessionId")?;
            let workspace_path = get_string(&request, "workspacePath")?;
            state.coordinator
                .delete_session(&PathBuf::from(workspace_path), &session_id)
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!({ "success": true }))
        }
        "start_dialog_turn" => {
            let request = extract_request(&params)?;
            let session_id = get_string(&request, "sessionId")?;
            let user_input = get_string(&request, "userInput")?;
            let original_user_input = request.get("originalUserInput")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let agent_type = get_string(&request, "agentType")?;
            let workspace_path = request.get("workspacePath")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let turn_id = request.get("turnId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            state.scheduler
                .submit(
                    session_id,
                    user_input,
                    original_user_input,
                    turn_id,
                    agent_type,
                    workspace_path,
                    DialogSubmissionPolicy::for_source(DialogTriggerSource::DesktopUi),
                    None,
                )
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!({ "success": true, "message": "Dialog turn started" }))
        }
        "cancel_dialog_turn" => {
            let request = extract_request(&params)?;
            let session_id = get_string(&request, "sessionId")?;
            let dialog_turn_id = get_string(&request, "dialogTurnId")?;
            state.coordinator
                .cancel_dialog_turn(&session_id, &dialog_turn_id)
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!({ "success": true }))
        }
        "get_session_messages" => {
            let request = extract_request(&params)?;
            let session_id = get_string(&request, "sessionId")?;
            let messages = state.coordinator
                .get_messages(&session_id)
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::to_value(&messages).unwrap_or_default())
        }
        "confirm_tool_execution" => {
            let request = extract_request(&params)?;
            let tool_id = get_string(&request, "toolId")?;
            let updated_input = request.get("updatedInput").cloned();
            state.coordinator
                .confirm_tool(&tool_id, updated_input)
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!({ "success": true }))
        }
        "reject_tool_execution" => {
            let request = extract_request(&params)?;
            let tool_id = get_string(&request, "toolId")?;
            let reason = request.get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("User rejected")
                .to_string();
            state.coordinator
                .reject_tool(&tool_id, reason)
                .await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!({ "success": true }))
        }

        // ── I18n ─────────────────────────────────────────────
        "i18n_get_current_language" => {
            let lang: String = state.config_service
                .get_config(Some("app.language")).await
                .unwrap_or_else(|_| "zh-CN".to_string());
            Ok(serde_json::json!(lang))
        }
        "i18n_set_language" => {
            let request = extract_request(&params)?;
            let language = get_string(&request, "language")?;
            state.config_service.set_config("app.language", language.clone()).await
                .map_err(|e| anyhow!("{}", e))?;
            Ok(serde_json::json!(language))
        }
        "i18n_get_supported_languages" => {
            Ok(serde_json::json!([
                {"id": "zh-CN", "name": "Chinese (Simplified)", "englishName": "Chinese (Simplified)", "nativeName": "简体中文", "rtl": false},
                {"id": "en-US", "name": "English", "englishName": "English", "nativeName": "English", "rtl": false}
            ]))
        }

        // ── Tools ────────────────────────────────────────────
        "get_all_tools_info" => {
            let tools: Vec<serde_json::Value> = state.tool_registry_snapshot
                .iter()
                .map(|t| serde_json::json!({
                    "name": t.name().to_string(),
                }))
                .collect();
            Ok(serde_json::json!(tools))
        }

        // ── Fallback ─────────────────────────────────────────
        _ => {
            log::warn!("Unknown RPC method: {}", method);
            Err(anyhow!("Unknown command: {}", method))
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Extract the `request` field from params (Tauri convention: `{ request: { ... } }`).
fn extract_request(params: &serde_json::Value) -> Result<&serde_json::Value> {
    params
        .get("request")
        .ok_or_else(|| anyhow!("Missing 'request' field in params"))
}

fn get_string(obj: &serde_json::Value, key: &str) -> Result<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing or invalid '{}' field", key))
}
