//! Plan-todo binding between agent sessions and plan todos.
//!
//! `SessionMessage` can bind a dispatched session to a plan todo by carrying
//! `planFile` / `todoId` in the forwarded turn metadata (see
//! `session_message_tool.rs`). The scheduler reads that binding and issues
//! best-effort PlanUpdate status changes:
//! - when a bound execution turn starts        -> todo `in_progress`
//! - when a bound execution turn finishes OK   -> todo `completed`
//!
//! Every failure is logged and swallowed: the binding layer must never break,
//! delay, or block a dialog turn (best-effort semantics). Callers gate on
//! `reply_route.is_some()` so reply turns (which inherit the metadata) never
//! re-trigger the hooks.

use crate::agentic::tools::implementations::plan_update_tool::{
    apply_todo_status_update, resolve_plan_path_for_backend,
};
use crate::util::errors::BitFunError;
use bitfun_agent_runtime::scheduler::{TurnOutcome, TurnOutcomeStatus};
use log::{debug, info, warn};
use serde_json::Value;
use std::path::Path;

/// Metadata key injected by SessionMessage when a dispatch is bound to a plan file.
pub(crate) const PLAN_FILE_METADATA_KEY: &str = "planFile";
/// Metadata key injected by SessionMessage when a dispatch is bound to a plan todo.
pub(crate) const TODO_ID_METADATA_KEY: &str = "todoId";

/// Read the optional plan-todo binding from turn metadata. Returns
/// `(plan_file, todo_id)` when both keys are present and non-empty.
pub(crate) fn read_todo_binding(metadata: Option<&Value>) -> Option<(String, String)> {
    let metadata = metadata?;
    let plan_file = metadata.get(PLAN_FILE_METADATA_KEY)?.as_str()?;
    let todo_id = metadata.get(TODO_ID_METADATA_KEY)?.as_str()?;
    let plan_file = plan_file.trim();
    let todo_id = todo_id.trim();
    if plan_file.is_empty() || todo_id.is_empty() {
        return None;
    }
    Some((plan_file.to_string(), todo_id.to_string()))
}

/// Pure decision: should the auto-complete hook fire for this outcome? Only
/// Completed outcomes advance the todo; Failed/Cancelled outcomes are kept
/// pending for the commander to adjudicate.
pub(crate) fn should_auto_complete_todo(outcome: &TurnOutcome) -> bool {
    outcome.status() == TurnOutcomeStatus::Completed
}

/// Best-effort: mark the bound todo `in_progress` when the turn metadata
/// carries a plan-todo binding. Caller gates on `reply_route.is_some()` so
/// only execution turns (never reply turns) reach this hook.
pub(crate) async fn auto_mark_todo_in_progress_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
) {
    mark_todo_status_if_bound(
        metadata,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        "in_progress",
        "auto_mark_todo_in_progress",
    )
    .await;
}

/// Best-effort: mark the bound todo `completed` when the finished turn carried
/// a plan-todo binding AND completed normally. Failed/Cancelled outcomes are
/// left untouched. Caller gates on `reply_route.is_some()` so reply turns
/// (which inherit the binding metadata) never re-mark.
pub(crate) async fn auto_mark_todo_completed_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
    outcome: &TurnOutcome,
) {
    if !should_auto_complete_todo(outcome) {
        return;
    }
    mark_todo_status_if_bound(
        metadata,
        workspace_path,
        remote_connection_id,
        remote_ssh_host,
        "completed",
        "auto_mark_todo_completed",
    )
    .await;
}

async fn mark_todo_status_if_bound(
    metadata: Option<&Value>,
    workspace_path: Option<&str>,
    remote_connection_id: Option<&str>,
    remote_ssh_host: Option<&str>,
    status: &str,
    hook: &str,
) {
    let Some((plan_file, todo_id)) = read_todo_binding(metadata) else {
        return;
    };
    // Remote workspaces keep their plan files on the remote host; the local
    // scheduler cannot read or write them. Skip instead of failing noisily.
    if remote_connection_id.is_some() || remote_ssh_host.is_some() {
        debug!(
            "{}: skipping plan-todo binding on remote workspace (plan files live on the remote host): plan_file={}, todo_id={}",
            hook, plan_file, todo_id
        );
        return;
    }
    let Some(workspace_path) = workspace_path else {
        warn!(
            "{}: cannot resolve plan-todo binding without a workspace path: plan_file={}, todo_id={}",
            hook, plan_file, todo_id
        );
        return;
    };
    let result = async {
        let plan_path = resolve_plan_path_for_backend(&plan_file, Some(Path::new(workspace_path)))
            .await?;
        apply_todo_status_update(&plan_path, &todo_id, status).await?;
        Ok::<_, BitFunError>(())
    }
    .await;
    match result {
        Ok(()) => info!(
            "{}: plan todo marked {}: plan_file={}, todo_id={}",
            hook, status, plan_file, todo_id
        ),
        Err(error) => warn!(
            "{}: failed to update bound plan todo (best-effort, turn continues): plan_file={}, todo_id={}, error={}",
            hook, plan_file, todo_id, error
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn completed_outcome(turn_id: &str) -> TurnOutcome {
        TurnOutcome::Completed {
            turn_id: turn_id.to_string(),
            final_response: "done".to_string(),
        }
    }

    #[test]
    fn read_todo_binding_returns_none_without_metadata() {
        assert_eq!(read_todo_binding(None), None);
    }

    #[test]
    fn read_todo_binding_returns_none_without_binding_keys() {
        let metadata = json!({ "senderSessionId": "source-1" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn read_todo_binding_requires_both_keys() {
        let metadata = json!({ "planFile": "my_plan_1234.plan.md" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
        let metadata = json!({ "todoId": "setup-auth" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn read_todo_binding_returns_binding_when_both_present() {
        let metadata = json!({
            "planFile": "my_plan_1234.plan.md",
            "todoId": "setup-auth",
        });
        assert_eq!(
            read_todo_binding(Some(&metadata)),
            Some(("my_plan_1234.plan.md".to_string(), "setup-auth".to_string()))
        );
    }

    #[test]
    fn read_todo_binding_rejects_empty_values() {
        let metadata = json!({ "planFile": "  ", "todoId": "setup-auth" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
        let metadata = json!({ "planFile": "my_plan.plan.md", "todoId": "" });
        assert_eq!(read_todo_binding(Some(&metadata)), None);
    }

    #[test]
    fn should_auto_complete_todo_only_for_completed_outcomes() {
        assert!(should_auto_complete_todo(&completed_outcome("turn-1")));
        assert!(!should_auto_complete_todo(&TurnOutcome::Cancelled {
            turn_id: "turn-2".to_string()
        }));
        assert!(!should_auto_complete_todo(&TurnOutcome::Failed {
            turn_id: "turn-3".to_string(),
            error: "boom".to_string()
        }));
    }
}
