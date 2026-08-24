//! Desktop ACP observation publisher.
//!
//! Maps ACP protocol stream events to `AgenticEvent` and enqueues them onto
//! the existing Desktop `EventQueue` with `ExternalAcp` origin. ACP protocol
//! execution stays in `bitfun-acp`; this type only owns ordered observation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bitfun_acp::client::{
    AcpAvailableCommand, AcpClientStreamEvent, AcpPlanEntry, AcpSessionContextUsage,
};
use bitfun_core::agentic::events::EventQueue;
use bitfun_core::util::errors::BitFunError;
use bitfun_events::{
    AcpAvailableCommandFact, AcpPlanEntryFact, AgenticEvent, AgenticEventOrigin,
    AgenticEventPriority, ModelRoundIdentity, ModelRoundRenderHints,
};
use log::warn;

/// Soft in-flight watermark for the unbounded publisher channel.
/// Best-effort stream chunks are dropped above this; control events are not.
const ACP_EVENT_CHANNEL_WATERMARK: usize = 2048;

#[derive(Debug, Clone)]
pub(crate) enum AcpPublishJob {
    BestEffort(AgenticEvent),
    Guaranteed(AgenticEvent),
    Fence(AgenticEvent),
}

#[derive(Clone)]
pub struct AcpEventPublisher {
    tx: tokio::sync::mpsc::UnboundedSender<AcpPublishJob>,
    /// In-flight depth of this publisher channel (jobs sent, not yet `recv`'d).
    /// It is not the EventQueue depth. Best-effort drops compare against this.
    queued: Arc<AtomicUsize>,
    watermark: usize,
}

impl AcpEventPublisher {
    pub(crate) fn start(queue: Arc<EventQueue>) -> Arc<Self> {
        Self::start_with_watermark(queue, ACP_EVENT_CHANNEL_WATERMARK)
    }

    fn start_with_watermark(queue: Arc<EventQueue>, watermark: usize) -> Arc<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let queued = Arc::new(AtomicUsize::new(0));
        let publisher = Arc::new(Self {
            tx,
            queued: queued.clone(),
            watermark,
        });
        tokio::spawn(run_publisher_worker(queue, rx, queued));
        publisher
    }

    #[cfg(test)]
    fn start_paused(
        watermark: usize,
    ) -> (
        Arc<Self>,
        tokio::sync::mpsc::UnboundedReceiver<AcpPublishJob>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (
            Arc::new(Self {
                tx,
                queued: Arc::new(AtomicUsize::new(0)),
                watermark,
            }),
            rx,
        )
    }

    pub(crate) fn publish_session_created(&self, event: AgenticEvent) -> Result<(), String> {
        self.send(AcpPublishJob::Guaranteed(event))
    }

    pub(crate) fn publish_turn_started(&self, event: AgenticEvent) -> Result<(), String> {
        self.send(AcpPublishJob::Guaranteed(event))
    }

    pub(crate) fn publish_jobs(&self, jobs: Vec<AcpPublishJob>) -> Result<(), String> {
        for job in jobs {
            self.send(job)?;
        }
        Ok(())
    }

    fn send(&self, job: AcpPublishJob) -> Result<(), String> {
        match &job {
            AcpPublishJob::BestEffort(_) => {
                if !self.try_reserve_best_effort() {
                    warn!("ACP stream channel is above watermark; dropping best-effort event");
                    return Ok(());
                }
            }
            AcpPublishJob::Guaranteed(_) | AcpPublishJob::Fence(_) => {
                self.queued.fetch_add(1, Ordering::Relaxed);
            }
        }

        if let Err(error) = self.tx.send(job) {
            self.queued.fetch_sub(1, Ordering::Relaxed);
            return Err(format!("ACP event channel closed: {error}"));
        }
        Ok(())
    }

    fn try_reserve_best_effort(&self) -> bool {
        let mut current = self.queued.load(Ordering::Relaxed);
        loop {
            if current >= self.watermark {
                return false;
            }
            match self.queued.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }
}

async fn run_publisher_worker(
    queue: Arc<EventQueue>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<AcpPublishJob>,
    queued: Arc<AtomicUsize>,
) {
    while let Some(job) = rx.recv().await {
        queued.fetch_sub(1, Ordering::Relaxed);
        if let Err(error) = publish_job(&queue, job).await {
            warn!("ACP event enqueue failed: {error}");
        }
    }
}

async fn publish_job(queue: &EventQueue, job: AcpPublishJob) -> Result<(), String> {
    let origin = AgenticEventOrigin::ExternalAcp;
    match job {
        AcpPublishJob::BestEffort(event) => queue
            .enqueue_with_origin(event, None, origin)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
        AcpPublishJob::Guaranteed(event) => queue
            .enqueue_with_guaranteed_legacy_storage_with_origin(event, None, origin)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
        AcpPublishJob::Fence(event) => {
            let (_, ack) = queue
                .enqueue_with_legacy_dequeue_ack_with_origin(
                    event,
                    Some(AgenticEventPriority::Normal),
                    origin,
                )
                .await
                .map_err(|error| error.to_string())?;
            ack.wait()
                .await
                .map_err(|error| format!("ACP terminal fence ack failed: {error}"))
        }
    }
}

pub(crate) struct AcpTurnMapper {
    session_id: String,
    turn_id: String,
    client_id: String,
    current_round_id: Option<String>,
    current_round_has_tool_calls: bool,
    closed_rounds: usize,
    total_tools: usize,
    started_at: Instant,
    terminal_emitted: bool,
}

impl AcpTurnMapper {
    pub(crate) fn new(session_id: String, turn_id: String, client_id: String) -> Self {
        Self {
            session_id,
            turn_id,
            client_id,
            current_round_id: None,
            current_round_has_tool_calls: false,
            closed_rounds: 0,
            total_tools: 0,
            started_at: Instant::now(),
            terminal_emitted: false,
        }
    }

    pub(crate) fn map(
        &mut self,
        event: AcpClientStreamEvent,
    ) -> Result<Vec<AcpPublishJob>, BitFunError> {
        match event {
            AcpClientStreamEvent::ModelRoundStarted {
                round_id,
                round_index,
                disable_explore_grouping,
            } => {
                let mut jobs = Vec::new();
                if let Some(job) = self.close_current_round() {
                    jobs.push(job);
                }
                self.current_round_id = Some(round_id.clone());
                self.current_round_has_tool_calls = false;
                jobs.push(AcpPublishJob::BestEffort(AgenticEvent::ModelRoundStarted {
                    session_id: self.session_id.clone(),
                    turn_id: self.turn_id.clone(),
                    round_id,
                    round_group_id: None,
                    round_index,
                    identity: ModelRoundIdentity::External {
                        provider: "acp".to_string(),
                        client_id: self.client_id.clone(),
                        model_id: None,
                        display_name: None,
                    },
                    render_hints: Some(ModelRoundRenderHints {
                        disable_explore_grouping,
                    }),
                }));
                Ok(jobs)
            }
            AcpClientStreamEvent::AgentText(text) => {
                let round_id = self.require_round("ACP text arrived before model round start")?;
                Ok(vec![AcpPublishJob::BestEffort(AgenticEvent::TextChunk {
                    session_id: self.session_id.clone(),
                    turn_id: self.turn_id.clone(),
                    round_id,
                    attempt_id: None,
                    attempt_index: None,
                    text,
                })])
            }
            AcpClientStreamEvent::AgentThought(text) => {
                let round_id =
                    self.require_round("ACP thought arrived before model round start")?;
                Ok(vec![AcpPublishJob::BestEffort(
                    AgenticEvent::ThinkingChunk {
                        session_id: self.session_id.clone(),
                        turn_id: self.turn_id.clone(),
                        round_id,
                        attempt_id: None,
                        attempt_index: None,
                        content: text,
                        reasoning_kind: None,
                        is_end: false,
                    },
                )])
            }
            AcpClientStreamEvent::ToolEvent(tool_event) => {
                let round_id =
                    self.require_round("ACP tool event arrived before model round start")?;
                self.current_round_has_tool_calls = true;
                self.total_tools += 1;
                Ok(vec![AcpPublishJob::BestEffort(AgenticEvent::ToolEvent {
                    session_id: self.session_id.clone(),
                    turn_id: self.turn_id.clone(),
                    round_id,
                    attempt_id: None,
                    attempt_index: None,
                    tool_event,
                })])
            }
            AcpClientStreamEvent::ContextUsageUpdated(usage) => {
                Ok(vec![AcpPublishJob::BestEffort(map_context_usage(
                    &self.session_id,
                    &self.turn_id,
                    &self.client_id,
                    usage,
                ))])
            }
            AcpClientStreamEvent::AvailableCommandsUpdated(commands) => {
                Ok(vec![AcpPublishJob::BestEffort(
                    AgenticEvent::AcpAvailableCommandsUpdated {
                        session_id: self.session_id.clone(),
                        client_id: self.client_id.clone(),
                        commands: commands.into_iter().map(map_available_command).collect(),
                    },
                )])
            }
            AcpClientStreamEvent::PlanUpdated(entries) => Ok(vec![AcpPublishJob::BestEffort(
                AgenticEvent::AcpPlanUpdated {
                    session_id: self.session_id.clone(),
                    turn_id: self.turn_id.clone(),
                    client_id: self.client_id.clone(),
                    entries: entries.into_iter().map(map_plan_entry).collect(),
                },
            )]),
            AcpClientStreamEvent::ConfigOptionsUpdated(_) => Ok(vec![AcpPublishJob::BestEffort(
                AgenticEvent::AcpSessionOptionsChanged {
                    session_id: self.session_id.clone(),
                    client_id: self.client_id.clone(),
                },
            )]),
            AcpClientStreamEvent::Completed => Ok(self.complete_turn()),
            AcpClientStreamEvent::Cancelled => Ok(self.cancel_turn()),
        }
    }

    pub(crate) fn fail(&mut self, error: String) -> Vec<AcpPublishJob> {
        if self.terminal_emitted {
            return Vec::new();
        }
        self.terminal_emitted = true;
        let mut jobs = Vec::new();
        if let Some(job) = self.close_current_round() {
            jobs.push(job);
        }
        jobs.push(AcpPublishJob::Fence(AgenticEvent::DialogTurnFailed {
            session_id: self.session_id.clone(),
            turn_id: self.turn_id.clone(),
            error,
            error_category: None,
            error_detail: None,
        }));
        jobs
    }

    fn complete_turn(&mut self) -> Vec<AcpPublishJob> {
        if self.terminal_emitted {
            return Vec::new();
        }
        self.terminal_emitted = true;
        let mut jobs = Vec::new();
        if let Some(job) = self.close_current_round() {
            jobs.push(job);
        }
        jobs.push(AcpPublishJob::Guaranteed(
            AgenticEvent::DialogTurnCompleted {
                session_id: self.session_id.clone(),
                turn_id: self.turn_id.clone(),
                total_rounds: self.closed_rounds,
                total_tools: self.total_tools,
                duration_ms: elapsed_ms(self.started_at),
                partial_recovery_reason: None,
                success: Some(true),
                finish_reason: Some("complete".to_string()),
                has_final_response: None,
            },
        ));
        jobs
    }

    fn cancel_turn(&mut self) -> Vec<AcpPublishJob> {
        if self.terminal_emitted {
            return Vec::new();
        }
        self.terminal_emitted = true;
        let mut jobs = Vec::new();
        if let Some(job) = self.close_current_round() {
            jobs.push(job);
        }
        jobs.push(AcpPublishJob::Fence(AgenticEvent::DialogTurnCancelled {
            session_id: self.session_id.clone(),
            turn_id: self.turn_id.clone(),
        }));
        jobs
    }

    fn close_current_round(&mut self) -> Option<AcpPublishJob> {
        let round_id = self.current_round_id.take()?;
        let has_tool_calls = self.current_round_has_tool_calls;
        self.current_round_has_tool_calls = false;
        self.closed_rounds += 1;
        Some(AcpPublishJob::BestEffort(
            AgenticEvent::ModelRoundCompleted {
                session_id: self.session_id.clone(),
                turn_id: self.turn_id.clone(),
                round_id,
                has_tool_calls,
                duration_ms: None,
                provider_id: None,
                model_config_id: String::new(),
                effective_model_name: String::new(),
                first_chunk_ms: None,
                first_visible_output_ms: None,
                stream_duration_ms: None,
                attempt_count: None,
                failure_category: None,
                token_details: None,
            },
        ))
    }

    fn require_round(&self, message: &str) -> Result<String, BitFunError> {
        self.current_round_id
            .clone()
            .ok_or_else(|| BitFunError::service(message.to_string()))
    }
}

fn map_context_usage(
    session_id: &str,
    turn_id: &str,
    client_id: &str,
    usage: AcpSessionContextUsage,
) -> AgenticEvent {
    AgenticEvent::AcpContextUsageUpdated {
        session_id: session_id.to_string(),
        turn_id: turn_id.to_string(),
        client_id: client_id.to_string(),
        used: usage.used,
        size: usage.size,
        cost: usage.cost.and_then(|cost| serde_json::to_value(cost).ok()),
    }
}

fn map_available_command(command: AcpAvailableCommand) -> AcpAvailableCommandFact {
    AcpAvailableCommandFact {
        name: command.name,
        description: command.description,
        input_hint: command.input_hint,
    }
}

fn map_plan_entry(entry: AcpPlanEntry) -> AcpPlanEntryFact {
    AcpPlanEntryFact {
        content: entry.content,
        priority: entry.priority,
        status: entry.status,
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis() as u64
}

pub(crate) fn acp_session_created_event(
    session_id: String,
    session_name: String,
    agent_type: String,
    workspace_path: String,
    remote_connection_id: Option<String>,
    remote_ssh_host: Option<String>,
) -> AgenticEvent {
    AgenticEvent::SessionCreated {
        session_id,
        session_name,
        agent_type,
        workspace_path: Some(workspace_path),
        project_workspace_path: None,
        execution_target: None,
        workspace_id: None,
        remote_connection_id,
        remote_ssh_host,
    }
}

pub(crate) fn acp_dialog_turn_started_event(
    session_id: String,
    turn_id: String,
    user_input: String,
    original_user_input: Option<String>,
) -> AgenticEvent {
    AgenticEvent::DialogTurnStarted {
        session_id,
        turn_id,
        turn_index: 0,
        user_input,
        original_user_input,
        user_message_metadata: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AcpPublishJob, AcpTurnMapper};
    use bitfun_acp::client::AcpClientStreamEvent;
    use bitfun_events::{AgenticEvent, ModelRoundIdentity};

    #[test]
    fn maps_round_text_and_complete_without_token_usage() {
        let mut mapper = AcpTurnMapper::new(
            "session-1".to_string(),
            "turn-1".to_string(),
            "gemini".to_string(),
        );
        let started = mapper
            .map(AcpClientStreamEvent::ModelRoundStarted {
                round_id: "round-1".to_string(),
                round_index: 0,
                disable_explore_grouping: true,
            })
            .expect("map start");
        assert!(matches!(
            started[0],
            AcpPublishJob::BestEffort(AgenticEvent::ModelRoundStarted {
                identity: ModelRoundIdentity::External { .. },
                ..
            })
        ));
        let text = mapper
            .map(AcpClientStreamEvent::AgentText("hello".to_string()))
            .expect("map text");
        assert!(matches!(
            text[0],
            AcpPublishJob::BestEffort(AgenticEvent::TextChunk { .. })
        ));
        let completed = mapper
            .map(AcpClientStreamEvent::Completed)
            .expect("map complete");
        assert!(completed.iter().any(|job| matches!(
            job,
            AcpPublishJob::Guaranteed(AgenticEvent::DialogTurnCompleted { .. })
        )));
        assert!(completed.iter().all(|job| {
            !matches!(
                job,
                AcpPublishJob::BestEffort(AgenticEvent::TokenUsageUpdated { .. })
            )
        }));
        assert!(mapper.fail("late".to_string()).is_empty());
    }

    #[test]
    fn text_before_round_fails_loud() {
        let mut mapper = AcpTurnMapper::new(
            "session-1".to_string(),
            "turn-1".to_string(),
            "gemini".to_string(),
        );
        let error = mapper
            .map(AcpClientStreamEvent::AgentText("early".to_string()))
            .expect_err("must fail");
        assert!(error.to_string().contains("before model round start"));
    }

    #[tokio::test]
    async fn publish_turn_started_from_async_context_reaches_queue() {
        use super::{acp_dialog_turn_started_event, AcpEventPublisher};
        use bitfun_core::agentic::events::EventQueue;
        use bitfun_events::AgenticEventOrigin;
        use std::sync::Arc;
        use std::time::Duration;

        let queue = Arc::new(EventQueue::new(Default::default()));
        let publisher = AcpEventPublisher::start(queue.clone());
        publisher
            .publish_turn_started(acp_dialog_turn_started_event(
                "session-1".to_string(),
                "turn-1".to_string(),
                "hello".to_string(),
                None,
            ))
            .expect("publish_turn_started must not panic or fail inside a Tokio runtime");

        let batch = wait_for_queue_event(&queue).await;
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].origin, AgenticEventOrigin::ExternalAcp);
        assert!(matches!(
            batch[0].event,
            AgenticEvent::DialogTurnStarted {
                ref session_id,
                ref turn_id,
                ..
            } if session_id == "session-1" && turn_id == "turn-1"
        ));

        async fn wait_for_queue_event(
            queue: &EventQueue,
        ) -> Vec<bitfun_events::AgenticEventEnvelope> {
            for _ in 0..50 {
                let batch = queue.dequeue_configured_batch().await;
                if !batch.is_empty() {
                    return batch;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("publisher worker did not enqueue DialogTurnStarted");
        }
    }

    #[test]
    fn watermark_drops_only_best_effort_and_keeps_channel_order() {
        use super::AcpEventPublisher;

        let (publisher, mut rx) = AcpEventPublisher::start_paused(1);
        publisher
            .publish_jobs(vec![
                AcpPublishJob::BestEffort(text_chunk("kept")),
                AcpPublishJob::BestEffort(text_chunk("dropped")),
                AcpPublishJob::Guaranteed(dialog_turn_completed()),
                AcpPublishJob::Fence(dialog_turn_failed()),
            ])
            .expect("control events must never fail at the watermark");

        let first = rx.try_recv().expect("accepted best-effort event");
        let second = rx.try_recv().expect("guaranteed event");
        let third = rx.try_recv().expect("fence event");
        assert!(
            rx.try_recv().is_err(),
            "overflow best-effort events must be dropped, not reordered"
        );
        assert!(matches!(
            first,
            AcpPublishJob::BestEffort(AgenticEvent::TextChunk { ref text, .. })
                if text == "kept"
        ));
        assert!(matches!(
            second,
            AcpPublishJob::Guaranteed(AgenticEvent::DialogTurnCompleted { .. })
        ));
        assert!(matches!(
            third,
            AcpPublishJob::Fence(AgenticEvent::DialogTurnFailed { .. })
        ));
    }

    fn text_chunk(text: &str) -> AgenticEvent {
        AgenticEvent::TextChunk {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            round_id: "round-1".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: text.to_string(),
        }
    }

    fn dialog_turn_completed() -> AgenticEvent {
        AgenticEvent::DialogTurnCompleted {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            total_rounds: 1,
            total_tools: 0,
            duration_ms: 1,
            partial_recovery_reason: None,
            success: Some(true),
            finish_reason: None,
            has_final_response: None,
        }
    }

    fn dialog_turn_failed() -> AgenticEvent {
        AgenticEvent::DialogTurnFailed {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            error: "failed".to_string(),
            error_category: None,
            error_detail: None,
        }
    }
}
