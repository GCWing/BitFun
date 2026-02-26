//! Web tool implementation - WebSearchTool and URLFetcherTool

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::infrastructure::get_path_manager_arc;
use crate::service::config::types::GlobalConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;

/// ZhipuAI Web Search API response
#[derive(Debug, Deserialize)]
struct ZhipuSearchResponse {
    search_result: Vec<ZhipuSearchResult>,
}

#[derive(Debug, Deserialize)]
struct ZhipuSearchResult {
    title: String,
    content: String,
    link: String,
    #[serde(default)]
    media: String,
}

/// Search API configuration
#[derive(Debug, Clone)]
struct SearchApiConfig {
    api_key: String,
    base_url: String,
    model_name: String,
}

/// Web search tool - supports reading API configuration from config file or environment variables
pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }

    /// Load search API configuration from config file
    async fn load_search_config(&self) -> Option<SearchApiConfig> {
        // 1. Prefer environment variables
        if let Ok(api_key) = env::var("ZHIPU_API_KEY").or_else(|_| env::var("API_KEY")) {
            info!("WebSearchTool: Loaded API key from environment variable");
            return Some(SearchApiConfig {
                api_key,
                base_url: "https://open.bigmodel.cn/api/paas/v4/web_search".to_string(),
                model_name: "ZhipuAI Web Search".to_string(),
            });
        }

        // 2. Read from config file
        match self.load_config_from_file().await {
            Ok(Some(config)) => {
                info!(
                    "WebSearchTool: Loaded search API config from file: {}",
                    config.model_name
                );
                Some(config)
            }
            Ok(None) => {
                warn!("WebSearchTool: Search API not configured, will return mock data");
                None
            }
            Err(e) => {
                warn!(
                    "WebSearchTool: Failed to load config, will return mock data: {}",
                    e
                );
                None
            }
        }
    }

    /// Load from config file
    async fn load_config_from_file(&self) -> BitFunResult<Option<SearchApiConfig>> {
        // Get config file path
        let path_manager = get_path_manager_arc();
        let config_file = path_manager.app_config_file();

        if !config_file.exists() {
            debug!("Config file does not exist: {:?}", config_file);
            return Ok(None);
        }

        // Read and parse config file
        let config_content = fs::read_to_string(&config_file)
            .map_err(|e| BitFunError::tool(format!("Failed to read config file: {}", e)))?;

        let global_config: GlobalConfig = serde_json::from_str(&config_content)
            .map_err(|e| BitFunError::tool(format!("Failed to parse config file: {}", e)))?;

        // Get search model ID
        let search_model_id = match global_config.ai.default_models.search {
            Some(id) if !id.is_empty() => id,
            _ => {
                debug!("Search model not configured");
                return Ok(None);
            }
        };

        // Find corresponding model configuration
        let model_config = global_config
            .ai
            .models
            .iter()
            .find(|m| m.id == search_model_id)
            .ok_or_else(|| {
                BitFunError::tool(format!(
                    "Search model config not found: {}",
                    search_model_id
                ))
            })?;

        // Validate API Key
        if model_config.api_key.trim().is_empty() {
            warn!("Search model API key not configured: {}", model_config.name);
            return Ok(None);
        }

        Ok(Some(SearchApiConfig {
            api_key: model_config.api_key.clone(),
            base_url: model_config.base_url.clone(),
            model_name: model_config.name.clone(),
        }))
    }

    /// Call search API (supports ZhipuAI and other compatible search APIs)
    async fn search_api(
        &self,
        config: &SearchApiConfig,
        query: &str,
        count: u64,
        search_engine: &str,
        content_size: &str,
        recency_filter: &str,
    ) -> BitFunResult<Vec<Value>> {
        info!(
            "Search API call: model={}, query={}, engine={}, content_size={}, recency={}",
            config.model_name, query, search_engine, content_size, recency_filter
        );

        // Create HTTP client
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| BitFunError::tool(format!("Failed to create HTTP client: {}", e)))?;

        // Build request (ZhipuAI format)
        let request_body = json!({
            "search_query": query,
            "search_engine": search_engine,
            "search_intent": true,  // Always enable search intent recognition
            "count": count,
            "content_size": content_size,
            "search_recency_filter": recency_filter
        });

        debug!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body).unwrap_or_default()
        );

        // Send request
        let response = client
            .post(&config.base_url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to send request: {}", e)))?;

        // Check status code
        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("Unknown error"));
            error!("Search API error: status={}, error={}", status, error_text);
            return Err(BitFunError::tool(format!(
                "Search API error {}: {}",
                status, error_text
            )));
        }

        // Parse response
        let search_response: ZhipuSearchResponse = response
            .json()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to parse response: {}", e)))?;

        info!(
            "Found {} results from {}",
            search_response.search_result.len(),
            config.model_name
        );

        // Convert to unified format
        let results: Vec<Value> = search_response
            .search_result
            .into_iter()
            .map(|r| {
                json!({
                    "title": r.title,
                    "url": r.link,
                    "snippet": r.content,
                    "source": r.media
                })
            })
            .collect();

        Ok(results)
    }

    /// Return mock search results (when search API is not configured)
    fn mock_search(&self, query: &str) -> Vec<Value> {
        vec![json!({
            "title": format!("Search results for: {}", query),
            "url": "https://example.com",
            "snippet": "This is mock data. To get real search results, please configure as follows:\n\n1. Open Config Center → AI Model Configuration\n2. Create new configuration, select \"Search Enhancement Model\" category\n3. Fill in Search API URL and API Key (supports ZhipuAI, etc.)\n4. In Config Center → Super Agent, select the search model you just configured\n\nOr set environment variable ZHIPU_API_KEY (visit https://open.bigmodel.cn/ to get)",
            "source": "Mock Data"
        })]
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
- Automatically uses advanced search engine (search_pro) for best results
- Control content size: small (brief), medium (moderate), high (detailed)
- Filter by recency: oneDay, oneWeek, oneMonth, oneYear, or noLimit
- Return up to 50 results per query
- Search intent recognition is automatically enabled"#
                .to_string(),
        )
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
                    "description": "Number of search results to return (1-50, default: 5)",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 50
                },
                "search_engine": {
                    "type": "string",
                    "enum": ["search_std", "search_pro", "search_pro_sogou", "search_pro_quark"],
                    "description": "Search engine to use. MUST be one of: 'search_std' (standard engine), 'search_pro' (default, advanced engine), 'search_pro_sogou' (Sogou search), 'search_pro_quark' (Quark search). No other values are accepted.",
                    "default": "search_pro"
                },
                "content_size": {
                    "type": "string",
                    "enum": ["small", "medium", "high"],
                    "description": "Content snippet size. MUST be one of: 'small' (brief snippets), 'medium' (default, moderate length), 'high' (detailed content). No other values are accepted.",
                    "default": "medium"
                },
                "search_recency_filter": {
                    "type": "string",
                    "enum": ["noLimit", "oneDay", "oneWeek", "oneMonth", "oneYear"],
                    "description": "Filter results by time range. MUST be one of: 'noLimit' (default, no time limit), 'oneDay' (past 24 hours), 'oneWeek' (past 7 days), 'oneMonth' (past 30 days), 'oneYear' (past 365 days). No other values are accepted.",
                    "default": "noLimit"
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
            .unwrap_or(5)
            .min(50) // Limit to maximum 50 results
            .max(1); // At least 1 result

        // Get optional parameters
        let search_engine = input
            .get("search_engine")
            .and_then(|v| v.as_str())
            .unwrap_or("search_pro");

        let content_size = input
            .get("content_size")
            .and_then(|v| v.as_str())
            .unwrap_or("medium");

        let recency_filter = input
            .get("search_recency_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("noLimit");

        info!(
            "WebSearch tool called: query='{}', num_results={}",
            query, num_results
        );

        // Load search API configuration
        let search_config = self.load_search_config().await;

        // Try real search, return mock data on failure
        let (results, api_used) = if let Some(config) = search_config {
            info!("Using search API: {}", config.model_name);
            match self
                .search_api(
                    &config,
                    query,
                    num_results,
                    search_engine,
                    content_size,
                    recency_filter,
                )
                .await
            {
                Ok(results) => {
                    info!("Search succeeded, returning {} results", results.len());
                    (results, true)
                }
                Err(e) => {
                    warn!("Search failed, returning mock data: {}", e);
                    (self.mock_search(query), false)
                }
            }
        } else {
            info!("Search API not configured, returning mock data");
            (self.mock_search(query), false)
        };

        // Format results
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

        let result = ToolResult::Result {
            data: json!({
                "query": query,
                "results": results,
                "result_count": results.len(),
                "api_used": api_used
            }),
            result_for_assistant: Some(format!(
                "Search query: '{}'\nFound {} results:\n\n{}",
                query,
                results.len(),
                formatted_results
            )),
        };

        Ok(vec![result])
    }
}

/// WebFetch tool
pub struct WebFetchTool;

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Fetch content from a URL.

Use this tool to:
- Read documentation from websites
- Fetch API responses
- Download text content from web pages
- Access online resources

Supports different output formats:
- text: Plain text content
- markdown: Convert HTML to markdown
- json: Parse JSON responses

Example usage:
- Fetch documentation: {"url": "https://doc.rust-lang.org/book/", "format": "markdown"}
- Get API data: {"url": "https://api.example.com/data", "format": "json"}
- Read webpage: {"url": "https://example.com/article"}"#
            .to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "format": {
                    "type": "string",
                    "enum": ["text", "markdown", "json"],
                    "description": "Output format (default: text)",
                    "default": "text"
                }
            },
            "required": ["url"]
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

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
            if url.is_empty() {
                return ValidationResult {
                    result: false,
                    message: Some("URL cannot be empty".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }

            // Basic URL validation
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return ValidationResult {
                    result: false,
                    message: Some("URL must start with http:// or https://".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        } else {
            return ValidationResult {
                result: false,
                message: Some("url is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("url is required".to_string()))?;

        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // Use reqwest to fetch URL content
        let client = reqwest::Client::builder()
            .user_agent("BitFun/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| BitFunError::tool(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to fetch URL: {}", e)))?;

        if !response.status().is_success() {
            return Err(BitFunError::tool(format!(
                "HTTP error {}: {}",
                response.status(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error")
            )));
        }

        let content = response
            .text()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to read response: {}", e)))?;

        let processed_content = match format {
            "markdown" => {
                // Simplified HTML to Markdown conversion
                content
                    .replace("<h1>", "# ")
                    .replace("</h1>", "\n")
                    .replace("<h2>", "## ")
                    .replace("</h2>", "\n")
                    .replace("<p>", "")
                    .replace("</p>", "\n\n")
            }
            "json" => {
                // Validate if it's valid JSON
                serde_json::from_str::<Value>(&content)
                    .map_err(|e| BitFunError::tool(format!("Invalid JSON response: {}", e)))?;
                content
            }
            _ => content,
        };

        let result = ToolResult::Result {
            data: json!({
                "url": url,
                "format": format,
                "content": processed_content,
                "content_length": processed_content.len()
            }),
            result_for_assistant: Some(processed_content),
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::WebFetchTool;
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::collections::HashMap;
    use std::io::ErrorKind;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            message_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            safe_mode: None,
            abort_controller: None,
            read_file_timestamps: HashMap::new(),
            options: None,
            response_state: None,
            image_context_provider: None,
            subagent_parent_info: None,
            cancellation_token: None,
        }
    }

    #[tokio::test]
    async fn webfetch_can_fetch_local_http_content() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                eprintln!(
                    "Skipping webfetch local server test due to sandbox socket restrictions: {}",
                    e
                );
                return;
            }
            Err(e) => panic!("bind local test server: {}", e),
        };
        let addr = listener.local_addr().expect("read local addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut req_buf = [0u8; 1024];
            let _ = socket.read(&mut req_buf).await;

            let body = "hello from webfetch";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
            let _ = socket.shutdown().await;
        });

        let tool = WebFetchTool::new();
        let input = json!({
            "url": format!("http://{}/test", addr),
            "format": "text"
        });

        let results = tool.call(&input, &empty_context()).await.unwrap_or_else(|e| {
            panic!("tool call failed with detailed error: {:?}", e);
        });
        assert_eq!(results.len(), 1);

        match &results[0] {
            ToolResult::Result {
                data,
                result_for_assistant,
            } => {
                assert_eq!(data["content"], "hello from webfetch");
                assert_eq!(data["format"], "text");
                assert_eq!(
                    result_for_assistant.as_deref(),
                    Some("hello from webfetch")
                );
            }
            other => panic!("unexpected tool result variant: {:?}", other),
        }

        server.await.expect("server task");
    }

    #[tokio::test]
    #[ignore = "requires outbound network"]
    async fn webfetch_can_fetch_real_website() {
        let tool = WebFetchTool::new();
        let input = json!({
            "url": "https://example.com",
            "format": "text"
        });

        let results = tool.call(&input, &empty_context()).await.unwrap_or_else(|e| {
            panic!("tool call failed with detailed error: {:?}", e);
        });
        assert_eq!(results.len(), 1);

        match &results[0] {
            ToolResult::Result {
                data,
                result_for_assistant,
            } => {
                let content = data["content"].as_str().expect("content should be string");
                assert!(content.contains("Example Domain"));
                assert_eq!(data["format"], "text");

                let assistant_text = result_for_assistant
                    .as_deref()
                    .expect("assistant output should exist");
                assert!(assistant_text.contains("Example Domain"));
            }
            other => panic!("unexpected tool result variant: {:?}", other),
        }
    }

}
