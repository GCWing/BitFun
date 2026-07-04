//! MCP server runtime state owner.
//!
//! This type groups reusable MCP runtime state that is independent from product
//! assembly side effects such as global tool registration, frontend events, and
//! OAuth callback UI.

use super::{
    MCPCatalogCache, MCPConnection, MCPConnectionPool, MCPReconnectTracker, MCPRuntimeResult,
    MCPServerConfig, MCPServerProcess, MCPServerRegistry, MCPServerStatus,
};
use crate::mcp::protocol::{MCPPrompt, MCPResource};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

pub struct MCPServerRuntimeState {
    registry: MCPServerRegistry,
    connection_pool: MCPConnectionPool,
    reconnect_tracker: MCPReconnectTracker,
    catalog_cache: MCPCatalogCache,
}

impl MCPServerRuntimeState {
    pub fn new() -> Self {
        Self {
            registry: MCPServerRegistry::new(),
            connection_pool: MCPConnectionPool::new(),
            reconnect_tracker: MCPReconnectTracker::default(),
            catalog_cache: MCPCatalogCache::new(),
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.registry.get_all_server_ids().await.is_empty()
    }

    pub async fn contains(&self, server_id: &str) -> bool {
        self.registry.contains(server_id).await
    }

    pub async fn register(&self, config: &MCPServerConfig) -> MCPRuntimeResult<()> {
        self.registry.register(config).await
    }

    pub async fn unregister(&self, server_id: &str) -> MCPRuntimeResult<()> {
        self.registry.unregister(server_id).await
    }

    pub async fn clear_registry(&self) -> MCPRuntimeResult<()> {
        self.registry.clear().await
    }

    pub async fn get_process(&self, server_id: &str) -> Option<Arc<RwLock<MCPServerProcess>>> {
        self.registry.get_process(server_id).await
    }

    pub async fn get_all_server_ids(&self) -> Vec<String> {
        self.registry.get_all_server_ids().await
    }

    async fn get_all_processes(&self) -> Vec<Arc<RwLock<MCPServerProcess>>> {
        self.registry.get_all_processes().await
    }

    pub async fn insert_runtime_config(&self, config: MCPServerConfig) -> MCPRuntimeResult<()> {
        self.registry.insert_runtime_config(config).await
    }

    pub async fn get_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.registry.get_runtime_config(server_id).await
    }

    pub async fn remove_runtime_config(&self, server_id: &str) -> Option<MCPServerConfig> {
        self.registry.remove_runtime_config(server_id).await
    }

    pub async fn add_connection(&self, server_id: String, connection: Arc<MCPConnection>) {
        self.connection_pool
            .add_connection(server_id, connection)
            .await;
    }

    pub async fn get_connection(&self, server_id: &str) -> Option<Arc<MCPConnection>> {
        self.connection_pool.get_connection(server_id).await
    }

    pub async fn remove_connection(&self, server_id: &str) {
        self.connection_pool.remove_connection(server_id).await;
    }

    pub fn reconnect_poll_interval(&self) -> Duration {
        self.reconnect_tracker.poll_interval()
    }

    pub async fn has_pending_reconnects(&self) -> bool {
        self.reconnect_tracker.has_pending().await
    }

    pub async fn next_due_reconnect_attempt(&self, server_id: &str) -> Option<(u32, Duration)> {
        self.reconnect_tracker.next_due_attempt(server_id).await
    }

    pub async fn clear_reconnect_state(&self, server_id: &str) {
        self.reconnect_tracker.clear(server_id).await;
    }

    pub async fn clear_all_reconnect_state(&self) {
        self.reconnect_tracker.clear_all().await;
    }

    pub async fn refresh_resources(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        self.catalog_cache
            .refresh_resources(server_id, connection)
            .await
    }

    pub async fn refresh_prompts(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        self.catalog_cache
            .refresh_prompts(server_id, connection)
            .await
    }

    pub async fn warm_catalog(&self, server_id: &str, connection: Arc<MCPConnection>) {
        self.catalog_cache.warm(server_id, connection).await;
    }

    pub async fn get_cached_resources(&self, server_id: &str) -> Vec<MCPResource> {
        self.catalog_cache.get_resources(server_id).await
    }

    pub async fn get_cached_prompts(&self, server_id: &str) -> Vec<MCPPrompt> {
        self.catalog_cache.get_prompts(server_id).await
    }

    pub async fn remove_catalog(&self, server_id: &str) {
        self.catalog_cache.remove_server(server_id).await;
    }

    pub async fn clear_catalog(&self) {
        self.catalog_cache.clear().await;
    }

    pub async fn get_all_statuses(&self) -> Vec<(String, MCPServerStatus)> {
        let processes = self.get_all_processes().await;
        let mut statuses = Vec::new();

        for process in processes {
            let proc = process.read().await;
            let id = proc.id().to_string();
            let status = proc.status().await;
            statuses.push((id, status));
        }

        statuses
    }
}

impl Default for MCPServerRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}
