use log::{debug, info, warn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Registration {
    pub id: u64,
    pub token: CancellationToken,
}

type Slot = Arc<Mutex<Option<Registration>>>;

static SLOT: std::sync::OnceLock<Slot> = std::sync::OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn get_slot() -> Slot {
    SLOT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Registers a new insights generation task, cancelling any previous one.
pub async fn register() -> Registration {
    let token = CancellationToken::new();
    let registration = Registration {
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        token: token.clone(),
    };
    let arc = get_slot();
    let mut slot = arc.lock().await;
    if let Some(old) = slot.take() {
        old.token.cancel();
        debug!("Cancelled previous insights generation");
    }
    *slot = Some(registration.clone());
    registration
}

/// Cancels the current insights generation task.
pub async fn cancel() -> Result<(), String> {
    let arc = get_slot();
    let mut slot = arc.lock().await;
    match slot.take() {
        Some(token) => {
            token.token.cancel();
            info!("Insights generation cancelled by user");
            Ok(())
        }
        None => {
            warn!("No insights generation in progress to cancel");
            Err("No insights generation in progress".into())
        }
    }
}

/// Unregisters the current task (call on completion).
pub async fn unregister(registration_id: u64) {
    let arc = get_slot();
    let mut slot = arc.lock().await;
    if slot
        .as_ref()
        .is_some_and(|current| current.id == registration_id)
    {
        *slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn old_task_cannot_unregister_a_new_generation() {
        let old = register().await;
        let current = register().await;
        assert!(old.token.is_cancelled());

        unregister(old.id).await;
        cancel()
            .await
            .expect("current generation remains registered");

        assert!(current.token.is_cancelled());
    }
}
