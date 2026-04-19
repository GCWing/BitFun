//! Shared-context fork execution primitives.
//!
//! A fork is a hidden child execution that inherits the parent session's
//! model-visible message context, but still runs as an isolated session with
//! its own rounds, tools, cancellation, and cleanup lifecycle.

use crate::agentic::core::{Message, Session, SessionConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use std::collections::HashMap;

/// Immutable snapshot of a parent session's runtime context at fork time.
#[derive(Debug, Clone)]
pub struct ForkContextSnapshot {
    pub parent_session_id: String,
    pub parent_agent_type: String,
    pub workspace_path: String,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
    pub storage_scope: Option<crate::agentic::core::SessionStorageScope>,
    pub session_model_id: Option<String>,
    pub session_config: SessionConfig,
    pub messages: Vec<Message>,
}

impl ForkContextSnapshot {
    pub fn from_parent_session(
        parent_session: &Session,
        messages: Vec<Message>,
    ) -> BitFunResult<Self> {
        let workspace_path = parent_session
            .config
            .workspace_path
            .clone()
            .ok_or_else(|| {
                BitFunError::Validation(format!(
                    "workspace_path is required when forking session: {}",
                    parent_session.session_id
                ))
            })?;

        Ok(Self {
            parent_session_id: parent_session.session_id.clone(),
            parent_agent_type: parent_session.agent_type.clone(),
            workspace_path,
            remote_connection_id: parent_session.config.remote_connection_id.clone(),
            remote_ssh_host: parent_session.config.remote_ssh_host.clone(),
            storage_scope: parent_session.config.storage_scope,
            session_model_id: parent_session.config.model_id.clone(),
            session_config: parent_session.config.clone(),
            messages,
        })
    }

    pub fn inherited_message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn build_child_session_config(&self, max_turns_override: Option<usize>) -> SessionConfig {
        let mut config = self.session_config.clone();
        config.workspace_path = Some(self.workspace_path.clone());
        config.remote_connection_id = self.remote_connection_id.clone();
        config.remote_ssh_host = self.remote_ssh_host.clone();
        config.storage_scope = self.storage_scope;
        config.model_id = self.session_model_id.clone();
        if let Some(max_turns) = max_turns_override {
            config.max_turns = max_turns;
        }
        config
    }

    pub fn compose_initial_messages(&self, prompt_messages: &[Message]) -> Vec<Message> {
        let mut messages = self.messages.clone();
        messages.extend(prompt_messages.iter().cloned());
        messages
    }
}

/// Semantic fork request.
#[derive(Debug, Clone)]
pub struct ForkExecutionRequest {
    pub snapshot: ForkContextSnapshot,
    pub agent_type: String,
    pub description: String,
    pub prompt_messages: Vec<Message>,
    pub context: HashMap<String, String>,
    pub max_turns: Option<usize>,
}

impl ForkExecutionRequest {
    pub fn composed_initial_messages(&self) -> Vec<Message> {
        self.snapshot
            .compose_initial_messages(&self.prompt_messages)
    }

    pub fn child_session_config(&self) -> SessionConfig {
        self.snapshot.build_child_session_config(self.max_turns)
    }
}

/// Result returned by a completed semantic fork.
#[derive(Debug, Clone)]
pub struct ForkExecutionResult {
    pub text: String,
    pub inherited_message_count: usize,
    pub prompt_message_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::core::{Message, Session, SessionConfig, SessionStorageScope};

    fn parent_session() -> Session {
        let config = SessionConfig {
            workspace_path: Some("/workspace/project".to_string()),
            remote_connection_id: Some("remote-1".to_string()),
            remote_ssh_host: Some("prod-box".to_string()),
            storage_scope: Some(SessionStorageScope::AgenticOs),
            model_id: Some("primary".to_string()),
            max_turns: 42,
            ..SessionConfig::default()
        };
        Session::new("Parent".to_string(), "agentic".to_string(), config)
    }

    #[test]
    fn snapshot_composes_parent_and_prompt_messages_in_order() {
        let parent = parent_session();
        let inherited = vec![Message::user("hello".to_string())];
        let prompt = vec![Message::user("fork directive".to_string())];
        let snapshot =
            ForkContextSnapshot::from_parent_session(&parent, inherited.clone()).expect("snapshot");

        let combined = snapshot.compose_initial_messages(&prompt);

        assert_eq!(combined.len(), 2);
        assert!(matches!(
            combined[0].content,
            crate::agentic::core::MessageContent::Text(_)
        ));
        assert_eq!(combined[0].id, inherited[0].id);
        assert_eq!(combined[1].id, prompt[0].id);
    }

    #[test]
    fn snapshot_builds_child_session_config_from_parent() {
        let parent = parent_session();
        let snapshot =
            ForkContextSnapshot::from_parent_session(&parent, Vec::new()).expect("snapshot");

        let child_config = snapshot.build_child_session_config(Some(7));

        assert_eq!(
            child_config.workspace_path.as_deref(),
            Some("/workspace/project")
        );
        assert_eq!(
            child_config.remote_connection_id.as_deref(),
            Some("remote-1")
        );
        assert_eq!(child_config.remote_ssh_host.as_deref(), Some("prod-box"));
        assert_eq!(
            child_config.storage_scope,
            Some(SessionStorageScope::AgenticOs)
        );
        assert_eq!(child_config.model_id.as_deref(), Some("primary"));
        assert_eq!(child_config.max_turns, 7);
    }
}
