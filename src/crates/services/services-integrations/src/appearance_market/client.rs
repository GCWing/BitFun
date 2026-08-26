use bitfun_product_domains::appearance_market::{
    AppearanceAdminSubmissionDetail, AppearanceCursorPage, AppearanceMarketListingDetail,
    AppearanceMarketListingSummary, AppearanceMarketSort, AppearanceMarketSubmission,
    AppearanceMarketSubmissionDraftRequest, AppearanceMarketSubmissionStatus,
    AppearanceReviewDecisionRequest, APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
    APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
};
use reqwest::{RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::miniapp_market::{
    DesktopAuthPollRequest, DesktopAuthPollResponse, DesktopAuthStart, MarketClient,
    MarketClientError, MarketMe,
};

const DEFAULT_APPEARANCE_MARKET_API_URL: &str = "https://market.openbitfun.com/skin/api/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketBrowseRequest {
    #[serde(default)]
    pub query: String,
    pub mode: Option<String>,
    #[serde(default)]
    pub sort: AppearanceMarketSort,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug)]
pub struct AppearanceMarketClient {
    base_url: String,
    client: reqwest::Client,
    identity: MarketClient,
}

impl AppearanceMarketClient {
    pub async fn from_environment() -> Result<Self, MarketClientError> {
        let base_url = std::env::var("BITFUN_APPEARANCE_MARKET_API_URL")
            .unwrap_or_else(|_| DEFAULT_APPEARANCE_MARKET_API_URL.to_string());
        let identity = MarketClient::from_environment().await?;
        Self::with_identity(base_url, identity)
    }

    pub async fn new(
        base_url: impl Into<String>,
        identity_base_url: impl Into<String>,
    ) -> Result<Self, MarketClientError> {
        let identity = MarketClient::new(identity_base_url).await?;
        Self::with_identity(base_url, identity)
    }

    fn with_identity(
        base_url: impl Into<String>,
        identity: MarketClient,
    ) -> Result<Self, MarketClientError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        validate_market_url(&base_url)?;
        let client = crate::reqwest_client_builder()
            .user_agent(format!("BitFun-Desktop/{}", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| local_error("market_client_init_failed", error.to_string()))?;
        Ok(Self {
            base_url,
            client,
            identity,
        })
    }

    pub async fn browse(
        &self,
        request: &AppearanceMarketBrowseRequest,
    ) -> Result<AppearanceCursorPage<AppearanceMarketListingSummary>, MarketClientError> {
        let mut query = vec![
            ("sort", sort_value(request.sort).to_string()),
            ("limit", request.limit.unwrap_or(20).to_string()),
        ];
        if !request.query.trim().is_empty() {
            query.push(("q", request.query.trim().to_string()));
        }
        if let Some(mode) = request
            .mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
        {
            query.push(("mode", mode.to_string()));
        }
        if let Some(cursor) = request.cursor.as_ref().filter(|value| !value.is_empty()) {
            query.push(("cursor", cursor.clone()));
        }
        self.json(self.client.get(self.url("/listings")).query(&query))
            .await
    }

    pub async fn listing(
        &self,
        slug: &str,
    ) -> Result<AppearanceMarketListingDetail, MarketClientError> {
        self.json(
            self.client
                .get(self.url(&format!("/listings/{}", urlencoding::encode(slug)))),
        )
        .await
    }

    pub async fn download_release(
        &self,
        slug: &str,
        release_number: u32,
    ) -> Result<Vec<u8>, MarketClientError> {
        let response = self
            .client
            .get(self.url(&format!(
                "/listings/{}/releases/{release_number}/download",
                urlencoding::encode(slug)
            )))
            .send()
            .await
            .map_err(transport_error)?;
        let mut response = checked_response(response).await?;
        if response
            .content_length()
            .is_some_and(|length| length > APPEARANCE_MARKET_MAX_PACKAGE_BYTES)
        {
            return Err(local_error(
                "package_too_large",
                "The Appearance market download exceeds 96 MiB.",
            ));
        }
        let mut bytes = Vec::with_capacity(
            response
                .content_length()
                .unwrap_or_default()
                .min(APPEARANCE_MARKET_MAX_PACKAGE_BYTES) as usize,
        );
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if bytes.len().saturating_add(chunk.len())
                > APPEARANCE_MARKET_MAX_PACKAGE_BYTES as usize
            {
                return Err(local_error(
                    "package_too_large",
                    "The Appearance market download exceeds 96 MiB.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    pub async fn me(&mut self) -> Result<Option<MarketMe>, MarketClientError> {
        self.identity.me().await
    }

    pub async fn start_desktop_auth(&self) -> Result<DesktopAuthStart, MarketClientError> {
        self.identity.start_desktop_auth().await
    }

    pub async fn poll_desktop_auth(
        &mut self,
        request: &DesktopAuthPollRequest,
    ) -> Result<DesktopAuthPollResponse, MarketClientError> {
        self.identity.poll_desktop_auth(request).await
    }

    pub async fn list_submissions(
        &mut self,
    ) -> Result<Vec<AppearanceMarketSubmission>, MarketClientError> {
        let mut submissions = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let mut query = vec![("limit", "50".to_string())];
            if let Some(value) = &cursor {
                query.push(("cursor", value.clone()));
            }
            let request = self
                .authorized(self.client.get(self.url("/submissions")).query(&query))
                .await?;
            let page: AppearanceCursorPage<AppearanceMarketSubmission> = self.json(request).await?;
            submissions.extend(page.items);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(submissions);
            }
        }
        Err(local_error(
            "submission_history_too_large",
            "Skin submission history exceeds the client pagination safety limit.",
        ))
    }

    pub async fn create_submission(
        &mut self,
        draft: &AppearanceMarketSubmissionDraftRequest,
    ) -> Result<AppearanceMarketSubmission, MarketClientError> {
        let request = self
            .authorized(self.client.post(self.url("/submissions")))
            .await?;
        self.json(request.json(draft)).await
    }

    pub async fn upload_submission_package(
        &mut self,
        submission_id: &str,
        bytes: Vec<u8>,
    ) -> Result<AppearanceMarketSubmission, MarketClientError> {
        let request = self
            .authorized(self.client.put(self.url(&format!(
                "/submissions/{}/package",
                urlencoding::encode(submission_id)
            ))))
            .await?;
        self.json(
            request
                .header(
                    reqwest::header::CONTENT_TYPE,
                    APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE,
                )
                .body(bytes),
        )
        .await
    }

    pub async fn submit_submission(
        &mut self,
        submission_id: &str,
    ) -> Result<AppearanceMarketSubmission, MarketClientError> {
        let request = self
            .authorized(self.client.post(self.url(&format!(
                "/submissions/{}/submit",
                urlencoding::encode(submission_id)
            ))))
            .await?;
        self.json(request).await
    }

    pub async fn withdraw_submission(
        &mut self,
        submission_id: &str,
    ) -> Result<AppearanceMarketSubmission, MarketClientError> {
        let request = self
            .authorized(self.client.delete(self.url(&format!(
                "/submissions/{}",
                urlencoding::encode(submission_id)
            ))))
            .await?;
        self.json(request).await
    }

    pub async fn list_admin_submissions(
        &mut self,
        status: AppearanceMarketSubmissionStatus,
    ) -> Result<Vec<AppearanceMarketSubmission>, MarketClientError> {
        let mut submissions = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let mut query = vec![
                ("status", submission_status_value(status).to_string()),
                ("limit", "50".to_string()),
            ];
            if let Some(value) = &cursor {
                query.push(("cursor", value.clone()));
            }
            let request = self
                .authorized(
                    self.client
                        .get(self.url("/admin/submissions"))
                        .query(&query),
                )
                .await?;
            let page: AppearanceCursorPage<AppearanceMarketSubmission> = self.json(request).await?;
            submissions.extend(page.items);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(submissions);
            }
        }
        Err(local_error(
            "review_queue_too_large",
            "Skin review queue exceeds the client pagination safety limit.",
        ))
    }

    pub async fn admin_submission(
        &mut self,
        submission_id: &str,
    ) -> Result<AppearanceAdminSubmissionDetail, MarketClientError> {
        let request = self
            .authorized(self.client.get(self.url(&format!(
                "/admin/submissions/{}",
                urlencoding::encode(submission_id)
            ))))
            .await?;
        self.json(request).await
    }

    pub async fn review_submission(
        &mut self,
        submission_id: &str,
        decision: &AppearanceReviewDecisionRequest,
    ) -> Result<AppearanceAdminSubmissionDetail, MarketClientError> {
        let request = self
            .authorized(self.client.post(self.url(&format!(
                "/admin/submissions/{}/decision",
                urlencoding::encode(submission_id)
            ))))
            .await?;
        self.json(request.json(decision)).await
    }

    async fn authorized(
        &mut self,
        request: RequestBuilder,
    ) -> Result<RequestBuilder, MarketClientError> {
        let token = self.identity.access_token().await?.ok_or_else(|| {
            local_error(
                "authentication_required",
                "Sign in with GitHub to continue.",
            )
        })?;
        Ok(request.bearer_auth(token))
    }

    async fn json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, MarketClientError> {
        let response = request.send().await.map_err(transport_error)?;
        decode_json(checked_response(response).await?).await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn validate_market_url(url: &str) -> Result<(), MarketClientError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| local_error("invalid_market_url", error.to_string()))?;
    let local_http = parsed.scheme() == "http"
        && parsed
            .host_str()
            .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "::1"));
    if parsed.scheme() != "https" && !local_http {
        return Err(local_error(
            "invalid_market_url",
            "The Appearance market API must use HTTPS.",
        ));
    }
    Ok(())
}

fn sort_value(sort: AppearanceMarketSort) -> &'static str {
    match sort {
        AppearanceMarketSort::Newest => "newest",
        AppearanceMarketSort::Downloads => "downloads",
    }
}

fn submission_status_value(status: AppearanceMarketSubmissionStatus) -> &'static str {
    match status {
        AppearanceMarketSubmissionStatus::Draft => "draft",
        AppearanceMarketSubmissionStatus::Submitted => "submitted",
        AppearanceMarketSubmissionStatus::Approved => "approved",
        AppearanceMarketSubmissionStatus::Rejected => "rejected",
        AppearanceMarketSubmissionStatus::Withdrawn => "withdrawn",
    }
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: String,
    message: String,
    request_id: Option<String>,
}

async fn checked_response(response: Response) -> Result<Response, MarketClientError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(response_error(response).await)
    }
}

async fn decode_json<T: DeserializeOwned>(response: Response) -> Result<T, MarketClientError> {
    response
        .json()
        .await
        .map_err(|error| local_error("invalid_market_response", error.to_string()))
}

async fn response_error(response: Response) -> MarketClientError {
    let status = response.status();
    match response.json::<ErrorEnvelope>().await {
        Ok(envelope) => MarketClientError {
            code: envelope.error.code,
            message: envelope.error.message,
            request_id: envelope.error.request_id,
        },
        Err(_) => local_error(
            "market_request_failed",
            format!("The Appearance market returned HTTP {status}."),
        ),
    }
}

fn transport_error(error: reqwest::Error) -> MarketClientError {
    local_error("market_unavailable", error.to_string())
}

fn local_error(code: impl Into<String>, message: impl Into<String>) -> MarketClientError {
    MarketClientError {
        code: code.into(),
        message: message.into(),
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_local_plain_http() {
        let error = validate_market_url("http://example.com/skin/api/v1").unwrap_err();
        assert_eq!(error.code, "invalid_market_url");
        assert!(validate_market_url("http://127.0.0.1:9720/skin/api/v1").is_ok());
    }
}
