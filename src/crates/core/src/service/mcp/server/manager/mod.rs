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

use super::connection::{MCPConnection, MCPConnectionEvent, MCPConnectionPool};
use super::{MCPServerConfig, MCPServerRegistry, MCPServerStatus};
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::service::mcp::adapter::MCPToolAdapter;
use crate::service::mcp::auth::MCPRemoteOAuthSessionSnapshot;
use crate::service::mcp::config::MCPConfigService;
use crate::service::mcp::protocol::{MCPError, MCPPrompt, MCPResource};
use crate::service::runtime::{RuntimeManager, RuntimeSource};
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Reconnect policy for unhealthy MCP servers.
#[derive(Debug, Clone, Copy)]
struct ReconnectPolicy {
    poll_interval: Duration,
    base_delay: Duration,
    max_delay: Duration,
    max_attempts: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            max_attempts: 6,
        }
    }
}

#[derive(Debug, Clone)]
struct ReconnectAttemptState {
    attempts: u32,
    next_retry_at: Instant,
    exhausted_logged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListChangedKind {
    Tools,
    Prompts,
    Resources,
}

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

impl ReconnectAttemptState {
    fn new(now: Instant) -> Self {
        Self {
            attempts: 0,
            next_retry_at: now,
            exhausted_logged: false,
        }
    }
}

/// MCP server manager.
#[derive(Clone)]
pub struct MCPServerManager {
    registry: Arc<MCPServerRegistry>,
    connection_pool: Arc<MCPConnectionPool>,
    config_service: Arc<MCPConfigService>,
    reconnect_policy: ReconnectPolicy,
    reconnect_states: Arc<tokio::sync::RwLock<HashMap<String, ReconnectAttemptState>>>,
    reconnect_monitor_started: Arc<AtomicBool>,
    connection_event_tasks: Arc<tokio::sync::RwLock<HashMap<String, JoinHandle<()>>>>,
    resource_catalog_cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<MCPResource>>>>,
    prompt_catalog_cache: Arc<tokio::sync::RwLock<HashMap<String, Vec<MCPPrompt>>>>,
    pending_interactions: Arc<tokio::sync::RwLock<HashMap<String, PendingMCPInteraction>>>,
    oauth_sessions: Arc<tokio::sync::RwLock<HashMap<String, Arc<ActiveRemoteOAuthSession>>>>,
    ephemeral_configs: Arc<tokio::sync::RwLock<HashMap<String, MCPServerConfig>>>,
}

impl MCPServerManager {
    /// Creates a new server manager.
    pub fn new(config_service: Arc<MCPConfigService>) -> Self {
        Self {
            registry: Arc::new(MCPServerRegistry::new()),
            connection_pool: Arc::new(MCPConnectionPool::new()),
            config_service,
            reconnect_policy: ReconnectPolicy::default(),
            reconnect_states: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            reconnect_monitor_started: Arc::new(AtomicBool::new(false)),
            connection_event_tasks: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            resource_catalog_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            prompt_catalog_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            pending_interactions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            oauth_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            ephemeral_configs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}
