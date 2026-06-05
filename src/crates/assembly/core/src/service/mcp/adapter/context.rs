//! MCP context provider
//!
//! Intelligently matches and injects MCP resources into the agent context.

use super::resource::ResourceAdapter;
use crate::service::mcp::protocol::{MCPResource, MCPResourceContent};
use crate::service::mcp::server::MCPServerManager;
use crate::util::errors::{BitFunError, BitFunResult};
pub use bitfun_services_integrations::mcp::adapter::{
    MCPContextEnhancer as ContextEnhancer, MCPContextEnhancerConfig as ContextEnhancerConfig,
};
use log::{debug, info, warn};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// MCP context provider.
pub struct MCPContextProvider {
    server_manager: Arc<MCPServerManager>,
    enhancer: ContextEnhancer,
}

impl MCPContextProvider {
    /// Creates a new context provider.
    pub fn new(server_manager: Arc<MCPServerManager>) -> Self {
        Self {
            server_manager,
            enhancer: ContextEnhancer::default(),
        }
    }

    /// Creates with a custom configuration.
    pub fn with_config(
        server_manager: Arc<MCPServerManager>,
        config: ContextEnhancerConfig,
    ) -> Self {
        Self {
            server_manager,
            enhancer: ContextEnhancer::new(config),
        }
    }

    /// Queries relevant resources.
    pub async fn query_resources(
        &self,
        query: &str,
        server_ids: Option<Vec<String>>,
    ) -> BitFunResult<Vec<(MCPResource, MCPResourceContent)>> {
        let mut all_resources = Vec::new();

        let server_ids = server_ids.unwrap_or_else(|| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { self.server_manager.get_all_server_ids().await })
            })
        });

        let mut tasks = Vec::new();

        for server_id in server_ids {
            let manager = self.server_manager.clone();
            let query = query.to_string();

            let task = tokio::spawn(async move {
                Self::query_server_resources(&manager, &server_id, &query).await
            });

            tasks.push(task);
        }

        for task in tasks {
            if let Ok(Ok(resources)) = task.await {
                all_resources.extend(resources);
            }
        }

        Ok(all_resources)
    }

    /// Queries resources from a single server.
    async fn query_server_resources(
        manager: &MCPServerManager,
        server_id: &str,
        query: &str,
    ) -> BitFunResult<Vec<(MCPResource, MCPResourceContent)>> {
        let connection = manager.get_connection(server_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("MCP server connection not found: {}", server_id))
        })?;

        let mut resources = manager.get_cached_resources(server_id).await;
        if resources.is_empty() {
            if let Err(e) = manager.refresh_server_resource_catalog(server_id).await {
                debug!(
                    "Failed to refresh resources catalog cache; falling back to direct list: server_id={} error={}",
                    server_id, e
                );
            }
            resources = manager.get_cached_resources(server_id).await;
        }

        if resources.is_empty() {
            resources = connection.list_resources(None).await?.resources;
        }

        let relevant = ResourceAdapter::filter_and_rank(
            resources, query, 0.1, // Lower threshold; we do additional filtering later
            50,  // Up to 50 per server
        );

        let mut resources_with_content = Vec::new();

        for (resource, _score) in relevant {
            match connection.read_resource(&resource.uri).await {
                Ok(read_result) => {
                    if let Some(content) = read_result.contents.first() {
                        resources_with_content.push((resource, content.clone()));
                    }
                }
                Err(e) => {
                    warn!("Failed to read MCP resource {}: {}", resource.uri, e);
                }
            }
        }

        Ok(resources_with_content)
    }

    /// Enhances agent context.
    pub async fn enhance_context(
        &self,
        query: &str,
        existing_context: Option<Value>,
        server_ids: Option<Vec<String>>,
    ) -> BitFunResult<Value> {
        let resources = self.query_resources(query, server_ids).await?;

        if resources.is_empty() {
            debug!("No relevant MCP resources found for query: {}", query);
            return Ok(existing_context.unwrap_or(json!({})));
        }

        info!("Found {} relevant MCP resource(s)", resources.len());

        let mcp_context = self.enhancer.enhance(query, resources).await?;

        if let Some(mut ctx) = existing_context {
            if let Some(obj) = ctx.as_object_mut() {
                obj.insert("mcp_resources".to_string(), mcp_context);
            }
            Ok(ctx)
        } else {
            Ok(json!({
                "mcp_resources": mcp_context
            }))
        }
    }

    /// Gets prompt enhancements.
    pub async fn get_prompt_enhancements(
        &self,
        prompt_names: Vec<String>,
        arguments: HashMap<String, String>,
    ) -> BitFunResult<Vec<String>> {
        let mut enhancements = Vec::new();
        let server_ids = self.server_manager.get_all_server_ids().await;

        for server_id in server_ids {
            if let Some(connection) = self.server_manager.get_connection(&server_id).await {
                let mut prompts = self.server_manager.get_cached_prompts(&server_id).await;
                if prompts.is_empty() {
                    let _ = self
                        .server_manager
                        .refresh_server_prompt_catalog(&server_id)
                        .await;
                    prompts = self.server_manager.get_cached_prompts(&server_id).await;
                }
                if prompts.is_empty() {
                    if let Ok(result) = connection.list_prompts(None).await {
                        prompts = result.prompts;
                    }
                }

                for prompt in prompts {
                    if prompt_names.contains(&prompt.name) {
                        if let Ok(content) = connection
                            .get_prompt(&prompt.name, Some(arguments.clone()))
                            .await
                        {
                            let text = super::prompt::PromptAdapter::to_system_prompt(
                                &crate::service::mcp::protocol::MCPPromptContent {
                                    name: prompt.name.clone(),
                                    messages: content.messages,
                                },
                            );
                            enhancements.push(text);
                        }
                    }
                }
            }
        }

        Ok(enhancements)
    }
}
