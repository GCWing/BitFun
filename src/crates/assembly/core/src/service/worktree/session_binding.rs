//! Per-session worktree isolation.
//!
//! A session either executes in the project checkout or in a managed worktree
//! of the same repository. This module owns the transition between the two:
//! it creates or releases the worktree and rebinds the session in one step, so
//! callers never have to keep the two halves consistent themselves.
//!
//! Rebinding is only offered while a session is still empty. Once a transcript
//! exists it describes work done in a specific directory, and moving that
//! directory underneath it would silently invalidate the history.

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::keyed_lock::KeyedAsyncLock;
use crate::agentic::session::{SessionExecutionBindingError, SessionExecutionBindingUpdate};
use crate::service::remote_ssh::lookup_remote_connection;
use crate::service::workspace::get_global_workspace_service;
use crate::service::worktree::{
    WorktreeCreateRequest, WorktreeListRequest, WorktreeRemoveRequest, WorktreeService,
};
use bitfun_core_types::{
    SessionExecutionTarget, WorktreeError, WorktreeErrorCode, WorktreeLifecycle,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

/// Serializes the complete Git-create/rebind/release transition for one session.
///
/// The SessionManager mutation lock closes the race with turn start, but it is
/// intentionally held only around the final session mutation. A separate lock
/// is needed here so concurrent adapters in one product runtime cannot both
/// preflight the same empty session, create different worktrees, and then
/// overwrite each other's binding.
static SESSION_BINDING_LOCKS: LazyLock<KeyedAsyncLock> = LazyLock::new(KeyedAsyncLock::default);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSessionBindingRequest {
    pub request_id: String,
    pub session_id: String,
    /// Stable owner path used to locate view-only or evicted persisted sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_workspace_path: Option<String>,
    /// `true` moves the session into a managed worktree, `false` back to the project.
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSessionBindingResult {
    pub session_id: String,
    pub workspace_path: String,
    pub project_workspace_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub execution_target: SessionExecutionTarget,
    /// Set when a released worktree was kept because it still held local work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_worktree_path: Option<String>,
}

/// Session facts the binding decision depends on.
struct SessionBindingContext {
    project_workspace_path: String,
    execution_target: SessionExecutionTarget,
}

fn error(code: WorktreeErrorCode, message: impl Into<String>) -> WorktreeError {
    WorktreeError {
        code,
        message: message.into(),
        recovery_path: None,
    }
}

async fn load_binding_context(
    request: &WorktreeSessionBindingRequest,
) -> Result<SessionBindingContext, WorktreeError> {
    let coordinator = get_global_coordinator().ok_or_else(|| {
        error(
            WorktreeErrorCode::IoFailed,
            "Session coordinator is not initialized",
        )
    })?;
    let session_manager = coordinator.get_session_manager();
    let session = session_manager.get_session(&request.session_id);

    let (workspace_path, project_workspace_path, execution_target) = if let Some(session) = session
    {
        if !session.dialog_turn_ids.is_empty() {
            return Err(error(
                WorktreeErrorCode::WorktreeBusy,
                "Worktree isolation can only be changed before the session's first message",
            ));
        }
        if !matches!(session.state, crate::agentic::core::SessionState::Idle) {
            return Err(error(
                WorktreeErrorCode::WorktreeBusy,
                "Worktree isolation cannot be changed while the session is processing",
            ));
        }
        if session.config.remote_connection_id.is_some() {
            return Err(error(
                WorktreeErrorCode::RemoteUnsupported,
                "Managed worktrees are not supported for remote SSH workspaces yet",
            ));
        }

        let workspace_path = session.config.workspace_path.clone().ok_or_else(|| {
            error(
                WorktreeErrorCode::InvalidPath,
                "Session is not bound to a workspace",
            )
        })?;
        let project_workspace_path = session
            .config
            .project_workspace_path
            .clone()
            .unwrap_or_else(|| workspace_path.clone());
        let execution_target = session
            .config
            .execution_target
            .clone()
            .unwrap_or_else(|| SessionExecutionTarget::local(workspace_path.clone()));
        (workspace_path, project_workspace_path, execution_target)
    } else {
        let project_workspace_path = request
                .project_workspace_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    error(
                        WorktreeErrorCode::WorktreeNotFound,
                        format!(
                            "Session not found: {}. The project workspace path is required to restore historical sessions",
                            request.session_id
                        ),
                    )
                })?
                .to_string();
        let metadata = session_manager
            .load_session_metadata(Path::new(&project_workspace_path), &request.session_id)
            .await
            .map_err(|metadata_error| {
                error(
                    WorktreeErrorCode::IoFailed,
                    format!("Failed to load session metadata: {metadata_error}"),
                )
            })?
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    format!("Session not found: {}", request.session_id),
                )
            })?;
        if metadata.turn_count > 0 {
            return Err(error(
                WorktreeErrorCode::WorktreeBusy,
                "Worktree isolation can only be changed before the session's first message",
            ));
        }

        let workspace_path = metadata
            .workspace_path
            .clone()
            .unwrap_or_else(|| project_workspace_path.clone());
        let persisted_project_path = metadata
            .project_workspace_path
            .clone()
            .unwrap_or(project_workspace_path);
        let execution_target = metadata
            .execution_target
            .clone()
            .unwrap_or_else(|| SessionExecutionTarget::local(workspace_path.clone()));
        (workspace_path, persisted_project_path, execution_target)
    };

    if lookup_remote_connection(&project_workspace_path)
        .await
        .is_some()
    {
        return Err(error(
            WorktreeErrorCode::RemoteUnsupported,
            "Managed worktrees are not supported for remote SSH workspaces yet",
        ));
    }

    if workspace_path.trim().is_empty() {
        return Err(error(
            WorktreeErrorCode::InvalidPath,
            "Session is not bound to a workspace",
        ));
    }

    Ok(SessionBindingContext {
        project_workspace_path,
        execution_target,
    })
}

async fn current_workspace_id(root_path: &str) -> Option<String> {
    get_global_workspace_service()?
        .get_workspace_by_path(Path::new(root_path))
        .await
        .map(|workspace| workspace.id)
}

async fn rebind(
    session_id: &str,
    project_workspace_path: &str,
    execution_target: SessionExecutionTarget,
) -> Result<WorktreeSessionBindingResult, WorktreeError> {
    let coordinator = get_global_coordinator().ok_or_else(|| {
        error(
            WorktreeErrorCode::IoFailed,
            "Session coordinator is not initialized",
        )
    })?;
    let workspace_id = current_workspace_id(&execution_target.root_path).await;

    coordinator
        .get_session_manager()
        .update_session_execution_binding(
            session_id,
            SessionExecutionBindingUpdate {
                workspace_path: execution_target.root_path.clone(),
                project_workspace_path: project_workspace_path.to_string(),
                workspace_id: workspace_id.clone(),
                execution_target: execution_target.clone(),
            },
        )
        .await
        .map_err(|session_error| match session_error {
            SessionExecutionBindingError::Busy(message) => {
                error(WorktreeErrorCode::WorktreeBusy, message)
            }
            SessionExecutionBindingError::NotFound(message) => {
                error(WorktreeErrorCode::WorktreeNotFound, message)
            }
            SessionExecutionBindingError::Internal(internal) => error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to rebind session workspace: {internal}"),
            ),
        })?;

    Ok(WorktreeSessionBindingResult {
        session_id: session_id.to_string(),
        workspace_path: execution_target.root_path.clone(),
        project_workspace_path: project_workspace_path.to_string(),
        workspace_id,
        execution_target,
        retained_worktree_path: None,
    })
}

impl WorktreeService {
    /// Move a session into a fresh managed worktree, or back to the project checkout.
    ///
    /// Enabling is idempotent through `request_id`: a retried request replays the
    /// worktree that request already created instead of allocating another one.
    pub async fn bind_session(
        request: WorktreeSessionBindingRequest,
    ) -> Result<WorktreeSessionBindingResult, WorktreeError> {
        bitfun_core_types::validate_session_id(&request.session_id)
            .map_err(|message| error(WorktreeErrorCode::InvalidPath, message))?;
        let _binding_guard = SESSION_BINDING_LOCKS.lock(&request.session_id).await;
        let context = load_binding_context(&request).await?;
        let is_worktree = context.execution_target.worktree_id.is_some();

        if request.enabled == is_worktree {
            // Already in the requested state; report it rather than churn Git.
            return Ok(WorktreeSessionBindingResult {
                session_id: request.session_id,
                workspace_path: context.execution_target.root_path.clone(),
                project_workspace_path: context.project_workspace_path,
                workspace_id: current_workspace_id(&context.execution_target.root_path).await,
                execution_target: context.execution_target,
                retained_worktree_path: None,
            });
        }

        if request.enabled {
            Self::enable_session_worktree(&request, &context).await
        } else {
            Self::disable_session_worktree(&request, &context).await
        }
    }

    async fn enable_session_worktree(
        request: &WorktreeSessionBindingRequest,
        context: &SessionBindingContext,
    ) -> Result<WorktreeSessionBindingResult, WorktreeError> {
        let settings = Self::settings().await;
        let created = Self::create(WorktreeCreateRequest {
            request_id: request.request_id.clone(),
            project_workspace_path: context.project_workspace_path.clone(),
            source_workspace_path: Some(context.execution_target.root_path.clone()),
            base_ref: None,
            copy_local_changes: settings.copy_local_changes,
            // A bound session is the claim: it already blocks automatic removal.
            claimed_by: None,
        })
        .await?;

        let worktree_id = created.execution_target.worktree_id.clone();
        match rebind(
            &request.session_id,
            &created.worktree.project_workspace_path,
            created.execution_target,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(bind_error) => {
                // The worktree only exists to host this session; drop it again so a
                // failed toggle does not leave an orphan directory behind.
                if created.created {
                    if let Some(worktree_id) = worktree_id.as_deref() {
                        if let Err(rollback_error) =
                            Self::rollback_created(&context.project_workspace_path, worktree_id)
                                .await
                        {
                            log::warn!(
                                "Failed to roll back worktree {worktree_id} after a failed session rebind: {rollback_error}"
                            );
                        }
                    }
                }
                Err(bind_error)
            }
        }
    }

    async fn disable_session_worktree(
        request: &WorktreeSessionBindingRequest,
        context: &SessionBindingContext,
    ) -> Result<WorktreeSessionBindingResult, WorktreeError> {
        let worktree_id = context
            .execution_target
            .worktree_id
            .clone()
            .ok_or_else(|| {
                error(
                    WorktreeErrorCode::WorktreeNotFound,
                    "Session is not bound to a worktree",
                )
            })?;
        let worktree_path = context.execution_target.root_path.clone();

        // Detach first: removal safety checks count sessions still pointing here.
        let mut result = rebind(
            &request.session_id,
            &context.project_workspace_path,
            SessionExecutionTarget::local(context.project_workspace_path.clone()),
        )
        .await?;

        let removable = Self::list(WorktreeListRequest {
            project_workspace_path: context.project_workspace_path.clone(),
        })
        .await
        .ok()
        .and_then(|worktrees| {
            worktrees
                .into_iter()
                .find(|worktree| worktree.worktree_id == worktree_id)
        })
        .map(|worktree| {
            worktree.lifecycle == WorktreeLifecycle::Managed
                && !worktree.dirty
                && !worktree.has_unpublished_commits
                && !worktree.locked
                && !worktree.missing
                && worktree.associated_session_count == 0
        })
        .unwrap_or(false);

        if removable {
            match Self::remove(WorktreeRemoveRequest {
                request_id: request.request_id.clone(),
                project_workspace_path: context.project_workspace_path.clone(),
                worktree_id,
                force: false,
            })
            .await
            {
                Ok(_) => return Ok(result),
                Err(remove_error) => {
                    log::warn!("Released worktree could not be removed: {remove_error}");
                }
            }
        }

        result.retained_worktree_path = Some(worktree_path);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{WorktreeSessionBindingRequest, SESSION_BINDING_LOCKS};
    use std::time::Duration;

    #[test]
    fn binding_request_keeps_legacy_callers_compatible() {
        let request: WorktreeSessionBindingRequest = serde_json::from_value(serde_json::json!({
            "requestId": "request-1",
            "sessionId": "session-1",
            "enabled": true
        }))
        .expect("legacy request should deserialize");

        assert_eq!(request.project_workspace_path, None);
    }

    #[test]
    fn binding_request_uses_a_cross_platform_project_locator() {
        let request: WorktreeSessionBindingRequest = serde_json::from_value(serde_json::json!({
            "requestId": "request-2",
            "sessionId": "session-2",
            "projectWorkspacePath": "D:\\workspace\\BitFun",
            "enabled": false
        }))
        .expect("request should deserialize");

        assert_eq!(
            request.project_workspace_path.as_deref(),
            Some(r"D:\workspace\BitFun")
        );
    }

    #[tokio::test]
    async fn binding_transitions_for_the_same_session_are_serialized() {
        let session_id = format!("binding-lock-{}", uuid::Uuid::new_v4());
        let first = SESSION_BINDING_LOCKS.lock(&session_id).await;

        assert!(
            tokio::time::timeout(
                Duration::from_millis(20),
                SESSION_BINDING_LOCKS.lock(&session_id),
            )
            .await
            .is_err(),
            "a second transition must wait for the first"
        );

        drop(first);
        tokio::time::timeout(
            Duration::from_secs(1),
            SESSION_BINDING_LOCKS.lock(&session_id),
        )
        .await
        .expect("the next transition should proceed after release");
    }
}
