use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_services_integrations::web_tools::{ExaSearchRequest, WebToolNetworkProvider};
use log::{error, info};
use serde_json::{json, Value};
use tool_runtime::web_search::{parse_exa_text_results, WebSearchResult};

const EXA_RESULTS: u64 = 10;
const EXA_MAX_RESULTS: u64 = 20;

pub struct WebSearchTool;

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }

    async fn search(&self, query: &str, num: u64) -> BitFunResult<String> {
        WebToolNetworkProvider::search_exa(ExaSearchRequest {
            query,
            num_results: num,
        })
        .await
        .map_err(|error| {
            error!("WebSearch Exa error: {}", error);
            BitFunError::tool(error.to_string())
        })
    }

    pub(crate) fn results(&self, text: &str) -> Vec<Value> {
        parse_exa_text_results(text)
            .into_iter()
            .map(search_result_to_value)
            .collect()
    }
}

fn search_result_to_value(result: WebSearchResult) -> Value {
    json!({
        "title": result.title,
        "url": result.url,
        "published": result.published,
        "author": result.author,
    })
}

pub(super) fn build_web_search_tool_result(query: &str, results: Vec<Value>) -> ToolResult {
    let formatted_results = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "{}. {}\n   URL: {}\n   Published: {}\n   Author: {}\n",
                i + 1,
                r["title"].as_str().unwrap_or("Untitled"),
                r["url"].as_str().unwrap_or(""),
                r["published"].as_str().unwrap_or(""),
                r["author"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    ToolResult::Result {
        data: json!({
            "query": query,
            "results": results,
            "result_count": results.len(),
            "provider": "exa_mcp"
        }),
        result_for_assistant: Some(format!(
            "Search query: '{}'\nFound {} results:\n\n{}",
            query,
            results.len(),
            formatted_results
        )),
        image_attachments: None,
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Search the web for up-to-date information and sources.".to_string())
    }

    fn short_description(&self) -> String {
        "Search the web for up-to-date information and sources.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (recommended max 70 characters)"
                },
                "num_results": {
                    "type": "number",
                    "description": "Number of search results to return (1-20, default: 10)"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let query = input
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .ok_or_else(|| BitFunError::validation("query is required".to_string()))?;
        Ok(vec![PermissionIntent::new(
            "websearch",
            vec![query.to_string()],
        )])
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("query is required".to_string()))?;

        let num_results = input
            .get("num_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(EXA_RESULTS)
            .clamp(1, EXA_MAX_RESULTS);

        info!(
            "WebSearch Exa call: query='{}', num_results={}",
            query, num_results
        );

        let raw = self.search(query, num_results).await?;
        let results = self.results(&raw);
        Ok(vec![build_web_search_tool_result(query, results)])
    }
}
