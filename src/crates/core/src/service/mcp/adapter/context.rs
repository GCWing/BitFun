//! MCP context provider
//!
//! Intelligently matches and injects MCP resources into the agent context.

use super::resource::ResourceAdapter;
use crate::service::mcp::protocol::{MCPResource, MCPResourceContent};
use crate::service::mcp::server::MCPServerManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, info, warn};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

/// Context enhancer configuration.
#[derive(Debug, Clone)]
pub struct ContextEnhancerConfig {
    pub min_relevance: f64,    // Minimum relevance score (0-1)
    pub max_resources: usize,  // Max number of resources
    pub max_total_size: usize, // Max total size (bytes)
    pub enable_caching: bool,  // Enable caching
}

impl Default for ContextEnhancerConfig {
    fn default() -> Self {
        Self {
            min_relevance: 0.3,
            max_resources: 10,
            max_total_size: 100 * 1024, // 100KB
            enable_caching: true,
        }
    }
}

/// Context enhancer.
pub struct ContextEnhancer {
    config: ContextEnhancerConfig,
}

impl ContextEnhancer {
    /// Creates a new context enhancer.
    pub fn new(config: ContextEnhancerConfig) -> Self {
        Self { config }
    }

    /// Enhances context.
    pub async fn enhance(
        &self,
        query: &str,
        resources: Vec<(MCPResource, MCPResourceContent)>,
    ) -> BitFunResult<Value> {
        let scored_resources = resources
            .into_iter()
            .map(|(r, c)| {
                let score = ResourceAdapter::calculate_relevance(&r, query);
                (r, c, score)
            })
            .filter(|(_, _, score)| *score >= self.config.min_relevance)
            .collect::<Vec<_>>();

        let mut sorted = scored_resources;
        sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));

        let mut selected = Vec::new();
        let mut total_size = 0;

        for (resource, content, score) in sorted {
            // Only include text content in model context; skip blob-only (binary) resources
            let content_size = content.content.as_ref().map_or(0, |s| s.len());

            if selected.len() >= self.config.max_resources {
                break;
            }

            if total_size + content_size > self.config.max_total_size {
                break;
            }

            // Skip resources with no text content (e.g. video/blob-only)
            if content_size == 0 {
                continue;
            }

            selected.push((resource, content, score));
            total_size += content_size;
        }

        let context_blocks: Vec<Value> = selected
            .into_iter()
            .map(|(r, c, score)| {
                let mut block = ResourceAdapter::to_context_block(&r, Some(&c));
                if let Some(obj) = block.as_object_mut() {
                    obj.insert("relevance_score".to_string(), json!(score));
                }
                block
            })
            .collect();

        Ok(json!({
            "type": "mcp_context",
            "resources": context_blocks,
            "total_size": total_size,
            "query": query,
        }))
    }
}

impl Default for ContextEnhancer {
    fn default() -> Self {
        Self::new(ContextEnhancerConfig::default())
    }
}

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

        let result = connection.list_resources(None).await?;

        let relevant = ResourceAdapter::filter_and_rank(
            result.resources,
            query,
            0.1, // Lower threshold; we do additional filtering later
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
                if let Ok(result) = connection.list_prompts(None).await {
                    for prompt in result.prompts {
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
        }

        Ok(enhancements)
    }
}
