use super::readable::{
    extract_html_title, extract_markdown_with_text_fallback, is_html, normalize_requested_format,
    RequestedFormat,
};
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolExposure, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_services_integrations::web_tools::{WebToolNetworkError, WebToolNetworkProvider};
use serde_json::{json, Value};

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
- Download readable content from web pages
- Access online resources

Best for static pages that need no login. For pages requiring the user's login session or JavaScript rendering, use ControlHub domain="browser" instead: connect -> navigate -> snapshot / read_article. Chrome 144+ and Edge can connect to the user's current profile after explicit approval, preserving tabs and login state; other supported Chromium browsers reuse a real-profile endpoint when available and otherwise use BitFun's persistent managed profile. (browser.fetch only works when a session is already connected and the current page is same-origin with the target URL — it runs inside that page and is subject to its CORS policy.)

HTTP 401, 403, and 429 responses are returned as structured access restrictions, not readable page content. Do not retry the same URL in the same turn. For public GitHub API rate limits, use the public github.com page, WebSearch, or an authenticated Git/review provider. For login-gated pages, use the user's connected browser session.

Supports different output formats:
- raw: Raw response content (original HTML or text)
- markdown: Readable content mode. For HTML pages, BitFun extracts the main content and returns markdown when possible, automatically falling back to plain text when markdown conversion is not reliable.
- json: Parse JSON responses

Example usage:
- Fetch raw HTML: {"url": "https://example.com", "format": "raw"}
- Fetch readable content: {"url": "https://example.com/article", "format": "markdown"}
- Get API data: {"url": "https://api.example.com/data", "format": "json"}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Fetch content from a URL in raw, markdown, or JSON format.".to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        ToolExposure::Deferred
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
                    "enum": ["raw", "markdown", "json"],
                    "description": "Output format. Use 'raw' for the original response, 'markdown' for readable content with automatic plain-text fallback, or 'json' for parsed JSON.",
                    "default": "markdown"
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

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let url = input
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| BitFunError::validation("url is required".to_string()))?;
        Ok(vec![PermissionIntent::new(
            "webfetch",
            vec![url.to_string()],
        )])
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

        let requested_format =
            normalize_requested_format(input.get("format").and_then(|v| v.as_str()))?;

        let response = match WebToolNetworkProvider::fetch_text(url).await {
            Ok(response) => response,
            Err(error) => {
                if let Some(result) = handled_access_result(url, &error) {
                    return Ok(vec![result]);
                }
                return Err(BitFunError::tool(error.to_string()));
            }
        };
        let content_type = response.content_type;
        let content = response.content;

        let is_html_response = is_html(content_type.as_deref(), &content);
        let fallback_title = if is_html_response {
            extract_html_title(&content)
        } else {
            None
        };

        let (processed_content, content_representation, extractor, title) = match requested_format {
            RequestedFormat::Raw => (content, "raw", "raw", fallback_title),
            RequestedFormat::Json => {
                serde_json::from_str::<Value>(&content)
                    .map_err(|e| BitFunError::tool(format!("Invalid JSON response: {}", e)))?;
                (content, "json", "json", None)
            }
            RequestedFormat::Markdown => {
                if is_html_response {
                    let readable = extract_markdown_with_text_fallback(&content, url)?;
                    (
                        readable.content,
                        readable.content_representation,
                        readable.extractor,
                        readable.title,
                    )
                } else {
                    (content, "plain_text", "plain_text", None)
                }
            }
        };

        let result = ToolResult::Result {
            data: json!({
                "url": url,
                "title": title,
                "format": match requested_format {
                    RequestedFormat::Raw => "raw",
                    RequestedFormat::Markdown => "markdown",
                    RequestedFormat::Json => "json",
                },
                "content_representation": content_representation,
                "extractor": extractor,
                "content": processed_content,
                "content_length": processed_content.len()
            }),
            result_for_assistant: Some(processed_content),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

pub(super) fn handled_access_result(url: &str, error: &WebToolNetworkError) -> Option<ToolResult> {
    let WebToolNetworkError::HttpStatus {
        status_code,
        status,
        reason,
        retry_after,
        rate_limit_remaining,
        rate_limit_reset,
    } = error
    else {
        return None;
    };
    if !matches!(*status_code, 401 | 403 | 429) {
        return None;
    }

    let github_rate_limited = url.starts_with("https://api.github.com/")
        && (rate_limit_remaining.as_deref() == Some("0") || *status_code == 429);
    let category = if github_rate_limited {
        "rate_limited"
    } else if *status_code == 401 {
        "authentication_required"
    } else if *status_code == 429 {
        "rate_limited"
    } else {
        "access_denied"
    };
    let guidance = if github_rate_limited {
        "Do not retry this GitHub API URL in this turn. Use the public github.com page, WebSearch, or an authenticated Git/review provider."
    } else if *status_code == 401 {
        "This page requires authentication. Use the user's connected browser session or an authenticated provider."
    } else if *status_code == 429 {
        "The server rate-limited direct fetching. Respect Retry-After when present and use another available source in this turn."
    } else {
        "The server denied direct fetching. Do not retry the same URL in this turn; use WebSearch or the user's connected browser session when appropriate."
    };
    let assistant_text =
        format!("WebFetch access restriction: HTTP {status} ({reason}). {guidance}");

    Some(ToolResult::Result {
        data: json!({
            "url": url,
            "status": category,
            "http_status": status_code,
            "reason": reason,
            "retry_after": retry_after,
            "rate_limit_remaining": rate_limit_remaining,
            "rate_limit_reset": rate_limit_reset,
            "retry_same_url": false,
            "guidance": guidance,
        }),
        result_for_assistant: Some(assistant_text),
        image_attachments: None,
    })
}
