use super::coordination_store::{
    BackgroundTaskRecord, BackgroundTaskRegistration, BackgroundTaskStatus, CoordinationStore,
    RegisteredBackgroundTask,
};
use super::coordinator::{SubagentResult, SubagentResultStatus};
use crate::agentic::session::SessionManager;
use crate::service::session::TurnStatus;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use log::warn;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{sleep_until, Duration, Instant};
use tokio_util::sync::CancellationToken;

const RESULT_DEBOUNCE: Duration = Duration::from_secs(5);

pub(crate) type BackgroundSubagentOutcomeStatus = BackgroundTaskStatus;

#[derive(Debug, Clone)]
pub(crate) struct BackgroundSubagentOutcome {
    task_pk: i64,
    pub bg_task_id: String,
    pub agent_id: String,
    pub status: BackgroundSubagentOutcomeStatus,
    pub content: Option<String>,
    pub error: Option<String>,
}

impl BackgroundSubagentOutcome {
    pub(crate) fn model_bg_task_id(&self) -> &str {
        &self.bg_task_id
    }

    pub(crate) fn model_agent_id(&self) -> &str {
        &self.agent_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundSubagentWaitStatus {
    Completed,
    TimedOut,
    NoMatchingTasks,
}

impl BackgroundSubagentWaitStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::TimedOut => "timed_out",
            Self::NoMatchingTasks => "no_matching_tasks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundSubagentWaitMode {
    Any,
    All,
}

impl BackgroundSubagentWaitMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundSubagentWaitResult {
    pub status: BackgroundSubagentWaitStatus,
    pub outcomes: Vec<BackgroundSubagentOutcome>,
    pub pending_bg_task_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct LiveBackgroundResult {
    status: BackgroundTaskStatus,
    content: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct AvailableOutcomes {
    outcomes: Vec<BackgroundSubagentOutcome>,
    pending_bg_task_ids: Vec<String>,
}

pub(crate) struct BackgroundSubagentOutcomeStore {
    live_results: DashMap<i64, LiveBackgroundResult>,
    changes: Notify,
    session_manager: Arc<SessionManager>,
    coordination_store: Arc<CoordinationStore>,
}

impl BackgroundSubagentOutcomeStore {
    pub(crate) fn new(
        session_manager: Arc<SessionManager>,
        coordination_store: Arc<CoordinationStore>,
    ) -> Self {
        Self {
            live_results: DashMap::new(),
            changes: Notify::new(),
            session_manager,
            coordination_store,
        }
    }

    pub(crate) async fn register(
        &self,
        registration: BackgroundTaskRegistration,
    ) -> BitFunResult<RegisteredBackgroundTask> {
        let registered = self
            .coordination_store
            .register_background_task(registration)
            .await?;
        self.changes.notify_waiters();
        Ok(registered)
    }

    pub(crate) async fn complete(
        &self,
        task_pk: i64,
        result: Result<&SubagentResult, &BitFunError>,
    ) {
        let live_result = match result {
            Ok(result) => LiveBackgroundResult {
                status: match result.status {
                    SubagentResultStatus::Completed => BackgroundTaskStatus::Completed,
                    SubagentResultStatus::PartialTimeout => BackgroundTaskStatus::PartialTimeout,
                },
                content: Some(result.text.clone()),
                error: result.reason.clone(),
            },
            Err(error) => LiveBackgroundResult {
                status: if matches!(error, BitFunError::Cancelled(_)) {
                    BackgroundTaskStatus::Cancelled
                } else {
                    BackgroundTaskStatus::Failed
                },
                content: None,
                error: Some(error.to_string()),
            },
        };
        match self
            .coordination_store
            .update_task_status(task_pk, live_result.status, None, live_result.error.clone())
            .await
        {
            Ok(true) => {
                self.live_results.insert(task_pk, live_result);
                self.changes.notify_waiters();
            }
            Ok(false) => {}
            Err(error) => {
                warn!(
                    "Failed to persist background subagent completion: task_pk={}, error={}",
                    task_pk, error
                );
            }
        }
    }

    pub(crate) async fn cancel(&self, task_pks: &[i64]) {
        for task_pk in task_pks {
            match self
                .coordination_store
                .update_task_status(
                    *task_pk,
                    BackgroundTaskStatus::Cancelled,
                    Some("cancelled".to_string()),
                    Some("Background subagent task was cancelled".to_string()),
                )
                .await
            {
                Ok(true) => {
                    self.live_results.insert(
                        *task_pk,
                        LiveBackgroundResult {
                            status: BackgroundTaskStatus::Cancelled,
                            content: None,
                            error: Some("Background subagent task was cancelled".to_string()),
                        },
                    );
                }
                Ok(false) => {}
                Err(error) => {
                    warn!(
                        "Failed to persist background subagent cancellation: task_pk={}, error={}",
                        task_pk, error
                    );
                }
            }
        }
        self.changes.notify_waiters();
    }

    pub(crate) async fn discard(&self, task_pk: i64) -> BitFunResult<()> {
        self.live_results.remove(&task_pk);
        self.coordination_store
            .delete_background_task(task_pk)
            .await?;
        self.changes.notify_waiters();
        Ok(())
    }

    pub(crate) async fn wait_for(
        &self,
        parent_session_id: &str,
        requested_bg_task_ids: &[String],
        wait_mode: BackgroundSubagentWaitMode,
        timeout: Duration,
        delivered_parent_dialog_turn_id: &str,
        cancellation_token: Option<&CancellationToken>,
    ) -> BitFunResult<BackgroundSubagentWaitResult> {
        self.reconcile_stale_running_tasks(parent_session_id)
            .await?;
        let selected = self
            .coordination_store
            .wait_candidates(parent_session_id, requested_bg_task_ids)
            .await?;
        if selected.is_empty() {
            return Ok(wait_result(
                BackgroundSubagentWaitStatus::NoMatchingTasks,
                Vec::new(),
                Vec::new(),
            ));
        }
        let selected_task_pks = selected
            .iter()
            .map(|record| record.task_pk)
            .collect::<Vec<_>>();
        let deadline = Instant::now() + timeout;
        let mut debounce_deadline = None;

        loop {
            let notified = self.changes.notified();
            tokio::pin!(notified);

            let available = self.collect_available(&selected_task_pks).await?;
            if !available.outcomes.is_empty() && available.pending_bg_task_ids.is_empty() {
                if let Some(result) = self
                    .claim_result(
                        parent_session_id,
                        delivered_parent_dialog_turn_id,
                        BackgroundSubagentWaitStatus::Completed,
                        available,
                    )
                    .await?
                {
                    return Ok(result);
                }
                debounce_deadline = None;
                continue;
            }
            if wait_mode == BackgroundSubagentWaitMode::Any
                && !available.outcomes.is_empty()
                && debounce_deadline.is_none()
            {
                debounce_deadline = Some((Instant::now() + RESULT_DEBOUNCE).min(deadline));
            }

            let wake_at = debounce_deadline.unwrap_or(deadline);
            if Instant::now() >= wake_at {
                if available.outcomes.is_empty() {
                    return Ok(wait_result(
                        BackgroundSubagentWaitStatus::TimedOut,
                        Vec::new(),
                        available.pending_bg_task_ids,
                    ));
                }
                if let Some(result) = self
                    .claim_result(
                        parent_session_id,
                        delivered_parent_dialog_turn_id,
                        if wake_at == deadline {
                            BackgroundSubagentWaitStatus::TimedOut
                        } else {
                            BackgroundSubagentWaitStatus::Completed
                        },
                        available,
                    )
                    .await?
                {
                    return Ok(result);
                }
                debounce_deadline = None;
                continue;
            }

            match cancellation_token {
                Some(token) => {
                    tokio::select! {
                        _ = token.cancelled() => {
                            return Err(BitFunError::cancelled("AgentWait was cancelled".to_string()));
                        }
                        _ = &mut notified => {}
                        _ = sleep_until(wake_at) => {}
                    }
                }
                None => {
                    tokio::select! {
                        _ = &mut notified => {}
                        _ = sleep_until(wake_at) => {}
                    }
                }
            }
        }
    }

    async fn collect_available(&self, task_pks: &[i64]) -> BitFunResult<AvailableOutcomes> {
        let records = self
            .coordination_store
            .records_by_task_pks(task_pks)
            .await?;
        let mut available = AvailableOutcomes::default();
        for record in records {
            if record.delivered_at_ms.is_some() {
                continue;
            }
            if record.status.is_terminal() {
                available
                    .outcomes
                    .push(self.outcome_from_record(&record).await?);
            } else {
                available.pending_bg_task_ids.push(record.bg_task_id);
            }
        }
        Ok(available)
    }

    async fn claim_result(
        &self,
        parent_session_id: &str,
        delivered_parent_dialog_turn_id: &str,
        status: BackgroundSubagentWaitStatus,
        available: AvailableOutcomes,
    ) -> BitFunResult<Option<BackgroundSubagentWaitResult>> {
        let task_pks = available
            .outcomes
            .iter()
            .map(|outcome| outcome.task_pk)
            .collect::<Vec<_>>();
        let claimed = self
            .coordination_store
            .claim_terminal_tasks(
                parent_session_id,
                &task_pks,
                delivered_parent_dialog_turn_id,
            )
            .await?;
        let claimed_task_pks = claimed
            .into_iter()
            .map(|record| record.task_pk)
            .collect::<HashSet<_>>();
        let outcomes = available
            .outcomes
            .into_iter()
            .filter(|outcome| claimed_task_pks.contains(&outcome.task_pk))
            .collect::<Vec<_>>();
        Ok((!outcomes.is_empty())
            .then(|| wait_result(status, outcomes, available.pending_bg_task_ids)))
    }

    async fn outcome_from_record(
        &self,
        record: &BackgroundTaskRecord,
    ) -> BitFunResult<BackgroundSubagentOutcome> {
        if let Some(live) = self.live_results.get(&record.task_pk) {
            return Ok(BackgroundSubagentOutcome {
                task_pk: record.task_pk,
                bg_task_id: record.bg_task_id.clone(),
                agent_id: record.agent_id.clone(),
                status: live.status,
                content: live.content.clone(),
                error: live.error.clone(),
            });
        }

        let content = if matches!(
            record.status,
            BackgroundTaskStatus::Completed | BackgroundTaskStatus::PartialTimeout
        ) {
            self.load_persisted_result_text(record).await?
        } else {
            None
        };
        Ok(BackgroundSubagentOutcome {
            task_pk: record.task_pk,
            bg_task_id: record.bg_task_id.clone(),
            agent_id: record.agent_id.clone(),
            status: record.status,
            content,
            error: record.error_message.clone(),
        })
    }

    async fn load_persisted_result_text(
        &self,
        record: &BackgroundTaskRecord,
    ) -> BitFunResult<Option<String>> {
        let turn = self
            .session_manager
            .load_related_dialog_turn(
                &record.parent_session_id,
                &record.child_session_id,
                &record.child_dialog_turn_id,
            )
            .await?
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "Persisted subagent result is unavailable for {}",
                    record.bg_task_id
                ))
            })?;
        Ok(turn
            .model_rounds
            .iter()
            .rev()
            .flat_map(|round| round.text_items.iter().rev())
            .find(|item| !item.content.trim().is_empty())
            .map(|item| item.content.clone()))
    }

    async fn reconcile_stale_running_tasks(&self, parent_session_id: &str) -> BitFunResult<()> {
        let stale = self
            .coordination_store
            .stale_running_tasks(parent_session_id)
            .await?;
        for record in stale {
            let turn = self
                .session_manager
                .load_related_dialog_turn(
                    &record.parent_session_id,
                    &record.child_session_id,
                    &record.child_dialog_turn_id,
                )
                .await?;
            let (status, error_code, error_message) = match turn.map(|turn| turn.status) {
                Some(TurnStatus::Completed) => (BackgroundTaskStatus::Completed, None, None),
                Some(TurnStatus::Error) => (
                    BackgroundTaskStatus::Failed,
                    Some("child_turn_failed".to_string()),
                    Some("The persisted subagent turn failed".to_string()),
                ),
                Some(TurnStatus::Cancelled) => (
                    BackgroundTaskStatus::Cancelled,
                    Some("child_turn_cancelled".to_string()),
                    Some("The persisted subagent turn was cancelled".to_string()),
                ),
                Some(TurnStatus::InProgress) | None => (
                    BackgroundTaskStatus::Interrupted,
                    Some("execution_interrupted".to_string()),
                    Some("Background subagent execution was interrupted".to_string()),
                ),
            };
            let _ = self
                .coordination_store
                .update_task_status(record.task_pk, status, error_code, error_message)
                .await?;
        }
        if !parent_session_id.is_empty() {
            self.changes.notify_waiters();
        }
        Ok(())
    }

    pub(crate) async fn agent_id_for_session(
        &self,
        parent_session_id: &str,
        child_session_id: &str,
    ) -> BitFunResult<String> {
        self.coordination_store
            .agent_id_for_session(parent_session_id, child_session_id)
            .await
    }

    pub(crate) async fn resolve_agent_id(
        &self,
        parent_session_id: &str,
        agent_id: &str,
    ) -> BitFunResult<String> {
        self.coordination_store
            .resolve_agent_id(parent_session_id, agent_id)
            .await
    }

    pub(crate) async fn delete_session_references(&self, session_id: &str) -> BitFunResult<()> {
        let deleted_task_pks = self
            .coordination_store
            .delete_session_references(session_id)
            .await?;
        for task_pk in deleted_task_pks {
            self.live_results.remove(&task_pk);
        }
        self.changes.notify_waiters();
        Ok(())
    }

    pub(crate) async fn rollback_parent_turns(
        &self,
        parent_session_id: &str,
        parent_dialog_turn_ids: &[String],
    ) -> BitFunResult<()> {
        let deleted_task_pks = self
            .coordination_store
            .rollback_parent_turns(parent_session_id, parent_dialog_turn_ids)
            .await?;
        for task_pk in deleted_task_pks {
            self.live_results.remove(&task_pk);
        }
        self.changes.notify_waiters();
        Ok(())
    }

    pub(crate) async fn initialize_fork(
        &self,
        source_parent_session_id: &str,
        target_parent_session_id: &str,
    ) -> BitFunResult<()> {
        self.coordination_store
            .initialize_fork(source_parent_session_id, target_parent_session_id)
            .await
    }
}

fn wait_result(
    status: BackgroundSubagentWaitStatus,
    outcomes: Vec<BackgroundSubagentOutcome>,
    pending_bg_task_ids: Vec<String>,
) -> BackgroundSubagentWaitResult {
    BackgroundSubagentWaitResult {
        status,
        outcomes,
        pending_bg_task_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{SessionConfig, TurnStats};
    use crate::agentic::persistence::PersistenceManager;
    use crate::agentic::session::{PromptCachePolicy, SessionContextStore, SessionManagerConfig};
    use crate::infrastructure::PathManager;

    #[tokio::test]
    async fn restart_recovers_completed_result_from_child_turn_history() {
        let root = tempfile::tempdir().expect("background outcome temp directory");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let path_manager = Arc::new(PathManager::with_user_root_for_tests(
            root.path().join("config"),
        ));
        let session_manager = Arc::new(SessionManager::new(
            Arc::new(SessionContextStore::new()),
            Arc::new(PersistenceManager::new(path_manager.clone()).expect("persistence manager")),
            SessionManagerConfig {
                max_active_sessions: 10,
                session_idle_timeout: Duration::from_secs(3600),
                auto_save_interval: Duration::from_secs(300),
                enable_persistence: true,
                prompt_cache_policy: PromptCachePolicy::default(),
            },
        ));
        let config = SessionConfig {
            workspace_path: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        };
        session_manager
            .create_session_with_id(
                Some("parent-session".to_string()),
                "Parent".to_string(),
                "agentic".to_string(),
                config.clone(),
            )
            .await
            .expect("create parent session");
        session_manager
            .create_session_with_id(
                Some("child-session".to_string()),
                "Child".to_string(),
                "GeneralPurpose".to_string(),
                config,
            )
            .await
            .expect("create child session");
        session_manager
            .start_dialog_turn(
                "child-session",
                "GeneralPurpose".to_string(),
                "Inspect implementation".to_string(),
                Some("child-turn".to_string()),
                None,
                None,
            )
            .await
            .expect("start child turn");
        session_manager
            .complete_dialog_turn(
                "child-session",
                "child-turn",
                "persisted child result".to_string(),
                &[],
                TurnStats::default(),
            )
            .await
            .expect("complete child turn");

        let db_path = path_manager.agent_coordination_database_file();
        let previous_owner = CoordinationStore::new(db_path.clone());
        let registered = previous_owner
            .register_background_task(BackgroundTaskRegistration {
                parent_session_id: "parent-session".to_string(),
                requested_agent_id: None,
                child_session_id: "child-session".to_string(),
                parent_dialog_turn_id: "parent-turn".to_string(),
                parent_tool_call_id: "task-tool".to_string(),
                child_dialog_turn_id: "child-turn".to_string(),
            })
            .await
            .expect("register task for previous owner");
        let recovered = BackgroundSubagentOutcomeStore::new(
            session_manager,
            Arc::new(CoordinationStore::new(db_path)),
        )
        .wait_for(
            "parent-session",
            &[],
            BackgroundSubagentWaitMode::All,
            Duration::from_millis(50),
            "wait-turn",
            None,
        )
        .await
        .expect("recover persisted background result");

        assert_eq!(recovered.status, BackgroundSubagentWaitStatus::Completed);
        assert_eq!(recovered.outcomes.len(), 1);
        assert_eq!(recovered.outcomes[0].bg_task_id, registered.bg_task_id);
        assert_eq!(
            recovered.outcomes[0].content.as_deref(),
            Some("persisted child result")
        );
    }
}
