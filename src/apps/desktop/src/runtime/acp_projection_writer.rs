//! Durable ACP transcript writer.
//!
//! Consumes already-ordered `ExternalAcp` envelopes and writes settled turns
//! through the existing externally-projected persistence path. It does not load
//! ACP sessions into SessionManager.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bitfun_core::agentic::events::{EventQueue, EventSubscriber};
use bitfun_core::service::remote_connect::remote_server::get_global_dispatcher;
use bitfun_core::service::session::{
    DialogTurnData, DialogTurnRecoveryData, DialogTurnRecoveryStatus, ModelRoundData, TextItemData,
    ThinkingItemData, ToolCallData, ToolItemData, ToolResultData, TurnStatus, UserMessageData,
};
use bitfun_core_types::ReasoningContentKind;
use bitfun_events::{
    AgenticEvent, AgenticEventEnvelope, AgenticEventOrigin, AgenticEventPriority, ToolEventData,
    ToolEventIdentity,
};
use log::{error, warn};
use tokio::sync::Mutex as AsyncMutex;

use super::session_application::{DesktopSessionApplication, DesktopSessionScopeRequest};

/// Mid-turn streaming checkpoints are "fresh enough", not every token.
#[cfg(not(test))]
const STREAMING_CHECKPOINT_MIN_INTERVAL: Duration = Duration::from_secs(2);
const STREAMING_CHECKPOINT_MIN_BYTES: usize = 4 * 1024;

#[cfg(test)]
fn streaming_checkpoint_min_interval() -> Duration {
    Duration::from_millis(20)
}

#[cfg(not(test))]
fn streaming_checkpoint_min_interval() -> Duration {
    STREAMING_CHECKPOINT_MIN_INTERVAL
}

#[async_trait]
pub(crate) trait AcpTurnPersister: Send + Sync + 'static {
    async fn next_turn_index(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        session_id: &str,
        turn_id: &str,
    ) -> Result<usize, String>;

    async fn persist_turn(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        turn: DialogTurnData,
    ) -> Result<(), String>;

    async fn load_turns(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        session_id: &str,
    ) -> Result<Vec<DialogTurnData>, String>;

    fn mark_history_unreadable(&self, session_id: &str);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AcpSessionScope {
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AcpSessionScopeRegistrationError {
    Conflict { session_id: String },
    Recovery(String),
}

impl std::fmt::Display for AcpSessionScopeRegistrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict { session_id } => write!(
                formatter,
                "ACP projection scope conflicts with the registered session scope: {session_id}"
            ),
            Self::Recovery(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for AcpSessionScopeRegistrationError {}

#[derive(Clone)]
struct AcpTurnDraft {
    session_id: String,
    turn_id: String,
    /// Resolved once on `DialogTurnStarted` (reuse by turn_id for §10.6).
    turn_index: usize,
    user_input: String,
    original_user_input: Option<String>,
    user_message_metadata: Option<serde_json::Value>,
    rounds: Vec<ModelRoundData>,
    last_checkpoint_at: Option<Instant>,
    bytes_since_checkpoint: usize,
}

pub(crate) struct AcpDurableProjectionWriter<P> {
    queue: Arc<EventQueue>,
    persister: Arc<P>,
    scope_registration: AsyncMutex<()>,
    scopes: Mutex<HashMap<String, AcpSessionScope>>,
    drafts: Mutex<HashMap<(String, String), AcpTurnDraft>>,
}

impl<P: AcpTurnPersister> AcpDurableProjectionWriter<P> {
    pub(crate) fn new(queue: Arc<EventQueue>, persister: P) -> Arc<Self> {
        Arc::new(Self {
            queue,
            persister: Arc::new(persister),
            scope_registration: AsyncMutex::new(()),
            scopes: Mutex::new(HashMap::new()),
            drafts: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn ensure_session_scope(
        &self,
        session_id: &str,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
    ) -> Result<(), AcpSessionScopeRegistrationError> {
        let requested = AcpSessionScope {
            workspace_path: workspace_path.to_string(),
            remote_connection_id: remote_connection_id.map(ToOwned::to_owned),
            remote_ssh_host: remote_ssh_host.map(ToOwned::to_owned),
        };
        let _registration = self.scope_registration.lock().await;
        {
            let mut scopes = self.scopes.lock().expect("ACP projection scopes");
            if let Some(existing) = scopes.get(session_id) {
                if existing == &requested {
                    return Ok(());
                }
                return Err(AcpSessionScopeRegistrationError::Conflict {
                    session_id: session_id.to_string(),
                });
            }
            scopes.insert(session_id.to_string(), requested.clone());
        }

        if let Err(error) = self.recover_abandoned_turns(session_id).await {
            let mut scopes = self.scopes.lock().expect("ACP projection scopes");
            if scopes.get(session_id) == Some(&requested) {
                scopes.remove(session_id);
            }
            return Err(AcpSessionScopeRegistrationError::Recovery(error));
        }
        Ok(())
    }

    async fn handle_envelope(&self, envelope: &AgenticEventEnvelope) -> Result<(), String> {
        if envelope.origin != AgenticEventOrigin::ExternalAcp {
            return Ok(());
        }
        match &envelope.event {
            AgenticEvent::SessionCreated {
                session_id,
                workspace_path,
                remote_connection_id,
                remote_ssh_host,
                ..
            } => {
                let Some(workspace_path) = workspace_path.clone() else {
                    return Err(format!(
                        "ACP SessionCreated is missing workspace_path: {session_id}"
                    ));
                };
                self.ensure_session_scope(
                    session_id,
                    &workspace_path,
                    remote_connection_id.as_deref(),
                    remote_ssh_host.as_deref(),
                )
                .await
                .map_err(|error| error.to_string())
            }
            AgenticEvent::DialogTurnStarted {
                session_id,
                turn_id,
                user_input,
                original_user_input,
                user_message_metadata,
                ..
            } => {
                let scope = self.scope_for(session_id)?;
                let turn_index = self
                    .persister
                    .next_turn_index(
                        &scope.workspace_path,
                        scope.remote_connection_id.as_deref(),
                        scope.remote_ssh_host.as_deref(),
                        session_id,
                        turn_id,
                    )
                    .await?;
                self.drafts.lock().expect("ACP projection drafts").insert(
                    (session_id.clone(), turn_id.clone()),
                    AcpTurnDraft {
                        session_id: session_id.clone(),
                        turn_id: turn_id.clone(),
                        turn_index,
                        user_input: user_input.clone(),
                        original_user_input: original_user_input.clone(),
                        user_message_metadata: user_message_metadata.clone(),
                        rounds: Vec::new(),
                        last_checkpoint_at: None,
                        bytes_since_checkpoint: 0,
                    },
                );
                self.checkpoint_draft(session_id, turn_id).await
            }
            AgenticEvent::ModelRoundStarted {
                session_id,
                turn_id,
                round_id,
                round_index,
                ..
            } => {
                self.mutate_draft(session_id, turn_id, |draft| {
                    let round = current_round(draft, round_id);
                    round.round_index = *round_index;
                })?;
                self.checkpoint_draft(session_id, turn_id).await
            }
            AgenticEvent::TextChunk {
                session_id,
                turn_id,
                round_id,
                text,
                ..
            } => {
                self.mutate_draft(session_id, turn_id, |draft| {
                    append_text(draft, round_id, text);
                })?;
                self.maybe_checkpoint_streaming(session_id, turn_id).await
            }
            AgenticEvent::ThinkingChunk {
                session_id,
                turn_id,
                round_id,
                content,
                reasoning_kind,
                ..
            } => {
                self.mutate_draft(session_id, turn_id, |draft| {
                    append_thinking(draft, round_id, content, *reasoning_kind);
                })?;
                self.maybe_checkpoint_streaming(session_id, turn_id).await
            }
            AgenticEvent::ToolEvent {
                session_id,
                turn_id,
                round_id,
                tool_event,
                ..
            } => {
                self.mutate_draft(session_id, turn_id, |draft| {
                    apply_tool(draft, round_id, tool_event);
                })?;
                self.checkpoint_draft(session_id, turn_id).await
            }
            AgenticEvent::DialogTurnCompleted {
                session_id,
                turn_id,
                duration_ms,
                finish_reason,
                has_final_response,
                success,
                ..
            } => {
                self.settle_turn(
                    session_id,
                    turn_id,
                    TurnStatus::Completed,
                    None,
                    *duration_ms,
                    finish_reason.clone(),
                    *has_final_response,
                    *success,
                )
                .await
            }
            AgenticEvent::DialogTurnFailed {
                session_id,
                turn_id,
                error,
                ..
            } => {
                self.settle_turn(
                    session_id,
                    turn_id,
                    TurnStatus::Error,
                    Some(error.clone()),
                    0,
                    None,
                    None,
                    None,
                )
                .await
            }
            AgenticEvent::DialogTurnCancelled {
                session_id,
                turn_id,
            } => {
                self.settle_turn(
                    session_id,
                    turn_id,
                    TurnStatus::Cancelled,
                    None,
                    0,
                    None,
                    None,
                    None,
                )
                .await
            }
            _ => Ok(()),
        }
    }

    fn mutate_draft(
        &self,
        session_id: &str,
        turn_id: &str,
        update: impl FnOnce(&mut AcpTurnDraft),
    ) -> Result<(), String> {
        let mut drafts = self.drafts.lock().expect("ACP projection drafts");
        let draft = drafts
            .get_mut(&(session_id.to_string(), turn_id.to_string()))
            .ok_or_else(|| {
                format!("ACP projection has no draft for session={session_id} turn={turn_id}")
            })?;
        update(draft);
        Ok(())
    }

    fn clone_draft(&self, session_id: &str, turn_id: &str) -> Result<AcpTurnDraft, String> {
        self.drafts
            .lock()
            .expect("ACP projection drafts")
            .get(&(session_id.to_string(), turn_id.to_string()))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "ACP projection has no draft to persist for session={session_id} turn={turn_id}"
                )
            })
    }

    fn remove_draft(&self, session_id: &str, turn_id: &str) {
        self.drafts
            .lock()
            .expect("ACP projection drafts")
            .remove(&(session_id.to_string(), turn_id.to_string()));
    }

    fn scope_for(&self, session_id: &str) -> Result<AcpSessionScope, String> {
        self.scopes
            .lock()
            .expect("ACP projection scopes")
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("ACP projection is missing workspace scope for {session_id}"))
    }

    async fn persist_snapshot(
        &self,
        session_id: &str,
        turn_id: &str,
        turn: DialogTurnData,
        emit_history_changed: bool,
        mark_unreadable_on_failure: bool,
    ) -> Result<(), String> {
        let scope = self.scope_for(session_id)?;
        if let Err(persist_error) = self
            .persister
            .persist_turn(
                &scope.workspace_path,
                scope.remote_connection_id.as_deref(),
                scope.remote_ssh_host.as_deref(),
                turn,
            )
            .await
        {
            if mark_unreadable_on_failure {
                self.persister.mark_history_unreadable(session_id);
            }
            return Err(persist_error);
        }
        if emit_history_changed {
            if let Err(error) = self
                .queue
                .enqueue_with_origin(
                    AgenticEvent::SessionHistoryChanged {
                        session_id: session_id.to_string(),
                        settled_turn_id: Some(turn_id.to_string()),
                    },
                    Some(AgenticEventPriority::Normal),
                    AgenticEventOrigin::ExternalAcp,
                )
                .await
            {
                if mark_unreadable_on_failure {
                    self.persister.mark_history_unreadable(session_id);
                }
                return Err(error.to_string());
            }
        }
        Ok(())
    }

    fn build_turn_from_draft(
        &self,
        session_id: &str,
        turn_id: &str,
        status: TurnStatus,
        error: Option<String>,
        duration_ms: Option<u64>,
        finish_reason: Option<String>,
        has_final_response: Option<bool>,
        success: Option<bool>,
        recovery: Option<DialogTurnRecoveryData>,
    ) -> Result<DialogTurnData, String> {
        let draft = self.clone_draft(session_id, turn_id)?;
        let mut turn = DialogTurnData::new(
            draft.turn_id.clone(),
            draft.turn_index,
            draft.session_id.clone(),
            UserMessageData {
                id: format!("{}-user", draft.turn_id),
                content: draft
                    .original_user_input
                    .clone()
                    .unwrap_or_else(|| draft.user_input.clone()),
                timestamp: now_ms(),
                metadata: draft.user_message_metadata.clone(),
            },
        );
        turn.model_rounds = draft.rounds;
        let in_progress = matches!(status, TurnStatus::InProgress);
        turn.status = status;
        turn.duration_ms = duration_ms;
        if !in_progress {
            turn.end_time = Some(now_ms());
        }
        turn.error = error;
        turn.finish_reason = finish_reason;
        turn.has_final_response = has_final_response.or(success);
        turn.recovery = recovery;
        Ok(turn)
    }

    fn should_checkpoint_streaming(&self, session_id: &str, turn_id: &str) -> Result<bool, String> {
        let drafts = self.drafts.lock().expect("ACP projection drafts");
        let draft = drafts
            .get(&(session_id.to_string(), turn_id.to_string()))
            .ok_or_else(|| {
                format!("ACP projection has no draft for session={session_id} turn={turn_id}")
            })?;
        Ok(should_checkpoint_streaming(draft))
    }

    async fn maybe_checkpoint_streaming(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Result<(), String> {
        if !self.should_checkpoint_streaming(session_id, turn_id)? {
            return Ok(());
        }
        self.checkpoint_draft(session_id, turn_id).await
    }

    async fn checkpoint_draft(&self, session_id: &str, turn_id: &str) -> Result<(), String> {
        let turn = self.build_turn_from_draft(
            session_id,
            turn_id,
            TurnStatus::InProgress,
            None,
            None,
            None,
            None,
            None,
            None,
        )?;
        match self
            .persist_snapshot(session_id, turn_id, turn, false, false)
            .await
        {
            Ok(()) => {
                self.mutate_draft(session_id, turn_id, mark_checkpointed)?;
                Ok(())
            }
            Err(error) => {
                // Mid-turn checkpoints are best-effort: keep the draft and do not
                // declare the whole session history unreadable for a transient IO fail.
                // Still advance throttle state so a sustained disk failure does not
                // retry a doomed write on every subsequent streaming chunk.
                warn!(
                    "ACP in-progress checkpoint failed: session_id={session_id}, turn_id={turn_id}, error={error}"
                );
                self.mutate_draft(session_id, turn_id, mark_checkpointed)?;
                Ok(())
            }
        }
    }

    async fn settle_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        status: TurnStatus,
        error: Option<String>,
        duration_ms: u64,
        finish_reason: Option<String>,
        has_final_response: Option<bool>,
        success: Option<bool>,
    ) -> Result<(), String> {
        let turn = self.build_turn_from_draft(
            session_id,
            turn_id,
            status,
            error,
            Some(duration_ms),
            finish_reason,
            has_final_response,
            success,
            None,
        )?;
        self.persist_snapshot(session_id, turn_id, turn, true, true)
            .await?;
        self.remove_draft(session_id, turn_id);
        Ok(())
    }

    async fn persist_interrupted_draft(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> Result<(), String> {
        let turn = self.build_turn_from_draft(
            session_id,
            turn_id,
            TurnStatus::Cancelled,
            None,
            None,
            Some("interrupted".to_string()),
            None,
            None,
            Some(interrupted_recovery()),
        )?;
        self.persist_snapshot(session_id, turn_id, turn, true, true)
            .await?;
        self.remove_draft(session_id, turn_id);
        Ok(())
    }

    async fn recover_abandoned_turns(&self, session_id: &str) -> Result<(), String> {
        let scope = self.scope_for(session_id)?;
        let turns = self
            .persister
            .load_turns(
                &scope.workspace_path,
                scope.remote_connection_id.as_deref(),
                scope.remote_ssh_host.as_deref(),
                session_id,
            )
            .await?;
        let live_turn_ids: Vec<String> = self
            .drafts
            .lock()
            .expect("ACP projection drafts")
            .keys()
            .filter(|(draft_session, _)| draft_session == session_id)
            .map(|(_, turn_id)| turn_id.clone())
            .collect();
        for mut turn in turns {
            if turn.status != TurnStatus::InProgress {
                continue;
            }
            if live_turn_ids.iter().any(|turn_id| turn_id == &turn.turn_id) {
                continue;
            }
            apply_interrupted(&mut turn);
            let turn_id = turn.turn_id.clone();
            self.persist_snapshot(session_id, &turn_id, turn, true, true)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn flush_interrupted(&self) -> Result<(), String> {
        let keys: Vec<(String, String)> = self
            .drafts
            .lock()
            .expect("ACP projection drafts")
            .keys()
            .cloned()
            .collect();
        let mut first_error = None;
        for (session_id, turn_id) in keys {
            if let Err(error) = self.persist_interrupted_draft(&session_id, &turn_id).await {
                error!(
                    "ACP interrupted flush failed: session_id={session_id}, turn_id={turn_id}, error={error}"
                );
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    fn has_draft(&self, session_id: &str, turn_id: &str) -> bool {
        self.drafts
            .lock()
            .expect("ACP projection drafts")
            .contains_key(&(session_id.to_string(), turn_id.to_string()))
    }
}

#[async_trait]
impl<P: AcpTurnPersister> EventSubscriber for AcpDurableProjectionWriter<P> {
    async fn on_event(
        &self,
        _event: &AgenticEvent,
    ) -> bitfun_agent_runtime::event_bus::EventSubscriberResult {
        Ok(())
    }

    async fn on_envelope(
        &self,
        envelope: &AgenticEventEnvelope,
    ) -> bitfun_agent_runtime::event_bus::EventSubscriberResult {
        if let Err(error) = self.handle_envelope(envelope).await {
            error!("ACP durable projection failed: {error}");
            return Err(bitfun_agent_runtime::event_bus::EventBusError::subscriber(
                error,
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl AcpTurnPersister for DesktopSessionApplication {
    async fn next_turn_index(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        session_id: &str,
        turn_id: &str,
    ) -> Result<usize, String> {
        let request = DesktopSessionScopeRequest {
            workspace_path: workspace_path.to_string(),
            remote_connection_id: remote_connection_id.map(ToString::to_string),
            remote_ssh_host: remote_ssh_host.map(ToString::to_string),
        };
        let turns = self
            .load_session_turns(request, session_id, None)
            .await
            .map_err(|error| error.to_string())?;
        if let Some(existing) = turns.iter().find(|turn| turn.turn_id == turn_id) {
            return Ok(existing.turn_index);
        }
        Ok(turns.len())
    }

    async fn persist_turn(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        turn: DialogTurnData,
    ) -> Result<(), String> {
        let request = DesktopSessionScopeRequest {
            workspace_path: workspace_path.to_string(),
            remote_connection_id: remote_connection_id.map(ToString::to_string),
            remote_ssh_host: remote_ssh_host.map(ToString::to_string),
        };
        self.save_session_turn(request, &turn)
            .await
            .map_err(|error| error.to_string())
    }

    async fn load_turns(
        &self,
        workspace_path: &str,
        remote_connection_id: Option<&str>,
        remote_ssh_host: Option<&str>,
        session_id: &str,
    ) -> Result<Vec<DialogTurnData>, String> {
        let request = DesktopSessionScopeRequest {
            workspace_path: workspace_path.to_string(),
            remote_connection_id: remote_connection_id.map(ToString::to_string),
            remote_ssh_host: remote_ssh_host.map(ToString::to_string),
        };
        self.load_session_turns(request, session_id, None)
            .await
            .map_err(|error| error.to_string())
    }

    fn mark_history_unreadable(&self, session_id: &str) {
        if let Some(tracker) =
            get_global_dispatcher().and_then(|dispatcher| dispatcher.get_tracker(session_id))
        {
            tracker.require_history_snapshot();
            return;
        }
        warn!(
            "ACP durable projection failed with no remote tracker to mark snapshot-required: {session_id}"
        );
    }
}

fn empty_round(round_id: &str, turn_id: &str, round_index: usize) -> ModelRoundData {
    let now = now_ms();
    ModelRoundData {
        id: round_id.to_string(),
        turn_id: turn_id.to_string(),
        round_index,
        round_group_id: None,
        timestamp: now,
        text_items: Vec::new(),
        tool_items: Vec::new(),
        thinking_items: Vec::new(),
        start_time: now,
        end_time: None,
        duration_ms: None,
        provider_id: None,
        model_config_id: None,
        effective_model_name: None,
        first_chunk_ms: None,
        first_visible_output_ms: None,
        stream_duration_ms: None,
        attempt_count: None,
        attempt_diagnostics: Vec::new(),
        failure_category: None,
        token_details: None,
        status: "completed".to_string(),
    }
}

fn current_round<'a>(draft: &'a mut AcpTurnDraft, round_id: &str) -> &'a mut ModelRoundData {
    if let Some(index) = draft.rounds.iter().position(|round| round.id == round_id) {
        return &mut draft.rounds[index];
    }
    let index = draft.rounds.len();
    draft
        .rounds
        .push(empty_round(round_id, &draft.turn_id, index));
    draft.rounds.last_mut().expect("round just inserted")
}

fn append_text(draft: &mut AcpTurnDraft, round_id: &str, text: &str) {
    draft.bytes_since_checkpoint = draft.bytes_since_checkpoint.saturating_add(text.len());
    let round = current_round(draft, round_id);
    if let Some(last) = round.text_items.last_mut() {
        last.content.push_str(text);
        last.is_streaming = false;
        return;
    }
    round.text_items.push(TextItemData {
        id: format!("{}-text", round.id),
        content: text.to_string(),
        is_streaming: false,
        timestamp: now_ms(),
        is_markdown: true,
        order_index: None,
        is_subagent_item: None,
        parent_task_tool_id: None,
        subagent_session_id: None,
        status: None,
        attempt_id: None,
        attempt_index: None,
    });
}

fn append_thinking(
    draft: &mut AcpTurnDraft,
    round_id: &str,
    content: &str,
    reasoning_kind: Option<ReasoningContentKind>,
) {
    draft.bytes_since_checkpoint = draft.bytes_since_checkpoint.saturating_add(content.len());
    let round = current_round(draft, round_id);
    if let Some(last) = round.thinking_items.last_mut() {
        if last.reasoning_kind == reasoning_kind {
            last.content.push_str(content);
            last.is_streaming = false;
            return;
        }
    }
    let thinking_index = round.thinking_items.len();
    let thinking_id = if thinking_index == 0 {
        format!("{}-thinking", round.id)
    } else {
        format!("{}-thinking-{thinking_index}", round.id)
    };
    round.thinking_items.push(ThinkingItemData {
        id: thinking_id,
        content: content.to_string(),
        reasoning_kind,
        is_streaming: false,
        is_collapsed: true,
        timestamp: now_ms(),
        order_index: None,
        status: None,
        is_subagent_item: None,
        parent_task_tool_id: None,
        subagent_session_id: None,
        attempt_id: None,
        attempt_index: None,
    });
}

fn apply_tool(draft: &mut AcpTurnDraft, round_id: &str, tool_event: &ToolEventData) {
    let identity = tool_identity(tool_event);
    let round = current_round(draft, round_id);
    let existing = round
        .tool_items
        .iter_mut()
        .find(|item| item.id == identity.tool_id);
    match tool_event {
        ToolEventData::Started { params, .. }
        | ToolEventData::ConfirmationNeeded { params, .. } => {
            if let Some(item) = existing {
                item.tool_call.input = params.clone();
                item.status = Some("running".to_string());
                return;
            }
            round.tool_items.push(ToolItemData {
                id: identity.tool_id.clone(),
                tool_name: identity.tool_name.clone(),
                tool_call: ToolCallData {
                    input: params.clone(),
                    id: identity.tool_id.clone(),
                },
                tool_result: None,
                ai_intent: None,
                start_time: now_ms(),
                end_time: None,
                duration_ms: None,
                queue_wait_ms: None,
                preflight_ms: None,
                confirmation_wait_ms: None,
                execution_ms: None,
                order_index: None,
                is_subagent_item: None,
                parent_task_tool_id: None,
                subagent_session_id: None,
                subagent_dialog_turn_id: None,
                attempt_id: None,
                attempt_index: None,
                subagent_model_id: None,
                subagent_model_display_name: None,
                status: Some("running".to_string()),
                interruption_reason: None,
            });
        }
        ToolEventData::Completed {
            result,
            result_for_assistant,
            duration_ms,
            ..
        } => {
            if let Some(item) = existing {
                item.tool_result = Some(ToolResultData {
                    result: result.clone(),
                    success: true,
                    result_for_assistant: result_for_assistant.clone(),
                    image_attachments: None,
                    error: None,
                    duration_ms: Some(*duration_ms),
                });
                item.duration_ms = Some(*duration_ms);
                item.status = Some("completed".to_string());
            }
        }
        ToolEventData::Failed {
            error, duration_ms, ..
        } => {
            if let Some(item) = existing {
                item.tool_result = Some(ToolResultData {
                    result: serde_json::Value::Null,
                    success: false,
                    result_for_assistant: None,
                    image_attachments: None,
                    error: Some(error.clone()),
                    duration_ms: *duration_ms,
                });
                item.status = Some("failed".to_string());
            }
        }
        ToolEventData::Cancelled { reason, .. } => {
            if let Some(item) = existing {
                item.status = Some("cancelled".to_string());
                item.interruption_reason = Some(reason.clone());
            }
        }
        _ => {}
    }
}

fn tool_identity(tool_event: &ToolEventData) -> &ToolEventIdentity {
    match tool_event {
        ToolEventData::EarlyDetected { identity }
        | ToolEventData::ParamsPartial { identity, .. }
        | ToolEventData::Queued { identity, .. }
        | ToolEventData::Waiting { identity, .. }
        | ToolEventData::Started { identity, .. }
        | ToolEventData::Progress { identity, .. }
        | ToolEventData::Streaming { identity, .. }
        | ToolEventData::StreamChunk { identity, .. }
        | ToolEventData::ConfirmationNeeded { identity, .. }
        | ToolEventData::Confirmed { identity }
        | ToolEventData::Rejected { identity }
        | ToolEventData::Completed { identity, .. }
        | ToolEventData::Failed { identity, .. }
        | ToolEventData::Cancelled { identity, .. } => identity,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn should_checkpoint_streaming(draft: &AcpTurnDraft) -> bool {
    if draft.bytes_since_checkpoint >= STREAMING_CHECKPOINT_MIN_BYTES {
        return true;
    }
    match draft.last_checkpoint_at {
        None => true,
        Some(at) => at.elapsed() >= streaming_checkpoint_min_interval(),
    }
}

fn mark_checkpointed(draft: &mut AcpTurnDraft) {
    draft.last_checkpoint_at = Some(Instant::now());
    draft.bytes_since_checkpoint = 0;
}

fn interrupted_recovery() -> DialogTurnRecoveryData {
    DialogTurnRecoveryData {
        status: DialogTurnRecoveryStatus::Interrupted,
        execution_generation: 1,
        resume_count: 0,
        interrupted_at: Some(now_ms()),
        model_id: None,
    }
}

fn apply_interrupted(turn: &mut DialogTurnData) {
    turn.status = TurnStatus::Cancelled;
    turn.end_time = Some(now_ms());
    turn.finish_reason = Some("interrupted".to_string());
    turn.recovery = Some(interrupted_recovery());
}

static DESKTOP_ACP_WRITER: OnceLock<Arc<AcpDurableProjectionWriter<DesktopSessionApplication>>> =
    OnceLock::new();

pub(crate) fn install_desktop_acp_writer(
    writer: Arc<AcpDurableProjectionWriter<DesktopSessionApplication>>,
) -> Arc<AcpDurableProjectionWriter<DesktopSessionApplication>> {
    if DESKTOP_ACP_WRITER.set(writer.clone()).is_err() {
        warn!("ACP durable projection writer was installed twice");
    }
    writer
}

pub(crate) fn flush_desktop_acp_writer_blocking() {
    let Some(writer) = DESKTOP_ACP_WRITER.get() else {
        return;
    };
    let writer = Arc::clone(writer);
    let join = std::thread::Builder::new()
        .name("acp-projection-flush".to_string())
        .spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(writer.flush_interrupted()),
                Err(error) => Err(format!("ACP interrupted flush runtime failed: {error}")),
            }
        });
    match join {
        Ok(handle) => match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => error!("ACP interrupted flush failed: {error}"),
            Err(_) => error!("ACP interrupted flush thread panicked"),
        },
        Err(error) => error!("ACP interrupted flush thread failed to start: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{AcpDurableProjectionWriter, AcpSessionScopeRegistrationError, AcpTurnPersister};
    use bitfun_core::agentic::events::{EventQueue, EventSubscriber};
    use bitfun_core::service::session::{DialogTurnData, DialogTurnRecoveryStatus, TurnStatus};
    use bitfun_events::{
        AgenticEvent, AgenticEventEnvelope, AgenticEventOrigin, AgenticEventPriority,
        ModelRoundIdentity, ToolEventData, ToolEventIdentity,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::sync::Notify;

    #[derive(Default)]
    struct RecordingPersister {
        turns: Mutex<Vec<DialogTurnData>>,
        unreadable: Mutex<Vec<String>>,
        fail_next: Mutex<bool>,
        fail_next_load: Mutex<bool>,
        index_lookups: Mutex<usize>,
        persist_calls: Mutex<usize>,
        load_calls: Mutex<usize>,
        load_scopes: Mutex<Vec<(String, Option<String>, Option<String>, String)>>,
        block_next_load: AtomicBool,
        load_started: Notify,
        release_load: Notify,
    }

    #[async_trait::async_trait]
    impl AcpTurnPersister for Arc<RecordingPersister> {
        async fn next_turn_index(
            &self,
            _workspace_path: &str,
            _remote_connection_id: Option<&str>,
            _remote_ssh_host: Option<&str>,
            _session_id: &str,
            turn_id: &str,
        ) -> Result<usize, String> {
            *self.index_lookups.lock().expect("index lookups") += 1;
            let turns = self.turns.lock().expect("turns");
            Ok(turns
                .iter()
                .find(|turn| turn.turn_id == turn_id)
                .map(|turn| turn.turn_index)
                .unwrap_or(turns.len()))
        }

        async fn persist_turn(
            &self,
            _workspace_path: &str,
            _remote_connection_id: Option<&str>,
            _remote_ssh_host: Option<&str>,
            turn: DialogTurnData,
        ) -> Result<(), String> {
            *self.persist_calls.lock().expect("persist calls") += 1;
            if *self.fail_next.lock().expect("fail flag") {
                *self.fail_next.lock().expect("fail flag") = false;
                return Err("disk full".to_string());
            }
            let mut turns = self.turns.lock().expect("turns");
            if let Some(existing) = turns
                .iter_mut()
                .find(|existing| existing.turn_id == turn.turn_id)
            {
                *existing = turn;
            } else {
                turns.push(turn);
            }
            Ok(())
        }

        async fn load_turns(
            &self,
            workspace_path: &str,
            remote_connection_id: Option<&str>,
            remote_ssh_host: Option<&str>,
            session_id: &str,
        ) -> Result<Vec<DialogTurnData>, String> {
            *self.load_calls.lock().expect("load calls") += 1;
            self.load_scopes.lock().expect("load scopes").push((
                workspace_path.to_string(),
                remote_connection_id.map(ToOwned::to_owned),
                remote_ssh_host.map(ToOwned::to_owned),
                session_id.to_string(),
            ));
            let fail = {
                let mut fail_next_load = self.fail_next_load.lock().expect("load fail flag");
                std::mem::take(&mut *fail_next_load)
            };
            if fail {
                return Err("load failed".to_string());
            }
            if self.block_next_load.swap(false, Ordering::SeqCst) {
                self.load_started.notify_one();
                self.release_load.notified().await;
            }
            Ok(self.turns.lock().expect("turns").clone())
        }

        fn mark_history_unreadable(&self, session_id: &str) {
            self.unreadable
                .lock()
                .expect("unreadable")
                .push(session_id.to_string());
        }
    }

    fn envelope(origin: AgenticEventOrigin, event: AgenticEvent) -> AgenticEventEnvelope {
        AgenticEventEnvelope::new_with_origin(event, AgenticEventPriority::Normal, origin)
    }

    async fn drive_completed_turn(writer: &AcpDurableProjectionWriter<Arc<RecordingPersister>>) {
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("session");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    turn_index: 0,
                    user_input: "hello".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("start");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("round");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::TextChunk {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: "hi".to_string(),
                },
            ))
            .await
            .expect("text");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ToolEvent {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    tool_event: ToolEventData::Started {
                        identity: ToolEventIdentity::direct("tool-1", "read"),
                        params: serde_json::json!({"path": "a.rs"}),
                        timeout_seconds: None,
                    },
                },
            ))
            .await
            .expect("tool start");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ToolEvent {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    tool_event: ToolEventData::Completed {
                        identity: ToolEventIdentity::direct("tool-1", "read"),
                        result: serde_json::json!("ok"),
                        result_for_assistant: Some("ok".to_string()),
                        image_attachments: None,
                        duration_ms: 3,
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                },
            ))
            .await
            .expect("tool complete");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnCompleted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    total_rounds: 1,
                    total_tools: 1,
                    duration_ms: 10,
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: Some("complete".to_string()),
                    has_final_response: Some(true),
                },
            ))
            .await
            .expect("complete");
    }

    #[tokio::test]
    async fn persists_settled_acp_turn_and_emits_history_fence() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue.clone(), persister.clone());
        drive_completed_turn(&writer).await;

        let turns = persister.turns.lock().expect("turns");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].turn_id, "turn-1");
        assert_eq!(turns[0].status, TurnStatus::Completed);
        assert_eq!(turns[0].user_message.content, "hello");
        assert_eq!(turns[0].model_rounds[0].text_items[0].content, "hi");
        assert_eq!(turns[0].model_rounds[0].tool_items[0].id, "tool-1");
        assert!(turns[0].model_rounds[0].tool_items[0]
            .tool_result
            .as_ref()
            .is_some_and(|result| result.success));

        let batch = queue.dequeue_configured_batch().await;
        assert!(batch.iter().any(|envelope| matches!(
            &envelope.event,
            AgenticEvent::SessionHistoryChanged {
                session_id,
                settled_turn_id: Some(turn_id),
            } if session_id == "acp-1" && turn_id == "turn-1"
        )));
        assert_eq!(batch[0].origin, AgenticEventOrigin::ExternalAcp);
    }

    #[tokio::test]
    async fn repeated_round_start_updates_the_existing_round() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        open_session_and_turn(&writer, "turn-repeat").await;

        let round_started = || AgenticEvent::ModelRoundStarted {
            session_id: "acp-1".to_string(),
            turn_id: "turn-repeat".to_string(),
            round_id: "round-1".to_string(),
            round_group_id: None,
            round_index: 0,
            identity: ModelRoundIdentity::External {
                provider: "acp".to_string(),
                client_id: "gemini".to_string(),
                model_id: None,
                display_name: None,
            },
            render_hints: None,
        };
        writer
            .on_envelope(&envelope(AgenticEventOrigin::ExternalAcp, round_started()))
            .await
            .expect("first round start");
        writer
            .on_envelope(&envelope(AgenticEventOrigin::ExternalAcp, round_started()))
            .await
            .expect("repeated round start");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ToolEvent {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-repeat".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    tool_event: ToolEventData::Started {
                        identity: ToolEventIdentity::direct("tool-1", "Bash"),
                        params: serde_json::json!({"command": "sleep 30"}),
                        timeout_seconds: None,
                    },
                },
            ))
            .await
            .expect("tool start");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ToolEvent {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-repeat".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    tool_event: ToolEventData::Failed {
                        identity: ToolEventIdentity::direct("tool-1", "Bash"),
                        error: "tool call aborted".to_string(),
                        duration_ms: Some(10),
                        queue_wait_ms: None,
                        preflight_ms: None,
                        confirmation_wait_ms: None,
                        execution_ms: None,
                    },
                },
            ))
            .await
            .expect("tool failure");

        let turns = persister.turns.lock().expect("turns");
        assert_eq!(turns[0].model_rounds.len(), 1);
        assert_eq!(turns[0].model_rounds[0].tool_items.len(), 1);
        assert_eq!(
            turns[0].model_rounds[0].tool_items[0].status.as_deref(),
            Some("failed")
        );
    }

    #[tokio::test]
    async fn ignores_native_envelopes() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::NativeRuntime,
                AgenticEvent::DialogTurnCompleted {
                    session_id: "native-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    total_rounds: 1,
                    total_tools: 0,
                    duration_ms: 1,
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: None,
                    has_final_response: None,
                },
            ))
            .await
            .expect("native ignored");
        assert!(persister.turns.lock().expect("turns").is_empty());
    }

    #[tokio::test]
    async fn existing_session_scope_registration_recovers_once_before_turn_start() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());

        writer
            .ensure_session_scope(
                "acp-existing",
                "/remote/workspace",
                Some("connection-1"),
                Some("remote.example"),
            )
            .await
            .expect("register existing session scope");
        writer
            .ensure_session_scope(
                "acp-existing",
                "/remote/workspace",
                Some("connection-1"),
                Some("remote.example"),
            )
            .await
            .expect("same scope is idempotent");

        assert_eq!(*persister.load_calls.lock().expect("load calls"), 1);
        assert_eq!(
            persister
                .load_scopes
                .lock()
                .expect("load scopes")
                .as_slice(),
            [(
                "/remote/workspace".to_string(),
                Some("connection-1".to_string()),
                Some("remote.example".to_string()),
                "acp-existing".to_string(),
            )]
        );

        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-existing".to_string(),
                    turn_id: "turn-after-restart".to_string(),
                    turn_index: 0,
                    user_input: "continue".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("turn starts after explicit scope registration");
        assert!(writer.has_draft("acp-existing", "turn-after-restart"));
        assert_eq!(*persister.load_calls.lock().expect("load calls"), 1);
    }

    #[tokio::test]
    async fn conflicting_session_scope_fails_loud_without_recovery() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());

        writer
            .ensure_session_scope("acp-1", "/tmp/ws", None, None)
            .await
            .expect("initial scope");
        let error = writer
            .ensure_session_scope(
                "acp-1",
                "/remote/ws",
                Some("connection-1"),
                Some("remote.example"),
            )
            .await
            .expect_err("conflicting scope must fail");

        assert_eq!(
            error,
            AcpSessionScopeRegistrationError::Conflict {
                session_id: "acp-1".to_string(),
            }
        );
        assert_eq!(*persister.load_calls.lock().expect("load calls"), 1);
    }

    #[tokio::test]
    async fn failed_scope_recovery_rolls_back_and_can_retry() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        *persister.fail_next_load.lock().expect("load fail flag") = true;

        let error = writer
            .ensure_session_scope("acp-1", "/tmp/ws", None, None)
            .await
            .expect_err("first recovery fails");
        assert_eq!(
            error,
            AcpSessionScopeRegistrationError::Recovery("load failed".to_string())
        );

        writer
            .ensure_session_scope("acp-1", "/tmp/ws", None, None)
            .await
            .expect("recovery retry");
        assert_eq!(*persister.load_calls.lock().expect("load calls"), 2);
    }

    #[tokio::test]
    async fn concurrent_first_scope_registration_waits_for_recovery() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        persister.block_next_load.store(true, Ordering::SeqCst);
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());

        let first_writer = writer.clone();
        let first = tokio::spawn(async move {
            first_writer
                .ensure_session_scope("acp-1", "/tmp/ws", None, None)
                .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            persister.load_started.notified(),
        )
        .await
        .expect("first recovery started");

        let second_writer = writer.clone();
        let second = tokio::spawn(async move {
            second_writer
                .ensure_session_scope("acp-1", "/tmp/ws", None, None)
                .await
        });
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        assert!(!second.is_finished());

        persister.release_load.notify_one();
        first
            .await
            .expect("first task")
            .expect("first registration");
        second
            .await
            .expect("second task")
            .expect("second registration");
        assert_eq!(*persister.load_calls.lock().expect("load calls"), 1);
    }

    #[tokio::test]
    async fn persist_failure_marks_history_snapshot_required() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("session");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-fail".to_string(),
                    turn_index: 0,
                    user_input: "hello".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("start");
        *persister.fail_next.lock().expect("fail") = true;
        let error = writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnCompleted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-fail".to_string(),
                    total_rounds: 0,
                    total_tools: 0,
                    duration_ms: 1,
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: None,
                    has_final_response: None,
                },
            ))
            .await
            .expect_err("persist must fail loud");
        assert!(error.to_string().contains("disk full"));
        assert!(writer.has_draft("acp-1", "turn-fail"));
        assert_eq!(
            persister.unreadable.lock().expect("unreadable").as_slice(),
            ["acp-1"]
        );

        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnCompleted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-fail".to_string(),
                    total_rounds: 0,
                    total_tools: 0,
                    duration_ms: 1,
                    partial_recovery_reason: None,
                    success: Some(true),
                    finish_reason: None,
                    has_final_response: None,
                },
            ))
            .await
            .expect("retry after persist failure");
        assert!(!writer.has_draft("acp-1", "turn-fail"));
        assert_eq!(
            persister.turns.lock().expect("turns")[0].status,
            TurnStatus::Completed
        );
    }

    #[tokio::test]
    async fn restart_recovers_in_progress_turn_as_interrupted() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue.clone(), persister.clone());
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("session");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-open".to_string(),
                    turn_index: 0,
                    user_input: "hello".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("start");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-open".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("round");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::TextChunk {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-open".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: "partial".to_string(),
                },
            ))
            .await
            .expect("text");
        // Structural tool boundary forces a checkpoint so the partial text is on disk.
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ToolEvent {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-open".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    tool_event: ToolEventData::Started {
                        identity: ToolEventIdentity::direct("tool-1", "read"),
                        params: serde_json::json!({"path": "a.rs"}),
                        timeout_seconds: None,
                    },
                },
            ))
            .await
            .expect("tool");
        assert_eq!(
            persister.turns.lock().expect("turns")[0].status,
            TurnStatus::InProgress
        );
        drop(writer);

        let recovered = AcpDurableProjectionWriter::new(queue.clone(), persister.clone());
        recovered
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("recover");
        let turns = persister.turns.lock().expect("turns");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].status, TurnStatus::Cancelled);
        assert_eq!(turns[0].user_message.content, "hello");
        assert_eq!(turns[0].model_rounds[0].text_items[0].content, "partial");
        assert_eq!(
            turns[0].recovery.as_ref().map(|recovery| recovery.status),
            Some(DialogTurnRecoveryStatus::Interrupted)
        );
        drop(turns);

        let batch = queue.dequeue_configured_batch().await;
        assert!(batch.iter().any(|envelope| matches!(
            &envelope.event,
            AgenticEvent::SessionHistoryChanged {
                session_id,
                settled_turn_id: Some(turn_id),
            } if session_id == "acp-1" && turn_id == "turn-open"
        )));
    }

    #[tokio::test]
    async fn flush_interrupted_persists_open_drafts() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue.clone(), persister.clone());
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("session");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-open".to_string(),
                    turn_index: 0,
                    user_input: "hello".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("start");
        writer.flush_interrupted().await.expect("flush");
        assert!(!writer.has_draft("acp-1", "turn-open"));
        let turns = persister.turns.lock().expect("turns");
        assert_eq!(turns[0].status, TurnStatus::Cancelled);
        assert_eq!(
            turns[0].recovery.as_ref().map(|recovery| recovery.status),
            Some(DialogTurnRecoveryStatus::Interrupted)
        );
    }

    async fn open_session_and_turn(
        writer: &AcpDurableProjectionWriter<Arc<RecordingPersister>>,
        turn_id: &str,
    ) {
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::SessionCreated {
                    session_id: "acp-1".to_string(),
                    session_name: "ACP".to_string(),
                    agent_type: "acp:gemini".to_string(),
                    workspace_path: Some("/tmp/ws".to_string()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: None,
                    remote_ssh_host: None,
                },
            ))
            .await
            .expect("session");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::DialogTurnStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: turn_id.to_string(),
                    turn_index: 0,
                    user_input: "hello".to_string(),
                    original_user_input: None,
                    user_message_metadata: None,
                },
            ))
            .await
            .expect("start");
    }

    #[tokio::test]
    async fn turn_index_is_resolved_once_per_turn() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        drive_completed_turn(&writer).await;
        assert_eq!(*persister.index_lookups.lock().expect("lookups"), 1);
    }

    #[tokio::test]
    async fn streaming_text_chunks_do_not_checkpoint_every_token() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        open_session_and_turn(&writer, "turn-stream").await;
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-stream".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("round");
        let after_structural = *persister.persist_calls.lock().expect("persist");
        for _ in 0..20 {
            writer
                .on_envelope(&envelope(
                    AgenticEventOrigin::ExternalAcp,
                    AgenticEvent::TextChunk {
                        session_id: "acp-1".to_string(),
                        turn_id: "turn-stream".to_string(),
                        round_id: "round-1".to_string(),
                        attempt_id: None,
                        attempt_index: None,
                        text: "x".to_string(),
                    },
                ))
                .await
                .expect("text");
        }
        assert_eq!(
            *persister.persist_calls.lock().expect("persist"),
            after_structural,
            "small streaming chunks must not write every token"
        );
        assert_eq!(*persister.index_lookups.lock().expect("lookups"), 1);
    }

    #[tokio::test]
    async fn streaming_byte_threshold_forces_checkpoint() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        open_session_and_turn(&writer, "turn-bytes").await;
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-bytes".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("round");
        let after_structural = *persister.persist_calls.lock().expect("persist");
        let bulky = "a".repeat(super::STREAMING_CHECKPOINT_MIN_BYTES);
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::TextChunk {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-bytes".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: bulky,
                },
            ))
            .await
            .expect("bulky text");
        assert_eq!(
            *persister.persist_calls.lock().expect("persist"),
            after_structural + 1
        );
        assert!(
            persister.turns.lock().expect("turns")[0]
                .model_rounds
                .last()
                .unwrap()
                .text_items
                .last()
                .unwrap()
                .content
                .len()
                >= super::STREAMING_CHECKPOINT_MIN_BYTES
        );
    }

    #[tokio::test]
    async fn streaming_time_threshold_forces_checkpoint() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        open_session_and_turn(&writer, "turn-time").await;
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-time".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("round");
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::TextChunk {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-time".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: "a".to_string(),
                },
            ))
            .await
            .expect("early text");
        let after_early = *persister.persist_calls.lock().expect("persist");
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::TextChunk {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-time".to_string(),
                    round_id: "round-1".to_string(),
                    attempt_id: None,
                    attempt_index: None,
                    text: "b".to_string(),
                },
            ))
            .await
            .expect("late text");
        assert_eq!(
            *persister.persist_calls.lock().expect("persist"),
            after_early + 1
        );
    }

    #[tokio::test]
    async fn mid_turn_checkpoint_failure_does_not_mark_history_unreadable() {
        let queue = Arc::new(EventQueue::new(Default::default()));
        let persister = Arc::new(RecordingPersister::default());
        let writer = AcpDurableProjectionWriter::new(queue, persister.clone());
        open_session_and_turn(&writer, "turn-ckpt-fail").await;
        *persister.fail_next.lock().expect("fail") = true;
        writer
            .on_envelope(&envelope(
                AgenticEventOrigin::ExternalAcp,
                AgenticEvent::ModelRoundStarted {
                    session_id: "acp-1".to_string(),
                    turn_id: "turn-ckpt-fail".to_string(),
                    round_id: "round-1".to_string(),
                    round_group_id: None,
                    round_index: 0,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: "gemini".to_string(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: None,
                },
            ))
            .await
            .expect("checkpoint failure must not fail the subscriber");
        assert!(persister.unreadable.lock().expect("unreadable").is_empty());
        assert!(writer.has_draft("acp-1", "turn-ckpt-fail"));

        // Sustained failure must not retry a doomed write on every stream chunk:
        // throttle advances even when the checkpoint itself failed.
        *persister.fail_next.lock().expect("fail") = true;
        let before = *persister.persist_calls.lock().expect("persist");
        for _ in 0..10 {
            writer
                .on_envelope(&envelope(
                    AgenticEventOrigin::ExternalAcp,
                    AgenticEvent::TextChunk {
                        session_id: "acp-1".to_string(),
                        turn_id: "turn-ckpt-fail".to_string(),
                        round_id: "round-1".to_string(),
                        attempt_id: None,
                        attempt_index: None,
                        text: "x".to_string(),
                    },
                ))
                .await
                .expect("stream");
        }
        assert_eq!(
            *persister.persist_calls.lock().expect("persist"),
            before,
            "failed checkpoint must advance throttle so tiny chunks do not re-hit disk"
        );
    }
}
