//! Codex (ChatGPT) subscription login and credential resolution.
//!
//! Aligned with OpenCode's `plugin/openai/codex.ts`: browser PKCE login against
//! `auth.openai.com` on registered loopback ports, then Bearer access to
//! `chatgpt.com/backend-api/codex/responses`.

use super::store::{self, StoredCredential};
use super::{
    jwt, oauth_server, pkce::Pkce, ResolvedCredential, StartedLogin, SubscriptionHttpOptions,
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ISSUER: &str = "https://auth.openai.com";
const CALLBACK_PATH: &str = "/auth/callback";
const CALLBACK_PORT: u16 = 1455;
const DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const DEVICE_AUTHORIZATION_URL: &str = "https://auth.openai.com/codex/device";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
const OAUTH_POLLING_SAFETY_MARGIN_SECS: u64 = 3;
const SCOPE: &str = "openid profile email offline_access";
const CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const CHATGPT_REQUEST_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const DEFAULT_MODEL: &str = "gpt-5.5";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "codex";

fn redirect_uri(port: u16) -> String {
    oauth_server::loopback_redirect_uri(port, CALLBACK_PATH)
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_auth_id: String,
    user_code: String,
    interval: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationResponse {
    authorization_code: String,
    code_verifier: String,
}

fn opencode_user_agent() -> String {
    format!(
        "opencode/{} ({}; {})",
        super::OPENCODE_COMPAT_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn device_poll_interval(value: &serde_json::Value) -> u64 {
    value
        .as_str()
        .and_then(|value| value.parse::<u64>().ok())
        .or_else(|| value.as_u64())
        .filter(|value| *value > 0)
        .unwrap_or(5)
}

fn build_authorize_url(pkce: &Pkce, state: &str, redirect_uri: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", SCOPE),
        ("code_challenge", pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", "opencode"),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", key, urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{ISSUER}/oauth/authorize?{query}")
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "Codex")
}

async fn exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    options: &SubscriptionHttpOptions,
) -> Result<TokenResponse> {
    let client = http_client(options)?;
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier),
    ];
    let resp = client
        .post(format!("{ISSUER}/oauth/token"))
        .form(&params)
        .send()
        .await
        .context("call codex token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codex token exchange failed: HTTP {status}: {body}"
        ));
    }
    resp.json().await.context("parse codex token response")
}

async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<TokenResponse> {
    let client = http_client(options)?;
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
    ];
    let resp = client
        .post(format!("{ISSUER}/oauth/token"))
        .form(&params)
        .send()
        .await
        .context("call codex token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("codex token refresh failed: HTTP {status}: {body}"));
    }
    resp.json().await.context("parse codex token response")
}

async fn request_device_code(options: &SubscriptionHttpOptions) -> Result<DeviceCodeResponse> {
    let response = http_client(options)?
        .post(DEVICE_USER_CODE_URL)
        .header(reqwest::header::USER_AGENT, opencode_user_agent())
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        .context("call codex device authorization endpoint")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "codex device authorization failed: HTTP {status}: {body}"
        ));
    }
    let device = response
        .json::<DeviceCodeResponse>()
        .await
        .context("parse codex device authorization response")?;
    if device.device_auth_id.trim().is_empty() || device.user_code.trim().is_empty() {
        return Err(anyhow!(
            "codex device authorization response is missing an id or user code"
        ));
    }
    Ok(device)
}

async fn poll_device_authorization(
    device: &DeviceCodeResponse,
    options: &SubscriptionHttpOptions,
) -> Result<DeviceAuthorizationResponse> {
    let interval =
        device_poll_interval(&device.interval).saturating_add(OAUTH_POLLING_SAFETY_MARGIN_SECS);
    loop {
        let response = http_client(options)?
            .post(DEVICE_TOKEN_URL)
            .header(reqwest::header::USER_AGENT, opencode_user_agent())
            .json(&serde_json::json!({
                "device_auth_id": device.device_auth_id,
                "user_code": device.user_code,
            }))
            .send()
            .await
            .context("poll codex device authorization endpoint")?;
        if response.status().is_success() {
            return response
                .json::<DeviceAuthorizationResponse>()
                .await
                .context("parse codex device authorization token response");
        }
        if !matches!(
            response.status(),
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
        ) {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "codex device authorization failed: HTTP {status}: {body}"
            ));
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn account_id_from(tokens: &TokenResponse) -> Option<String> {
    tokens
        .id_token
        .as_deref()
        .and_then(jwt::chatgpt_account_id)
        .or_else(|| {
            tokens
                .access_token
                .as_deref()
                .and_then(jwt::chatgpt_account_id)
        })
}

fn metadata_from(tokens: &TokenResponse) -> Option<serde_json::Value> {
    let email = tokens.id_token.as_deref().and_then(jwt::email)?;
    Some(serde_json::json!({ "email": email }))
}

async fn persist_tokens(tokens: TokenResponse, expected_revision: u64) -> Result<()> {
    let access = tokens
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("codex token response missing access_token"))?;
    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow!("codex token response missing refresh_token"))?;
    let expires = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
    let account_id = account_id_from(&tokens);
    let metadata = metadata_from(&tokens);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Codex, outcome)?;
    log::info!("codex subscription tokens saved");
    Ok(())
}

async fn begin_browser_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let pkce = Pkce::generate();
    let state = super::pkce::random_state();
    let (listener, callback_port) = oauth_server::bind_loopback_ports(&[CALLBACK_PORT]).await?;
    let redirect_uri = redirect_uri(callback_port);
    let authorization_url = build_authorize_url(&pkce, &state, &redirect_uri);
    let verifier = pkce.verifier.clone();

    let runner = async move {
        super::authorize_then_persist(
            super::SubscriptionProvider::Codex,
            cancel,
            async {
                let params =
                    oauth_server::wait_for_callback(listener, CALLBACK_PATH, &state).await?;
                let code = params
                    .get("code")
                    .cloned()
                    .ok_or_else(|| anyhow!("codex callback missing code"))?;
                exchange_code(&code, &verifier, &redirect_uri, &options).await
            },
            move |tokens| persist_tokens(tokens, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        method: super::SubscriptionLoginMethod::Browser,
        authorization_url,
        user_code: None,
        instructions: "Complete authorization in your browser, then return to OpenBitFun."
            .to_string(),
        runner: Box::pin(runner),
    })
}

async fn begin_device_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let device = request_device_code(&options).await?;
    let user_code = device.user_code.clone();
    let runner = async move {
        super::authorize_then_persist(
            super::SubscriptionProvider::Codex,
            cancel,
            async {
                let authorization = poll_device_authorization(&device, &options).await?;
                exchange_code(
                    &authorization.authorization_code,
                    &authorization.code_verifier,
                    DEVICE_REDIRECT_URI,
                    &options,
                )
                .await
            },
            move |tokens| persist_tokens(tokens, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        method: super::SubscriptionLoginMethod::Device,
        authorization_url: DEVICE_AUTHORIZATION_URL.to_string(),
        user_code: Some(user_code.clone()),
        instructions: format!("Open the verification link and enter code: {user_code}"),
        runner: Box::pin(runner),
    })
}

/// Starts the selected OpenCode-compatible Codex authorization flow. Legacy
/// callers that do not select a method keep browser-first behavior, with a
/// device-flow fallback when the registered loopback callback is unavailable.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    method: Option<super::SubscriptionLoginMethod>,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    match method {
        Some(super::SubscriptionLoginMethod::Browser) => {
            begin_browser_login(cancel, expected_revision, options).await
        }
        Some(super::SubscriptionLoginMethod::Device) => {
            begin_device_login(cancel, expected_revision, options).await
        }
        None => match begin_browser_login(cancel.clone(), expected_revision, options.clone()).await
        {
            Ok(started) => Ok(started),
            Err(browser_error) => {
                log::info!(
                "Codex browser login unavailable; using device authorization: {browser_error:#}"
            );
                begin_device_login(cancel, expected_revision, options)
                .await
                .with_context(|| {
                    format!(
                        "start Codex device authorization after browser login was unavailable: {browser_error:#}"
                    )
                })
            }
        },
    }
}

/// Ensures the stored access token is fresh, refreshing it when needed. Returns
/// the current `(access, account_id, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, Option<String>, i64)> {
    let _refresh_lease = store::acquire_provider_refresh_lease(STORE_KEY).await?;
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("Codex is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("Codex credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, account_id, expires));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("codex refresh response missing access_token"))?;
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_account_id = account_id_from(&refreshed).or(account_id);
    let new_metadata = metadata_from(&refreshed).or(metadata);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id.clone(),
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("codex subscription tokens refreshed");
            Ok((new_access, new_account_id, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::Codex,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access,
                    expires,
                    account_id,
                    ..
                }) if expires > now_ms() => {
                    log::info!("codex refresh reused tokens committed by a concurrent refresh");
                    Ok((access, account_id, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::Codex,
                    current_revision,
                )),
            }
        }
    }
}

/// Resolves the runtime credential (refreshing tokens if required).
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, account_id, expires) = ensure_fresh(options).await?;
    let mut headers = HashMap::new();
    if let Some(account) = account_id {
        headers.insert("ChatGPT-Account-ID".to_string(), account);
    }
    headers.insert("originator".to_string(), "opencode".to_string());
    headers.insert("session-id".to_string(), Uuid::new_v4().to_string());
    headers.insert("User-Agent".to_string(), opencode_user_agent());
    if let Some(residency) = jwt::chatgpt_compute_residency(&access) {
        headers.insert("x-openai-internal-codex-residency".to_string(), residency);
    }

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(CHATGPT_BASE_URL.to_string()),
        request_url: Some(CHATGPT_REQUEST_URL.to_string()),
        format: Some("responses".to_string()),
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("responses", CHATGPT_BASE_URL, DEFAULT_MODEL)
}

#[cfg(test)]
mod tests {
    use super::{
        build_authorize_url, device_poll_interval, opencode_user_agent, redirect_uri,
        CALLBACK_PORT, DEFAULT_MODEL,
    };
    use crate::subscription_auth::pkce::Pkce;

    #[test]
    fn uses_registered_localhost_redirect_uri() {
        let primary_redirect_uri = redirect_uri(CALLBACK_PORT);
        assert_eq!(primary_redirect_uri, "http://localhost:1455/auth/callback");

        let authorize_url = build_authorize_url(&Pkce::generate(), "state", &primary_redirect_uri);
        assert!(
            authorize_url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback")
        );
    }

    #[test]
    fn matches_opencode_device_polling_and_current_default_model() {
        assert_eq!(device_poll_interval(&serde_json::json!("5")), 5);
        assert_eq!(device_poll_interval(&serde_json::json!(2)), 2);
        assert_eq!(device_poll_interval(&serde_json::json!(0)), 5);
        assert_eq!(DEFAULT_MODEL, "gpt-5.5");
        assert_eq!(super::super::OPENCODE_COMPAT_VERSION, "1.18.25");
        assert!(opencode_user_agent().starts_with("opencode/1.18.25 ("));
    }
}
