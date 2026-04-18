//! Live App API — Tauri commands for CRUD, JS Worker, and dialog.

use crate::api::app_state::AppState;
use bitfun_core::infrastructure::events::{emit_global_event, BackendEvent};
use bitfun_core::live_app::{
    InstallResult as CoreInstallResult, LiveApp, LiveAppAiContext, LiveAppMeta, LiveAppPermissions,
    LiveAppSource,
};
use bitfun_core::service::config::types::GlobalConfig;
use bitfun_core::util::types::Message;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

// ============== Request/Response DTOs ==============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLiveAppRequest {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: LiveAppSourceDto,
    #[serde(default)]
    pub permissions: LiveAppPermissions,
    pub ai_context: Option<LiveAppAiContext>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppSourceDto {
    pub html: String,
    pub css: String,
    #[serde(default)]
    pub ui_js: String,
    #[serde(default)]
    pub esm_dependencies: Vec<EsmDepDto>,
    #[serde(default)]
    pub worker_js: String,
    #[serde(default)]
    pub npm_dependencies: Vec<NpmDepDto>,
}

#[derive(Debug, Deserialize)]
pub struct EsmDepDto {
    pub name: String,
    pub version: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NpmDepDto {
    pub name: String,
    pub version: String,
}

impl From<LiveAppSourceDto> for LiveAppSource {
    fn from(d: LiveAppSourceDto) -> Self {
        LiveAppSource {
            html: d.html,
            css: d.css,
            ui_js: d.ui_js,
            esm_dependencies: d
                .esm_dependencies
                .into_iter()
                .map(|x| bitfun_core::live_app::EsmDep {
                    name: x.name,
                    version: x.version,
                    url: x.url,
                })
                .collect(),
            worker_js: d.worker_js,
            npm_dependencies: d
                .npm_dependencies
                .into_iter()
                .map(|x| bitfun_core::live_app::NpmDep {
                    name: x.name,
                    version: x.version,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLiveAppRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<LiveAppSourceDto>,
    pub permissions: Option<LiveAppPermissions>,
    pub ai_context: Option<LiveAppAiContext>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLiveAppRequest {
    pub app_id: String,
    pub theme: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppWorkerCallRequest {
    pub app_id: String,
    pub method: String,
    pub params: Value,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppRecompileRequest {
    pub app_id: String,
    pub theme: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppImportFromPathRequest {
    pub path: String,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppSyncFromFsRequest {
    pub app_id: String,
    pub theme: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RuntimeStatus {
    pub available: bool,
    pub kind: Option<String>,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RecompileResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

fn live_app_payload(app: &LiveApp, reason: &str) -> Value {
    json!({
        "id": app.id,
        "name": app.name,
        "version": app.version,
        "updatedAt": app.updated_at,
        "reason": reason,
        "runtime": {
            "sourceRevision": app.runtime.source_revision,
            "depsRevision": app.runtime.deps_revision,
            "depsDirty": app.runtime.deps_dirty,
            "workerRestartRequired": app.runtime.worker_restart_required,
            "uiRecompileRequired": app.runtime.ui_recompile_required,
        }
    })
}

async fn emit_live_app_event(event_name: &str, payload: Value) {
    let _ = emit_global_event(BackendEvent::Custom {
        event_name: event_name.to_string(),
        payload,
    })
    .await;
}

fn workspace_root_from_input(workspace_path: Option<&str>) -> Option<PathBuf> {
    workspace_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

async fn maybe_stop_worker(state: &State<'_, AppState>, app: &LiveApp) {
    if app.runtime.worker_restart_required {
        if let Some(ref pool) = state.js_worker_pool {
            pool.stop(&app.id).await;
        }
        emit_live_app_event(
            "liveapp-worker-stopped",
            json!({ "id": app.id, "reason": "pending-restart" }),
        )
        .await;
    }
}

async fn ensure_worker_dependencies(
    state: &State<'_, AppState>,
    app_id: &str,
    app: &mut LiveApp,
) -> Result<bool, String> {
    let pool = state
        .js_worker_pool
        .as_ref()
        .ok_or_else(|| "JS Worker pool not initialized".to_string())?;

    let needs_install = !app.source.npm_dependencies.is_empty()
        && (app.runtime.deps_dirty || !pool.has_installed_deps(app_id));
    if !needs_install {
        return Ok(false);
    }

    let install = pool
        .install_deps(app_id, &app.source.npm_dependencies)
        .await
        .map_err(|e| e.to_string())?;
    if !install.success {
        let details = if !install.stderr.trim().is_empty() {
            install.stderr
        } else {
            install.stdout
        };
        return Err(format!(
            "Live App dependencies install failed for {app_id}: {}",
            details.trim()
        ));
    }

    pool.stop(app_id).await;
    *app = state
        .live_app_manager
        .mark_deps_installed(app_id)
        .await
        .map_err(|e| e.to_string())?;
    emit_live_app_event("liveapp-updated", live_app_payload(app, "deps-installed")).await;
    Ok(true)
}

// ============== App management commands ==============

#[tauri::command]
pub async fn list_live_apps(state: State<'_, AppState>) -> Result<Vec<LiveAppMeta>, String> {
    state
        .live_app_manager
        .list()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_live_app(
    state: State<'_, AppState>,
    request: GetLiveAppRequest,
) -> Result<LiveApp, String> {
    let mut app = state
        .live_app_manager
        .get(&request.app_id)
        .await
        .map_err(|e| e.to_string())?;

    let theme_type = request.theme.as_deref().unwrap_or("dark");
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    match state.live_app_manager.compile_source(
        &request.app_id,
        &app.source,
        &app.permissions,
        theme_type,
        workspace_root.as_deref(),
    ) {
        Ok(html) => app.compiled_html = html,
        Err(e) => log::warn!("get_live_app: recompile failed, using cached: {}", e),
    }
    Ok(app)
}

#[tauri::command]
pub async fn create_live_app(
    state: State<'_, AppState>,
    request: CreateLiveAppRequest,
) -> Result<LiveApp, String> {
    let source: LiveAppSource = request.source.into();
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let app = state
        .live_app_manager
        .create(
            request.name,
            request.description,
            request.icon,
            request.category,
            request.tags,
            source,
            request.permissions,
            request.ai_context,
            workspace_root.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;
    emit_live_app_event("liveapp-created", live_app_payload(&app, "create")).await;
    Ok(app)
}

#[tauri::command]
pub async fn update_live_app(
    state: State<'_, AppState>,
    app_id: String,
    request: UpdateLiveAppRequest,
) -> Result<LiveApp, String> {
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let app = state
        .live_app_manager
        .update(
            &app_id,
            request.name,
            request.description,
            request.icon,
            request.category,
            request.tags,
            request.source.map(Into::into),
            request.permissions,
            request.ai_context,
            workspace_root.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_live_app_event("liveapp-updated", live_app_payload(&app, "update")).await;
    Ok(app)
}

#[tauri::command]
pub async fn delete_live_app(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    if let Some(ref pool) = state.js_worker_pool {
        pool.stop(app_id.as_str()).await;
    }
    state
        .live_app_manager
        .delete(&app_id)
        .await
        .map_err(|e| e.to_string())?;
    emit_live_app_event(
        "liveapp-deleted",
        json!({ "id": app_id, "reason": "delete" }),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_live_app_versions(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<Vec<u32>, String> {
    state
        .live_app_manager
        .list_versions(&app_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rollback_live_app(
    state: State<'_, AppState>,
    app_id: String,
    version: u32,
) -> Result<LiveApp, String> {
    let app = state
        .live_app_manager
        .rollback(&app_id, version)
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_live_app_event("liveapp-rolled-back", live_app_payload(&app, "rollback")).await;
    emit_live_app_event("liveapp-updated", live_app_payload(&app, "rollback")).await;
    Ok(app)
}

#[tauri::command]
pub async fn get_live_app_storage(
    state: State<'_, AppState>,
    app_id: String,
    key: String,
) -> Result<Value, String> {
    state
        .live_app_manager
        .get_storage(&app_id, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_live_app_storage(
    state: State<'_, AppState>,
    app_id: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    state
        .live_app_manager
        .set_storage(&app_id, &key, value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn grant_live_app_workspace(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<(), String> {
    state.live_app_manager.grant_workspace(&app_id).await;
    Ok(())
}

#[tauri::command]
pub async fn grant_live_app_path(
    state: State<'_, AppState>,
    app_id: String,
    path: String,
) -> Result<(), String> {
    state
        .live_app_manager
        .grant_path(&app_id, PathBuf::from(path))
        .await;
    Ok(())
}

// ============== JS Worker & Runtime ==============

#[tauri::command]
pub async fn live_app_runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatus, String> {
    let Some(ref pool) = state.js_worker_pool else {
        return Ok(RuntimeStatus {
            available: false,
            kind: None,
            version: None,
            path: None,
        });
    };
    let info = pool.runtime_info();
    Ok(RuntimeStatus {
        available: true,
        kind: Some(match info.kind {
            bitfun_core::live_app::RuntimeKind::Bun => "bun".to_string(),
            bitfun_core::live_app::RuntimeKind::Node => "node".to_string(),
        }),
        version: Some(info.version.clone()),
        path: Some(info.path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub async fn live_app_worker_call(
    state: State<'_, AppState>,
    request: LiveAppWorkerCallRequest,
) -> Result<Value, String> {
    let pool = state
        .js_worker_pool
        .as_ref()
        .ok_or_else(|| "JS Worker pool not initialized".to_string())?;
    let was_running = pool.is_running(&request.app_id).await;
    let mut app = state
        .live_app_manager
        .get(&request.app_id)
        .await
        .map_err(|e| e.to_string())?;
    let deps_installed = ensure_worker_dependencies(&state, &request.app_id, &mut app).await?;
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let policy = state
        .live_app_manager
        .resolve_policy_for_app(&request.app_id, &app.permissions, workspace_root.as_deref())
        .await;
    let policy_json = serde_json::to_string(&policy).map_err(|e| e.to_string())?;
    let worker_revision = state
        .live_app_manager
        .build_worker_revision(&app, &policy_json);
    let should_emit_restart = !was_running || deps_installed || app.runtime.worker_restart_required;
    let result = pool
        .call(
            &request.app_id,
            &worker_revision,
            &policy_json,
            app.permissions.node.as_ref(),
            &request.method,
            request.params,
        )
        .await
        .map_err(|e| e.to_string())?;
    if should_emit_restart {
        let app = state
            .live_app_manager
            .clear_worker_restart_required(&request.app_id)
            .await
            .map_err(|e| e.to_string())?;
        emit_live_app_event(
            "liveapp-worker-restarted",
            live_app_payload(
                &app,
                if deps_installed {
                    "deps-installed"
                } else {
                    "runtime-restart"
                },
            ),
        )
        .await;
    }
    Ok(result)
}

#[tauri::command]
pub async fn live_app_worker_stop(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    if let Some(ref pool) = state.js_worker_pool {
        pool.stop(&app_id).await;
    }
    emit_live_app_event(
        "liveapp-worker-stopped",
        json!({ "id": app_id, "reason": "manual-stop" }),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn live_app_worker_list_running(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let Some(ref pool) = state.js_worker_pool else {
        return Ok(vec![]);
    };
    Ok(pool.list_running().await)
}

#[tauri::command]
pub async fn live_app_install_deps(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<CoreInstallResult, String> {
    let pool = state
        .js_worker_pool
        .as_ref()
        .ok_or_else(|| "JS Worker pool not initialized".to_string())?;
    let app = state
        .live_app_manager
        .get(&app_id)
        .await
        .map_err(|e| e.to_string())?;
    let install = pool
        .install_deps(&app_id, &app.source.npm_dependencies)
        .await
        .map_err(|e| e.to_string())?;
    if install.success {
        pool.stop(&app_id).await;
        let app = state
            .live_app_manager
            .mark_deps_installed(&app_id)
            .await
            .map_err(|e| e.to_string())?;
        emit_live_app_event("liveapp-updated", live_app_payload(&app, "deps-installed")).await;
    }
    Ok(install)
}

#[tauri::command]
pub async fn live_app_recompile(
    state: State<'_, AppState>,
    request: LiveAppRecompileRequest,
) -> Result<RecompileResult, String> {
    let theme_type = request.theme.as_deref().unwrap_or("dark");
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let app = state
        .live_app_manager
        .recompile(&request.app_id, theme_type, workspace_root.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    emit_live_app_event("liveapp-recompiled", live_app_payload(&app, "recompile")).await;
    emit_live_app_event("liveapp-updated", live_app_payload(&app, "recompile")).await;
    Ok(RecompileResult {
        success: true,
        warnings: None,
    })
}

#[tauri::command]
pub async fn live_app_dialog_message(
    _state: State<'_, AppState>,
    _app_id: String,
    _options: Value,
) -> Result<Value, String> {
    // Tauri dialog is handled by frontend useLiveAppBridge via @tauri-apps/plugin-dialog.
    // This command can be used if we want backend to show message box; for now return not implemented.
    Err("Use dialog from frontend bridge".to_string())
}

#[tauri::command]
pub async fn live_app_import_from_path(
    state: State<'_, AppState>,
    request: LiveAppImportFromPathRequest,
) -> Result<LiveApp, String> {
    let path_buf = PathBuf::from(&request.path);
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let app = state
        .live_app_manager
        .import_from_path(path_buf, workspace_root.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_live_app_event("liveapp-created", live_app_payload(&app, "import")).await;
    Ok(app)
}

#[tauri::command]
pub async fn live_app_sync_from_fs(
    state: State<'_, AppState>,
    request: LiveAppSyncFromFsRequest,
) -> Result<LiveApp, String> {
    let theme_type = request.theme.as_deref().unwrap_or("dark");
    let workspace_root = workspace_root_from_input(request.workspace_path.as_deref());
    let app = state
        .live_app_manager
        .sync_from_fs(&request.app_id, theme_type, workspace_root.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_live_app_event("liveapp-updated", live_app_payload(&app, "sync-from-fs")).await;
    Ok(app)
}

// ============== AI commands ==============

/// Active AI stream cancellation flags: stream_id → cancel flag.
static AI_STREAM_REGISTRY: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();

/// Per-app rate limiter state: app_id → (request_count, window_start_ms).
static AI_RATE_LIMITER: OnceLock<Mutex<HashMap<String, (u32, u64)>>> = OnceLock::new();

fn ai_stream_registry() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    AI_STREAM_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ai_rate_limiter() -> &'static Mutex<HashMap<String, (u32, u64)>> {
    AI_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Check and increment the rate limiter for a given app. Returns Err if rate limit exceeded.
fn check_rate_limit(app_id: &str, rate_limit_per_minute: u32) -> Result<(), String> {
    if rate_limit_per_minute == 0 {
        return Ok(());
    }
    let now = now_ms();
    let window_ms: u64 = 60_000;
    let mut map = ai_rate_limiter().lock().unwrap_or_else(|p| p.into_inner());
    let entry = map.entry(app_id.to_string()).or_insert((0, now));
    if now - entry.1 >= window_ms {
        *entry = (1, now);
    } else {
        entry.0 += 1;
        if entry.0 > rate_limit_per_minute {
            return Err(format!(
                "AI rate limit exceeded: max {} requests/minute",
                rate_limit_per_minute
            ));
        }
    }
    Ok(())
}

/// Validate the requested model against the app's allowed_models list.
/// Returns the resolved model id (may be "primary" / "fast") to pass to AIClientFactory.
fn validate_model(
    model: Option<&str>,
    ai_perms: &bitfun_core::live_app::AiPermissions,
) -> Result<String, String> {
    let requested = model.unwrap_or("primary");
    if let Some(ref allowed) = ai_perms.allowed_models {
        if !allowed.is_empty() && !allowed.iter().any(|m| m == requested) {
            return Err(format!(
                "Model '{}' is not allowed by this Live App's AI permissions",
                requested
            ));
        }
    }
    Ok(requested.to_string())
}

// ---- Request/Response DTOs for AI commands ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiCompleteRequest {
    pub app_id: String,
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiCompleteResponse {
    pub text: String,
    pub usage: Option<LiveAppAiUsage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiChatRequest {
    pub app_id: String,
    pub messages: Vec<LiveAppAiChatMessage>,
    pub stream_id: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiChatStartedResponse {
    pub stream_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiCancelRequest {
    pub app_id: String,
    pub stream_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiListModelsRequest {
    pub app_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAppAiModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub is_default: bool,
}

// ---- Payload structs for Tauri events ----

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiStreamChunkPayload {
    pub app_id: String,
    pub stream_id: String,
    #[serde(rename = "type")]
    pub payload_type: String,
    pub data: serde_json::Value,
}

// ---- Helper: build Message list from request ----

fn build_messages_for_ai(
    system_prompt: Option<&str>,
    chat_messages: &[LiveAppAiChatMessage],
) -> Vec<Message> {
    let mut msgs = Vec::new();
    if let Some(sp) = system_prompt {
        if !sp.is_empty() {
            msgs.push(Message::system(sp.to_string()));
        }
    }
    for m in chat_messages {
        let role = m.role.to_lowercase();
        if role == "assistant" {
            msgs.push(Message::assistant(m.content.clone()));
        } else {
            // Treat any unrecognized role as "user" for safety
            msgs.push(Message::user(m.content.clone()));
        }
    }
    msgs
}

// ---- Commands ----

/// Non-streaming AI completion — waits for the full response before returning.
#[tauri::command]
pub async fn live_app_ai_complete(
    state: State<'_, AppState>,
    request: LiveAppAiCompleteRequest,
) -> Result<LiveAppAiCompleteResponse, String> {
    let app = state
        .live_app_manager
        .get(&request.app_id)
        .await
        .map_err(|e| e.to_string())?;

    let ai_perms = app
        .permissions
        .ai
        .as_ref()
        .ok_or("AI access is not enabled for this Live App")?;

    if !ai_perms.enabled {
        return Err("AI access is not enabled for this Live App".to_string());
    }

    let rate_limit = ai_perms.rate_limit_per_minute.unwrap_or(0);
    check_rate_limit(&request.app_id, rate_limit)?;

    let model_ref = validate_model(request.model.as_deref(), ai_perms)?;

    let ai_client = state
        .ai_client_factory
        .get_client_resolved(&model_ref)
        .await
        .map_err(|e| format!("Failed to get AI client: {}", e))?;

    let messages = build_messages_for_ai(
        request.system_prompt.as_deref(),
        &[LiveAppAiChatMessage {
            role: "user".to_string(),
            content: request.prompt.clone(),
        }],
    );

    let stream_response = ai_client
        .send_message_stream(messages, None)
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;

    let mut stream = stream_response.stream;
    let mut full_text = String::new();
    let mut usage: Option<LiveAppAiUsage> = None;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if let Some(text) = chunk.text {
                    full_text.push_str(&text);
                }
                if let Some(u) = chunk.usage {
                    usage = Some(LiveAppAiUsage {
                        prompt_tokens: u.prompt_token_count,
                        completion_tokens: u.candidates_token_count,
                        total_tokens: u.total_token_count,
                    });
                }
            }
            Err(e) => {
                return Err(format!("AI stream error: {}", e));
            }
        }
    }

    Ok(LiveAppAiCompleteResponse {
        text: full_text,
        usage,
    })
}

/// Streaming AI chat — returns immediately, emits chunks via "liveapp://ai-stream" events.
#[tauri::command]
pub async fn live_app_ai_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    request: LiveAppAiChatRequest,
) -> Result<LiveAppAiChatStartedResponse, String> {
    if request.stream_id.trim().is_empty() {
        return Err("streamId is required".to_string());
    }
    if request.messages.is_empty() {
        return Err("messages must not be empty".to_string());
    }

    let live_app = state
        .live_app_manager
        .get(&request.app_id)
        .await
        .map_err(|e| e.to_string())?;

    let ai_perms = live_app
        .permissions
        .ai
        .as_ref()
        .ok_or("AI access is not enabled for this Live App")?;

    if !ai_perms.enabled {
        return Err("AI access is not enabled for this Live App".to_string());
    }

    let rate_limit = ai_perms.rate_limit_per_minute.unwrap_or(0);
    check_rate_limit(&request.app_id, rate_limit)?;

    let model_ref = validate_model(request.model.as_deref(), ai_perms)?;

    let ai_client = state
        .ai_client_factory
        .get_client_resolved(&model_ref)
        .await
        .map_err(|e| format!("Failed to get AI client: {}", e))?;

    let messages = build_messages_for_ai(request.system_prompt.as_deref(), &request.messages);

    let stream_response = ai_client
        .send_message_stream(messages, None)
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;

    // Register a cancellation flag for this stream
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut registry = ai_stream_registry()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        registry.insert(request.stream_id.clone(), cancel_flag.clone());
    }

    let stream_id = request.stream_id.clone();
    let app_id = request.app_id.clone();
    let app_handle = app.clone();

    tokio::spawn(async move {
        let mut stream = stream_response.stream;
        let mut full_text = String::new();
        let mut last_usage: Option<LiveAppAiUsage> = None;

        while let Some(chunk_result) = stream.next().await {
            // Check cancellation
            if cancel_flag.load(Ordering::SeqCst) {
                break;
            }

            match chunk_result {
                Ok(chunk) => {
                    let has_text = chunk.text.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
                    let has_reasoning = chunk
                        .reasoning_content
                        .as_ref()
                        .map(|t| !t.is_empty())
                        .unwrap_or(false);

                    if has_text || has_reasoning {
                        if let Some(ref t) = chunk.text {
                            full_text.push_str(t);
                        }
                        let payload = AiStreamChunkPayload {
                            app_id: app_id.clone(),
                            stream_id: stream_id.clone(),
                            payload_type: "chunk".to_string(),
                            data: json!({
                                "text": chunk.text,
                                "reasoningContent": chunk.reasoning_content,
                            }),
                        };
                        if let Err(e) = app_handle.emit("liveapp://ai-stream", &payload) {
                            log::warn!("Failed to emit AI stream chunk: {}", e);
                        }
                    }

                    if let Some(u) = chunk.usage {
                        last_usage = Some(LiveAppAiUsage {
                            prompt_tokens: u.prompt_token_count,
                            completion_tokens: u.candidates_token_count,
                            total_tokens: u.total_token_count,
                        });
                    }

                    if let Some(ref reason) = chunk.finish_reason {
                        if !reason.is_empty() && reason != "null" {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let payload = AiStreamChunkPayload {
                        app_id: app_id.clone(),
                        stream_id: stream_id.clone(),
                        payload_type: "error".to_string(),
                        data: json!({ "message": e.to_string() }),
                    };
                    let _ = app_handle.emit("liveapp://ai-stream", &payload);
                    // Clean up registry
                    let mut registry = ai_stream_registry()
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    registry.remove(&stream_id);
                    return;
                }
            }
        }

        // Emit done
        let usage_val = last_usage.map(|u| {
            json!({
                "promptTokens": u.prompt_tokens,
                "completionTokens": u.completion_tokens,
                "totalTokens": u.total_tokens,
            })
        });
        let done_payload = AiStreamChunkPayload {
            app_id: app_id.clone(),
            stream_id: stream_id.clone(),
            payload_type: "done".to_string(),
            data: json!({
                "fullText": full_text,
                "usage": usage_val,
            }),
        };
        let _ = app_handle.emit("liveapp://ai-stream", &done_payload);

        // Clean up registry
        let mut registry = ai_stream_registry()
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        registry.remove(&stream_id);
    });

    Ok(LiveAppAiChatStartedResponse {
        stream_id: request.stream_id,
    })
}

/// Cancel an ongoing AI stream.
#[tauri::command]
pub async fn live_app_ai_cancel(
    _state: State<'_, AppState>,
    request: LiveAppAiCancelRequest,
) -> Result<(), String> {
    let mut registry = ai_stream_registry()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(flag) = registry.get(&request.stream_id) {
        flag.store(true, Ordering::SeqCst);
    }
    // Remove from registry so it gets GC'd
    registry.remove(&request.stream_id);
    Ok(())
}

/// List AI models available to a Live App (no sensitive fields).
#[tauri::command]
pub async fn live_app_ai_list_models(
    state: State<'_, AppState>,
    request: LiveAppAiListModelsRequest,
) -> Result<Vec<LiveAppAiModelInfo>, String> {
    let live_app = state
        .live_app_manager
        .get(&request.app_id)
        .await
        .map_err(|e| e.to_string())?;

    let ai_perms = live_app
        .permissions
        .ai
        .as_ref()
        .ok_or("AI access is not enabled for this Live App")?;

    if !ai_perms.enabled {
        return Err("AI access is not enabled for this Live App".to_string());
    }

    let global_config = state
        .config_service
        .get_config::<GlobalConfig>(None)
        .await
        .map_err(|e| e.to_string())?;

    let primary_id = global_config
        .ai
        .resolve_model_selection("primary")
        .unwrap_or_default();
    let fast_id = global_config
        .ai
        .resolve_model_selection("fast")
        .unwrap_or_default();

    let allowed = ai_perms.allowed_models.as_deref().unwrap_or(&[]);

    let models: Vec<LiveAppAiModelInfo> = global_config
        .ai
        .models
        .iter()
        .filter(|m| m.enabled)
        .filter(|m| {
            if allowed.is_empty() {
                // No restriction — allow all
                true
            } else {
                // Allow if model id/name matches any entry in allowed list,
                // or if "primary"/"fast" is in allowed and this model is the resolved target.
                allowed.iter().any(|a| match a.as_str() {
                    "primary" => m.id == primary_id,
                    "fast" => m.id == fast_id,
                    other => m.id == other || m.name == other,
                })
            }
        })
        .map(|m| LiveAppAiModelInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            provider: m.provider.clone(),
            is_default: m.id == primary_id,
        })
        .collect();

    Ok(models)
}
