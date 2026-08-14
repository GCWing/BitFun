use super::*;

pub(crate) use bitfun_core::service::remote_connect::account_runtime::{
    validate_management_operation_id, AccountDevice, AccountInfo, AccountSnapshot,
    AccountSyncProgress, AccountSyncStatus,
};
use bitfun_core::service::remote_connect::account_runtime::{
    AccountLoginNextStep, AccountLoginResult,
};
pub(crate) type AccountSnapshotResponse = AccountSnapshot;
pub(crate) type SettingsSyncProgress = AccountSyncProgress;
pub(crate) type SettingsSyncStatus = AccountSyncStatus;

#[derive(Debug, Clone)]
pub(crate) struct SettingsSyncResponse {
    pub progress: AccountSyncProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountSyncChoice {
    Local,
    Cloud,
}
#[derive(Debug, Clone)]
pub(crate) struct AccountLoginView {
    pub user_id: String,
    pub relay_url: String,
    pub has_cloud_settings: bool,
    pub status_message: String,
}

fn account_login_status_message(result: &AccountLoginResult) -> String {
    match result.next_step() {
        AccountLoginNextStep::ChooseSettingsSource => format!(
            "Authenticated as user {} on {}. Choose cloud or local settings to finish login.",
            result.user_id, result.relay_url
        ),
        AccountLoginNextStep::Ready if result.routing_connected => format!(
            "Logged in as user {} on {}. Device routing connected.",
            result.user_id, result.relay_url
        ),
        AccountLoginNextStep::Ready => {
            format!(
                "Logged in as user {} on {}.",
                result.user_id, result.relay_url
            )
        }
        AccountLoginNextStep::RoutingUnavailable(error) => format!(
            "Logged in as user {} on {}. Device routing failed: {}",
            result.user_id, result.relay_url, error
        ),
    }
}

pub(crate) struct AccountProvider {
    account: Option<Arc<AccountRuntime>>,
}

impl AccountProvider {
    pub(crate) fn new(account: Option<Arc<AccountRuntime>>) -> Self {
        Self { account }
    }

    fn owner(&self) -> ManagementResult<&Arc<AccountRuntime>> {
        self.account.as_ref().ok_or_else(|| {
            ManagementError::unsupported(format!("{ACCOUNT_CAPABILITY} is unavailable"))
        })
    }

    pub(crate) async fn snapshot(
        &self,
        scope: &ManagementScope,
    ) -> ManagementResult<AccountSnapshot> {
        scope.local_workspace("Account management")?;
        Ok(self.owner()?.snapshot().await)
    }

    pub(crate) async fn login(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
        relay_url: &str,
        username: &str,
        password: &str,
    ) -> ManagementResult<AccountLoginView> {
        scope.local_workspace("Account management")?;
        validate_management_operation_id(operation_id).map_err(map_account_management_error)?;
        let result = self
            .owner()?
            .login_for_management(operation_id, relay_url, username, password)
            .await
            .map_err(map_account_management_error)?;
        Ok(AccountLoginView {
            user_id: result.user_id.clone(),
            relay_url: result.relay_url.clone(),
            has_cloud_settings: result.has_cloud_settings,
            status_message: account_login_status_message(&result),
        })
    }

    pub(crate) async fn finalize_login(
        &self,
        scope: &ManagementScope,
        operation_id: String,
        use_local_settings: bool,
        workspace_path: PathBuf,
    ) -> ManagementResult<AccountSnapshot> {
        scope.local_workspace("Account management")?;
        validate_management_operation_id(&operation_id).map_err(map_account_management_error)?;
        self.owner()?
            .finalize_login_for_management(operation_id, use_local_settings, workspace_path)
            .await
            .map_err(map_account_management_error)
    }

    pub(crate) async fn logout(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
    ) -> ManagementResult<AccountSnapshot> {
        scope.local_workspace("Account management")?;
        validate_management_operation_id(operation_id).map_err(map_account_management_error)?;
        self.owner()?
            .logout_for_management(operation_id.to_string())
            .await
            .map_err(map_account_management_error)
    }
}

pub(crate) struct SettingsSyncProvider {
    account: Option<Arc<AccountRuntime>>,
}

impl SettingsSyncProvider {
    pub(crate) fn new(account: Option<Arc<AccountRuntime>>) -> Self {
        Self { account }
    }

    fn owner(&self) -> ManagementResult<&Arc<AccountRuntime>> {
        self.account.as_ref().ok_or_else(|| {
            ManagementError::unsupported(format!("{SETTINGS_SYNC_CAPABILITY} is unavailable"))
        })
    }

    pub(crate) async fn start(
        &self,
        scope: &ManagementScope,
        operation_id: String,
        is_first_login: bool,
        workspace_path: PathBuf,
    ) -> ManagementResult<AccountSyncProgress> {
        scope.local_workspace("Settings sync management")?;
        validate_management_operation_id(&operation_id).map_err(map_account_management_error)?;
        self.owner()?
            .start_settings_sync_for_management(operation_id, is_first_login, workspace_path)
            .await
            .map_err(map_account_management_error)
    }

    pub(crate) async fn snapshot(
        &self,
        scope: &ManagementScope,
    ) -> ManagementResult<AccountSyncProgress> {
        scope.local_workspace("Settings sync management")?;
        Ok(self.owner()?.current_sync_progress().await)
    }

    pub(crate) async fn cancel(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
    ) -> ManagementResult<AccountSyncProgress> {
        scope.local_workspace("Settings sync management")?;
        validate_management_operation_id(operation_id).map_err(map_account_management_error)?;
        self.owner()?
            .cancel_settings_sync_for_management(operation_id.to_string())
            .await
            .map_err(map_account_management_error)
    }

    pub(crate) async fn local_changed(
        &self,
        scope: &ManagementScope,
        operation_id: &str,
    ) -> ManagementResult<AccountSyncProgress> {
        scope.local_workspace("Settings sync management")?;
        validate_management_operation_id(operation_id).map_err(map_account_management_error)?;
        self.owner()?
            .notify_local_settings_changed_for_management(operation_id)
            .await
            .map_err(map_account_management_error)
    }
}

pub(crate) struct WorktreeRepositoryStatus {
    pub is_repository: bool,
    pub current_branch: Option<String>,
}

#[async_trait]
pub(crate) trait WorktreeOwner: Send + Sync {
    async fn bind_session(
        &self,
        request: WorktreeSessionBindingRequest,
    ) -> Result<WorktreeSessionBindingResult, bitfun_core_types::WorktreeError>;
}

pub(crate) struct CoreWorktreeOwner;

#[async_trait]
impl WorktreeOwner for CoreWorktreeOwner {
    async fn bind_session(
        &self,
        request: WorktreeSessionBindingRequest,
    ) -> Result<WorktreeSessionBindingResult, bitfun_core_types::WorktreeError> {
        WorktreeService::bind_session(request).await
    }
}

pub(crate) struct WorktreeProvider {
    owner: Option<Arc<dyn WorktreeOwner>>,
}
pub(crate) struct WorktreeBindingRequest {
    pub scope: ManagementScope,
    pub operation_id: String,
    pub session_id: String,
    pub project_workspace_path: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorktreeBindingView {
    pub workspace_binding: AgentSessionWorkspaceBinding,
    pub retained_worktree_path: Option<String>,
}

impl WorktreeProvider {
    pub(crate) fn new(owner: Option<Arc<dyn WorktreeOwner>>) -> Self {
        Self { owner }
    }

    pub(crate) fn local() -> Self {
        Self::new(Some(Arc::new(CoreWorktreeOwner)))
    }

    fn owner(&self) -> ManagementResult<&Arc<dyn WorktreeOwner>> {
        self.owner.as_ref().ok_or_else(|| {
            ManagementError::unsupported(format!("{WORKTREES_CAPABILITY} is unavailable"))
        })
    }

    pub(crate) async fn repository_status(
        &self,
        scope: &ManagementScope,
    ) -> ManagementResult<WorktreeRepositoryStatus> {
        if scope.remote {
            return Err(ManagementError::unsupported(
                "Repository status is not supported for remote workspaces",
            ));
        }
        self.owner()?;
        let repository = match bitfun_core::service::git::GitService::resolve_worktree_repository(
            &scope.workspace_path,
        )
        .await
        {
            Ok(repository) => {
                bitfun_core::service::git::GitService::get_repository_basic(repository.query_path)
                    .await
            }
            Err(error) => Err(error),
        };
        Ok(match repository {
            Ok(repository) => WorktreeRepositoryStatus {
                is_repository: true,
                current_branch: Some(repository.current_branch),
            },
            Err(_) => WorktreeRepositoryStatus {
                is_repository: false,
                current_branch: None,
            },
        })
    }

    pub(crate) async fn bind(
        &self,
        request: WorktreeBindingRequest,
    ) -> ManagementResult<WorktreeBindingView> {
        self.transition(request, true).await
    }

    pub(crate) async fn release(
        &self,
        request: WorktreeBindingRequest,
    ) -> ManagementResult<WorktreeBindingView> {
        self.transition(request, false).await
    }

    async fn transition(
        &self,
        request: WorktreeBindingRequest,
        enabled: bool,
    ) -> ManagementResult<WorktreeBindingView> {
        validate_worktree_operation_id(&request.operation_id)?;
        if request.scope.remote {
            return Err(ManagementError::unsupported(
                "Managed worktrees are not supported for remote workspaces",
            ));
        }
        let result = self
            .owner()?
            .bind_session(WorktreeSessionBindingRequest {
                request_id: request.operation_id,
                session_id: request.session_id,
                project_workspace_path: request.project_workspace_path,
                enabled,
            })
            .await
            .map_err(|error| ManagementError::internal(bounded_error(error.message)))?;
        let execution_target = result.execution_target.clone();
        Ok(WorktreeBindingView {
            workspace_binding: AgentSessionWorkspaceBinding {
                workspace_id: result.workspace_id,
                workspace_path: result.workspace_path,
                project_workspace_path: Some(result.project_workspace_path),
                execution_target: Some(execution_target),
                remote_connection_id: None,
                remote_ssh_host: None,
            },
            retained_worktree_path: result.retained_worktree_path,
        })
    }
}
