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

const EXA_SEARCH_API_URL: &str = "https://api.exa.ai/search";
const EXA_SEARCH_TIMEOUT_SECS: u64 = 60;

pub struct ExaSearchApiProvider {
    api_key: String,
    endpoint: String,
}

impl ExaSearchApiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: EXA_SEARCH_API_URL.to_string(),
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
#[serde(rename_all = "camelCase")]
struct ExaRequest<'a> {
    query: &'a str,
    num_results: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
    request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExaResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    published_date: Option<String>,
    author: Option<String>,
}

#[async_trait]
impl WebSearchProvider for ExaSearchApiProvider {
    fn id(&self) -> WebSearchProviderId {
        WebSearchProviderId::new(WebSearchProviderId::EXA_SEARCH_API)
    }

    async fn search(&self, request: WebSearchRequest) -> Result<WebSearchResponse, WebSearchError> {
        let provider = self.id();
        validate_search_request(provider.clone(), &request)?;
        if self.api_key.trim().is_empty() {
            return Err(WebSearchError::new(
                provider,
                WebSearchErrorKind::InvalidConfiguration,
                "Exa Search API key is not configured on this device",
            ));
        }
        let client = search_client(provider.clone(), EXA_SEARCH_TIMEOUT_SECS)?;
        let response = client
            .post(&self.endpoint)
            .header("x-api-key", self.api_key.trim())
            .json(&ExaRequest {
                query: &request.query,
                num_results: request.max_results,
            })
            .send()
            .await
            .map_err(|error| {
                transport_error(provider.clone(), "Failed to call Exa Search API", error)
            })?;
        let (status, headers, body) = read_bounded_body(response, provider.clone()).await?;
        if !status.is_success() {
            let request_id = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .get("requestId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            return Err(
                classify_http_status(provider, status, &headers, &body).with_request_id(request_id)
            );
        }
        let response: ExaResponse = serde_json::from_slice(&body).map_err(|error| {
            WebSearchError::new(
                provider.clone(),
                WebSearchErrorKind::InvalidResponse,
                format!("Failed to parse Exa Search API response: {error}"),
            )
        })?;
        let request_id = response.request_id;
        let raw_results = response
            .results
            .into_iter()
            .map(|result| WebSearchResult {
                title: result.title,
                url: result.url,
                published_at: result.published_date,
                author: result.author,
            })
            .collect();
        let results = normalize_results(provider.clone(), raw_results, request.max_results)
            .map_err(|error| error.with_request_id(request_id.clone()))?;
        Ok(WebSearchResponse { provider, results })
    }
}
