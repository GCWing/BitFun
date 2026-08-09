use std::path::PathBuf;

use async_trait::async_trait;
use bitfun_app_server::management::{
    AccountManagementHost, AppManagementError, AppManagementResult,
};
use bitfun_app_server_protocol::account::*;
use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;

#[derive(Clone)]
pub(crate) struct CliAccountManagementHost {
    compatibility: CoreAgentRuntimeCompatibility,
}

impl CliAccountManagementHost {
    pub(crate) fn new(compatibility: CoreAgentRuntimeCompatibility) -> Self {
        Self { compatibility }
    }

    async fn snapshot(&self, workspace_path: String) -> AccountSnapshotResponse {
        let logged_in = crate::account::is_logged_in().await;
        let info = if logged_in {
            crate::account::account_info()
                .await
                .ok()
                .map(project_account_info)
        } else {
            None
        };
        let devices = if logged_in {
            crate::account::list_devices()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(project_account_device)
                .collect()
        } else {
            Vec::new()
        };
        let _ = workspace_path;
        AccountSnapshotResponse {
            logged_in,
            pending_sync_choice: crate::account::pending_sync_choice(),
            info,
            devices,
            sync: project_sync_progress(crate::account_sync::current_sync_progress().await),
        }
    }
}

#[async_trait]
impl AccountManagementHost for CliAccountManagementHost {
    async fn account_snapshot(
        &self,
        request: AccountSnapshotRequest,
    ) -> AppManagementResult<AccountSnapshotResponse> {
        Ok(self.snapshot(request.workspace_path).await)
    }

    async fn account_login(
        &self,
        request: AccountLoginRequest,
    ) -> AppManagementResult<AccountLoginResponse> {
        validate_operation_id(&request.operation_id)?;
        let result = crate::account::login_with_credentials(
            &request.relay_url,
            &request.username,
            &request.password,
        )
        .await
        .map_err(|error| account_error(error, &request))?;
        Ok(AccountLoginResponse {
            user_id: result.user_id,
            relay_url: result.relay_url,
            has_cloud_settings: result.has_cloud_settings,
            status_message: result.status_message,
        })
    }

    async fn account_finalize_login(
        &self,
        request: AccountFinalizeLoginRequest,
    ) -> AppManagementResult<AccountSnapshotResponse> {
        validate_operation_id(&request.operation_id)?;
        crate::account::finalize_login_after_sync_choice()
            .await
            .map_err(internal_account_error)?;
        if !crate::account_sync::start_auto_sync_background(
            self.compatibility.clone(),
            request.operation_id.clone(),
            request.choice == AccountSyncChoice::Local,
            PathBuf::from(&request.workspace_path),
        )
        .await
        {
            return Err(AppManagementError::invalid_request(
                "Account settings sync is already in progress",
            ));
        }
        Ok(self.snapshot(request.workspace_path).await)
    }

    async fn account_logout(
        &self,
        request: AccountLogoutRequest,
    ) -> AppManagementResult<AccountSnapshotResponse> {
        validate_operation_id(&request.operation_id)?;
        crate::account::logout()
            .await
            .map_err(internal_account_error)?;
        crate::account_sync::mark_sync_cancelled(request.operation_id).await;
        Ok(self.snapshot(request.workspace_path).await)
    }

    async fn settings_sync_start(
        &self,
        request: SettingsSyncStartRequest,
    ) -> AppManagementResult<SettingsSyncResponse> {
        validate_operation_id(&request.operation_id)?;
        if !crate::account::is_logged_in().await {
            return Err(AppManagementError::invalid_request(
                "Account login must be finalized before settings sync starts",
            ));
        }
        if !crate::account_sync::start_auto_sync_background(
            self.compatibility.clone(),
            request.operation_id,
            request.is_first_login,
            PathBuf::from(request.workspace_path),
        )
        .await
        {
            return Err(AppManagementError::invalid_request(
                "Account settings sync is already in progress",
            ));
        }
        Ok(current_sync_response().await)
    }

    async fn settings_sync_snapshot(
        &self,
        _request: SettingsSyncSnapshotRequest,
    ) -> AppManagementResult<SettingsSyncResponse> {
        Ok(current_sync_response().await)
    }

    async fn settings_sync_cancel(
        &self,
        request: SettingsSyncCancelRequest,
    ) -> AppManagementResult<SettingsSyncResponse> {
        validate_operation_id(&request.operation_id)?;
        crate::account::logout()
            .await
            .map_err(internal_account_error)?;
        crate::account_sync::mark_sync_cancelled(request.operation_id).await;
        Ok(current_sync_response().await)
    }

    async fn settings_sync_local_changed(
        &self,
        request: SettingsSyncLocalChangedRequest,
    ) -> AppManagementResult<SettingsSyncResponse> {
        validate_operation_id(&request.operation_id)?;
        crate::account_sync::notify_local_settings_changed();
        Ok(current_sync_response().await)
    }
}

async fn current_sync_response() -> SettingsSyncResponse {
    SettingsSyncResponse {
        progress: project_sync_progress(crate::account_sync::current_sync_progress().await),
    }
}

fn project_account_info(info: crate::account::AccountInfo) -> AccountInfo {
    AccountInfo {
        user_id: info.user_id,
        relay_url: info.relay_url,
        device_id: info.device_id,
        device_name: info.device_name,
    }
}

fn project_account_device(device: crate::account::AccountDevice) -> AccountDevice {
    AccountDevice {
        device_id: device.device_id,
        device_name: device.device_name,
        online: device.online,
    }
}

fn project_sync_progress(progress: crate::account_sync::SyncProgress) -> SettingsSyncProgress {
    SettingsSyncProgress {
        operation_id: progress.operation_id,
        status: match progress.status {
            crate::account_sync::SyncStatus::Idle => SettingsSyncStatus::Idle,
            crate::account_sync::SyncStatus::Syncing => SettingsSyncStatus::Syncing,
            crate::account_sync::SyncStatus::Done => SettingsSyncStatus::Done,
            crate::account_sync::SyncStatus::Failed => SettingsSyncStatus::Failed,
            crate::account_sync::SyncStatus::Cancelled => SettingsSyncStatus::Cancelled,
        },
        phase: progress.phase,
        percent: progress.percent,
        current: progress.current,
        total: progress.total,
        detail: progress.detail,
        error: progress.error,
        settings_synced: progress.settings_synced,
        sessions_exported: progress.sessions_exported,
    }
}

fn validate_operation_id(operation_id: &str) -> AppManagementResult<()> {
    let valid = !operation_id.trim().is_empty()
        && operation_id.len() <= 128
        && operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(AppManagementError::invalid_request(
            "Account operation ID is invalid",
        ))
    }
}

fn account_error(error: anyhow::Error, request: &AccountLoginRequest) -> AppManagementError {
    let mut message = error.to_string();
    for secret in [&request.relay_url, &request.username, &request.password] {
        if !secret.is_empty() {
            message = message.replace(secret, "<redacted>");
        }
    }
    AppManagementError::internal(bounded_error(message))
}

fn internal_account_error(error: anyhow::Error) -> AppManagementError {
    AppManagementError::internal(bounded_error(error.to_string()))
}

fn bounded_error(message: String) -> String {
    message
        .chars()
        .filter(|character| !character.is_control())
        .take(500)
        .collect()
}
