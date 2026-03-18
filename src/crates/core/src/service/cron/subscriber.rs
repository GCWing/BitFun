//! Scheduled job event subscriber.

use super::service::CronService;
use crate::agentic::events::{AgenticEvent, EventSubscriber};
use crate::util::errors::BitFunResult;
use log::error;
use std::sync::Arc;

pub struct CronEventSubscriber {
    cron_service: Arc<CronService>,
}

impl CronEventSubscriber {
    pub fn new(cron_service: Arc<CronService>) -> Self {
        Self { cron_service }
    }
}

#[async_trait::async_trait]
impl EventSubscriber for CronEventSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> BitFunResult<()> {
        let result = match event {
            AgenticEvent::DialogTurnStarted { turn_id, .. } => {
                self.cron_service.handle_turn_started(turn_id).await
            }
            AgenticEvent::DialogTurnCompleted {
                turn_id,
                duration_ms,
                ..
            } => {
                self.cron_service
                    .handle_turn_completed(turn_id, *duration_ms)
                    .await
            }
            AgenticEvent::DialogTurnFailed {
                turn_id, error, ..
            } => self.cron_service.handle_turn_failed(turn_id, error).await,
            AgenticEvent::DialogTurnCancelled { turn_id, .. } => {
                self.cron_service.handle_turn_cancelled(turn_id).await
            }
            _ => Ok(()),
        };

        if let Err(error) = &result {
            error!("Failed to update scheduled job state from event: {}", error);
        }

        result
    }
}
