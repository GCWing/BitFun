//! MCP server manager
//!
//! The manager is split into focused submodules so lifecycle, reconnect,
//! catalog, interaction, and tool-registration logic can evolve independently.

mod auth;
mod catalog;
mod interaction;
mod lifecycle;
mod reconnect;
#[cfg(test)]
mod tests;
mod tools;

use super::connection::{MCPConnection, MCPConnectionEvent};
use super::{MCPServerConfig, MCPServerStatus};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::service::mcp::adapter::MCPToolAdapter;
use crate::service::mcp::auth::MCPRemoteOAuthSessionSnapshot;
use crate::service::mcp::config::MCPConfigService;
use crate::service::mcp::protocol::{MCPError, MCPPrompt, MCPResource};
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_services_integrations::mcp::server::MCPServerRuntimeState;
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

#[derive(Debug)]
enum MCPInteractionDecision {
    Accept { result: Value },
    Reject { error: MCPError },
}

#[derive(Debug)]
struct PendingMCPInteraction {
    sender: oneshot::Sender<MCPInteractionDecision>,
}

struct ActiveRemoteOAuthSession {
    snapshot: Arc<tokio::sync::RwLock<MCPRemoteOAuthSessionSnapshot>>,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

/// MCP server manager.
#[derive(Clone)]
pub struct MCPServerManager {
    config_service: Arc<MCPConfigService>,
    runtime: Arc<MCPServerRuntimeState>,
    reconnect_monitor_started: Arc<AtomicBool>,
    connection_event_tasks: Arc<tokio::sync::RwLock<HashMap<String, JoinHandle<()>>>>,
    pending_interactions: Arc<tokio::sync::RwLock<HashMap<String, PendingMCPInteraction>>>,
    oauth_sessions: Arc<tokio::sync::RwLock<HashMap<String, Arc<ActiveRemoteOAuthSession>>>>,
}

impl MCPServerManager {
    /// Creates a new server manager.
    pub fn new(config_service: Arc<MCPConfigService>) -> Self {
        Self {
            config_service,
            runtime: Arc::new(MCPServerRuntimeState::new()),
            reconnect_monitor_started: Arc::new(AtomicBool::new(false)),
            connection_event_tasks: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pending_interactions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            oauth_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}
