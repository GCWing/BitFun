use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use bitfun_core::service::remote_connect::account_runtime::{
    AccountManagementError, AccountManagementErrorKind, AccountRuntime,
};
use bitfun_core::service::worktree::{
    WorktreeService, WorktreeSessionBindingRequest, WorktreeSessionBindingResult,
};
use bitfun_product_domains::external_hook_import::{
    ExternalHookImportApplyRequestV1, ExternalHookImportApplyResultV1,
    ExternalHookImportMutationRequestV1, ExternalHookImportPlanV1, ExternalHookImportSnapshotV1,
};
use bitfun_product_domains::external_source_control::{
    ExternalSourceControlRequestV1, ExternalSourceSurfaceSnapshotV1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceHostCapabilities, ExternalSourceOperationError, ExternalSourceOperationErrorCode,
    NativePromptCommandConflictSnapshot, NativePromptCommandDescriptor,
    PromptCommandInvocationOutcome, PromptCommandShellReviewDecision, SourceKey,
};
use bitfun_runtime_ports::AgentSessionWorkspaceBinding;

mod account_worktree;
mod external_sources;
mod hooks;
mod mcp;
mod model_registry;

pub(crate) use account_worktree::*;
pub(crate) use external_sources::*;
pub(crate) use hooks::*;
pub(crate) use mcp::*;
pub(crate) use model_registry::*;

fn map_core_management_error(error: bitfun_core::BitFunError) -> ManagementError {
    match error {
        bitfun_core::BitFunError::Validation(message) => {
            ManagementError::invalid_request(bounded_error(message))
        }
        bitfun_core::BitFunError::NotFound(message) => {
            ManagementError::not_found(bounded_error(message))
        }
        error => ManagementError::internal(bounded_error(error.to_string())),
    }
}

const ACCOUNT_CAPABILITY: &str = "tui.account";
const SETTINGS_SYNC_CAPABILITY: &str = "tui.settingsSync";
const WORKTREES_CAPABILITY: &str = "tui.worktrees";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagementErrorKind {
    Unsupported,
    InvalidRequest,
    NotFound,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagementError {
    pub kind: ManagementErrorKind,
    pub message: String,
}

pub(crate) type ManagementResult<T> = Result<T, ManagementError>;

impl fmt::Display for ManagementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ManagementError {}

impl ManagementError {
    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: ManagementErrorKind::Unsupported,
            message: message.into(),
        }
    }

    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            kind: ManagementErrorKind::InvalidRequest,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: ManagementErrorKind::NotFound,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: ManagementErrorKind::Internal,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagementScope {
    pub workspace_path: PathBuf,
    pub remote: bool,
}

impl ManagementScope {
    pub(crate) fn local(workspace_path: impl Into<PathBuf>) -> Self {
        Self {
            workspace_path: workspace_path.into(),
            remote: false,
        }
    }

    pub(crate) fn remote(workspace_path: impl Into<PathBuf>) -> Self {
        Self {
            workspace_path: workspace_path.into(),
            remote: true,
        }
    }

    pub(crate) fn local_workspace(&self, capability: &str) -> ManagementResult<&Path> {
        if self.remote {
            Err(ManagementError::unsupported(format!(
                "{capability} is not supported for remote workspaces"
            )))
        } else {
            Ok(&self.workspace_path)
        }
    }
}

fn validate_worktree_operation_id(operation_id: &str) -> ManagementResult<()> {
    let valid = !operation_id.trim().is_empty()
        && operation_id.len() <= 160
        && operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    valid
        .then_some(())
        .ok_or_else(|| ManagementError::invalid_request("Worktree operation ID is invalid"))
}

fn validate_external_operation_id(operation_id: &str) -> ManagementResult<()> {
    if operation_id.is_empty()
        || operation_id.len() > 160
        || operation_id.trim() != operation_id
        || operation_id.chars().any(char::is_control)
    {
        Err(ManagementError::invalid_request(
            "invalid external source operation id",
        ))
    } else {
        Ok(())
    }
}

fn bounded_error(message: String) -> String {
    message
        .chars()
        .filter(|character| !character.is_control())
        .take(500)
        .collect()
}

fn map_account_management_error(error: AccountManagementError) -> ManagementError {
    match error.kind {
        AccountManagementErrorKind::InvalidRequest => {
            ManagementError::invalid_request(error.message)
        }
        AccountManagementErrorKind::Internal => ManagementError::internal(error.message),
    }
}

pub(crate) fn map_external_error(error: ExternalSourceOperationError) -> ManagementError {
    let encoded = error.encode();
    match error.code {
        ExternalSourceOperationErrorCode::InvalidRequest => {
            ManagementError::invalid_request(encoded)
        }
        ExternalSourceOperationErrorCode::NotFound => ManagementError::not_found(encoded),
        ExternalSourceOperationErrorCode::HostCapabilityUnavailable
        | ExternalSourceOperationErrorCode::Unsupported => ManagementError::unsupported(encoded),
        _ => ManagementError::internal(encoded),
    }
}

fn map_external_string_error(error: String) -> ManagementError {
    map_external_error(
        bitfun_core::external_sources::sanitize_external_source_operation_error(error),
    )
}

fn map_external_string_error_with_id(error: String, operation_id: &str) -> ManagementError {
    let mut typed = bitfun_core::external_sources::sanitize_external_source_operation_error(error);
    if typed.correlation_id.is_none() {
        typed = typed.with_correlation_id(operation_id);
    } else if typed.causation_id.is_none() {
        typed = typed.with_causation_id(operation_id);
    }
    map_external_error(typed)
}

/// Concrete management owners composed by the CLI surface.
///
/// This is intentionally data, not a facade trait: each TUI operation still
/// delegates to the domain owner that defines its behavior and error contract.
pub(crate) struct TuiManagementOwners {
    pub model: ModelProvider,
    pub registry: RegistryProvider,
    pub mcp: McpProvider,
    pub account: AccountProvider,
    pub settings_sync: SettingsSyncProvider,
    pub worktree: WorktreeProvider,
    pub native_hook: NativeHookProvider,
    pub external_hook: ExternalHookProvider,
    pub external_source: ExternalSourceProvider,
    pub external_command: ExternalCommandProvider,
}

impl TuiManagementOwners {
    pub(crate) async fn load(
        account: Option<Arc<AccountRuntime>>,
        embedded_runtime_owner: bool,
    ) -> anyhow::Result<Self> {
        let config = bitfun_core::service::config::get_global_config_service()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to load CLI management config: {error}"))?;
        Ok(Self {
            model: ModelProvider::new(config),
            registry: RegistryProvider::new(),
            mcp: if embedded_runtime_owner {
                McpProvider::new(bitfun_core::service::mcp::get_global_mcp_service())
            } else {
                McpProvider::shared_runtime_unavailable()
            },
            account: AccountProvider::new(account.clone()),
            settings_sync: SettingsSyncProvider::new(account),
            worktree: if embedded_runtime_owner {
                WorktreeProvider::local()
            } else {
                WorktreeProvider::new(None)
            },
            native_hook: NativeHookProvider::new(),
            external_hook: ExternalHookProvider::new(),
            external_source: ExternalSourceProvider::new(),
            external_command: ExternalCommandProvider::new(),
        })
    }
}
