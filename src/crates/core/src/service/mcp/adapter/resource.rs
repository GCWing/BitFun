//! MCP resource adapter
//!
//! Converts MCP resources into usable context content.

use crate::service::mcp::protocol::{MCPResource, MCPResourceContent};
use serde_json::{json, Value};
use std::cmp::Ordering;

/// Resource adapter.
pub struct ResourceAdapter;

impl ResourceAdapter {
    /// Converts an MCP resource into a context block.
    pub fn to_context_block(resource: &MCPResource, content: Option<&MCPResourceContent>) -> Value {
        let content_value = content.and_then(|c| c.content.as_ref());
        let display_name = resource.title.as_ref().unwrap_or(&resource.name);
        json!({
            "type": "resource",
            "uri": resource.uri,
            "name": resource.name,
            "title": resource.title,
            "displayName": display_name,
            "description": resource.description,
            "mimeType": resource.mime_type,
            "size": resource.size,
            "content": content_value,
            "metadata": resource.metadata,
        })
    }

    /// Converts MCP resource content to plain text. Binary (blob) content is summarized.
    pub fn to_text(content: &MCPResourceContent) -> String {
        let text = content
            .content
            .as_deref()
            .unwrap_or_else(|| content.blob.as_ref().map_or("(empty)", |_| "(binary content)"));
        format!("Resource: {}\n\n{}\n", content.uri, text)
    }

    /// Calculates a resource relevance score (0-1).
    pub fn calculate_relevance(resource: &MCPResource, query: &str) -> f64 {
        let query_lower = query.to_lowercase();
        let mut score: f64 = 0.0;

        if resource.uri.to_lowercase().contains(&query_lower) {
            score += 0.3;
        }

        if resource.name.to_lowercase().contains(&query_lower) {
            score += 0.4;
        }

        if let Some(desc) = &resource.description {
            if desc.to_lowercase().contains(&query_lower) {
                score += 0.3;
            }
        }

        score.min(1.0)
    }

    /// Filters and ranks resources.
    pub fn filter_and_rank(
        resources: Vec<MCPResource>,
        query: &str,
        min_relevance: f64,
        max_results: usize,
    ) -> Vec<(MCPResource, f64)> {
        let mut scored_resources: Vec<(MCPResource, f64)> = resources
            .into_iter()
            .map(|r| {
                let score = Self::calculate_relevance(&r, query);
                (r, score)
            })
            .filter(|(_, score)| *score >= min_relevance)
            .collect();

        scored_resources.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        scored_resources.truncate(max_results);

        scored_resources
    }
}
