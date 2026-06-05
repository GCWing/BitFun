//! Runtime session context store.
//!
//! Holds the in-memory model context for each active session.

use crate::agentic::core::Message;
use dashmap::DashMap;
use log::debug;
use std::sync::Arc;

/// In-memory runtime context store for active sessions.
pub struct SessionContextStore {
    session_contexts: Arc<DashMap<String, Vec<Message>>>,
}

impl Default for SessionContextStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionContextStore {
    pub fn new() -> Self {
        Self {
            session_contexts: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(&self, session_id: &str) {
        self.session_contexts.insert(session_id.to_string(), vec![]);
        debug!("Created session context cache: session_id={}", session_id);
    }

    pub fn add_message(&self, session_id: &str, message: Message) {
        if let Some(mut cached_messages) = self.session_contexts.get_mut(session_id) {
            cached_messages.push(message);
        } else {
            self.session_contexts
                .insert(session_id.to_string(), vec![message]);
        }
    }

    pub fn replace_context(&self, session_id: &str, messages: Vec<Message>) {
        self.session_contexts
            .insert(session_id.to_string(), messages);
        debug!("Replaced session context cache: session_id={}", session_id);
    }

    pub fn get_context_messages(&self, session_id: &str) -> Vec<Message> {
        self.session_contexts
            .get(session_id)
            .map(|messages| messages.clone())
            .unwrap_or_default()
    }

    pub fn delete_session(&self, session_id: &str) {
        self.session_contexts.remove(session_id);
        debug!("Deleted session context cache: session_id={}", session_id);
    }
}
