//! Shared GitHub identity used by every OpenBitFun product surface.
mod credentials;
mod flow;
pub use credentials::{
    clear_market_credentials, load_market_credentials, save_market_credentials,
    StoredMarketCredentials,
};
pub use flow::{poll_auth_flow, start_auth_flow};
use openbitfun_product_domains::account::GitHubUser;
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
pub const DEFAULT_ACCOUNT_API_URL: &str = "https://auth.openbitfun.com/api/v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAuthStart {
    pub transaction_id: String,
    pub transaction_secret: String,
    pub authorization_url: String,
    pub expires_at: i64,
    pub poll_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAuthPollRequest {
    pub transaction_id: String,
    pub transaction_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAuthPollResponse {
    pub status: String,
    pub tokens: Option<MarketTokenPair>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketTokenPair {
    pub access_token: String,
    pub access_expires_at: i64,
    pub refresh_token: String,
    pub refresh_expires_at: i64,
}

impl From<MarketTokenPair> for StoredMarketCredentials {
    fn from(value: MarketTokenPair) -> Self {
        Self {
            access_token: value.access_token,
            access_expires_at: value.access_expires_at,
            refresh_token: value.refresh_token,
            refresh_expires_at: value.refresh_expires_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketMe {
    pub user: GitHubUser,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct MarketClientError {
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
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

#[derive(Debug)]
pub struct AccountIdentityClient {
    base_url: String,
    client: reqwest::Client,
    credentials: Option<StoredMarketCredentials>,
}
impl AccountIdentityClient {
    /// Verify a controller credential without reading or mutating host credentials.
    pub async fn verify_access_token(access_token: &str) -> Result<MarketMe, MarketClientError> {
        if access_token.is_empty()
            || access_token.len() > 8192
            || access_token.chars().any(char::is_control)
        {
            return Err(local_error(
                "invalid_identity_token",
                "Invalid GitHub account credential.",
            ));
        }
        let client = crate::reqwest_client_builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|error| local_error("identity_client_failed", error.to_string()))?;
        let response = client
            .get(format!("{DEFAULT_ACCOUNT_API_URL}/me"))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(transport_error)?;
        let mut response = checked_response(response).await?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if body.len() + chunk.len() > 65536 {
                return Err(local_error(
                    "invalid_identity_response",
                    "Account response is too large.",
                ));
            }
            body.extend_from_slice(&chunk);
        }
        let me: MarketMe = serde_json::from_slice(&body)
            .map_err(|_| local_error("invalid_identity_response", "Invalid account response."))?;
        if me.user.github_id <= 0 || me.user.login.trim().is_empty() {
            return Err(local_error(
                "invalid_identity_response",
                "Invalid GitHub identity.",
            ));
        }
        Ok(me)
    }

    pub async fn from_environment() -> Result<Self, MarketClientError> {
        let base_url = std::env::var("OPENBITFUN_ACCOUNT_API_URL")
            .or_else(|_| std::env::var("OPENBITFUN_MINIAPP_MARKET_API_URL"))
            .unwrap_or_else(|_| DEFAULT_ACCOUNT_API_URL.to_string());
        Self::new(base_url).await
    }

    pub async fn new(base_url: impl Into<String>) -> Result<Self, MarketClientError> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let parsed = reqwest::Url::parse(&base_url)
            .map_err(|error| local_error("invalid_market_url", error.to_string()))?;
        let local_http = parsed.scheme() == "http"
            && parsed
                .host_str()
                .is_some_and(|host| matches!(host, "127.0.0.1" | "localhost" | "::1"));
        if parsed.scheme() != "https" && !local_http {
            return Err(local_error(
                "invalid_market_url",
                "The GitHub account API must use HTTPS.",
            ));
        }
        let client = crate::reqwest_client_builder()
            .user_agent(format!("OpenBitFun/{}", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| local_error("market_client_init_failed", error.to_string()))?;
        let credentials = load_market_credentials()
            .await
            .map_err(|error| local_error("credential_store_unavailable", error))?;
        Ok(Self {
            base_url,
            client,
            credentials,
        })
    }

    pub async fn start_desktop_auth(&self) -> Result<DesktopAuthStart, MarketClientError> {
        self.json(self.client.post(self.url("/auth/desktop/start")))
            .await
    }

    pub async fn poll_desktop_auth(
        &mut self,
        request: &DesktopAuthPollRequest,
    ) -> Result<DesktopAuthPollResponse, MarketClientError> {
        let response: DesktopAuthPollResponse = self
            .json(
                self.client
                    .post(self.url("/auth/desktop/poll"))
                    .json(request),
            )
            .await?;
        if let Some(tokens) = response.tokens.clone() {
            let credentials: StoredMarketCredentials = tokens.into();
            save_market_credentials(&credentials)
                .await
                .map_err(|error| local_error("credential_store_unavailable", error))?;
            self.credentials = Some(credentials);
        }
        Ok(response)
    }

    pub async fn me(&mut self) -> Result<Option<MarketMe>, MarketClientError> {
        if self.credentials.is_none() {
            return Ok(None);
        }
        self.refresh_if_needed().await?;
        let Some(credentials) = self.credentials.as_ref() else {
            return Ok(None);
        };
        let response = self
            .client
            .get(self.url("/me"))
            .bearer_auth(&credentials.access_token)
            .send()
            .await
            .map_err(transport_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            clear_market_credentials()
                .await
                .map_err(|error| local_error("credential_store_unavailable", error))?;
            self.credentials = None;
            return Ok(None);
        }
        Ok(Some(decode_json(checked_response(response).await?).await?))
    }

    pub async fn access_token(&mut self) -> Result<Option<String>, MarketClientError> {
        self.refresh_if_needed().await?;
        Ok(self
            .credentials
            .as_ref()
            .map(|credentials| credentials.access_token.clone()))
    }

    pub async fn logout(&mut self) -> Result<(), MarketClientError> {
        if let Some(credentials) = self.credentials.as_ref() {
            let response = self
                .client
                .post(self.url("/auth/logout"))
                .bearer_auth(&credentials.access_token)
                .send()
                .await
                .map_err(transport_error)?;
            if !response.status().is_success() && response.status() != StatusCode::UNAUTHORIZED {
                return Err(response_error(response).await);
            }
        }
        clear_market_credentials()
            .await
            .map_err(|error| local_error("credential_store_unavailable", error))?;
        self.credentials = None;
        Ok(())
    }

    async fn refresh_if_needed(&mut self) -> Result<(), MarketClientError> {
        let Some(credentials) = self.credentials.as_ref() else {
            return Ok(());
        };
        let now = chrono::Utc::now().timestamp();
        if credentials.refresh_expires_at <= now {
            clear_market_credentials()
                .await
                .map_err(|error| local_error("credential_store_unavailable", error))?;
            self.credentials = None;
            return Ok(());
        }
        if credentials.access_expires_at > now + 30 {
            return Ok(());
        }
        let refresh_token = credentials.refresh_token.clone();
        let response = self
            .client
            .post(self.url("/auth/refresh"))
            .json(&serde_json::json!({ "refreshToken": refresh_token }))
            .send()
            .await
            .map_err(transport_error)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            clear_market_credentials()
                .await
                .map_err(|error| local_error("credential_store_unavailable", error))?;
            self.credentials = None;
            return Ok(());
        }
        let tokens: MarketTokenPair = decode_json(checked_response(response).await?).await?;
        let stored: StoredMarketCredentials = tokens.into();
        save_market_credentials(&stored)
            .await
            .map_err(|error| local_error("credential_store_unavailable", error))?;
        self.credentials = Some(stored);
        Ok(())
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
            format!("The GitHub account service returned HTTP {status}."),
        ),
    }
}

fn transport_error(error: reqwest::Error) -> MarketClientError {
    local_error("account_unavailable", error.to_string())
}

fn local_error(code: impl Into<String>, message: impl Into<String>) -> MarketClientError {
    MarketClientError {
        code: code.into(),
        message: message.into(),
        request_id: None,
    }
}

impl std::fmt::Debug for MarketTokenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountTokenPair").finish_non_exhaustive()
    }
}
