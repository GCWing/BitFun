//! MiniApp API — Tauri commands for MiniApp CRUD, JS Worker, and dialog.

use crate::api::app_state::AppState;
use bitfun_core::miniapp::{
    MiniApp, MiniAppAiContext, MiniAppMeta, MiniAppPermissions, MiniAppSource,
    InstallResult as CoreInstallResult,
};
use bitfun_core::infrastructure::events::{emit_global_event, BackendEvent};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::State;

// ============== Request/Response DTOs ==============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMiniAppRequest {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: MiniAppSourceDto,
    #[serde(default)]
    pub permissions: MiniAppPermissions,
    pub ai_context: Option<MiniAppAiContext>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppSourceDto {
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

impl From<MiniAppSourceDto> for MiniAppSource {
    fn from(d: MiniAppSourceDto) -> Self {
        MiniAppSource {
            html: d.html,
            css: d.css,
            ui_js: d.ui_js,
            esm_dependencies: d
                .esm_dependencies
                .into_iter()
                .map(|x| bitfun_core::miniapp::EsmDep {
                    name: x.name,
                    version: x.version,
                    url: x.url,
                })
                .collect(),
            worker_js: d.worker_js,
            npm_dependencies: d
                .npm_dependencies
                .into_iter()
                .map(|x| bitfun_core::miniapp::NpmDep {
                    name: x.name,
                    version: x.version,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMiniAppRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<MiniAppSourceDto>,
    pub permissions: Option<MiniAppPermissions>,
    pub ai_context: Option<MiniAppAiContext>,
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

fn miniapp_payload(app: &MiniApp, reason: &str) -> Value {
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

async fn emit_miniapp_event(event_name: &str, payload: Value) {
    let _ = emit_global_event(BackendEvent::Custom {
        event_name: event_name.to_string(),
        payload,
    })
    .await;
}

async fn maybe_stop_worker(state: &State<'_, AppState>, app: &MiniApp) {
    if app.runtime.worker_restart_required {
        if let Some(ref pool) = state.js_worker_pool {
            pool.stop(&app.id).await;
        }
        emit_miniapp_event(
            "miniapp-worker-stopped",
            json!({ "id": app.id, "reason": "pending-restart" }),
        )
        .await;
    }
}

async fn ensure_worker_dependencies(
    state: &State<'_, AppState>,
    app_id: &str,
    app: &mut MiniApp,
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
            "MiniApp dependencies install failed for {app_id}: {}",
            details.trim()
        ));
    }

    pool.stop(app_id).await;
    *app = state
        .miniapp_manager
        .mark_deps_installed(app_id)
        .await
        .map_err(|e| e.to_string())?;
    emit_miniapp_event("miniapp-updated", miniapp_payload(app, "deps-installed")).await;
    Ok(true)
}

// ============== App management commands ==============

#[tauri::command]
pub async fn list_miniapps(state: State<'_, AppState>) -> Result<Vec<MiniAppMeta>, String> {
    state
        .miniapp_manager
        .list()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_miniapp(
    state: State<'_, AppState>,
    app_id: String,
    theme: Option<String>,
) -> Result<MiniApp, String> {
    let mut app = state
        .miniapp_manager
        .get(&app_id)
        .await
        .map_err(|e| e.to_string())?;

    let workspace_dir = state
        .workspace_path
        .read()
        .await
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let path_manager = state.miniapp_manager.path_manager();
    let app_data_dir = path_manager.miniapp_dir(&app_id);
    let app_data_dir_str = app_data_dir.to_string_lossy().to_string();
    let theme_type = theme.as_deref().unwrap_or("dark");
    match bitfun_core::miniapp::compiler::compile(
        &app.source,
        &app.permissions,
        &app_id,
        &app_data_dir_str,
        &workspace_dir,
        theme_type,
    ) {
        Ok(html) => app.compiled_html = html,
        Err(e) => log::warn!("get_miniapp: recompile failed, using cached: {}", e),
    }
    Ok(app)
}

#[tauri::command]
pub async fn create_miniapp(
    state: State<'_, AppState>,
    request: CreateMiniAppRequest,
) -> Result<MiniApp, String> {
    let source: MiniAppSource = request.source.into();
    let app = state
        .miniapp_manager
        .create(
            request.name,
            request.description,
            request.icon,
            request.category,
            request.tags,
            source,
            request.permissions,
            request.ai_context,
        )
        .await
        .map_err(|e| e.to_string())?;
    emit_miniapp_event("miniapp-created", miniapp_payload(&app, "create")).await;
    Ok(app)
}

#[tauri::command]
pub async fn update_miniapp(
    state: State<'_, AppState>,
    app_id: String,
    request: UpdateMiniAppRequest,
) -> Result<MiniApp, String> {
    let app = state
        .miniapp_manager
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
        )
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_miniapp_event("miniapp-updated", miniapp_payload(&app, "update")).await;
    Ok(app)
}

#[tauri::command]
pub async fn delete_miniapp(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    if let Some(ref pool) = state.js_worker_pool {
        pool.stop(app_id.as_str()).await;
    }
    state
        .miniapp_manager
        .delete(&app_id)
        .await
        .map_err(|e| e.to_string())?;
    emit_miniapp_event(
        "miniapp-deleted",
        json!({ "id": app_id, "reason": "delete" }),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_miniapp_versions(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<Vec<u32>, String> {
    state
        .miniapp_manager
        .list_versions(&app_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rollback_miniapp(
    state: State<'_, AppState>,
    app_id: String,
    version: u32,
) -> Result<MiniApp, String> {
    let app = state
        .miniapp_manager
        .rollback(&app_id, version)
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_miniapp_event("miniapp-rolled-back", miniapp_payload(&app, "rollback")).await;
    emit_miniapp_event("miniapp-updated", miniapp_payload(&app, "rollback")).await;
    Ok(app)
}

#[tauri::command]
pub async fn get_miniapp_storage(
    state: State<'_, AppState>,
    app_id: String,
    key: String,
) -> Result<Value, String> {
    state
        .miniapp_manager
        .get_storage(&app_id, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_miniapp_storage(
    state: State<'_, AppState>,
    app_id: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    state
        .miniapp_manager
        .set_storage(&app_id, &key, value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn grant_miniapp_workspace(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<(), String> {
    state.miniapp_manager.grant_workspace(&app_id).await;
    Ok(())
}

#[tauri::command]
pub async fn grant_miniapp_path(
    state: State<'_, AppState>,
    app_id: String,
    path: String,
) -> Result<(), String> {
    state
        .miniapp_manager
        .grant_path(&app_id, PathBuf::from(path))
        .await;
    Ok(())
}

// ============== JS Worker & Runtime ==============

#[tauri::command]
pub async fn miniapp_runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatus, String> {
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
            bitfun_core::miniapp::RuntimeKind::Bun => "bun".to_string(),
            bitfun_core::miniapp::RuntimeKind::Node => "node".to_string(),
        }),
        version: Some(info.version.clone()),
        path: Some(info.path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub async fn miniapp_worker_call(
    state: State<'_, AppState>,
    app_id: String,
    method: String,
    params: Value,
) -> Result<Value, String> {
    let pool = state
        .js_worker_pool
        .as_ref()
        .ok_or_else(|| "JS Worker pool not initialized".to_string())?;
    let was_running = pool.is_running(&app_id).await;
    let mut app = state
        .miniapp_manager
        .get(&app_id)
        .await
        .map_err(|e| e.to_string())?;
    let deps_installed = ensure_worker_dependencies(&state, &app_id, &mut app).await?;
    let policy = state
        .miniapp_manager
        .resolve_policy_for_app(&app_id, &app.permissions)
        .await;
    let policy_json = serde_json::to_string(&policy).map_err(|e| e.to_string())?;
    let worker_revision = state.miniapp_manager.build_worker_revision(&app, &policy_json);
    let should_emit_restart = !was_running || deps_installed || app.runtime.worker_restart_required;
    let result = pool.call(
        &app_id,
        &worker_revision,
        &policy_json,
        app.permissions.node.as_ref(),
        &method,
        params,
    )
    .await
    .map_err(|e| e.to_string())?;
    if should_emit_restart {
        let app = state
            .miniapp_manager
            .clear_worker_restart_required(&app_id)
            .await
            .map_err(|e| e.to_string())?;
        emit_miniapp_event(
            "miniapp-worker-restarted",
            miniapp_payload(&app, if deps_installed { "deps-installed" } else { "runtime-restart" }),
        )
        .await;
    }
    Ok(result)
}

#[tauri::command]
pub async fn miniapp_worker_stop(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    if let Some(ref pool) = state.js_worker_pool {
        pool.stop(&app_id).await;
    }
    emit_miniapp_event(
        "miniapp-worker-stopped",
        json!({ "id": app_id, "reason": "manual-stop" }),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn miniapp_worker_list_running(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let Some(ref pool) = state.js_worker_pool else {
        return Ok(vec![]);
    };
    Ok(pool.list_running().await)
}

#[tauri::command]
pub async fn miniapp_install_deps(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<CoreInstallResult, String> {
    let pool = state
        .js_worker_pool
        .as_ref()
        .ok_or_else(|| "JS Worker pool not initialized".to_string())?;
    let app = state
        .miniapp_manager
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
            .miniapp_manager
            .mark_deps_installed(&app_id)
            .await
            .map_err(|e| e.to_string())?;
        emit_miniapp_event("miniapp-updated", miniapp_payload(&app, "deps-installed")).await;
    }
    Ok(install)
}

#[tauri::command]
pub async fn miniapp_recompile(
    state: State<'_, AppState>,
    app_id: String,
    theme: Option<String>,
) -> Result<RecompileResult, String> {
    let theme_type = theme.as_deref().unwrap_or("dark");
    let app = state
        .miniapp_manager
        .recompile(&app_id, theme_type)
        .await
        .map_err(|e| e.to_string())?;
    emit_miniapp_event("miniapp-recompiled", miniapp_payload(&app, "recompile")).await;
    emit_miniapp_event("miniapp-updated", miniapp_payload(&app, "recompile")).await;
    Ok(RecompileResult {
        success: true,
        warnings: None,
    })
}

#[tauri::command]
pub async fn miniapp_dialog_message(
    _state: State<'_, AppState>,
    _app_id: String,
    _options: Value,
) -> Result<Value, String> {
    // Tauri dialog is handled by frontend useMiniAppBridge via @tauri-apps/plugin-dialog.
    // This command can be used if we want backend to show message box; for now return not implemented.
    Err("Use dialog from frontend bridge".to_string())
}

#[tauri::command]
pub async fn miniapp_import_from_path(
    state: State<'_, AppState>,
    path: String,
) -> Result<MiniApp, String> {
    let path_buf = PathBuf::from(&path);
    let app = state
        .miniapp_manager
        .import_from_path(path_buf)
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_miniapp_event("miniapp-created", miniapp_payload(&app, "import")).await;
    Ok(app)
}

#[tauri::command]
pub async fn miniapp_sync_from_fs(
    state: State<'_, AppState>,
    app_id: String,
    theme: Option<String>,
) -> Result<MiniApp, String> {
    let theme_type = theme.as_deref().unwrap_or("dark");
    let app = state
        .miniapp_manager
        .sync_from_fs(&app_id, theme_type)
        .await
        .map_err(|e| e.to_string())?;
    maybe_stop_worker(&state, &app).await;
    emit_miniapp_event("miniapp-updated", miniapp_payload(&app, "sync-from-fs")).await;
    Ok(app)
}
