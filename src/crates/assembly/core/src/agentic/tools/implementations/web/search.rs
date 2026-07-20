use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_services_integrations::web_tools::{ExaSearchRequest, WebToolNetworkProvider};
use log::{error, info};
use serde_json::{json, Value};
use tool_runtime::web_search::{parse_exa_text_results, WebSearchResult};

const EXA_RESULTS: u64 = 5;
const EXA_CONTEXT: u64 = 8_000;

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

    async fn search(
        &self,
        query: &str,
        num: u64,
        kind: &str,
        crawl: &str,
        ctx: u64,
    ) -> BitFunResult<String> {
        WebToolNetworkProvider::search_exa(ExaSearchRequest {
            query,
            num_results: num,
            kind,
            livecrawl: crawl,
            context_max_characters: ctx,
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
        "snippet": result.snippet,
    })
}

pub(super) fn build_web_search_tool_result(query: &str, results: Vec<Value>) -> ToolResult {
    let formatted_results = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            format!(
                "{}. {}\n   URL: {}\n   Snippet: {}\n",
                i + 1,
                r["title"].as_str().unwrap_or("Untitled"),
                r["url"].as_str().unwrap_or(""),
                r["snippet"].as_str().unwrap_or("")
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
        Ok(
            r#"- Allows BitFun to search the web and use the results to inform responses
- Provides up-to-date information for current events and recent data
- Returns search result information formatted as search result blocks
- Use this tool for accessing information beyond BitFun's knowledge cutoff

Usage notes:
- Use when you need current information not in training data
- Effective for recent news, current events, product updates, or real-time data
- Search queries should be specific and well-targeted for best results
- Results include title, URL, snippet and source information

Advanced features:
- Choose search depth: auto, fast, or deep
- Control result count and context size for LLM-friendly output
- Optionally prefer live crawling for fresher pages
- Return up to 10 results per query"#
                .to_string(),
        )
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
                    "description": "Number of search results to return (1-10, default: 5)",
                    "default": EXA_RESULTS,
                    "minimum": 1,
                    "maximum": 10
                },
                "type": {
                    "type": "string",
                    "enum": ["auto", "fast", "deep"],
                    "description": "Search depth. Use 'auto' for balanced results, 'fast' for lower latency, or 'deep' for broader context.",
                    "default": "auto"
                },
                "livecrawl": {
                    "type": "string",
                    "enum": ["fallback", "preferred"],
                    "description": "Live crawl mode. Use 'preferred' to favor fresh crawling, or 'fallback' to use cached data when possible.",
                    "default": "fallback"
                },
                "context_max_characters": {
                    "type": "number",
                    "description": "Maximum characters of search context to request (default: 8000)",
                    "default": EXA_CONTEXT,
                    "minimum": 1000,
                    "maximum": 20000
                }
            },
            "required": ["query"]
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
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
            .clamp(1, 10);

        let kind = input.get("type").and_then(|v| v.as_str()).unwrap_or("auto");

        let crawl = input
            .get("livecrawl")
            .and_then(|v| v.as_str())
            .unwrap_or("fallback");

        let ctx = input
            .get("context_max_characters")
            .and_then(|v| v.as_u64())
            .unwrap_or(EXA_CONTEXT)
            .clamp(1_000, 20_000);

        info!(
            "WebSearch Exa call: query='{}', num_results={}, type={}, livecrawl={}, context_max_characters={}",
            query, num_results, kind, crawl, ctx
        );

        let raw = self.search(query, num_results, kind, crawl, ctx).await?;
        let results = self.results(&raw);
        Ok(vec![build_web_search_tool_result(query, results)])
    }
}
