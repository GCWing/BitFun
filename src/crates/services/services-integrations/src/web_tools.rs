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
    pub kind: &'a str,
    pub livecrawl: &'a str,
    pub context_max_characters: u64,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
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

pub struct WebToolNetworkProvider;

impl WebToolNetworkProvider {
    pub async fn fetch_text(url: &str) -> Result<WebFetchResponse, WebToolNetworkError> {
        let client = reqwest::Client::builder()
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(EXA_TIMEOUT_SECS))
            .build()
            .map_err(|error| WebToolNetworkError::BuildClient(error.to_string()))?;

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "web_search_exa",
                "arguments": {
                    "query": request.query,
                    "type": request.kind,
                    "numResults": request.num_results,
                    "livecrawl": request.livecrawl,
                    "contextMaxCharacters": request.context_max_characters,
                }
            }
        });

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
            return Err(WebToolNetworkError::SearchStatus {
                status: status.to_string(),
                body,
            });
        }

        let text = response
            .text()
            .await
            .map_err(|error| WebToolNetworkError::ReadResponse(error.to_string()))?;

        parse_exa_sse(&text)
    }
}

fn parse_exa_sse(text: &str) -> Result<String, WebToolNetworkError> {
    let out = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .find_map(|line| {
            serde_json::from_str::<ExaResponse>(line)
                .ok()
                .and_then(|response| response.result)
                .map(|result| {
                    result
                        .content
                        .into_iter()
                        .filter(|item| item.kind == "text")
                        .filter_map(|item| item.text)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|item| !item.trim().is_empty())
        });

    out.ok_or(WebToolNetworkError::SearchEmpty)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
