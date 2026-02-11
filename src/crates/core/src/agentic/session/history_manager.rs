//! Message History Manager
//!
//! Manages session message history, supports memory caching and persistence

use crate::agentic::core::Message;
use crate::agentic::persistence::PersistenceManager;
use crate::util::errors::BitFunResult;
use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

/// Message history configuration
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    pub enable_persistence: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enable_persistence: true,
        }
    }
}

/// Message history manager
pub struct MessageHistoryManager {
    /// Message history in memory (by session ID)
    histories: Arc<DashMap<String, Vec<Message>>>,

    /// Persistence manager
    persistence: Arc<PersistenceManager>,

    /// Configuration
    config: HistoryConfig,
}

impl MessageHistoryManager {
    pub fn new(persistence: Arc<PersistenceManager>, config: HistoryConfig) -> Self {
        Self {
            histories: Arc::new(DashMap::new()),
            persistence,
            config,
        }
    }

    /// Create session history
    pub async fn create_session(&self, session_id: &str) -> BitFunResult<()> {
        self.histories.insert(session_id.to_string(), vec![]);
        debug!("Created session history: session_id={}", session_id);
        Ok(())
    }

    /// Add message
    pub async fn add_message(&self, session_id: &str, message: Message) -> BitFunResult<()> {
        // 1. Add to memory
        if let Some(mut messages) = self.histories.get_mut(session_id) {
            messages.push(message.clone());
        } else {
            // Session doesn't exist, create and add
            self.histories
                .insert(session_id.to_string(), vec![message.clone()]);
        }

        // 2. Persist
        if self.config.enable_persistence {
            self.persistence
                .append_message(session_id, &message)
                .await?;
        }

        Ok(())
    }

    /// Get message history
    pub async fn get_messages(&self, session_id: &str) -> BitFunResult<Vec<Message>> {
        // First try to get from memory
        if let Some(messages) = self.histories.get(session_id) {
            return Ok(messages.clone());
        }

        // Load from persistence
        if self.config.enable_persistence {
            let messages = self.persistence.load_messages(session_id).await?;

            // Cache to memory
            if !messages.is_empty() {
                self.histories
                    .insert(session_id.to_string(), messages.clone());
            }

            Ok(messages)
        } else {
            Ok(vec![])
        }
    }

    /// Get recent N messages
    pub async fn get_recent_messages(
        &self,
        session_id: &str,
        count: usize,
    ) -> BitFunResult<Vec<Message>> {
        let messages = self.get_messages(session_id).await?;
        let start = messages.len().saturating_sub(count);
        Ok(messages[start..].to_vec())
    }

    /// Get message count
    pub async fn count_messages(&self, session_id: &str) -> usize {
        if let Some(messages) = self.histories.get(session_id) {
            messages.len()
        } else if self.config.enable_persistence {
            // Load from persistence
            self.persistence
                .load_messages(session_id)
                .await
                .map(|msgs| msgs.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Clear message history
    pub async fn clear_messages(&self, session_id: &str) -> BitFunResult<()> {
        // Clear memory
        if let Some(mut messages) = self.histories.get_mut(session_id) {
            messages.clear();
        }

        // Clear persistence
        if self.config.enable_persistence {
            self.persistence.clear_messages(session_id).await?;
        }

        debug!("Cleared session message history: session_id={}", session_id);
        Ok(())
    }

    /// Delete session
    pub async fn delete_session(&self, session_id: &str) -> BitFunResult<()> {
        // Remove from memory
        self.histories.remove(session_id);

        // Delete from persistence
        if self.config.enable_persistence {
            self.persistence.delete_messages(session_id).await?;
        }

        debug!("Deleted session history: session_id={}", session_id);
        Ok(())
    }

    /// Restore session (load from persistence)
    pub async fn restore_session(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> BitFunResult<()> {
        self.histories.insert(session_id.to_string(), messages);
        debug!("Restored session history: session_id={}", session_id);
        Ok(())
    }
}
