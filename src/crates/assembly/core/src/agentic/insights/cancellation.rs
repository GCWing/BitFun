use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

type Slot = Arc<Mutex<Option<CancellationToken>>>;

static SLOT: std::sync::OnceLock<Slot> = std::sync::OnceLock::new();

fn get_slot() -> Slot {
    SLOT.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Registers a new insights generation task, cancelling any previous one.
pub async fn register() -> CancellationToken {
    let token = CancellationToken::new();
    let arc = get_slot();
    let mut slot = arc.lock().await;
    if let Some(old) = slot.take() {
        old.cancel();
        debug!("Cancelled previous insights generation");
    }
    *slot = Some(token.clone());
    token
}

/// Cancels the current insights generation task.
pub async fn cancel() -> Result<(), String> {
    let arc = get_slot();
    let mut slot = arc.lock().await;
    match slot.take() {
        Some(token) => {
            token.cancel();
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
pub async fn unregister() {
    let arc = get_slot();
    let mut slot = arc.lock().await;
    *slot = None;
}
