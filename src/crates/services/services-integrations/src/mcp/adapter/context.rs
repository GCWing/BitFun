//! MCP resource context enhancement helpers.

use crate::mcp::adapter::ResourceAdapter;
use crate::mcp::protocol::{MCPResource, MCPResourceContent};
use crate::mcp::MCPRuntimeResult;
use serde_json::{json, Value};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct MCPContextEnhancerConfig {
    pub min_relevance: f64,
    pub max_resources: usize,
    pub max_total_size: usize,
    pub enable_caching: bool,
}

impl Default for MCPContextEnhancerConfig {
    fn default() -> Self {
        Self {
            min_relevance: 0.3,
            max_resources: 10,
            max_total_size: 100 * 1024,
            enable_caching: true,
        }
    }
}

pub struct MCPContextEnhancer {
    config: MCPContextEnhancerConfig,
}

impl MCPContextEnhancer {
    pub fn new(config: MCPContextEnhancerConfig) -> Self {
        Self { config }
    }

    pub async fn enhance(
        &self,
        query: &str,
        resources: Vec<(MCPResource, MCPResourceContent)>,
    ) -> MCPRuntimeResult<Value> {
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
            let content_size = content.content.as_ref().map_or(0, |s| s.len());

            if selected.len() >= self.config.max_resources {
                break;
            }

            if total_size + content_size > self.config.max_total_size {
                break;
            }

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

impl Default for MCPContextEnhancer {
    fn default() -> Self {
        Self::new(MCPContextEnhancerConfig::default())
    }
}
