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
use crate::agentic::session::SessionExecutionBindingUpdate;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSessionBindingRequest {
    pub request_id: String,
    pub session_id: String,
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

async fn load_binding_context(session_id: &str) -> Result<SessionBindingContext, WorktreeError> {
    let coordinator = get_global_coordinator().ok_or_else(|| {
        error(
            WorktreeErrorCode::IoFailed,
            "Session coordinator is not initialized",
        )
    })?;
    let session = coordinator
        .get_session_manager()
        .get_session(session_id)
        .ok_or_else(|| {
            error(
                WorktreeErrorCode::WorktreeNotFound,
                format!("Session not found: {session_id}"),
            )
        })?;

    if !session.dialog_turn_ids.is_empty() {
        return Err(error(
            WorktreeErrorCode::WorktreeBusy,
            "Worktree isolation can only be changed before the session's first message",
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
    if lookup_remote_connection(&project_workspace_path)
        .await
        .is_some()
    {
        return Err(error(
            WorktreeErrorCode::RemoteUnsupported,
            "Managed worktrees are not supported for remote SSH workspaces yet",
        ));
    }

    let execution_target = session
        .config
        .execution_target
        .clone()
        .unwrap_or_else(|| SessionExecutionTarget::local(workspace_path));

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
        .map_err(|session_error| {
            error(
                WorktreeErrorCode::IoFailed,
                format!("Failed to rebind session workspace: {session_error}"),
            )
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
        let context = load_binding_context(&request.session_id).await?;
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
