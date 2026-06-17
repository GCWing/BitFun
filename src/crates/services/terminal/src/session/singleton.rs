//! Session Manager Singleton
//!
//! This module provides a global singleton instance of SessionManager
//! for easy access throughout the application.

use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::config::TerminalConfig;

use super::SessionManager;

/// Global singleton instance of SessionManager
static SESSION_MANAGER: OnceCell<Arc<SessionManager>> = OnceCell::const_new();

/// Initialize the global SessionManager singleton with the given configuration.
///
/// This function should be called once at application startup.
/// Subsequent calls will return an error if the manager is already initialized.
///
/// # Arguments
/// * `config` - The terminal configuration to use
///
/// # Returns
/// * `Ok(Arc<SessionManager>)` - The initialized session manager
/// * `Err(&'static str)` - If already initialized
///
/// # Example
/// ```ignore
/// use terminal_core::config::TerminalConfig;
/// use terminal_core::session::init_session_manager;
///
/// let config = TerminalConfig::default();
/// let manager = init_session_manager(config).await?;
/// ```
pub async fn init_session_manager(
    config: TerminalConfig,
) -> Result<Arc<SessionManager>, &'static str> {
    let manager = Arc::new(SessionManager::new(config));
    SESSION_MANAGER
        .set(manager.clone())
        .map_err(|_| "SessionManager already initialized")?;

    Ok(manager)
}

/// Get the global SessionManager singleton.
///
/// Returns `None` if the manager has not been initialized yet.
/// Use `init_session_manager` to initialize it first.
///
/// # Example
/// ```ignore
/// use terminal_core::session::get_session_manager;
///
/// if let Some(manager) = get_session_manager() {
///     let sessions = manager.list_sessions().await;
/// }
/// ```
pub fn get_session_manager() -> Option<Arc<SessionManager>> {
    SESSION_MANAGER.get().cloned()
}

/// Get the global SessionManager singleton, panicking if not initialized.
///
/// # Panics
/// Panics if the SessionManager has not been initialized.
///
/// # Example
/// ```ignore
/// use terminal_core::session::session_manager;
///
/// let manager = session_manager();
/// let sessions = manager.list_sessions().await;
/// ```
pub fn session_manager() -> Arc<SessionManager> {
    match SESSION_MANAGER.get().cloned() {
        Some(manager) => manager,
        None => panic!("SessionManager not initialized. Call init_session_manager first."),
    }
}

/// Check if the SessionManager singleton has been initialized.
pub fn is_session_manager_initialized() -> bool {
    SESSION_MANAGER.get().is_some()
}

/// Initialize the global SessionManager with custom instance.
///
/// This is useful for testing or when you need more control over initialization.
///
/// # Arguments
/// * `manager` - The pre-configured SessionManager instance wrapped in Arc
///
/// # Returns
/// * `Ok(())` - If successfully set
/// * `Err(&'static str)` - If already initialized
pub fn set_session_manager(manager: Arc<SessionManager>) -> Result<(), &'static str> {
    SESSION_MANAGER
        .set(manager)
        .map_err(|_| "SessionManager already initialized")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests should be run in isolation due to global state
    // Use `cargo test -- --test-threads=1` for reliable test execution

    #[tokio::test]
    async fn test_session_manager_not_initialized() {
        // This test may fail if other tests have already initialized the manager
        // In a fresh process, this should work
        if !is_session_manager_initialized() {
            assert!(get_session_manager().is_none());
        }
    }
}
