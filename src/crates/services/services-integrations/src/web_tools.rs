//! Network providers for built-in web tools.

use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use thiserror::Error;

const USER_AGENT_VALUE: &str = "BitFun/1.0";
const WEB_FETCH_TIMEOUT_SECS: u64 = 30;
const EXA_URL: &str = "https://mcp.exa.ai/mcp";
const EXA_TIMEOUT_SECS: u64 = 25;

#[derive(Debug, Error)]
pub enum WebToolNetworkError {
    #[error("Failed to create HTTP client: {0}")]
    BuildClient(String),
    #[error("Failed to fetch URL: {0}")]
    Fetch(String),
    #[error("HTTP error {status}: {reason}")]
    HttpStatus { status: String, reason: String },
    #[error("Failed to read response: {0}")]
    ReadResponse(String),
    #[error("Failed to send request: {0}")]
    SearchRequest(String),
    #[error("Web search error {status}: {body}")]
    SearchStatus { status: String, body: String },
    #[error("Exa authentication failed: {message}")]
    SearchAuthentication { message: String },
    #[error("Exa credits or search quota exhausted: {message}")]
    SearchQuota { message: String },
    #[error("Exa search request is not permitted: {message}")]
    SearchPermission { message: String },
    #[error("Exa search rate limit exceeded: {message}")]
    SearchRateLimited { message: String },
    #[error("Exa MCP tool error: {message}")]
    SearchTool { message: String },
    #[error("Exa MCP protocol error {code}: {message}")]
    SearchProtocol { code: i64, message: String },
    #[error("Web search returned no content")]
    SearchEmpty,
}

#[derive(Debug, Clone)]
pub struct WebFetchResponse {
    pub content_type: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ExaSearchRequest<'a> {
    pub query: &'a str,
    pub num_results: u64,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
    result: Option<ExaData>,
    error: Option<ExaRpcError>,
}

#[derive(Debug, Deserialize)]
struct ExaData {
    #[serde(default)]
    content: Vec<ExaContent>,
    #[serde(rename = "isError", default)]
    is_error: bool,
}

#[derive(Debug, Deserialize)]
struct ExaRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ExaContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

pub struct WebToolNetworkProvider;

impl WebToolNetworkProvider {
    pub async fn fetch_text(url: &str) -> Result<WebFetchResponse, WebToolNetworkError> {
        let client = crate::reqwest_client_builder()
            .user_agent(USER_AGENT_VALUE)
            .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
            .build()
            .map_err(|error| WebToolNetworkError::BuildClient(error.to_string()))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|error| WebToolNetworkError::Fetch(error.to_string()))?;

        if !response.status().is_success() {
            return Err(WebToolNetworkError::HttpStatus {
                status: response.status().to_string(),
                reason: response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string(),
            });
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        let content = response
            .text()
            .await
            .map_err(|error| WebToolNetworkError::ReadResponse(error.to_string()))?;

        Ok(WebFetchResponse {
            content_type,
            content,
        })
    }

    pub async fn search_exa(request: ExaSearchRequest<'_>) -> Result<String, WebToolNetworkError> {
        let client = crate::reqwest_client_builder()
            .timeout(Duration::from_secs(EXA_TIMEOUT_SECS))
            .build()
            .map_err(|error| WebToolNetworkError::BuildClient(error.to_string()))?;

        let body = build_exa_request_body(&request);

        let response = client
            .post(EXA_URL)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|error| WebToolNetworkError::SearchRequest(error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("Unknown error"));
            return Err(classify_exa_http_error(
                status.as_u16(),
                status.to_string(),
                &body,
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|error| WebToolNetworkError::ReadResponse(error.to_string()))?;

        parse_exa_sse(&text)
    }
}

fn build_exa_request_body(request: &ExaSearchRequest<'_>) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "web_search_exa",
            "arguments": {
                "query": request.query,
                "numResults": request.num_results,
            }
        }
    })
}

fn parse_exa_sse(text: &str) -> Result<String, WebToolNetworkError> {
    for payload in text.lines().filter_map(|line| line.strip_prefix("data: ")) {
        if let Ok(response) = serde_json::from_str::<ExaResponse>(payload) {
            if let Some(result) = parse_exa_response(response) {
                return result;
            }
        }
    }

    if let Ok(response) = serde_json::from_str::<ExaResponse>(text.trim()) {
        if let Some(result) = parse_exa_response(response) {
            return result;
        }
    }

    Err(WebToolNetworkError::SearchEmpty)
}

fn parse_exa_response(response: ExaResponse) -> Option<Result<String, WebToolNetworkError>> {
    if let Some(error) = response.error {
        let message = bounded_error_message(&error.message);
        return Some(Err(classify_known_exa_error(&message).unwrap_or(
            WebToolNetworkError::SearchProtocol {
                code: error.code,
                message,
            },
        )));
    }

    let result = response.result?;
    let text = result
        .content
        .into_iter()
        .filter(|item| item.kind == "text")
        .filter_map(|item| item.text)
        .collect::<Vec<_>>()
        .join("\n");

    if result.is_error {
        let message = if text.trim().is_empty() {
            "Unknown Exa MCP tool error".to_string()
        } else {
            bounded_error_message(&text)
        };
        return Some(Err(classify_known_exa_error(&message)
            .unwrap_or(WebToolNetworkError::SearchTool { message })));
    }

    (!text.trim().is_empty()).then_some(Ok(text))
}

fn classify_exa_http_error(status_code: u16, status: String, body: &str) -> WebToolNetworkError {
    let message = extract_exa_error_message(body);
    match status_code {
        401 => WebToolNetworkError::SearchAuthentication { message },
        402 => WebToolNetworkError::SearchQuota { message },
        403 => WebToolNetworkError::SearchPermission { message },
        429 => WebToolNetworkError::SearchRateLimited { message },
        _ => classify_known_exa_error(&message).unwrap_or(WebToolNetworkError::SearchStatus {
            status,
            body: message,
        }),
    }
}

fn classify_known_exa_error(message: &str) -> Option<WebToolNetworkError> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("(429)")
        || lower.contains("rate limit")
        || lower.contains("too many requests")
    {
        return Some(WebToolNetworkError::SearchRateLimited {
            message: message.to_string(),
        });
    }
    if lower.contains("(401)")
        || lower.contains("invalid api key")
        || lower.contains("api key is invalid")
        || lower.contains("unauthorized")
        || lower.contains("authentication failed")
    {
        return Some(WebToolNetworkError::SearchAuthentication {
            message: message.to_string(),
        });
    }
    if lower.contains("(402)")
        || lower.contains("credit")
        || lower.contains("quota")
        || lower.contains("budget")
        || lower.contains("insufficient balance")
    {
        return Some(WebToolNetworkError::SearchQuota {
            message: message.to_string(),
        });
    }
    if lower.contains("(403)") || lower.contains("forbidden") || lower.contains("permission") {
        return Some(WebToolNetworkError::SearchPermission {
            message: message.to_string(),
        });
    }
    None
}

fn extract_exa_error_message(body: &str) -> String {
    let payloads = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .chain(std::iter::once(body.trim()));

    for payload in payloads {
        let Ok(response) = serde_json::from_str::<ExaResponse>(payload) else {
            continue;
        };
        if let Some(error) = response.error {
            return bounded_error_message(&error.message);
        }
        if let Some(result) = response.result {
            let message = result
                .content
                .into_iter()
                .filter(|item| item.kind == "text")
                .filter_map(|item| item.text)
                .collect::<Vec<_>>()
                .join("\n");
            if result.is_error && !message.trim().is_empty() {
                return bounded_error_message(&message);
            }
        }
    }

    let body = body.trim();
    if body.is_empty() {
        "Unknown error".to_string()
    } else {
        bounded_error_message(body)
    }
}

fn bounded_error_message(message: &str) -> String {
    const MAX_ERROR_CHARS: usize = 500;
    let message = message.trim();
    if message.chars().count() <= MAX_ERROR_CHARS {
        return message.to_string();
    }

    let mut bounded = message
        .chars()
        .take(MAX_ERROR_CHARS - 3)
        .collect::<String>();
    bounded.push_str("...");
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exa_request_contains_only_supported_arguments() {
        let body = build_exa_request_body(&ExaSearchRequest {
            query: "example query",
            num_results: 10,
        });

        assert_eq!(
            body["params"]["arguments"],
            json!({
                "query": "example query",
                "numResults": 10,
            })
        );
    }

    #[test]
    fn parse_exa_sse_returns_first_text_payload() {
        let text = concat!(
            "event: message\n",
            "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Title: A\\nURL: https://example.com\"}]}}\n",
            "\n"
        );

        let out = parse_exa_sse(text).expect("exa text should parse");

        assert_eq!(out, "Title: A\nURL: https://example.com");
    }

    #[test]
    fn parse_exa_sse_rejects_empty_text_payload() {
        let text = "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"   \"}]}}\n";

        let error = parse_exa_sse(text).unwrap_err();

        assert!(matches!(error, WebToolNetworkError::SearchEmpty));
    }

    #[test]
    fn parse_exa_sse_rejects_mcp_tool_authentication_error() {
        let text = "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"web_search_exa error (401): Invalid API key\"}],\"isError\":true}}\n";

        let error = parse_exa_sse(text).unwrap_err();

        assert!(matches!(
            error,
            WebToolNetworkError::SearchAuthentication { .. }
        ));
    }

    #[test]
    fn parse_exa_sse_rejects_mcp_tool_rate_limit_error() {
        let text = "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Free MCP rate limit reached; provide your own Exa API key\"}],\"isError\":true}}\n";

        let error = parse_exa_sse(text).unwrap_err();

        assert!(matches!(
            error,
            WebToolNetworkError::SearchRateLimited { .. }
        ));
    }

    #[test]
    fn parse_exa_sse_rejects_json_rpc_error() {
        let text = "data: {\"error\":{\"code\":-32000,\"message\":\"Provider unavailable\"}}\n";

        let error = parse_exa_sse(text).unwrap_err();

        assert!(matches!(
            error,
            WebToolNetworkError::SearchProtocol { code: -32000, .. }
        ));
    }

    #[test]
    fn classifies_byok_http_failures() {
        let cases = [
            (401, "401 Unauthorized", "invalid key", "authentication"),
            (402, "402 Payment Required", "no credits", "quota"),
            (403, "403 Forbidden", "forbidden", "permission"),
            (429, "429 Too Many Requests", "rate limited", "rate_limit"),
        ];

        for (code, status, body, expected) in cases {
            let error = classify_exa_http_error(code, status.to_string(), body);
            let matches_expected = match expected {
                "authentication" => {
                    matches!(error, WebToolNetworkError::SearchAuthentication { .. })
                }
                "quota" => matches!(error, WebToolNetworkError::SearchQuota { .. }),
                "permission" => matches!(error, WebToolNetworkError::SearchPermission { .. }),
                "rate_limit" => {
                    matches!(error, WebToolNetworkError::SearchRateLimited { .. })
                }
                _ => false,
            };
            assert!(
                matches_expected,
                "unexpected classification for HTTP {code}"
            );
        }
    }
}
