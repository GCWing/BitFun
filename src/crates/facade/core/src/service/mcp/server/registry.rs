//! MCP server registry
//!
//! Manages registration and lookup for all MCP servers.

use super::{MCPServerConfig, MCPServerProcess};
use log::info;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP server registry.
pub struct MCPServerRegistry {
    servers: Arc<RwLock<HashMap<String, Arc<RwLock<MCPServerProcess>>>>>,
}

impl MCPServerRegistry {
    /// Creates a new registry.
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a server.
    pub async fn register(
        &self,
        config: &MCPServerConfig,
    ) -> crate::util::errors::BitFunResult<()> {
        config.validate()?;

        let process =
            MCPServerProcess::new(config.id.clone(), config.name.clone(), config.server_type);

        let mut servers = self.servers.write().await;
        servers.insert(config.id.clone(), Arc::new(RwLock::new(process)));

        info!(
            "Registered MCP server: name={} id={}",
            config.name, config.id
        );
        Ok(())
    }

    /// Unregisters a server.
    pub async fn unregister(&self, server_id: &str) -> crate::util::errors::BitFunResult<()> {
        let mut servers = self.servers.write().await;

        if let Some(process) = servers.remove(server_id) {
            let mut proc = process.write().await;
            proc.stop().await?;
            info!("Unregistered MCP server: id={}", server_id);
            Ok(())
        } else {
            Err(crate::util::errors::BitFunError::NotFound(format!(
                "MCP server not found: {}",
                server_id
            )))
        }
    }

    /// Gets a server process.
    pub async fn get_process(&self, server_id: &str) -> Option<Arc<RwLock<MCPServerProcess>>> {
        let servers = self.servers.read().await;
        servers.get(server_id).cloned()
    }

    /// Returns all server IDs.
    pub async fn get_all_server_ids(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    /// Returns all server processes.
    pub async fn get_all_processes(&self) -> Vec<Arc<RwLock<MCPServerProcess>>> {
        let servers = self.servers.read().await;
        servers.values().cloned().collect()
    }

    /// Returns whether a server exists.
    pub async fn contains(&self, server_id: &str) -> bool {
        let servers = self.servers.read().await;
        servers.contains_key(server_id)
    }

    /// Clears the registry.
    pub async fn clear(&self) -> crate::util::errors::BitFunResult<()> {
        let mut servers = self.servers.write().await;

        for process in servers.values() {
            let mut proc = process.write().await;
            let _ = proc.stop().await;
        }

        servers.clear();
        info!("Cleared MCP server registry");
        Ok(())
    }
}

impl Default for MCPServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
