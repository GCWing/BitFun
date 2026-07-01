//! MCP server catalog cache state.

use super::connection::MCPConnection;
use crate::mcp::protocol::{MCPPrompt, MCPResource};
use crate::mcp::MCPRuntimeResult;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
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

    pub async fn refresh_resources(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        let mut resources = Vec::new();
        let mut cursor = None::<String>;
        let mut visited = HashSet::new();

        loop {
            let result = connection.list_resources(cursor.clone()).await?;
            resources.extend(result.resources);

            match result.next_cursor {
                Some(next) => {
                    if !visited.insert(next.clone()) {
                        break;
                    }
                    cursor = Some(next);
                }
                None => break,
            }
        }

        let count = resources.len();
        self.replace_resources(server_id, resources).await;
        Ok(count)
    }

    pub async fn refresh_prompts(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> MCPRuntimeResult<usize> {
        let mut prompts = Vec::new();
        let mut cursor = None::<String>;
        let mut visited = HashSet::new();

        loop {
            let result = connection.list_prompts(cursor.clone()).await?;
            prompts.extend(result.prompts);

            match result.next_cursor {
                Some(next) => {
                    if !visited.insert(next.clone()) {
                        break;
                    }
                    cursor = Some(next);
                }
                None => break,
            }
        }

        let count = prompts.len();
        self.replace_prompts(server_id, prompts).await;
        Ok(count)
    }

    pub async fn warm(&self, server_id: &str, connection: Arc<MCPConnection>) {
        if let Err(error) = self.refresh_resources(server_id, connection.clone()).await {
            log::debug!(
                "Skipping MCP resources catalog warmup: server_id={} error={}",
                server_id,
                error
            );
        }

        if let Err(error) = self.refresh_prompts(server_id, connection).await {
            log::debug!(
                "Skipping MCP prompts catalog warmup: server_id={} error={}",
                server_id,
                error
            );
        }
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
