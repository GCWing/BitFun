//! Bounded outbound transport shared by account device routing and RPC.
use std::sync::Arc;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tracing::debug;
pub type ConnId = u64;

const OUTBOUND_MEMORY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct OutboundMessage {
    pub text: String,
    _memory: Option<OwnedSemaphorePermit>,
}

impl OutboundMessage {
    /// Keep queued and actively written payloads inside one process-wide budget.
    pub fn try_text(text: impl AsRef<str>) -> Option<Self> {
        static MEMORY: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
        Self::with_budget(
            text.as_ref(),
            MEMORY.get_or_init(|| Arc::new(Semaphore::new(OUTBOUND_MEMORY_BYTES))),
        )
    }

    fn with_budget(text: &str, budget: &Arc<Semaphore>) -> Option<Self> {
        let size = u32::try_from(text.len().max(1)).ok()?;
        let permit = Arc::clone(budget).try_acquire_many_owned(size).ok()?;
        Some(Self {
            text: text.to_owned(),
            _memory: Some(permit),
        })
    }

    #[cfg(test)]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            _memory: None,
        }
    }
}

pub async fn send_outbound_message(
    tx: &mpsc::Sender<OutboundMessage>,
    message: OutboundMessage,
) -> bool {
    match tokio::time::timeout(std::time::Duration::from_secs(2), tx.send(message)).await {
        Ok(Ok(())) => true,
        _ => {
            debug!("Outbound websocket channel closed before message could be sent");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};
    #[tokio::test]
    async fn outbound_send_waits_for_bounded_queue_capacity() {
        let (tx, mut rx) = mpsc::channel(1);

        assert!(send_outbound_message(&tx, OutboundMessage::text("first"),).await);

        let blocked_send = tokio::spawn({
            let tx = tx.clone();
            async move { send_outbound_message(&tx, OutboundMessage::text("second")).await }
        });

        tokio::task::yield_now().await;
        assert!(
            !blocked_send.is_finished(),
            "bounded outbound send should apply backpressure instead of dropping"
        );

        assert_eq!(rx.recv().await.expect("first message").text, "first");
        assert!(timeout(Duration::from_secs(1), blocked_send)
            .await
            .expect("send should complete after capacity is released")
            .expect("send task should not panic"));
        assert_eq!(rx.recv().await.expect("second message").text, "second");
    }

    #[test]
    fn outbound_memory_is_bounded_and_reclaimed() {
        let budget = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
        let first = OutboundMessage::with_budget("12345678", &budget).unwrap();
        assert!(OutboundMessage::with_budget("x", &budget).is_none());
        drop(first);
        assert!(OutboundMessage::with_budget("12345678", &budget).is_some());
        assert!(OutboundMessage::with_budget("123456789", &budget).is_none());
    }
}
