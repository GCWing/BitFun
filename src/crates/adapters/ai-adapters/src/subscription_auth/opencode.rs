//! OpenCode Zen subscription login and credential resolution.
//!
//! Uses the OAuth 2.0 Device Authorization Grant against
//! `console.opencode.ai`, aligned with OpenCode's `provider/opencode.ts`. The
//! resolved credential targets the OpenAI-compatible Zen endpoint.

use super::store::{self, StoredCredential};
use super::{ResolvedCredential, StartedLogin};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const SERVER: &str = "https://console.opencode.ai";
const CLIENT_ID: &str = "opencode-cli";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const ZEN_BASE_URL: &str = "https://opencode.ai/zen/v1";
const ZEN_REQUEST_URL: &str = "https://opencode.ai/zen/v1/chat/completions";
const DEFAULT_MODEL: &str = "gpt-5.4";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "opencode";

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri_complete: String,
    #[serde(default)]
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct PendingResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrgResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build opencode http client")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn request_device_code() -> Result<DeviceCodeResponse> {
    let client = http_client()?;
    let resp = client
        .post(format!("{SERVER}/auth/device/code"))
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .context("call opencode device code endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "opencode device authorization failed: HTTP {status}: {body}"
        ));
    }
    resp.json().await.context("parse opencode device response")
}

/// Outcome of a single device-token poll.
enum DevicePoll {
    Authorized(TokenResponse),
    Pending,
    SlowDown,
}

/// One poll attempt against the device-token endpoint.
async fn poll_once(device_code: &str) -> Result<DevicePoll> {
    let client = http_client()?;
    let resp = client
        .post(format!("{SERVER}/auth/device/token"))
        .json(&serde_json::json!({
            "grant_type": DEVICE_GRANT,
            "device_code": device_code,
            "client_id": CLIENT_ID,
        }))
        .send()
        .await
        .context("call opencode device token endpoint")?;
    let body = resp.text().await.unwrap_or_default();
    if let Ok(tokens) = serde_json::from_str::<TokenResponse>(&body) {
        return Ok(DevicePoll::Authorized(tokens));
    }
    if let Ok(pending) = serde_json::from_str::<PendingResponse>(&body) {
        match pending.error.as_str() {
            "authorization_pending" => return Ok(DevicePoll::Pending),
            "slow_down" => return Ok(DevicePoll::SlowDown),
            other => return Err(anyhow!("opencode device authorization failed: {other}")),
        }
    }
    Err(anyhow!(
        "opencode device token response unrecognized: {body}"
    ))
}

async fn fetch_metadata(access: &str) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();
    metadata.insert(
        "server".to_string(),
        serde_json::Value::String(SERVER.to_string()),
    );
    let client = match http_client() {
        Ok(client) => client,
        Err(_) => return serde_json::Value::Object(metadata),
    };

    if let Ok(resp) = client
        .get(format!("{SERVER}/api/user"))
        .bearer_auth(access)
        .send()
        .await
    {
        if let Ok(user) = resp.json::<UserResponse>().await {
            if let Some(email) = user.email {
                metadata.insert("email".to_string(), serde_json::Value::String(email));
            }
            if let Some(id) = user.id {
                metadata.insert("account_id".to_string(), serde_json::Value::String(id));
            }
        }
    }

    if let Ok(resp) = client
        .get(format!("{SERVER}/api/orgs"))
        .bearer_auth(access)
        .send()
        .await
    {
        if let Ok(mut orgs) = resp.json::<Vec<OrgResponse>>().await {
            orgs.sort_by(|a, b| {
                a.name
                    .clone()
                    .unwrap_or_default()
                    .cmp(&b.name.clone().unwrap_or_default())
            });
            if let Some(org) = orgs.into_iter().next() {
                if let Some(id) = org.id {
                    metadata.insert("org_id".to_string(), serde_json::Value::String(id));
                }
                if let Some(name) = org.name {
                    metadata.insert("org_name".to_string(), serde_json::Value::String(name));
                }
            }
        }
    }

    serde_json::Value::Object(metadata)
}

async fn persist_tokens(tokens: TokenResponse) -> Result<()> {
    let _guard = super::store_lock(super::SubscriptionProvider::Opencode)
        .lock()
        .await;
    let expires = now_ms() + tokens.expires_in * 1000;
    let metadata = fetch_metadata(&tokens.access_token).await;
    let mut store = store::load().await.unwrap_or_default();
    store.insert(
        STORE_KEY.to_string(),
        StoredCredential::Oauth {
            refresh: tokens.refresh_token,
            access: tokens.access_token,
            expires,
            account_id: None,
            metadata: Some(metadata),
        },
    );
    store::save(&store).await?;
    log::info!("opencode subscription tokens saved");
    Ok(())
}

async fn refresh(refresh_token: &str) -> Result<TokenResponse> {
    let client = http_client()?;
    let resp = client
        .post(format!("{SERVER}/auth/device/token"))
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLIENT_ID,
        }))
        .send()
        .await
        .context("call opencode device token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "opencode token refresh failed: HTTP {status}: {body}"
        ));
    }
    resp.json().await.context("parse opencode token response")
}

/// Resolves OpenCode's device verification URI to an absolute URL.
///
/// The console API returns a path such as `/device?user_code=...` (same as
/// OpenCode's own client, which prefixes `https://console.opencode.ai`).
fn absolute_verification_url(uri: &str) -> String {
    let trimmed = uri.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return trimmed.to_string();
    }
    if trimmed.starts_with('/') {
        return format!("{SERVER}{trimmed}");
    }
    format!("{SERVER}/{trimmed}")
}

/// Starts the device-code login flow. The verification URL and user code are
/// returned immediately; the runner polls in the background.
pub(crate) async fn begin_login(cancel: CancellationToken) -> Result<StartedLogin> {
    let device = request_device_code().await?;
    let interval = device.interval.unwrap_or(5).max(1);
    let device_code = device.device_code.clone();
    let user_code = device.user_code.clone();
    let authorization_url = absolute_verification_url(&device.verification_uri_complete);

    let runner = async move {
        tokio::select! {
            _ = cancel.cancelled() => Err(anyhow!("login cancelled")),
            result = async {
                let mut wait = interval;
                loop {
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    match poll_once(&device_code).await? {
                        DevicePoll::Authorized(tokens) => return persist_tokens(tokens).await,
                        DevicePoll::Pending => {
                            wait = interval;
                        }
                        // RFC 8628: on slow_down, increase the poll interval
                        // by 5 seconds.
                        DevicePoll::SlowDown => {
                            wait += 5;
                        }
                    }
                }
            } => result,
        }
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: Some(user_code),
        instructions: "Open the verification link and enter the code, then return to BitFun."
            .to_string(),
        runner: Box::pin(runner),
    })
}

async fn ensure_fresh() -> Result<String> {
    let _guard = super::store_lock(super::SubscriptionProvider::Opencode)
        .lock()
        .await;
    let mut store = store::load().await.unwrap_or_default();
    let entry = store
        .get(STORE_KEY)
        .cloned()
        .ok_or_else(|| anyhow!("OpenCode Zen is not connected; sign in first"))?;
    match entry {
        StoredCredential::Api { key, .. } => Ok(key),
        StoredCredential::Oauth {
            refresh: refresh_token,
            access,
            expires,
            account_id,
            metadata,
        } => {
            if expires > now_ms() + REFRESH_LEEWAY_MS {
                return Ok(access);
            }
            let refreshed = refresh(&refresh_token).await?;
            let new_expires = now_ms() + refreshed.expires_in * 1000;
            store.insert(
                STORE_KEY.to_string(),
                StoredCredential::Oauth {
                    refresh: refreshed.refresh_token,
                    access: refreshed.access_token.clone(),
                    expires: new_expires,
                    account_id,
                    metadata,
                },
            );
            store::save(&store).await?;
            log::info!("opencode subscription tokens refreshed");
            Ok(refreshed.access_token)
        }
    }
}

/// Resolves the runtime credential (refreshing OAuth tokens if required).
pub(crate) async fn resolve() -> Result<ResolvedCredential> {
    let api_key = ensure_fresh().await?;
    Ok(ResolvedCredential {
        api_key,
        base_url: Some(ZEN_BASE_URL.to_string()),
        request_url: Some(ZEN_REQUEST_URL.to_string()),
        format: Some("openai".to_string()),
        extra_headers: HashMap::new(),
        expires_at: None,
    })
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", ZEN_BASE_URL, DEFAULT_MODEL)
}

#[cfg(test)]
mod tests {
    use super::absolute_verification_url;

    #[test]
    fn prefixes_relative_device_verification_path() {
        assert_eq!(
            absolute_verification_url("/device?user_code=FBPH-VLFC&client_id=opencode-cli"),
            "https://console.opencode.ai/device?user_code=FBPH-VLFC&client_id=opencode-cli"
        );
    }

    #[test]
    fn keeps_absolute_verification_url() {
        assert_eq!(
            absolute_verification_url(
                "https://console.opencode.ai/device?user_code=ABCD-1234&client_id=opencode-cli"
            ),
            "https://console.opencode.ai/device?user_code=ABCD-1234&client_id=opencode-cli"
        );
    }
}
