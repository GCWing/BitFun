use super::LoopxController;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use bitfun_agent_runtime::event_bus::{EventBusError, EventSubscriberResult};
use bitfun_product_domains::miniapp::loopx::LoopxAgentTurnStatus;
use bitfun_core_types::errors::{AiErrorDetail, ErrorCategory};
use std::sync::Arc;

pub struct LoopxEventSubscriber {
    controller: Arc<LoopxController>,
}

impl LoopxEventSubscriber {
    pub fn new(controller: Arc<LoopxController>) -> Self {
        Self { controller }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for LoopxEventSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        let result = match event {
            AgenticEvent::TextChunk { turn_id, .. }
            | AgenticEvent::ThinkingChunk { turn_id, .. }
            | AgenticEvent::ModelRoundStarted { turn_id, .. } => {
                self.controller.handle_agent_activity(turn_id, None).await
            }
            AgenticEvent::ToolEvent {
                turn_id,
                tool_event,
                ..
            } => {
                self.controller
                    .handle_agent_activity(
                        turn_id,
                        Some(tool_event.effective_tool_name().to_string()),
                    )
                    .await
            }
            AgenticEvent::DialogTurnCompleted {
                turn_id, success, ..
            } => {
                let status = if *success == Some(false) {
                    LoopxAgentTurnStatus::Failed
                } else {
                    LoopxAgentTurnStatus::Completed
                };
                self.controller
                    .handle_agent_terminal(turn_id, status, None, false)
                    .await
            }
            AgenticEvent::DialogTurnFailed {
                turn_id,
                error,
                error_category,
                error_detail,
                ..
            } => {
                let summary = failure_summary(error, error_detail.as_ref());
                let blocks_repository = failure_blocks_repository(
                    error_category.as_ref(),
                    error_detail.as_ref(),
                );
                self.controller
                    .handle_agent_terminal(
                        turn_id,
                        LoopxAgentTurnStatus::Failed,
                        Some(summary),
                        blocks_repository,
                    )
                    .await
            }
            AgenticEvent::DialogTurnCancelled { turn_id, .. } => {
                self.controller
                    .handle_agent_terminal(
                        turn_id,
                        LoopxAgentTurnStatus::Cancelled,
                        None,
                        false,
                    )
                    .await
            }
            AgenticEvent::DialogTurnInterrupted { turn_id, .. } => {
                self.controller
                    .handle_agent_terminal(
                        turn_id,
                        LoopxAgentTurnStatus::Interrupted,
                        None,
                        false,
                    )
                    .await
            }
            _ => Ok(()),
        };
        result.map_err(EventBusError::subscriber)
    }
}

fn failure_summary(error: &str, detail: Option<&AiErrorDetail>) -> String {
    detail
        .and_then(|detail| detail.provider_message.as_deref())
        .filter(|message| !message.trim().is_empty())
        .unwrap_or(error)
        .chars()
        .take(1_000)
        .collect()
}

fn failure_blocks_repository(
    category: Option<&ErrorCategory>,
    detail: Option<&AiErrorDetail>,
) -> bool {
    let category = category.or_else(|| detail.map(|detail| &detail.category));
    matches!(
        category,
        Some(
            ErrorCategory::Network
                | ErrorCategory::Auth
                | ErrorCategory::RateLimit
                | ErrorCategory::Timeout
                | ErrorCategory::ProviderQuota
                | ErrorCategory::ProviderBilling
                | ErrorCategory::ProviderUnavailable
                | ErrorCategory::Permission
                | ErrorCategory::InvalidRequest
                | ErrorCategory::ModelError
        )
    )
}
