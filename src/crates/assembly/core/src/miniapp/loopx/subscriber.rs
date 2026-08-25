use super::LoopxController;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use bitfun_agent_runtime::event_bus::{EventBusError, EventSubscriberResult};
use bitfun_product_domains::miniapp::loopx::LoopxAgentTurnStatus;
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
                    .handle_agent_terminal(turn_id, status, None)
                    .await
            }
            AgenticEvent::DialogTurnFailed { turn_id, error, .. } => {
                self.controller
                    .handle_agent_terminal(
                        turn_id,
                        LoopxAgentTurnStatus::Failed,
                        Some(error.clone()),
                    )
                    .await
            }
            AgenticEvent::DialogTurnCancelled { turn_id, .. } => {
                self.controller
                    .handle_agent_terminal(turn_id, LoopxAgentTurnStatus::Cancelled, None)
                    .await
            }
            AgenticEvent::DialogTurnInterrupted { turn_id, .. } => {
                self.controller
                    .handle_agent_terminal(turn_id, LoopxAgentTurnStatus::Interrupted, None)
                    .await
            }
            _ => Ok(()),
        };
        result.map_err(EventBusError::subscriber)
    }
}
