use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bitfun_core::native_hooks::{
    NativeHookManagementFile, NativeHookManagementHandler, NativeHookManagementOverview,
    NativeHookManagementRule,
};
use bitfun_product_domains::external_source_control::{
    ExternalSourceControlActionV1, ExternalSourceControlRequestV1,
    EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
};
use bitfun_product_domains::external_sources::{
    ExternalSourceOperationError, ExternalSourceOperationErrorCode,
};

use crate::tui_management::{
    project_native_hook_overview, AccountProvider, ExternalCommandProvider, ExternalHookProvider,
    ExternalSourceProvider, ManagementErrorKind, ManagementScope, McpProvider, NativeHookProvider,
    RegistryProvider, SettingsSyncProvider, WorktreeBindingRequest, WorktreeOwner,
    WorktreeProvider,
};

#[derive(Default)]
struct CountingWorktreeOwner {
    calls: AtomicUsize,
}

#[async_trait]
impl WorktreeOwner for CountingWorktreeOwner {
    async fn bind_session(
        &self,
        _request: bitfun_core::service::worktree::WorktreeSessionBindingRequest,
    ) -> Result<
        bitfun_core::service::worktree::WorktreeSessionBindingResult,
        bitfun_core_types::WorktreeError,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        panic!("remote requests must not reach the local worktree owner")
    }
}

#[tokio::test]
async fn account_and_settings_validation_precede_optional_owner_lookup() {
    let account = AccountProvider::new(None);
    let scope = ManagementScope::local("D:/project");
    let invalid = account
        .logout(&scope, "bad operation")
        .await
        .expect_err("invalid account operation id");
    assert_eq!(invalid.kind, ManagementErrorKind::InvalidRequest);
    assert_eq!(invalid.message, "Account operation ID is invalid");

    let unavailable = account
        .logout(&scope, "account-op-1")
        .await
        .expect_err("missing account owner");
    assert_eq!(unavailable.kind, ManagementErrorKind::Unsupported);
    assert_eq!(unavailable.message, "tui.account is unavailable");

    let settings = SettingsSyncProvider::new(None);
    let invalid = settings
        .cancel(&scope, "bad operation")
        .await
        .expect_err("invalid settings operation id");
    assert_eq!(invalid.kind, ManagementErrorKind::InvalidRequest);
    assert_eq!(invalid.message, "Account operation ID is invalid");
}

#[tokio::test]
async fn remote_worktree_binding_is_typed_unsupported_without_local_owner_call() {
    let owner = Arc::new(CountingWorktreeOwner::default());
    let provider = WorktreeProvider::new(Some(owner.clone()));
    let error = provider
        .bind(WorktreeBindingRequest {
            scope: ManagementScope::remote("D:/remote/project"),
            operation_id: "worktree-op-1".to_string(),
            session_id: "session-1".to_string(),
            project_workspace_path: None,
        })
        .await
        .expect_err("remote worktree binding");

    assert_eq!(error.kind, ManagementErrorKind::Unsupported);
    assert_eq!(
        error.message,
        "Managed worktrees are not supported for remote workspaces"
    );
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn remote_workspace_management_rejects_before_local_owner_or_input_validation() {
    let scope = ManagementScope::remote("/srv/remote-project");

    let account = AccountProvider::new(None)
        .logout(&scope, "bad operation")
        .await
        .expect_err("remote account access must stop before validation or owner lookup");
    assert_eq!(account.kind, ManagementErrorKind::Unsupported);

    let settings = SettingsSyncProvider::new(None)
        .cancel(&scope, "bad operation")
        .await
        .expect_err("remote settings sync must stop before validation or owner lookup");
    assert_eq!(settings.kind, ManagementErrorKind::Unsupported);

    let skills = RegistryProvider::new()
        .list_skills(&scope, "agent", true)
        .await
        .expect_err("remote skill registry must not read controller-local state");
    assert_eq!(skills.kind, ManagementErrorKind::Unsupported);

    let mcp = McpProvider::new(None)
        .toggle(&scope, "server-1")
        .await
        .expect_err("remote MCP mutation must stop before owner lookup");
    assert_eq!(mcp.kind, ManagementErrorKind::Unsupported);
    assert!(mcp.message.contains("remote workspaces"));

    let hooks = NativeHookProvider::new()
        .overview(&scope)
        .await
        .expect_err("remote hook inspection must not read controller-local files");
    assert_eq!(hooks.kind, ManagementErrorKind::Unsupported);

    let external_hooks = ExternalHookProvider::new()
        .snapshot(&scope, false)
        .await
        .expect_err("remote external-hook inspection must fail closed");
    assert_eq!(external_hooks.kind, ManagementErrorKind::Unsupported);

    let source = ExternalSourceProvider::new();
    let invalid_request = ExternalSourceControlRequestV1 {
        schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
        operation_id: " leading".to_string(),
        expected_preference_revision: None,
        action: ExternalSourceControlActionV1::Refresh,
    };
    let error = source
        .control(&scope, invalid_request)
        .await
        .expect_err("remote source control must reject before local validation or owner access");
    assert_eq!(error.kind, ManagementErrorKind::Unsupported);
}

#[tokio::test]
async fn shared_runtime_mcp_management_is_restart_required_without_controller_service() {
    let error = McpProvider::shared_runtime_unavailable()
        .list(&ManagementScope::local("D:/project"))
        .await
        .expect_err("Shared TUI must not manage a controller-local MCP duplicate");

    assert_eq!(error.kind, ManagementErrorKind::Unsupported);
    assert!(error.message.contains("Shared Runtime"));
    assert!(error.message.contains("restart"));
}

#[tokio::test]
async fn worktree_operation_id_validation_matches_existing_management_contract() {
    let provider = WorktreeProvider::new(None);
    let error = provider
        .bind(WorktreeBindingRequest {
            scope: ManagementScope::local("D:/project"),
            operation_id: "bad operation".to_string(),
            session_id: "session-1".to_string(),
            project_workspace_path: None,
        })
        .await
        .expect_err("invalid worktree operation id");
    assert_eq!(error.kind, ManagementErrorKind::InvalidRequest);
    assert_eq!(error.message, "Worktree operation ID is invalid");
}

#[tokio::test]
async fn external_control_and_command_reject_invalid_operation_ids_before_core() {
    let source = ExternalSourceProvider::new();
    let error = source
        .control(
            &ManagementScope::local("D:/project"),
            ExternalSourceControlRequestV1 {
                schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
                operation_id: " leading".to_string(),
                expected_preference_revision: None,
                action: ExternalSourceControlActionV1::Refresh,
            },
        )
        .await
        .expect_err("invalid external source control operation id");
    assert_eq!(error.kind, ManagementErrorKind::InvalidRequest);
    assert_eq!(
        error.message,
        "invalid external source control operation id"
    );

    let command = ExternalCommandProvider::new();
    let error = command
        .set_native_choice(
            &ManagementScope::local("D:/project"),
            " leading",
            Vec::new(),
            "bitfun.native",
            1,
        )
        .await
        .expect_err("invalid external command operation id");
    assert_eq!(error.kind, ManagementErrorKind::InvalidRequest);
    assert_eq!(error.message, "invalid external source operation id");
}

#[test]
fn external_owner_errors_keep_existing_kind_mapping() {
    for (code, expected) in [
        (
            ExternalSourceOperationErrorCode::InvalidRequest,
            ManagementErrorKind::InvalidRequest,
        ),
        (
            ExternalSourceOperationErrorCode::NotFound,
            ManagementErrorKind::NotFound,
        ),
        (
            ExternalSourceOperationErrorCode::HostCapabilityUnavailable,
            ManagementErrorKind::Unsupported,
        ),
        (
            ExternalSourceOperationErrorCode::Unsupported,
            ManagementErrorKind::Unsupported,
        ),
        (
            ExternalSourceOperationErrorCode::Internal,
            ManagementErrorKind::Internal,
        ),
    ] {
        let error = crate::tui_management::map_external_error(ExternalSourceOperationError::new(
            code,
            "owner detail",
            false,
        ));
        assert_eq!(error.kind, expected);
        assert!(error.message.contains("owner detail"));
    }
}

#[test]
fn native_hook_projection_redacts_paths_and_bounds_commands() {
    let overview = NativeHookManagementOverview {
        enabled: true,
        project_hooks_enabled: true,
        files: vec![NativeHookManagementFile {
            scope: "project".to_string(),
            location: "<workspace>/.bitfun/config/hooks.json".to_string(),
            exists: true,
            loaded: true,
        }],
        rules: vec![NativeHookManagementRule {
            event: "PreToolUse".to_string(),
            matcher: "Bash".to_string(),
            matcher_is_valid: true,
            scope: "project".to_string(),
            handlers: vec![NativeHookManagementHandler {
                command_summary: format!("{}...", "x".repeat(200)),
                command_truncated: true,
                timeout_seconds: 5,
                status_message: Some("Checking".to_string()),
            }],
        }],
        total_handlers: 1,
        issues: vec!["Failed to read <workspace>/.bitfun/config/hooks.json".to_string()],
    };

    let projected = project_native_hook_overview(overview);
    assert_eq!(
        projected.files[0].location,
        "<workspace>/.bitfun/config/hooks.json"
    );
    assert!(projected.rules[0].handlers[0]
        .command_summary
        .ends_with("..."));
    assert_eq!(
        projected.rules[0].handlers[0]
            .command_summary
            .chars()
            .count(),
        203
    );
    let debug = format!("{projected:?}");
    assert!(!debug.contains("D:/secret/project"));
    assert!(!debug.contains("secret-token"));
}
