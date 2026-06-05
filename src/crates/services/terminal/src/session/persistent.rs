//! Persistent Session - Session persistence and recovery

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::config::PersistenceConfig;
use crate::TerminalResult;

use super::{SessionStatus, TerminalSession};

/// Persistent session wrapper
pub struct PersistentSession {
    /// The underlying session
    session: Arc<RwLock<TerminalSession>>,

    /// Persistence configuration
    config: PersistenceConfig,

    /// Whether the session has been interacted with
    has_interaction: Arc<RwLock<bool>>,

    /// Whether a disconnect timer is running
    disconnect_timer_running: Arc<RwLock<bool>>,

    /// Shutdown handle
    shutdown_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl PersistentSession {
    /// Create a new persistent session
    pub fn new(session: TerminalSession, config: PersistenceConfig) -> Self {
        Self {
            session: Arc::new(RwLock::new(session)),
            config,
            has_interaction: Arc::new(RwLock::new(false)),
            disconnect_timer_running: Arc::new(RwLock::new(false)),
            shutdown_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the session
    pub async fn session(&self) -> TerminalSession {
        self.session.read().await.clone()
    }

    /// Get session ID
    pub async fn id(&self) -> String {
        self.session.read().await.id.clone()
    }

    /// Mark session as having interaction
    pub async fn mark_interaction(&self) {
        *self.has_interaction.write().await = true;
    }

    /// Check if session has been interacted with
    pub async fn has_had_interaction(&self) -> bool {
        *self.has_interaction.read().await
    }

    /// Attach to the session (client reconnected)
    pub async fn attach(&self) -> TerminalResult<()> {
        // Cancel any pending disconnect timer
        let handle = self.shutdown_handle.write().await.take();
        if let Some(h) = handle {
            h.abort();
        }

        *self.disconnect_timer_running.write().await = false;

        // Update session status
        let mut session = self.session.write().await;
        if session.status == SessionStatus::Orphaned {
            session.status = SessionStatus::Active;
        }
        session.touch();

        Ok(())
    }

    /// Detach from the session (client disconnected)
    pub async fn detach(&self, force_persist: bool) -> TerminalResult<()> {
        let should_persist = {
            let session = self.session.read().await;
            session.should_persist && (*self.has_interaction.read().await || force_persist)
        };

        if should_persist && self.config.enabled {
            // Mark as orphaned and start grace timer
            {
                let mut session = self.session.write().await;
                session.set_orphaned();
            }

            self.start_grace_timer().await;
        } else {
            // Session should be terminated
            let mut session = self.session.write().await;
            session.status = SessionStatus::Terminating;
        }

        Ok(())
    }

    /// Start the grace timer for orphaned sessions
    async fn start_grace_timer(&self) {
        if *self.disconnect_timer_running.read().await {
            return;
        }

        *self.disconnect_timer_running.write().await = true;

        let session = self.session.clone();
        let disconnect_timer_running = self.disconnect_timer_running.clone();
        let grace_time = Duration::from_secs(self.config.grace_time_secs);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(grace_time).await;

            // Check if still orphaned
            let mut session_guard = session.write().await;
            if session_guard.status == SessionStatus::Orphaned {
                session_guard.status = SessionStatus::Terminating;
            }

            *disconnect_timer_running.write().await = false;
        });

        *self.shutdown_handle.write().await = Some(handle);
    }

    /// Reduce grace time (e.g., when system is shutting down)
    pub async fn reduce_grace_time(&self) {
        if !*self.disconnect_timer_running.read().await {
            return;
        }

        // Cancel existing timer
        let handle = self.shutdown_handle.write().await.take();
        if let Some(h) = handle {
            h.abort();
        }

        // Start shorter timer
        let session = self.session.clone();
        let disconnect_timer_running = self.disconnect_timer_running.clone();
        let short_grace_time = Duration::from_secs(self.config.short_grace_time_secs);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(short_grace_time).await;

            let mut session_guard = session.write().await;
            if session_guard.status == SessionStatus::Orphaned {
                session_guard.status = SessionStatus::Terminating;
            }

            *disconnect_timer_running.write().await = false;
        });

        *self.shutdown_handle.write().await = Some(handle);
    }

    /// Check if the session is orphaned
    pub async fn is_orphaned(&self) -> bool {
        self.session.read().await.is_orphaned()
    }

    /// Check if the session should be terminated
    pub async fn should_terminate(&self) -> bool {
        matches!(
            self.session.read().await.status,
            SessionStatus::Terminating | SessionStatus::Exited { .. }
        )
    }
}

impl Drop for PersistentSession {
    fn drop(&mut self) {}
}
