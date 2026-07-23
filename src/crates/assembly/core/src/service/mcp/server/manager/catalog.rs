use super::*;

impl MCPServerManager {
    pub(super) async fn refresh_resources_catalog(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        self.runtime
            .refresh_resources(server_id, connection)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn refresh_prompts_catalog(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) -> BitFunResult<usize> {
        self.runtime
            .refresh_prompts(server_id, connection)
            .await
            .map_err(Into::into)
    }

    pub(super) async fn warm_catalog_caches(
        &self,
        server_id: &str,
        connection: Arc<MCPConnection>,
    ) {
        self.runtime.warm_catalog(server_id, connection).await;
    }

    /// Returns cached MCP resources for a server.
    pub async fn get_cached_resources(&self, server_id: &str) -> Vec<MCPResource> {
        self.runtime.get_cached_resources(server_id).await
    }

    /// Returns cached MCP prompts for a server.
    pub async fn get_cached_prompts(&self, server_id: &str) -> Vec<MCPPrompt> {
        self.runtime.get_cached_prompts(server_id).await
    }

    /// Refreshes resources catalog cache for one server.
    pub async fn refresh_server_resource_catalog(&self, server_id: &str) -> BitFunResult<usize> {
        let connection = self.get_connection(server_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server connection not found: {}", server_id))
        })?;
        self.refresh_resources_catalog(server_id, connection).await
    }

    /// Refreshes prompts catalog cache for one server.
    pub async fn refresh_server_prompt_catalog(&self, server_id: &str) -> BitFunResult<usize> {
        let connection = self.get_connection(server_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server connection not found: {}", server_id))
        })?;
        self.refresh_prompts_catalog(server_id, connection).await
    }
}
