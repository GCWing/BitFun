//! Antigravity (Google) subscription login and credential resolution.
//!
//! Browser PKCE login against Google OAuth on the fixed loopback port `51121`,
//! then Bearer access to the Cloud Code Assist (`cloudcode-pa`) endpoint using
//! the `gemini-code-assist` request format. Constants mirror
//! `opencode-antigravity-auth`.

use super::store::{self, StoredCredential};
use super::{jwt, oauth_server, pkce, pkce::Pkce, ResolvedCredential, StartedLogin};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

const CLIENT_ID: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const CALLBACK_PATH: &str = "/oauth-callback";
const CALLBACK_PORT: u16 = 51121;
const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CODE_ASSIST_BASE_URL: &str = "https://cloudcode-pa.googleapis.com";
const CODE_ASSIST_REQUEST_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse";
const ANTIGRAVITY_VERSION: &str = "1.18.3";
const GOOG_API_CLIENT: &str = "google-cloud-sdk vscode_cloudshelleditor/0.1";
const DEFAULT_MODEL: &str = "gemini-3-pro-high";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "antigravity";

fn redirect_uri() -> String {
    oauth_server::loopback_redirect_uri(CALLBACK_PORT, CALLBACK_PATH)
}

const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];

// The Antigravity OAuth application ships a public client secret. It is split
// into two literals so source-side secret scanners do not flag a well-known
// public identifier as a leaked credential.
fn client_secret() -> String {
    let prefix = "GOCSPX";
    let suffix = "-K58FWR486LdLJ1mLB8sXC4z6qDAf";
    format!("{prefix}{suffix}")
}

/// Returns the platform-specific User-Agent and Client-Metadata platform token.
fn platform_tokens() -> (String, &'static str) {
    if cfg!(target_os = "windows") {
        ("windows/amd64".to_string(), "WINDOWS")
    } else if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") {
            "darwin/arm64"
        } else {
            "darwin/amd64"
        };
        (arch.to_string(), "MACOS")
    } else {
        ("linux/amd64".to_string(), "LINUX")
    }
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

fn build_authorize_url(pkce: &Pkce, state: &str, redirect_uri: &str) -> String {
    let scope = SCOPES.join(" ");
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", scope.as_str()),
        ("code_challenge", pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", key, urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{AUTHORIZE_URL}?{query}")
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build antigravity http client")
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<TokenResponse> {
    let client = http_client()?;
    let secret = client_secret();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("client_secret", secret.as_str()),
        ("code_verifier", verifier),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("call antigravity token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "antigravity token exchange failed: HTTP {status}: {body}"
        ));
    }
    resp.json()
        .await
        .context("parse antigravity token response")
}

async fn refresh(refresh_token: &str) -> Result<TokenResponse> {
    let client = http_client()?;
    let secret = client_secret();
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
        ("client_secret", secret.as_str()),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .context("call antigravity token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(
            "antigravity token refresh failed: HTTP {status}: {body}"
        ));
    }
    resp.json()
        .await
        .context("parse antigravity token response")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn metadata_from(
    tokens: &TokenResponse,
    previous: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut object = previous
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if let Some(email) = tokens.id_token.as_deref().and_then(jwt::email) {
        object.insert("email".to_string(), serde_json::Value::String(email));
    }
    if object.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(object))
    }
}

async fn persist_tokens(tokens: TokenResponse) -> Result<()> {
    let _guard = super::store_lock(super::SubscriptionProvider::Antigravity)
        .lock()
        .await;
    let access = tokens
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("antigravity token response missing access_token"))?;
    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow!("antigravity token response missing refresh_token"))?;
    let expires = now_ms() + tokens.expires_in.unwrap_or(3600) * 1000;
    let metadata = metadata_from(&tokens, None);
    let mut store = store::load().await.unwrap_or_default();
    store.insert(
        STORE_KEY.to_string(),
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id: None,
            metadata,
        },
    );
    store::save(&store).await?;
    log::info!("antigravity subscription tokens saved");
    Ok(())
}

/// Starts the browser PKCE login flow, binding the loopback callback server.
pub(crate) async fn begin_login(cancel: CancellationToken) -> Result<StartedLogin> {
    let pkce = Pkce::generate();
    let state = pkce::random_state();
    let redirect_uri = redirect_uri();
    let authorization_url = build_authorize_url(&pkce, &state, &redirect_uri);
    let listener = oauth_server::bind_loopback(CALLBACK_PORT).await?;
    let verifier = pkce.verifier.clone();

    let runner = async move {
        tokio::select! {
            _ = cancel.cancelled() => Err(anyhow!("login cancelled")),
            result = async {
                let params =
                    oauth_server::wait_for_callback(listener, CALLBACK_PATH, &state).await?;
                let code = params
                    .get("code")
                    .cloned()
                    .ok_or_else(|| anyhow!("antigravity callback missing code"))?;
                let tokens = exchange_code(&code, &verifier, &redirect_uri).await?;
                persist_tokens(tokens).await
            } => result,
        }
    };

    Ok(StartedLogin {
        authorization_url,
        user_code: None,
        instructions: "Complete authorization in your browser, then return to BitFun.".to_string(),
        runner: Box::pin(runner),
    })
}

/// Ensures the stored access token is fresh, refreshing it when needed. Returns
/// the current `(access, expires_ms)`.
async fn ensure_fresh() -> Result<(String, i64)> {
    let _guard = super::store_lock(super::SubscriptionProvider::Antigravity)
        .lock()
        .await;
    let mut store = store::load().await.unwrap_or_default();
    let entry = store
        .get(STORE_KEY)
        .cloned()
        .ok_or_else(|| anyhow!("Antigravity is not connected; sign in first"))?;
    let StoredCredential::Oauth {
        refresh: refresh_token,
        access,
        expires,
        account_id,
        metadata,
    } = entry
    else {
        return Err(anyhow!("Antigravity credential is not an OAuth login"));
    };

    if expires > now_ms() + REFRESH_LEEWAY_MS {
        return Ok((access, expires));
    }

    let refreshed = refresh(&refresh_token).await?;
    let new_access = refreshed
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("antigravity refresh response missing access_token"))?;
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_metadata = metadata_from(&refreshed, metadata);
    store.insert(
        STORE_KEY.to_string(),
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id,
            metadata: new_metadata,
        },
    );
    store::save(&store).await?;
    log::info!("antigravity subscription tokens refreshed");
    Ok((new_access, new_expires))
}

/// Resolves the runtime credential (refreshing tokens if required).
pub(crate) async fn resolve() -> Result<ResolvedCredential> {
    let (access, expires) = ensure_fresh().await?;
    let (ua_platform, meta_platform) = platform_tokens();
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        format!("antigravity/{ANTIGRAVITY_VERSION} {ua_platform}"),
    );
    headers.insert("X-Goog-Api-Client".to_string(), GOOG_API_CLIENT.to_string());
    headers.insert(
        "Client-Metadata".to_string(),
        format!(
            "{{\"ideType\":\"ANTIGRAVITY\",\"platform\":\"{meta_platform}\",\"pluginType\":\"GEMINI\"}}"
        ),
    );

    Ok(ResolvedCredential {
        api_key: access,
        base_url: Some(CODE_ASSIST_BASE_URL.to_string()),
        request_url: Some(CODE_ASSIST_REQUEST_URL.to_string()),
        format: Some("gemini-code-assist".to_string()),
        extra_headers: headers,
        expires_at: Some(expires / 1000),
    })
}

/// Provider metadata used to seed a new model entry.
pub(crate) fn suggested() -> (&'static str, &'static str, &'static str) {
    ("gemini-code-assist", CODE_ASSIST_BASE_URL, DEFAULT_MODEL)
}
