//! Provider-neutral runtime event bus errors and subscriber result types.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EventBusError {
    #[error("event subscriber error: {0}")]
    Subscriber(String),
}

impl EventBusError {
    pub fn subscriber(error: impl ToString) -> Self {
        Self::Subscriber(error.to_string())
    }
}

pub type EventBusResult<T> = Result<T, EventBusError>;
pub type EventSubscriberResult = EventBusResult<()>;
