//! Document generation cancellation management
//!
//! Provides global cancellation for document generation tasks.

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Document generation task registry
type GenerationRegistry = Arc<Mutex<HashMap<String, CancellationToken>>>;

/// Global registry
static REGISTRY: std::sync::OnceLock<GenerationRegistry> = std::sync::OnceLock::new();

/// Gets the global registry.
fn get_registry() -> GenerationRegistry {
    REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// Registers a document generation task.
///
/// Returns a `CancellationToken` that can be used to cancel the task.
pub async fn register_generation(doc_id: &str) -> CancellationToken {
    let token = CancellationToken::new();
    let registry = get_registry();
    let mut registry_guard = registry.lock().await;
    registry_guard.insert(doc_id.to_string(), token.clone());
    token
}

/// Cancels a document generation task.
///
/// # Parameters
/// - doc_id: Document ID
///
/// # Returns
/// - Ok(()): Cancelled successfully
/// - Err(String): Task does not exist or has already been cancelled
pub async fn cancel_generation(doc_id: &str) -> Result<(), String> {
    let registry = get_registry();
    let mut registry_guard = registry.lock().await;

    if let Some(token) = registry_guard.remove(doc_id) {
        token.cancel();
        info!("Cancelled document generation task: doc_id={}", doc_id);
        Ok(())
    } else {
        warn!("Document generation task not found: doc_id={}", doc_id);
        Err(format!("Document generation task not found: {}", doc_id))
    }
}

/// Checks whether the task has been cancelled.
///
/// # Parameters
/// - doc_id: Document ID
///
/// # Returns
/// - true: Task has been cancelled
/// - false: Task is not cancelled or does not exist
pub async fn is_cancelled(doc_id: &str) -> bool {
    let registry = get_registry();
    let registry_guard = registry.lock().await;
    registry_guard
        .get(doc_id)
        .map(|t| t.is_cancelled())
        .unwrap_or(false)
}

/// Unregisters a document generation task.
///
/// Call this when the task completes (whether success or failure).
pub async fn unregister_generation(doc_id: &str) {
    let registry = get_registry();
    let mut registry_guard = registry.lock().await;
    registry_guard.remove(doc_id);
    debug!("Unregistered document generation task: doc_id={}", doc_id);
}
