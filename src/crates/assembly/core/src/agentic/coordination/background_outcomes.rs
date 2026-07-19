use super::coordinator::{SubagentResult, SubagentResultStatus};
use crate::agentic::session::SessionManager;
use crate::util::errors::{BitFunError, BitFunResult};
use dashmap::DashMap;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio::time::{sleep_until, Duration, Instant};
use tokio_util::sync::CancellationToken;

const OUTCOME_METADATA_KEY_PREFIX: &str = "backgroundSubagentOutcome:";
const RESULT_DEBOUNCE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackgroundSubagentOutcomeStatus {
    Running,
    Completed,
    PartialTimeout,
    Failed,
    Cancelled,
}

impl BackgroundSubagentOutcomeStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::PartialTimeout => "partial_timeout",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackgroundSubagentOutcome {
    pub background_task_id: String,
    pub parent_session_id: String,
    pub parent_dialog_turn_id: String,
    pub subagent_session_id: String,
    pub subagent_dialog_turn_id: String,
    pub subagent_type: String,
    pub task_description: String,
    pub status: BackgroundSubagentOutcomeStatus,
    pub content: Option<String>,
    pub error: Option<String>,
    pub created_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub consumed_at_ms: Option<u64>,
}

impl BackgroundSubagentOutcome {
    pub(crate) fn running(
        background_task_id: String,
        parent_session_id: String,
        parent_dialog_turn_id: String,
        subagent_session_id: String,
        subagent_dialog_turn_id: String,
        subagent_type: String,
        task_description: String,
    ) -> Self {
        Self {
            background_task_id,
            parent_session_id,
            parent_dialog_turn_id,
            subagent_session_id,
            subagent_dialog_turn_id,
            subagent_type,
            task_description,
            status: BackgroundSubagentOutcomeStatus::Running,
            content: None,
            error: None,
            created_at_ms: unix_time_ms(),
            completed_at_ms: None,
            consumed_at_ms: None,
        }
    }

    fn complete_from_subagent_result(&mut self, result: &SubagentResult) {
        self.status = match result.status {
            SubagentResultStatus::Completed => BackgroundSubagentOutcomeStatus::Completed,
            SubagentResultStatus::PartialTimeout => BackgroundSubagentOutcomeStatus::PartialTimeout,
        };
        self.content = Some(result.text.clone());
        self.error = result.reason.clone();
        self.completed_at_ms = Some(unix_time_ms());
    }

    fn fail(&mut self, error: &BitFunError) {
        self.status = BackgroundSubagentOutcomeStatus::Failed;
        self.content = None;
        self.error = Some(error.to_string());
        self.completed_at_ms = Some(unix_time_ms());
    }

    fn cancel(&mut self) {
        self.status = BackgroundSubagentOutcomeStatus::Cancelled;
        self.content = None;
        self.error = Some("Background subagent task was cancelled".to_string());
        self.completed_at_ms = Some(unix_time_ms());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundSubagentWaitStatus {
    Completed,
    TimedOut,
    Cancelled,
    NoMatchingTasks,
}

impl BackgroundSubagentWaitStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::NoMatchingTasks => "no_matching_tasks",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundSubagentWaitResult {
    pub status: BackgroundSubagentWaitStatus,
    pub outcomes: Vec<BackgroundSubagentOutcome>,
    pub pending_background_task_ids: Vec<String>,
}

#[derive(Debug, Default)]
struct OutcomeClaim {
    had_matching_tasks: bool,
    outcomes: Vec<BackgroundSubagentOutcome>,
    pending_background_task_ids: Vec<String>,
}

pub(crate) struct BackgroundSubagentOutcomeStore {
    outcomes: DashMap<String, BackgroundSubagentOutcome>,
    changes: Notify,
    session_manager: Arc<SessionManager>,
}

impl BackgroundSubagentOutcomeStore {
    pub(crate) fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            outcomes: DashMap::new(),
            changes: Notify::new(),
            session_manager,
        }
    }

    pub(crate) async fn register(&self, outcome: BackgroundSubagentOutcome) -> BitFunResult<()> {
        self.outcomes
            .insert(outcome.background_task_id.clone(), outcome.clone());
        if let Err(error) = self.persist(&outcome).await {
            self.outcomes.remove(&outcome.background_task_id);
            return Err(error);
        }
        self.changes.notify_waiters();
        Ok(())
    }

    pub(crate) async fn complete(
        &self,
        background_task_id: &str,
        result: Result<&SubagentResult, &BitFunError>,
    ) {
        let outcome = {
            let Some(mut entry) = self.outcomes.get_mut(background_task_id) else {
                warn!(
                    "Background subagent outcome record is missing at completion: background_task_id={}",
                    background_task_id
                );
                return;
            };
            if entry.status == BackgroundSubagentOutcomeStatus::Cancelled {
                return;
            }
            match result {
                Ok(result) => entry.complete_from_subagent_result(result),
                Err(error) => entry.fail(error),
            }
            entry.clone()
        };
        self.persist_best_effort(&outcome).await;
        self.changes.notify_waiters();
    }

    pub(crate) async fn cancel(&self, background_task_ids: &[String]) {
        for background_task_id in background_task_ids {
            let outcome = {
                let Some(mut entry) = self.outcomes.get_mut(background_task_id) else {
                    continue;
                };
                if entry.status.is_terminal() {
                    continue;
                }
                entry.cancel();
                entry.clone()
            };
            self.persist_best_effort(&outcome).await;
        }
        self.changes.notify_waiters();
    }

    pub(crate) async fn wait_for(
        &self,
        parent_session_id: &str,
        parent_dialog_turn_id: &str,
        requested_task_ids: &[String],
        timeout: Duration,
        cancellation_token: Option<&CancellationToken>,
    ) -> BitFunResult<BackgroundSubagentWaitResult> {
        self.hydrate_parent_session(parent_session_id).await?;

        let deadline = Instant::now() + timeout;
        let mut collected = Vec::new();
        let mut debounce_deadline = None;

        loop {
            let notified = self.changes.notified();
            tokio::pin!(notified);

            let claim = self
                .claim_available(parent_session_id, parent_dialog_turn_id, requested_task_ids)
                .await?;
            if !claim.had_matching_tasks && collected.is_empty() {
                return Ok(BackgroundSubagentWaitResult {
                    status: BackgroundSubagentWaitStatus::NoMatchingTasks,
                    outcomes: Vec::new(),
                    pending_background_task_ids: Vec::new(),
                });
            }

            collected.extend(claim.outcomes);
            if !collected.is_empty() && claim.pending_background_task_ids.is_empty() {
                return Ok(wait_result_for_outcomes(collected, Vec::new()));
            }
            if !collected.is_empty() && debounce_deadline.is_none() {
                debounce_deadline = Some((Instant::now() + RESULT_DEBOUNCE).min(deadline));
            }

            let wake_at = debounce_deadline.unwrap_or(deadline);
            if Instant::now() >= wake_at {
                if collected.is_empty() {
                    return Ok(BackgroundSubagentWaitResult {
                        status: BackgroundSubagentWaitStatus::TimedOut,
                        outcomes: Vec::new(),
                        pending_background_task_ids: claim.pending_background_task_ids,
                    });
                }
                return Ok(wait_result_for_outcomes(
                    collected,
                    claim.pending_background_task_ids,
                ));
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

    async fn claim_available(
        &self,
        parent_session_id: &str,
        parent_dialog_turn_id: &str,
        requested_task_ids: &[String],
    ) -> BitFunResult<OutcomeClaim> {
        let task_ids = if requested_task_ids.is_empty() {
            self.outcomes
                .iter()
                .filter(|entry| {
                    entry.parent_session_id == parent_session_id
                        && entry.parent_dialog_turn_id == parent_dialog_turn_id
                        && entry.consumed_at_ms.is_none()
                })
                .map(|entry| entry.background_task_id.clone())
                .collect::<Vec<_>>()
        } else {
            requested_task_ids.to_vec()
        };

        if task_ids.is_empty() {
            return Ok(OutcomeClaim::default());
        }

        let mut claim = OutcomeClaim::default();
        let mut consumed = Vec::new();
        for background_task_id in task_ids {
            let Some(mut entry) = self.outcomes.get_mut(&background_task_id) else {
                if requested_task_ids.is_empty() {
                    continue;
                }
                return Err(BitFunError::tool(format!(
                    "Background task was not found: {}",
                    background_task_id
                )));
            };
            if entry.parent_session_id != parent_session_id {
                return Err(BitFunError::tool(format!(
                    "Background task does not belong to the current session: {}",
                    background_task_id
                )));
            }
            if entry.consumed_at_ms.is_some() {
                continue;
            }
            claim.had_matching_tasks = true;
            if entry.status.is_terminal() {
                entry.consumed_at_ms = Some(unix_time_ms());
                let outcome = entry.clone();
                claim.outcomes.push(outcome.clone());
                consumed.push(outcome);
            } else {
                claim
                    .pending_background_task_ids
                    .push(entry.background_task_id.clone());
            }
        }

        for outcome in consumed {
            self.persist_best_effort(&outcome).await;
        }
        Ok(claim)
    }

    async fn hydrate_parent_session(&self, parent_session_id: &str) -> BitFunResult<()> {
        let Some(custom_metadata) = self
            .session_manager
            .load_session_custom_metadata(parent_session_id)
            .await?
        else {
            return Ok(());
        };
        let Some(metadata) = custom_metadata.as_object() else {
            return Ok(());
        };
        for (key, value) in metadata {
            if !key.starts_with(OUTCOME_METADATA_KEY_PREFIX) {
                continue;
            }
            let Ok(outcome) = serde_json::from_value::<BackgroundSubagentOutcome>(value.clone())
            else {
                warn!(
                    "Ignoring invalid persisted background subagent outcome: session_id={}, metadata_key={}",
                    parent_session_id, key
                );
                continue;
            };
            if outcome.parent_session_id == parent_session_id {
                self.outcomes
                    .entry(outcome.background_task_id.clone())
                    .or_insert(outcome);
            }
        }
        Ok(())
    }

    async fn persist(&self, outcome: &BackgroundSubagentOutcome) -> BitFunResult<()> {
        let value = serde_json::to_value(outcome).map_err(|error| {
            BitFunError::tool(format!(
                "Failed to serialize background task outcome: {}",
                error
            ))
        })?;
        self.session_manager
            .merge_session_custom_metadata(
                &outcome.parent_session_id,
                json!({ outcome_metadata_key(&outcome.background_task_id): value }),
            )
            .await
    }

    async fn persist_best_effort(&self, outcome: &BackgroundSubagentOutcome) {
        if let Err(error) = self.persist(outcome).await {
            warn!(
                "Failed to persist background subagent outcome: background_task_id={}, parent_session_id={}, error={}",
                outcome.background_task_id, outcome.parent_session_id, error
            );
        }
    }
}

fn wait_result_for_outcomes(
    outcomes: Vec<BackgroundSubagentOutcome>,
    pending_background_task_ids: Vec<String>,
) -> BackgroundSubagentWaitResult {
    let status = if outcomes
        .iter()
        .all(|outcome| outcome.status == BackgroundSubagentOutcomeStatus::Cancelled)
    {
        BackgroundSubagentWaitStatus::Cancelled
    } else {
        BackgroundSubagentWaitStatus::Completed
    };
    BackgroundSubagentWaitResult {
        status,
        outcomes,
        pending_background_task_ids,
    }
}

fn outcome_metadata_key(background_task_id: &str) -> String {
    format!("{}{}", OUTCOME_METADATA_KEY_PREFIX, background_task_id)
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
