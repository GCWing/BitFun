//! MCP server catalog cache state.

use crate::mcp::protocol::{MCPPrompt, MCPResource};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Caches MCP resources and prompts by server id.
pub struct MCPCatalogCache {
    resources: RwLock<HashMap<String, Vec<MCPResource>>>,
    prompts: RwLock<HashMap<String, Vec<MCPPrompt>>>,
}

impl MCPCatalogCache {
    pub fn new() -> Self {
        Self {
            resources: RwLock::new(HashMap::new()),
            prompts: RwLock::new(HashMap::new()),
        }
    }

    pub async fn replace_resources(&self, server_id: &str, resources: Vec<MCPResource>) {
        self.resources
            .write()
            .await
            .insert(server_id.to_string(), resources);
    }

    pub async fn replace_prompts(&self, server_id: &str, prompts: Vec<MCPPrompt>) {
        self.prompts
            .write()
            .await
            .insert(server_id.to_string(), prompts);
    }

    pub async fn get_resources(&self, server_id: &str) -> Vec<MCPResource> {
        self.resources
            .read()
            .await
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn get_prompts(&self, server_id: &str) -> Vec<MCPPrompt> {
        self.prompts
            .read()
            .await
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn remove_server(&self, server_id: &str) {
        self.resources.write().await.remove(server_id);
        self.prompts.write().await.remove(server_id);
    }

    pub async fn clear(&self) {
        self.resources.write().await.clear();
        self.prompts.write().await.clear();
    }
}

impl Default for MCPCatalogCache {
    fn default() -> Self {
        Self::new()
    }
}
