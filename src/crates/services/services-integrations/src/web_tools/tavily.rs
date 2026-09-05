use super::{
    classify_http_status, normalize_results, read_bounded_body, search_client, transport_error,
    validate_search_request,
};
use async_trait::async_trait;
use openbitfun_runtime_ports::{
    WebSearchError, WebSearchErrorKind, WebSearchProvider, WebSearchProviderId, WebSearchRequest,
    WebSearchResponse, WebSearchResult,
};
use serde::{Deserialize, Serialize};

const TAVILY_SEARCH_API_URL: &str = "https://api.tavily.com/search";
const TAVILY_SEARCH_TIMEOUT_SECS: u64 = 60;

pub struct TavilySearchProvider {
    api_key: String,
    endpoint: String,
}

impl TavilySearchProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: TAVILY_SEARCH_API_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub(super) fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: endpoint.into(),
        }
    }
}

#[derive(Serialize)]
struct TavilyRequest<'a> {
    query: &'a str,
    max_results: u32,
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
}

#[async_trait]
impl WebSearchProvider for TavilySearchProvider {
    fn id(&self) -> WebSearchProviderId {
        WebSearchProviderId::new(WebSearchProviderId::TAVILY)
    }

    async fn search(&self, request: WebSearchRequest) -> Result<WebSearchResponse, WebSearchError> {
        let provider = self.id();
        validate_search_request(provider.clone(), &request)?;
        if self.api_key.trim().is_empty() {
            return Err(WebSearchError::new(
                provider,
                WebSearchErrorKind::InvalidConfiguration,
                "Tavily API key is not configured on this device",
            ));
        }
        let client = search_client(provider.clone(), TAVILY_SEARCH_TIMEOUT_SECS)?;
        let response = client
            .post(&self.endpoint)
            .bearer_auth(self.api_key.trim())
            .json(&TavilyRequest {
                query: &request.query,
                max_results: request.max_results,
            })
            .send()
            .await
            .map_err(|error| {
                transport_error(provider.clone(), "Failed to call Tavily Search API", error)
            })?;
        let (status, headers, body) = read_bounded_body(response, provider.clone()).await?;
        if !status.is_success() {
            return Err(classify_http_status(provider, status, &headers, &body));
        }
        let response: TavilyResponse = serde_json::from_slice(&body).map_err(|error| {
            WebSearchError::new(
                provider.clone(),
                WebSearchErrorKind::InvalidResponse,
                format!("Failed to parse Tavily response: {error}"),
            )
        })?;
        let raw_results = response
            .results
            .into_iter()
            .map(|result| WebSearchResult {
                title: result.title,
                url: result.url,
                published_at: None,
                author: None,
            })
            .collect();
        let results = normalize_results(provider.clone(), raw_results, request.max_results)?;
        Ok(WebSearchResponse { provider, results })
    }
}
