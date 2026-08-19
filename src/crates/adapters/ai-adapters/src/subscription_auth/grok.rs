//! Grok Build (SuperGrok) account login and credential resolution.
//!
//! Authentication uses xAI's public Grok CLI OAuth client with the RFC 8628
//! device flow. Subscription inference is pinned to the official Grok Build
//! proxy so the session token cannot be redirected by model configuration.

use super::jwt;
use super::store::{self, StoredCredential};
use super::{ResolvedCredential, StartedLogin, SubscriptionHttpOptions};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const DEVICE_AUTHORIZATION_URL: &str = "https://auth.x.ai/oauth2/device/code";
const TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
const GROK_BUILD_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
const GROK_BUILD_REQUEST_URL: &str = "https://cli-chat-proxy.grok.com/v1/chat/completions";
const DEFAULT_MODEL: &str = "grok-build";
const STORE_KEY: &str = "grok";
const DEFAULT_TOKEN_LIFETIME_SECS: i64 = 60 * 60;
const DEFAULT_DEVICE_LIFETIME_SECS: i64 = 5 * 60;
const DEFAULT_POLL_INTERVAL_SECS: i64 = 5;
const SLOW_DOWN_INCREMENT_SECS: i64 = 5;
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    expires_in: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    interval: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_i64")]
    expires_in: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct TokenErrorResponse {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn deserialize_optional_i64<'de, D>(deserializer: D) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(string) => string.parse::<i64>().ok(),
        _ => None,
    }))
}

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "Grok Build")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn positive_seconds(value: Option<i64>, fallback: i64) -> i64 {
    value.filter(|value| *value > 0).unwrap_or(fallback)
}

fn expires_at_ms(expires_in: Option<i64>) -> i64 {
    now_ms().saturating_add(
        positive_seconds(expires_in, DEFAULT_TOKEN_LIFETIME_SECS).saturating_mul(1000),
    )
}

fn oauth_request(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::USER_AGENT,
            format!("BitFun/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("x-grok-client-version", env!("CARGO_PKG_VERSION"))
        .header("x-grok-client-surface", "ui")
}

fn validate_user_code(code: &str) -> Result<()> {
    if code.is_empty()
        || !code
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(anyhow!(
            "Grok Build returned an invalid device authorization code"
        ));
    }
    Ok(())
}

fn validate_verification_url(url: &str) -> Result<()> {
    if url.chars().any(|character| character.is_ascii_control()) {
        return Err(anyhow!(
            "Grok Build returned an invalid device verification URL"
        ));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| anyhow!("Grok Build returned an invalid device verification URL"))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(anyhow!(
            "Grok Build returned an unsupported device verification URL"
        ));
    }
    Ok(())
}

async fn request_device_code(options: &SubscriptionHttpOptions) -> Result<DeviceCodeResponse> {
    let client = http_client(options)?;
    let response = oauth_request(client.post(DEVICE_AUTHORIZATION_URL))
        .form(&[
            ("client_id", CLIENT_ID),
            ("scope", SCOPE),
            ("referrer", "bitfun"),
        ])
        .send()
        .await
        .context("call Grok Build device code endpoint")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Grok Build device authorization failed: HTTP {status}: {body}"
        ));
    }

    let device = response
        .json::<DeviceCodeResponse>()
        .await
        .context("parse Grok Build device authorization response")?;
    if device.device_code.trim().is_empty() {
        return Err(anyhow!(
            "Grok Build device authorization response missing device_code"
        ));
    }
    validate_user_code(&device.user_code)?;
    validate_verification_url(&device.verification_uri)?;
    if let Some(url) = device.verification_uri_complete.as_deref() {
        validate_verification_url(url)?;
    }
    Ok(device)
}

enum DevicePoll {
    Authorized(TokenResponse),
    Pending,
    SlowDown,
}

fn classify_device_poll_error(
    status: reqwest::StatusCode,
    error: &TokenErrorResponse,
) -> Result<DevicePoll> {
    match error.error.as_str() {
        "authorization_pending" => Ok(DevicePoll::Pending),
        "slow_down" => Ok(DevicePoll::SlowDown),
        "access_denied" | "authorization_denied" => {
            Err(anyhow!("Grok Build device authorization was denied"))
        }
        "expired_token" => Err(anyhow!("Grok Build device authorization code expired")),
        _ => {
            let detail = error
                .error_description
                .as_deref()
                .filter(|detail| !detail.trim().is_empty())
                .unwrap_or_else(|| {
                    if error.error.is_empty() {
                        "unrecognized response"
                    } else {
                        &error.error
                    }
                });
            Err(anyhow!(
                "Grok Build device token exchange failed: HTTP {status}: {detail}"
            ))
        }
    }
}

async fn poll_once(device_code: &str, options: &SubscriptionHttpOptions) -> Result<DevicePoll> {
    let client = http_client(options)?;
    let response = oauth_request(client.post(TOKEN_URL))
        .form(&[
            ("grant_type", DEVICE_CODE_GRANT_TYPE),
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
        ])
        .send()
        .await
        .context("call Grok Build device token endpoint")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status.is_success() {
        let tokens = serde_json::from_str::<TokenResponse>(&body)
            .context("parse Grok Build token response")?;
        return Ok(DevicePoll::Authorized(tokens));
    }

    let error = serde_json::from_str::<TokenErrorResponse>(&body).unwrap_or_default();
    classify_device_poll_error(status, &error)
}

fn account_id_from(tokens: &TokenResponse) -> Option<String> {
    tokens
        .id_token
        .as_deref()
        .and_then(jwt::subject)
        .or_else(|| jwt::subject(&tokens.access_token))
}

fn metadata_from(
    tokens: &TokenResponse,
    previous: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut object = previous
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let email = tokens
        .id_token
        .as_deref()
        .and_then(jwt::email)
        .or_else(|| jwt::email(&tokens.access_token));
    if let Some(email) = email {
        object.insert("email".to_string(), serde_json::Value::String(email));
    }
    if object.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(object))
    }
}

async fn persist_tokens(tokens: TokenResponse, expected_revision: u64) -> Result<()> {
    if tokens.access_token.trim().is_empty() {
        return Err(anyhow!("Grok Build token response missing access_token"));
    }
    let refresh = tokens
        .refresh_token
        .clone()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| anyhow!("Grok Build token response missing refresh_token"))?;
    let expires = expires_at_ms(tokens.expires_in);
    let account_id = account_id_from(&tokens);
    let metadata = metadata_from(&tokens, None);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh,
            access: tokens.access_token,
            expires,
            account_id,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Grok, outcome)?;
    log::info!("Grok Build subscription tokens saved");
    Ok(())
}

async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<TokenResponse> {
    let client = http_client(options)?;
    let response = oauth_request(client.post(TOKEN_URL))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .context("call Grok Build token refresh endpoint")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "Grok Build token refresh failed: HTTP {status}: {body}"
        ));
    }
    response
        .json::<TokenResponse>()
        .await
        .context("parse Grok Build token refresh response")
}

/// Starts the device-code login flow. The verification URL and user code are
/// returned immediately; the runner polls in the background.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let device = request_device_code(&options).await?;
    let interval = positive_seconds(device.interval, DEFAULT_POLL_INTERVAL_SECS);
    let expires_in = positive_seconds(device.expires_in, DEFAULT_DEVICE_LIFETIME_SECS)
        .min(super::LOGIN_TIMEOUT.as_secs() as i64);
    let device_code = device.device_code.clone();
    let user_code = device.user_code.clone();
    let authorization_url = device
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| device.verification_uri.clone());

    let runner = async move {
        super::authorize_then_persist(
            super::SubscriptionProvider::Grok,
            cancel,
            async {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in as u64);
                let mut wait = interval;
                loop {
                    let sleep = Duration::from_secs(wait as u64);
                    if tokio::time::Instant::now() + sleep > deadline {
                        return Err(anyhow!("Grok Build device authorization code expired"));
                    }
                    tokio::time::sleep(sleep).await;
                    match poll_once(&device_code, &options).await? {
                        DevicePoll::Authorized(tokens) => return Ok(tokens),
                        DevicePoll::Pending => wait = interval,
                        DevicePoll::SlowDown => {
                            wait = wait.saturating_add(SLOW_DOWN_INCREMENT_SECS)
                        }
                    }
                }
            },
            move |tokens| persist_tokens(tokens, expected_revision),
        )
        .await
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: Some(user_code),
        instructions: "Open the verification link, confirm the code, then return to BitFun."
            .to_string(),
        runner: Box::pin(runner),
    })
}

async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, i64)> {
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
        .ok_or_else(|| anyhow!("Grok Build is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("Grok Build credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, expires));
    }

    let refreshed = refresh(&refresh_token, options).await?;
    if refreshed.access_token.trim().is_empty() {
        return Err(anyhow!(
            "Grok Build token refresh response missing access_token"
        ));
    }
    let new_refresh = refreshed
        .refresh_token
        .clone()
        .filter(|token| !token.trim().is_empty())
        .unwrap_or(refresh_token);
    let new_expires = expires_at_ms(refreshed.expires_in);
    let new_account_id = account_id_from(&refreshed).or(account_id);
    let new_metadata = metadata_from(&refreshed, metadata);
    let new_access = refreshed.access_token.clone();
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id,
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("Grok Build subscription tokens refreshed");
            Ok((new_access, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::Grok,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access, expires, ..
                }) if expires > now_ms() => {
                    log::info!(
                        "Grok Build refresh reused tokens committed by a concurrent refresh"
                    );
                    Ok((access, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::Grok,
                    current_revision,
                )),
            }
        }
    }
}

fn selected_model(model: &str) -> Result<String> {
    let model = model.trim();
    let model = if model.is_empty() {
        DEFAULT_MODEL
    } else {
        model
    };
    reqwest::header::HeaderValue::from_str(model).with_context(|| {
        format!("Grok Build model id cannot be used as a request header: {model}")
    })?;
    Ok(model.to_string())
}

fn inference_headers(model: &str) -> Result<HashMap<String, String>> {
    let mut headers = HashMap::new();
    headers.insert("X-XAI-Token-Auth".to_string(), "xai-grok-cli".to_string());
    headers.insert("x-grok-model-override".to_string(), selected_model(model)?);
    Ok(headers)
}

/// Resolves the runtime credential and pins it to the trusted Grok Build
/// subscription endpoint. The proxy routes by header rather than JSON model.
pub(crate) async fn resolve_for(
    model: &str,
    options: &SubscriptionHttpOptions,
) -> Result<ResolvedCredential> {
    let headers = inference_headers(model)?;
    let (access, expires) = ensure_fresh(options).await?;

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(GROK_BUILD_BASE_URL.to_string()),
        request_url: Some(GROK_BUILD_REQUEST_URL.to_string()),
        format: Some("openai".to_string()),
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    resolve_for(DEFAULT_MODEL, options).await
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("openai", GROK_BUILD_BASE_URL, DEFAULT_MODEL)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_device_poll_error, inference_headers, selected_model, suggested,
        validate_user_code, validate_verification_url, DeviceCodeResponse, DevicePoll,
        TokenErrorResponse, TokenResponse, DEFAULT_MODEL, GROK_BUILD_BASE_URL,
    };

    #[test]
    fn accepts_https_verification_urls_and_rejects_unsafe_urls() {
        validate_verification_url("https://accounts.x.ai/oauth2/device?user_code=ABCD-EFGH")
            .unwrap();
        assert!(validate_verification_url("javascript:alert(1)").is_err());
        assert!(validate_verification_url("http://accounts.x.ai/oauth2/device").is_err());
        assert!(validate_verification_url("https://accounts.x.ai/\nmalicious").is_err());
    }

    #[test]
    fn validates_device_user_code() {
        validate_user_code("ABCD-EFGH").unwrap();
        assert!(validate_user_code("").is_err());
        assert!(validate_user_code("ABCD\nEFGH").is_err());
    }

    #[test]
    fn parses_numeric_or_string_oauth_lifetimes() {
        let device: DeviceCodeResponse = serde_json::from_value(serde_json::json!({
            "device_code": "device",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://accounts.x.ai/oauth2/device",
            "expires_in": "300",
            "interval": 5
        }))
        .unwrap();
        assert_eq!(device.expires_in, Some(300));
        assert_eq!(device.interval, Some(5));

        let tokens: TokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "access",
            "refresh_token": "refresh",
            "expires_in": "3600"
        }))
        .unwrap();
        assert_eq!(tokens.expires_in, Some(3600));
    }

    #[test]
    fn defaults_blank_model_and_rejects_header_injection() {
        assert_eq!(selected_model("  ").unwrap(), DEFAULT_MODEL);
        assert_eq!(
            selected_model("grok-build-fast").unwrap(),
            "grok-build-fast"
        );
        assert!(selected_model("grok-build\r\nx-evil: value").is_err());
    }

    #[test]
    fn uses_official_subscription_route_and_required_headers() {
        assert_eq!(suggested(), ("openai", GROK_BUILD_BASE_URL, DEFAULT_MODEL));
        let headers = inference_headers("grok-build-fast").unwrap();
        assert_eq!(
            headers.get("X-XAI-Token-Auth").map(String::as_str),
            Some("xai-grok-cli")
        );
        assert_eq!(
            headers.get("x-grok-model-override").map(String::as_str),
            Some("grok-build-fast")
        );
    }

    #[test]
    fn follows_rfc_8628_pending_slow_down_and_terminal_errors() {
        let pending = TokenErrorResponse {
            error: "authorization_pending".to_string(),
            error_description: None,
        };
        assert!(matches!(
            classify_device_poll_error(reqwest::StatusCode::BAD_REQUEST, &pending).unwrap(),
            DevicePoll::Pending
        ));

        let slow_down = TokenErrorResponse {
            error: "slow_down".to_string(),
            error_description: None,
        };
        assert!(matches!(
            classify_device_poll_error(reqwest::StatusCode::BAD_REQUEST, &slow_down).unwrap(),
            DevicePoll::SlowDown
        ));

        let denied = TokenErrorResponse {
            error: "access_denied".to_string(),
            error_description: None,
        };
        let Err(error) = classify_device_poll_error(reqwest::StatusCode::BAD_REQUEST, &denied)
        else {
            panic!("access_denied must be terminal");
        };
        assert!(error.to_string().contains("denied"));
    }
}
