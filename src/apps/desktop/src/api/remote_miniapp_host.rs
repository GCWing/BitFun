//! Native-mobile H5 adapter. Calls the existing MiniApp owner and permission paths.
use super::{app_state::AppState, miniapp_api};
use openbitfun_core::miniapp::is_host_primitive;
use openbitfun_services_integrations::remote_connect::miniapp::{
    MiniAppRequest, RemoteMiniAppHost,
};
use serde_json::{json, Value};
use tauri::Manager;

pub struct DesktopRemoteMiniAppHost(pub tauri::AppHandle);

#[async_trait::async_trait]
impl RemoteMiniAppHost for DesktopRemoteMiniAppHost {
    async fn execute(&self, request: &MiniAppRequest) -> Result<Value, String> {
        let state = self.0.state::<AppState>();
        // These runtimes currently use host-local IO. Never accidentally read the
        // controller's files while its active workspace/runtime belongs elsewhere.
        if state.remote_workspace.read().await.is_some()
            || super::peer_host_invoke::is_peer_controller_active()
        {
            return Err(
                "Mobile MiniApps are not supported for SSH workspaces or Peer Device Mode yet."
                    .into(),
            );
        }
        match request {
            MiniAppRequest::List => {
                let apps = state
                    .miniapp_manager
                    .list()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(json!({"apps": apps.into_iter().map(|app| json!({
                    "id": app.id, "name": app.name, "description": app.description,
                })).collect::<Vec<_>>()}))
            }
            MiniAppRequest::Open { app_id } => {
                let app = state
                    .miniapp_manager
                    .get(app_id)
                    .await
                    .map_err(|e| e.to_string())?;
                // Compile without a workspace binding: mobile cannot silently
                // inherit a changing desktop selection or grant access to it.
                let html = if state
                    .miniapp_manager
                    .uses_market_strict_runtime(app_id)
                    .await
                {
                    state.miniapp_manager.compile_market_source(
                        app_id,
                        &app.source,
                        &app.permissions,
                        "light",
                        None,
                    )
                } else {
                    state.miniapp_manager.compile_source(
                        app_id,
                        &app.source,
                        &app.permissions,
                        "light",
                        None,
                    )
                }
                .map_err(|e| e.to_string())?;
                if html.len() > 2 * 1024 * 1024 {
                    return Err(
                        "This MiniApp page exceeds the mobile transfer limit (2 MiB).".into(),
                    );
                }
                Ok(json!({"id": app.id, "name": app.name, "version": app.version, "html": html}))
            }
            MiniAppRequest::Call {
                app_id,
                version,
                method,
                params,
            } => {
                let app = state
                    .miniapp_manager
                    .get(app_id)
                    .await
                    .map_err(|e| e.to_string())?;
                if app.version != *version {
                    return Err("This MiniApp has changed. Reopen it before continuing.".into());
                }
                if method.starts_with("os.")
                    && state
                        .miniapp_manager
                        .uses_market_strict_runtime(app_id)
                        .await
                    && !app
                        .permissions
                        .host
                        .as_ref()
                        .is_some_and(|host| host.system_info)
                {
                    return Err("This MiniApp does not have host.system_info permission.".into());
                }
                if method.starts_with("storage.") {
                    let key = params
                        .get("key")
                        .and_then(Value::as_str)
                        .ok_or("Storage key must be a string")?;
                    return match method.as_str() {
                        "storage.get" => state
                            .miniapp_manager
                            .get_storage(app_id, key)
                            .await
                            .map_err(|e| e.to_string()),
                        "storage.set" => state
                            .miniapp_manager
                            .set_storage(
                                app_id,
                                key,
                                params.get("value").cloned().unwrap_or(Value::Null),
                            )
                            .await
                            .map(|_| Value::Null)
                            .map_err(|e| e.to_string()),
                        _ => Err("Unsupported mobile storage method".into()),
                    };
                }
                if app
                    .permissions
                    .node
                    .as_ref()
                    .is_some_and(|node| !node.enabled)
                {
                    if !is_host_primitive(method) {
                        return Err(
                            "This MiniApp has no Worker; the requested method is unsupported."
                                .into(),
                        );
                    }
                    miniapp_api::miniapp_host_call(
                        state,
                        miniapp_api::MiniAppHostCallRequest {
                            app_id: app_id.clone(),
                            method: method.clone(),
                            params: params.clone(),
                            workspace_path: None,
                        },
                    )
                    .await
                } else {
                    miniapp_api::miniapp_worker_call(
                        state,
                        miniapp_api::MiniAppWorkerCallRequest {
                            app_id: app_id.clone(),
                            method: method.clone(),
                            params: params.clone(),
                            workspace_path: None,
                        },
                    )
                    .await
                }
            }
        }
    }
}
