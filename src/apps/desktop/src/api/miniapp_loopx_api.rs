use super::app_state::AppState;
use bitfun_core::miniapp::{
    loopx::LoopxController, MiniAppCustomizationOriginKind, BUILTIN_APPS,
};
use bitfun_product_domains::miniapp::builtin::builtin_source_matches;
use bitfun_product_domains::miniapp::loopx::{
    LoopxActionRequest, LoopxActionResponse, LoopxAttachRequest, LoopxAttachResponse,
    LoopxCreateTaskRequest, LoopxCreateTaskResponse, LoopxEventsSinceRequest,
    LoopxEventsSinceResponse, LoopxExecutionDomain, LoopxExecutionSupport,
    LoopxResolveIntakeRequest, LoopxResolveIntakeResponse, LOOPX_BUILTIN_APP_ID,
};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

pub const LOOPX_UNSUPPORTED_EXECUTION_DOMAIN: &str = "unsupported_execution_domain";

pub struct LoopxControllerState {
    pub controller: Arc<LoopxController>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppLoopxAttachRequest {
    pub app_id: String,
    #[serde(flatten)]
    pub input: LoopxAttachRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppLoopxResolveIntakeRequest {
    pub app_id: String,
    #[serde(flatten)]
    pub input: LoopxResolveIntakeRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppLoopxCreateTaskRequest {
    pub app_id: String,
    #[serde(flatten)]
    pub input: LoopxCreateTaskRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppLoopxActionRequest {
    pub app_id: String,
    #[serde(flatten)]
    pub input: LoopxActionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppLoopxEventsSinceRequest {
    pub app_id: String,
    #[serde(flatten)]
    pub input: LoopxEventsSinceRequest,
}

async fn authorize_builtin(state: &AppState, app_id: &str) -> Result<(), String> {
    if app_id != LOOPX_BUILTIN_APP_ID {
        return Err("LoopX controller is available only to the built-in LoopX MiniApp".to_string());
    }
    let builtin = BUILTIN_APPS
        .iter()
        .find(|app| app.id == LOOPX_BUILTIN_APP_ID)
        .ok_or_else(|| "Built-in LoopX bundle is unavailable".to_string())?;
    let app = state
        .miniapp_manager
        .get(app_id)
        .await
        .map_err(|error| format!("Failed to load built-in LoopX MiniApp: {error}"))?;
    if !builtin_source_matches(&app.source, builtin) {
        return Err("LoopX controller is disabled for modified MiniApp content".to_string());
    }
    if let Some(metadata) = state
        .miniapp_manager
        .load_customization_metadata(app_id)
        .await
        .map_err(|error| format!("Failed to load LoopX customization metadata: {error}"))?
    {
        if metadata.local_override
            || metadata.origin.kind != MiniAppCustomizationOriginKind::Builtin
            || metadata.origin.builtin_id.as_deref() != Some(LOOPX_BUILTIN_APP_ID)
        {
            return Err("LoopX controller is disabled for a local MiniApp override".to_string());
        }
    }
    Ok(())
}

async fn is_remote_workspace(state: &AppState) -> bool {
    state.remote_workspace.read().await.is_some()
}

fn unsupported_error() -> String {
    format!(
        "{LOOPX_UNSUPPORTED_EXECUTION_DOMAIN}: LoopX currently supports only a local Desktop workspace"
    )
}

#[tauri::command]
pub async fn miniapp_loopx_attach(
    app_state: State<'_, AppState>,
    controller: State<'_, LoopxControllerState>,
    request: MiniAppLoopxAttachRequest,
) -> Result<LoopxAttachResponse, String> {
    authorize_builtin(&app_state, &request.app_id).await?;
    if is_remote_workspace(&app_state).await {
        return Ok(controller
            .controller
            .attach(
                LoopxExecutionDomain::RemoteWorkspace,
                LoopxExecutionSupport::UnsupportedExecutionDomain,
                Some(unsupported_error()),
            )
            .await);
    }
    let _ = request.input;
    Ok(controller
        .controller
        .attach(
            LoopxExecutionDomain::LocalDesktop,
            LoopxExecutionSupport::Supported,
            None,
        )
        .await)
}

#[tauri::command]
pub async fn miniapp_loopx_resolve_intake(
    app_state: State<'_, AppState>,
    controller: State<'_, LoopxControllerState>,
    request: MiniAppLoopxResolveIntakeRequest,
) -> Result<LoopxResolveIntakeResponse, String> {
    authorize_builtin(&app_state, &request.app_id).await?;
    if is_remote_workspace(&app_state).await {
        return Err(unsupported_error());
    }
    controller.controller.resolve_intake(request.input).await
}

#[tauri::command]
pub async fn miniapp_loopx_create_task(
    app_state: State<'_, AppState>,
    controller: State<'_, LoopxControllerState>,
    request: MiniAppLoopxCreateTaskRequest,
) -> Result<LoopxCreateTaskResponse, String> {
    authorize_builtin(&app_state, &request.app_id).await?;
    if is_remote_workspace(&app_state).await {
        return Err(unsupported_error());
    }
    controller.controller.create_tasks(request.input).await
}

#[tauri::command]
pub async fn miniapp_loopx_action(
    app_state: State<'_, AppState>,
    controller: State<'_, LoopxControllerState>,
    request: MiniAppLoopxActionRequest,
) -> Result<LoopxActionResponse, String> {
    authorize_builtin(&app_state, &request.app_id).await?;
    if is_remote_workspace(&app_state).await {
        return Err(unsupported_error());
    }
    controller.controller.action(request.input).await
}

#[tauri::command]
pub async fn miniapp_loopx_events_since(
    app_state: State<'_, AppState>,
    controller: State<'_, LoopxControllerState>,
    request: MiniAppLoopxEventsSinceRequest,
) -> Result<LoopxEventsSinceResponse, String> {
    authorize_builtin(&app_state, &request.app_id).await?;
    if is_remote_workspace(&app_state).await {
        return Err(unsupported_error());
    }
    Ok(controller.controller.events_since(request.input).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_execution_domain_is_stable() {
        assert!(unsupported_error().starts_with(LOOPX_UNSUPPORTED_EXECUTION_DOMAIN));
    }
}
