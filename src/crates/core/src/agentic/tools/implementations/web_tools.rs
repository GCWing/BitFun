//! Web tool implementation - WebSearchTool and URLFetcherTool

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::{error, info};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const EXA_URL: &str = "https://mcp.exa.ai/mcp";
const EXA_RESULTS: u64 = 5;
const EXA_CONTEXT: u64 = 8_000;

#[derive(Debug, Deserialize)]
struct ExaRes {
    result: Option<ExaData>,
}

#[derive(Debug, Deserialize)]
struct ExaData {
    content: Vec<ExaContent>,
}

#[derive(Debug, Deserialize)]
struct ExaContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

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
        let cli = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|err| BitFunError::tool(format!("Failed to create HTTP client: {}", err)))?;

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_search_exa",
                "arguments": {
                    "query": query,
                    "type": kind,
                    "numResults": num,
                    "livecrawl": crawl,
                    "contextMaxCharacters": ctx,
                }
            }
        });

        let res = cli
            .post(EXA_URL)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|err| BitFunError::tool(format!("Failed to send request: {}", err)))?;

        let status = res.status();
        if !status.is_success() {
            let err = res
                .text()
                .await
                .unwrap_or_else(|_| String::from("Unknown error"));
            error!("WebSearch Exa error: status={}, error={}", status, err);
            return Err(BitFunError::tool(format!(
                "Web search error {}: {}",
                status, err
            )));
        }

        let text = res
            .text()
            .await
            .map_err(|err| BitFunError::tool(format!("Failed to read response: {}", err)))?;

        self.parse_sse(&text)
    }

    fn parse_sse(&self, text: &str) -> BitFunResult<String> {
        let out = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .find_map(|line| {
                serde_json::from_str::<ExaRes>(line)
                    .ok()
                    .and_then(|res| res.result)
                    .map(|res| {
                        res.content
                            .into_iter()
                            .filter(|item| item.kind == "text")
                            .filter_map(|item| item.text)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .filter(|item| !item.trim().is_empty())
            });

        out.ok_or_else(|| BitFunError::tool("Web search returned no content".to_string()))
    }

    fn results(&self, text: &str) -> Vec<Value> {
        let mut out = Vec::new();
        let mut cur: Option<(String, String, Vec<String>)> = None;
        let mut body = false;

        for line in text.lines() {
            if let Some(next) = line.strip_prefix("Title: ") {
                if let Some((title, url, text)) = cur.take() {
                    out.push(self.item(title, url, text));
                }
                cur = Some((next.trim().to_string(), String::new(), Vec::new()));
                body = false;
                continue;
            }

            let Some(cur) = cur.as_mut() else {
                continue;
            };

            if let Some(next) = line.strip_prefix("URL: ") {
                cur.1 = next.trim().to_string();
                continue;
            }

            if let Some(next) = line.strip_prefix("Text: ") {
                if !next.trim().is_empty() {
                    cur.2.push(next.trim().to_string());
                }
                body = true;
                continue;
            }

            if body {
                cur.2.push(line.to_string());
            }
        }

        if let Some((title, url, text)) = cur.take() {
            out.push(self.item(title, url, text));
        }

        if out.is_empty() && !text.trim().is_empty() {
            return vec![json!({
                "title": "Web search result",
                "url": "",
                "snippet": self.snippet(text)
            })];
        }

        out
    }

    fn item(&self, title: String, url: String, text: Vec<String>) -> Value {
        json!({
            "title": title,
            "url": url,
            "snippet": self.snippet(&text.join("\n"))
        })
    }

    fn snippet(&self, text: &str) -> String {
        let text = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join(" ");

        if text.chars().count() <= 320 {
            return text;
        }

        let mut out = String::new();
        for ch in text.chars().take(317) {
            out.push(ch);
        }
        out.push_str("...");
        out
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
- Uses Exa's hosted MCP web search service with no local API key setup
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
                    "description": "Maximum characters of search context to request from Exa (default: 8000)",
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
                "provider": "exa_mcp"
            }),
            result_for_assistant: Some(format!(
                "Search query: '{}'\nFound {} results:\n\n{}",
                query,
                results.len(),
                formatted_results
            )),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

/// WebFetch tool
pub struct WebFetchTool;

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self
    }

    fn is_html(content_type: Option<&str>, content: &str) -> bool {
        if let Some(ct) = content_type {
            let ct = ct.to_lowercase();
            if ct.contains("text/html") || ct.contains("application/xhtml") {
                return true;
            }
        }
        let sample = &content[..content.len().min(2048)];
        let sample_lower = sample.to_lowercase();
        sample_lower.contains("<!doctype html")
            || sample_lower.contains("<html")
            || sample_lower.contains("</html>")
    }

    fn html_to_text(html: &str) -> String {
        use regex::Regex;

        let mut text = html.to_string();
        for tag in [
            "script", "style", "noscript", "nav", "header", "footer", "aside", "iframe",
        ] {
            let pattern = format!(r"(?is)<{}[^\u003e]*>[\s\S]*?</\s*{}\s*>", tag, tag);
            if let Ok(re) = Regex::new(&pattern) {
                text = re.replace_all(&text, "\n").to_string();
            }
        }

        let text = Regex::new(r"(?i)<br\s*/?>")
            .unwrap()
            .replace_all(&text, "\n");

        let text = Regex::new(r"<[^>]+>").unwrap().replace_all(&text, " ");

        let text = text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&#x27;", "'")
            .replace("&nbsp;", " ")
            .replace("&#160;", " ");

        text.lines()
            .map(|line| {
                let mut result = String::new();
                let mut prev_space = true;
                for ch in line.chars() {
                    if ch.is_whitespace() {
                        if !prev_space {
                            result.push(' ');
                            prev_space = true;
                        }
                    } else {
                        result.push(ch);
                        prev_space = false;
                    }
                }
                result.trim().to_string()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
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
- raw: Raw response content (original HTML or text)
- text: Plain text content (extracts text from HTML pages, leaves other content unchanged)
- markdown: Convert HTML to markdown
- json: Parse JSON responses

Example usage:
- Fetch raw HTML: {"url": "https://example.com", "format": "raw"}
- Fetch plain text: {"url": "https://example.com/article", "format": "text"}
- Fetch documentation: {"url": "https://doc.rust-lang.org/book/", "format": "markdown"}
- Get API data: {"url": "https://api.example.com/data", "format": "json"}"#
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
                    "enum": ["raw", "text", "markdown", "json"],
                    "description": "Output format. Use 'raw' for original HTML, 'text' for extracted plain text, 'markdown' for simple HTML-to-markdown, or 'json' for parsed JSON.",
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

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let content = response
            .text()
            .await
            .map_err(|e| BitFunError::tool(format!("Failed to read response: {}", e)))?;

        let processed_content = match format {
            "raw" => content,
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
            _ => {
                if Self::is_html(content_type.as_deref(), &content) {
                    Self::html_to_text(&content)
                } else {
                    content
                }
            }
        };

        let result = ToolResult::Result {
            data: json!({
                "url": url,
                "format": format,
                "content": processed_content,
                "content_length": processed_content.len()
            }),
            result_for_assistant: Some(processed_content),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

#[cfg(test)]
mod tests {
    use super::{WebFetchTool, WebSearchTool};
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::io::ErrorKind;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            custom_data: std::collections::HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            workspace_services: None,
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

        let results = tool
            .call(&input, &empty_context())
            .await
            .unwrap_or_else(|e| {
                panic!("tool call failed with detailed error: {:?}", e);
            });
        assert_eq!(results.len(), 1);

        match &results[0] {
            ToolResult::Result {
                data,
                result_for_assistant,
                ..
            } => {
                assert_eq!(data["content"], "hello from webfetch");
                assert_eq!(data["format"], "text");
                assert_eq!(result_for_assistant.as_deref(), Some("hello from webfetch"));
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

        let results = tool
            .call(&input, &empty_context())
            .await
            .unwrap_or_else(|e| {
                panic!("tool call failed with detailed error: {:?}", e);
            });
        assert_eq!(results.len(), 1);

        match &results[0] {
            ToolResult::Result {
                data,
                result_for_assistant,
                ..
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

    #[test]
    fn webfetch_html_to_text_extracts_plain_text() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
<script>alert('ignore me');</script>
<style>.hidden { display: none; }</style>
<h1>Hello World</h1>
<p>This is a paragraph with <strong>bold</strong> text.</p>
<ul><li>Item one</li><li>Item two</li></ul>
</body>
</html>"#;

        let text = WebFetchTool::html_to_text(html);
        assert!(!text.contains("<script>"));
        assert!(!text.contains("alert("));
        assert!(!text.contains(".hidden"));
        assert!(text.contains("Hello World"));
        assert!(text.contains("This is a paragraph with bold text."));
        assert!(text.contains("Item one"));
        assert!(text.contains("Item two"));
    }

    #[test]
    fn webfetch_is_html_detects_html_content() {
        assert!(WebFetchTool::is_html(
            Some("text/html; charset=utf-8"),
            "any"
        ));
        assert!(WebFetchTool::is_html(Some("application/xhtml+xml"), "any"));
        assert!(WebFetchTool::is_html(None, "<!DOCTYPE html><html></html>"));
        assert!(WebFetchTool::is_html(None, "<html lang=\"en\"></html>"));
        assert!(!WebFetchTool::is_html(Some("application/json"), "{}"));
        assert!(!WebFetchTool::is_html(Some("text/plain"), "hello"));
        assert!(!WebFetchTool::is_html(None, "just plain text"));
    }

    #[test]
    fn websearch_parses_exa_text_into_results() {
        let tool = WebSearchTool::new();
        let text = r#"Title: Result One
URL: https://example.com/one
Text: Result One

First paragraph.

Title: Result Two
URL: https://example.com/two
Text: Result Two

Second paragraph.
"#;

        let out = tool.results(text);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["title"], "Result One");
        assert_eq!(out[0]["url"], "https://example.com/one");
        assert_eq!(out[0]["snippet"], "Result One First paragraph.");
        assert_eq!(out[1]["title"], "Result Two");
    }
}
