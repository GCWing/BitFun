//! ACP Session Management
//!
//! Manages ACP sessions and maps them to BitFun sessions.

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;

/// ACP Session Manager
///
/// Maps ACP session IDs to BitFun session IDs
pub struct AcpSessionManager {
    /// ACP session ID -> BitFun session ID mapping
    sessions: Arc<DashMap<String, AcpSession>>,
}

/// ACP Session metadata
#[derive(Debug, Clone)]
pub struct AcpSession {
    /// ACP session ID (used in ACP protocol)
    pub acp_session_id: String,
    /// BitFun session ID
    pub bitfun_session_id: String,
    /// Working directory
    pub cwd: String,
    /// Client capabilities
    pub client_capabilities: crate::acp::protocol::ClientCapabilities,
}

impl AcpSessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Create a new ACP session
    pub fn create_session(
        &self,
        cwd: String,
        client_capabilities: crate::acp::protocol::ClientCapabilities,
    ) -> Result<AcpSession> {
        let acp_session_id = uuid::Uuid::new_v4().to_string();
        let bitfun_session_id = uuid::Uuid::new_v4().to_string();

        let session = AcpSession {
            acp_session_id: acp_session_id.clone(),
            bitfun_session_id,
            cwd,
            client_capabilities,
        };

        self.sessions
            .insert(acp_session_id.clone(), session.clone());

        tracing::info!(
            "Created ACP session: acp_id={}, bitfun_id={}",
            session.acp_session_id,
            session.bitfun_session_id
        );

        Ok(session)
    }

    /// Get an ACP session by ID
    pub fn get_session(&self, acp_session_id: &str) -> Option<AcpSession> {
        self.sessions.get(acp_session_id).map(|s| s.clone())
    }

    /// Remove an ACP session
    pub fn remove_session(&self, acp_session_id: &str) -> Option<AcpSession> {
        self.sessions
            .remove(acp_session_id)
            .map(|(_, session)| session)
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<AcpSession> {
        self.sessions.iter().map(|s| s.clone()).collect()
    }

    /// Update the BitFun session ID for an ACP session
    pub fn update_bitfun_session_id(
        &self,
        acp_session_id: &str,
        bitfun_session_id: String,
    ) -> Option<AcpSession> {
        self.sessions.get_mut(acp_session_id).map(|mut mut_ref| {
            mut_ref.bitfun_session_id = bitfun_session_id;
            mut_ref.clone()
        })
    }
}

impl Default for AcpSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
