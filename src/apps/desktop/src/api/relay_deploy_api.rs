//! Relay server self-deploy Tauri commands.
//!
//! Lets a user deploy the open-source BitFun relay server to their own host
//! over an existing SSH connection (preflight → Docker install → source
//! download + compose deploy → account import). The account is provisioned
//! locally: the plaintext password never leaves this machine — only Argon2id
//! derived artifacts are transferred and handed to `relay-admin import-user`.
//!
//! Orchestration: `bitfun_services_integrations::remote_ssh::relay_deploy`.
//! Product invariants / wizard entry points:
//! `src/web-ui/src/features/relay-deploy/README.md`.

use bitfun_core::service::remote_ssh::relay_deploy::{
    self, RelayDeployTask, RelayPreflight, RelayTaskPoll, RelayTaskStart,
};
use serde::Serialize;
use tauri::State;

use super::app_state::AppState;

#[tauri::command]
pub async fn relay_deploy_preflight(
    state: State<'_, AppState>,
    connection_id: String,
    port: Option<u16>,
) -> Result<RelayPreflight, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::run_preflight(&manager, &connection_id, port.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

/// Stage the interactive Docker-install driver (run it in a remote PTY; poll via
/// `relay_deploy_poll` with task `install_docker`).
#[tauri::command]
pub async fn relay_deploy_install_docker(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<RelayTaskStart, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::start_task(&manager, &connection_id, RelayDeployTask::InstallDocker, 0)
        .await
        .map_err(|e| e.to_string())
}

/// Stage the interactive deploy driver (run it in a remote PTY; poll via
/// `relay_deploy_poll` with task `deploy`).
#[tauri::command]
pub async fn relay_deploy_start(
    state: State<'_, AppState>,
    connection_id: String,
    port: Option<u16>,
) -> Result<RelayTaskStart, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::start_task(
        &manager,
        &connection_id,
        RelayDeployTask::Deploy,
        port.unwrap_or(0),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn relay_deploy_poll(
    state: State<'_, AppState>,
    connection_id: String,
    task: RelayDeployTask,
    cursor: u64,
) -> Result<RelayTaskPoll, String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::poll_task(&manager, &connection_id, task, cursor)
        .await
        .map_err(|e| e.to_string())
}

/// Cancel a running install/deploy task (wizard closed or user navigated away).
#[tauri::command]
pub async fn relay_deploy_cancel(
    state: State<'_, AppState>,
    connection_id: String,
    task: RelayDeployTask,
) -> Result<(), String> {
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::cancel_task(&manager, &connection_id, task)
        .await
        .map_err(|e| e.to_string())
}

/// Provision a relay account locally and import it into the deployed relay.
///
/// The plaintext password is consumed only by the local Argon2id/AES-GCM
/// provisioning step; it is never transmitted to the server.
#[tauri::command]
pub async fn relay_deploy_register(
    state: State<'_, AppState>,
    connection_id: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let username = username.trim().to_string();
    if username.is_empty() || username.chars().any(char::is_whitespace) {
        return Err("invalid username".to_string());
    }
    if password.len() < 8 {
        return Err("password must be at least 8 characters".to_string());
    }
    let account = bitfun_relay_service::admin::provision(&username, &password)
        .map_err(|e| format!("provision account: {e}"))?;
    let import = bitfun_relay_service::admin::ImportableAccount { username, account };
    let json = serde_json::to_string(&import).map_err(|e| format!("serialize account: {e}"))?;
    let manager = state
        .get_ssh_manager_async()
        .await
        .map_err(|e| e.to_string())?;
    relay_deploy::import_account(&manager, &connection_id, &json)
        .await
        .map_err(|e| e.to_string())
}

/// Client-side reachability check for a relay URL (catches firewalls /
/// security-group rules that block the relay port from the public internet).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayVerifyResult {
    pub reachable: bool,
    pub version: Option<String>,
}

#[tauri::command]
pub async fn relay_deploy_verify(relay_url: String) -> Result<RelayVerifyResult, String> {
    let base = relay_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return Err("empty relay url".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let health_ok = client
        .get(format!("{base}/health"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    if !health_ok {
        return Ok(RelayVerifyResult {
            reachable: false,
            version: None,
        });
    }
    let version = match client.get(format!("{base}/api/info")).send().await {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(String::from)),
        Err(_) => None,
    };
    Ok(RelayVerifyResult {
        reachable: true,
        version,
    })
}
