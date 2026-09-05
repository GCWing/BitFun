use super::{
    bounded_error_message, classify_http_status, normalize_results, read_bounded_body,
    search_client, transport_error, validate_search_request,
};
use async_trait::async_trait;
use openbitfun_runtime_ports::{
    WebSearchError, WebSearchErrorKind, WebSearchProvider, WebSearchProviderId, WebSearchRequest,
    WebSearchResponse, WebSearchResult,
};
use reqwest::header::{HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

const OPENBITFUN_SEARCH_TIMEOUT_SECS: u64 = 60;
const OPENBITFUN_SEARCH_MEDIA_TYPE: &str = "application/vnd.openbitfun.web-search.v1+json";

pub enum OpenBitFunSearchHttpAuth {
    None,
    Bearer(String),
    Header { name: String, value: String },
}

pub struct OpenBitFunSearchHttpProvider {
    endpoint: Url,
    auth: OpenBitFunSearchHttpAuth,
}

impl OpenBitFunSearchHttpProvider {
    pub fn new(
        endpoint: impl AsRef<str>,
        auth: OpenBitFunSearchHttpAuth,
    ) -> Result<Self, WebSearchError> {
        let provider = WebSearchProviderId::new(WebSearchProviderId::OPENBITFUN_SEARCH_HTTP);
        let endpoint = Url::parse(endpoint.as_ref()).map_err(|error| {
            WebSearchError::new(
                provider.clone(),
                WebSearchErrorKind::InvalidConfiguration,
                format!("OpenBitFun Search endpoint is invalid: {error}"),
            )
        })?;
        validate_endpoint(&endpoint, provider.clone())?;
        validate_auth(&auth, provider)?;
        Ok(Self { endpoint, auth })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolRequest<'a> {
    query: &'a str,
    max_results: u32,
}

#[derive(Deserialize)]
struct ProtocolResponse {
    #[serde(default)]
    results: Vec<ProtocolResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    published_at: Option<String>,
    author: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolErrorEnvelope {
    error: Option<ProtocolErrorBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolErrorBody {
    code: Option<String>,
    message: Option<String>,
    retry_after_seconds: Option<u64>,
}

#[async_trait]
impl WebSearchProvider for OpenBitFunSearchHttpProvider {
    fn id(&self) -> WebSearchProviderId {
        WebSearchProviderId::new(WebSearchProviderId::OPENBITFUN_SEARCH_HTTP)
    }

    async fn search(&self, request: WebSearchRequest) -> Result<WebSearchResponse, WebSearchError> {
        let provider = self.id();
        validate_search_request(provider.clone(), &request)?;
        let client = search_client(provider.clone(), OPENBITFUN_SEARCH_TIMEOUT_SECS)?;
        let mut builder = client
            .post(self.endpoint.clone())
            .header(ACCEPT, OPENBITFUN_SEARCH_MEDIA_TYPE)
            .json(&ProtocolRequest {
                query: &request.query,
                max_results: request.max_results,
            });
        builder = match &self.auth {
            OpenBitFunSearchHttpAuth::None => builder,
            OpenBitFunSearchHttpAuth::Bearer(secret) => builder.bearer_auth(secret),
            OpenBitFunSearchHttpAuth::Header { name, value } => {
                let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                    WebSearchError::new(
                        provider.clone(),
                        WebSearchErrorKind::InvalidConfiguration,
                        format!("OpenBitFun Search auth header name is invalid: {error}"),
                    )
                })?;
                let value = HeaderValue::from_str(value).map_err(|error| {
                    WebSearchError::new(
                        provider.clone(),
                        WebSearchErrorKind::InvalidConfiguration,
                        format!("OpenBitFun Search auth header value is invalid: {error}"),
                    )
                })?;
                builder.header(name, value)
            }
        };
        let response = builder.send().await.map_err(|error| {
            transport_error(
                provider.clone(),
                "Failed to call OpenBitFun Search HTTP endpoint",
                error,
            )
        })?;
        let (status, headers, body) = read_bounded_body(response, provider.clone()).await?;
        if !status.is_success() {
            let mut error = classify_http_status(provider, status, &headers, &body);
            if let Some(protocol_error) = serde_json::from_slice::<ProtocolErrorEnvelope>(&body)
                .ok()
                .and_then(|envelope| envelope.error)
            {
                if let Some(kind) = protocol_error.code.as_deref().and_then(protocol_error_kind) {
                    error.kind = kind;
                }
                if let Some(message) = protocol_error
                    .message
                    .as_deref()
                    .map(str::trim)
                    .filter(|message| !message.is_empty())
                {
                    error.message = bounded_error_message(message);
                }
                if error.retry_after_seconds.is_none() {
                    error.retry_after_seconds = protocol_error.retry_after_seconds;
                }
            }
            return Err(error);
        }
        validate_content_type(&headers, provider.clone())?;
        let response: ProtocolResponse = serde_json::from_slice(&body).map_err(|error| {
            WebSearchError::new(
                provider.clone(),
                WebSearchErrorKind::InvalidResponse,
                format!("Failed to parse OpenBitFun Search HTTP response: {error}"),
            )
        })?;
        let raw_results = response
            .results
            .into_iter()
            .map(|result| WebSearchResult {
                title: result.title,
                url: result.url,
                published_at: result.published_at,
                author: result.author,
            })
            .collect();
        let results = normalize_results(provider.clone(), raw_results, request.max_results)?;
        Ok(WebSearchResponse { provider, results })
    }
}

fn protocol_error_kind(code: &str) -> Option<WebSearchErrorKind> {
    match code {
        "invalid_request" => Some(WebSearchErrorKind::InvalidRequest),
        "authentication_failed" => Some(WebSearchErrorKind::Authentication),
        "permission_denied" => Some(WebSearchErrorKind::PermissionDenied),
        "quota_exhausted" => Some(WebSearchErrorKind::QuotaExhausted),
        "rate_limited" => Some(WebSearchErrorKind::RateLimited),
        "provider_unavailable" => Some(WebSearchErrorKind::ProviderUnavailable),
        "invalid_response" => Some(WebSearchErrorKind::InvalidResponse),
        _ => None,
    }
}

fn validate_endpoint(endpoint: &Url, provider: WebSearchProviderId) -> Result<(), WebSearchError> {
    if !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(WebSearchError::new(
            provider,
            WebSearchErrorKind::InvalidConfiguration,
            "OpenBitFun Search endpoint cannot contain user info or a fragment",
        ));
    }
    if endpoint.scheme() == "https" {
        return Ok(());
    }
    if endpoint.scheme() != "http" || !is_loopback_host(endpoint.host_str()) {
        return Err(WebSearchError::new(
            provider,
            WebSearchErrorKind::InvalidConfiguration,
            "OpenBitFun Search endpoint must use HTTPS; HTTP is allowed only for localhost or loopback IPs",
        ));
    }
    Ok(())
}

fn is_loopback_host(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn validate_auth(
    auth: &OpenBitFunSearchHttpAuth,
    provider: WebSearchProviderId,
) -> Result<(), WebSearchError> {
    match auth {
        OpenBitFunSearchHttpAuth::None => Ok(()),
        OpenBitFunSearchHttpAuth::Bearer(value) if !value.trim().is_empty() => Ok(()),
        OpenBitFunSearchHttpAuth::Header { name, value } if !value.trim().is_empty() => {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                WebSearchError::new(
                    provider.clone(),
                    WebSearchErrorKind::InvalidConfiguration,
                    format!("OpenBitFun Search auth header name is invalid: {error}"),
                )
            })?;
            let forbidden = [
                "host",
                "content-length",
                "content-type",
                "accept",
                "user-agent",
                "authorization",
                "proxy-authorization",
                "cookie",
                "set-cookie",
                "connection",
                "transfer-encoding",
                "te",
                "trailer",
                "upgrade",
                "origin",
                "referer",
            ];
            if forbidden.contains(&name.as_str()) {
                return Err(WebSearchError::new(
                    provider,
                    WebSearchErrorKind::InvalidConfiguration,
                    "OpenBitFun Search auth header cannot override a protocol-managed header",
                ));
            }
            HeaderValue::from_str(value).map_err(|error| {
                WebSearchError::new(
                    provider,
                    WebSearchErrorKind::InvalidConfiguration,
                    format!("OpenBitFun Search auth header value is invalid: {error}"),
                )
            })?;
            Ok(())
        }
        _ => Err(WebSearchError::new(
            provider,
            WebSearchErrorKind::InvalidConfiguration,
            "OpenBitFun Search authentication secret is not configured on this device",
        )),
    }
}

fn validate_content_type(
    headers: &reqwest::header::HeaderMap,
    provider: WebSearchProviderId,
) -> Result<(), WebSearchError> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if content_type.starts_with("application/json")
        || content_type.starts_with(OPENBITFUN_SEARCH_MEDIA_TYPE)
    {
        return Ok(());
    }
    Err(WebSearchError::new(
        provider,
        WebSearchErrorKind::InvalidResponse,
        "OpenBitFun Search endpoint did not return a supported JSON content type",
    ))
}
