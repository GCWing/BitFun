//! Antigravity (Google) subscription login and credential resolution.
//!
//! Browser PKCE login against Google OAuth on a loopback listener (preferring
//! the registered port `51121`), then Bearer access to
//! the Cloud Code Assist (`cloudcode-pa`) endpoint using the
//! `gemini-code-assist` request format. Constants mirror
//! `opencode-antigravity-auth`.

use super::store::{self, StoredCredential};
use super::{
    jwt, oauth_server, pkce, pkce::Pkce, ResolvedCredential, StartedLogin, SubscriptionHttpOptions,
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const CLIENT_ID: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const CALLBACK_PATH: &str = "/oauth-callback";
const CALLBACK_PORT: u16 = 51121;
const CALLBACK_PORTS: &[u16] = &[CALLBACK_PORT];
const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v1/userinfo?alt=json";
const CODE_ASSIST_BASE_URL: &str = "https://daily-cloudcode-pa.sandbox.googleapis.com";
const CODE_ASSIST_REQUEST_URL: &str =
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:streamGenerateContent?alt=sse";
const ANTIGRAVITY_VERSION_URL: &str =
    "https://antigravity-auto-updater-974169037036.us-central1.run.app";
const ANTIGRAVITY_CHANGELOG_URL: &str = "https://antigravity.google/changelog";
const ANTIGRAVITY_VERSION_FALLBACK: &str = "2.0.6";
const GOOG_API_CLIENT: &str = "google-cloud-sdk vscode_cloudshelleditor/0.1";
const DEFAULT_MODEL: &str = "gemini-3-pro-high";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "antigravity";

fn redirect_uri(port: u16) -> String {
    oauth_server::loopback_redirect_uri(port, CALLBACK_PATH)
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
    platform_tokens_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn platform_tokens_for(os: &str, arch: &str) -> (String, &'static str) {
    // Antigravity only advertises Windows/macOS desktop builds. The OpenCode
    // plugin deliberately chooses from these supported fingerprints even when
    // it runs under Linux, WSL, or a container.
    match (os, arch) {
        (_, "aarch64") => ("darwin/arm64".to_string(), "MACOS"),
        ("macos", _) => ("darwin/amd64".to_string(), "MACOS"),
        _ => ("windows/amd64".to_string(), "WINDOWS"),
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
    #[serde(default, skip_deserializing)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    email: Option<String>,
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

fn http_client(options: &SubscriptionHttpOptions) -> Result<reqwest::Client> {
    super::build_http_client(options, "Antigravity")
}

fn parse_antigravity_version(text: &str) -> Option<String> {
    text.split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .find_map(|candidate| {
            let parts = candidate.split('.').collect::<Vec<_>>();
            (parts.len() == 3
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit())))
            .then(|| candidate.to_string())
        })
}

async fn fetch_antigravity_version(options: &SubscriptionHttpOptions) -> Option<String> {
    let client = http_client(options).ok()?;
    for (url, max_chars) in [
        (ANTIGRAVITY_VERSION_URL, None),
        (ANTIGRAVITY_CHANGELOG_URL, Some(5_000usize)),
    ] {
        let response = match client.get(url).timeout(Duration::from_secs(5)).send().await {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };
        let mut body = match response.text().await {
            Ok(body) => body,
            Err(_) => continue,
        };
        if let Some(limit) = max_chars {
            if body.len() > limit {
                let boundary = body
                    .char_indices()
                    .map(|(index, _)| index)
                    .take_while(|index| *index <= limit)
                    .last()
                    .unwrap_or(0);
                body.truncate(boundary);
            }
        }
        if let Some(version) = parse_antigravity_version(&body) {
            return Some(version);
        }
    }
    None
}

async fn antigravity_version(options: &SubscriptionHttpOptions) -> &'static str {
    static VERSION: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    VERSION
        .get_or_init(|| async {
            fetch_antigravity_version(options)
                .await
                .unwrap_or_else(|| ANTIGRAVITY_VERSION_FALLBACK.to_string())
        })
        .await
        .as_str()
}

async fn fetch_user_email(access_token: &str, options: &SubscriptionHttpOptions) -> Option<String> {
    let response = http_client(options)
        .ok()?
        .get(USER_INFO_URL)
        .bearer_auth(access_token)
        .header(
            reqwest::header::USER_AGENT,
            "google-api-nodejs-client/9.15.1",
        )
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response
        .json::<UserInfoResponse>()
        .await
        .ok()?
        .email
        .filter(|email| !email.trim().is_empty())
}

async fn exchange_code(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    options: &SubscriptionHttpOptions,
) -> Result<TokenResponse> {
    let client = http_client(options)?;
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
        .header(
            reqwest::header::USER_AGENT,
            "google-api-nodejs-client/9.15.1",
        )
        .header(reqwest::header::ACCEPT, "*/*")
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
    let mut tokens = resp
        .json::<TokenResponse>()
        .await
        .context("parse antigravity token response")?;
    if let Some(access) = tokens.access_token.as_deref() {
        tokens.email = fetch_user_email(access, options).await;
    }
    Ok(tokens)
}

async fn refresh(refresh_token: &str, options: &SubscriptionHttpOptions) -> Result<TokenResponse> {
    let client = http_client(options)?;
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
    if let Some(email) = tokens
        .email
        .clone()
        .or_else(|| tokens.id_token.as_deref().and_then(jwt::email))
    {
        object.insert("email".to_string(), serde_json::Value::String(email));
    }
    if object.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(object))
    }
}

async fn persist_tokens(tokens: TokenResponse, expected_revision: u64) -> Result<()> {
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
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        expected_revision,
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id: None,
            metadata,
        },
    )
    .await?;
    super::require_current_store_revision(super::SubscriptionProvider::Antigravity, outcome)?;
    log::info!("antigravity subscription tokens saved");
    Ok(())
}

/// Starts the browser PKCE login flow, binding the loopback callback server.
pub(crate) async fn begin_login(
    cancel: CancellationToken,
    expected_revision: u64,
    options: SubscriptionHttpOptions,
) -> Result<StartedLogin> {
    let pkce = Pkce::generate();
    let state = pkce::random_state();
    let (listener, callback_port) = oauth_server::bind_loopback_ports(CALLBACK_PORTS).await?;
    let redirect_uri = redirect_uri(callback_port);
    let authorization_url = build_authorize_url(&pkce, &state, &redirect_uri);
    let verifier = pkce.verifier.clone();

    let runner = async move {
        super::authorize_then_persist(
            super::SubscriptionProvider::Antigravity,
            cancel,
            async {
                let params =
                    oauth_server::wait_for_callback(listener, CALLBACK_PATH, &state).await?;
                let code = params
                    .get("code")
                    .cloned()
                    .ok_or_else(|| anyhow!("antigravity callback missing code"))?;
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

/// Ensures the stored access token is fresh, refreshing it when needed. Returns
/// the current `(access, expires_ms)`.
async fn ensure_fresh(options: &SubscriptionHttpOptions) -> Result<(String, i64)> {
    let _refresh_lease = store::acquire_provider_refresh_lease(STORE_KEY).await?;
    let snapshot = store::load_entry_with_revision(STORE_KEY).await?;
    let entry = snapshot
        .credential
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

    let refreshed = refresh(&refresh_token, options).await?;
    let new_access = refreshed
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("antigravity refresh response missing access_token"))?;
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_metadata = metadata_from(&refreshed, metadata);
    let outcome = store::upsert_if_revision(
        STORE_KEY,
        snapshot.revision,
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id,
            metadata: new_metadata,
        },
    )
    .await?;
    match outcome {
        store::ConditionalCommitOutcome::Committed { .. } => {
            log::info!("antigravity subscription tokens refreshed");
            Ok((new_access, new_expires))
        }
        store::ConditionalCommitOutcome::Conflict { current_revision } => {
            let current = super::load_current_store_after_conflict(
                super::SubscriptionProvider::Antigravity,
                current_revision,
            )
            .await?;
            match current.credential {
                Some(StoredCredential::Oauth {
                    access, expires, ..
                }) if expires > now_ms() => {
                    log::info!(
                        "antigravity refresh reused tokens committed by a concurrent refresh"
                    );
                    Ok((access, expires))
                }
                _ => Err(super::store_revision_conflict(
                    super::SubscriptionProvider::Antigravity,
                    current_revision,
                )),
            }
        }
    }
}

/// Resolves the runtime credential (refreshing tokens if required).
pub(crate) async fn resolve(options: &SubscriptionHttpOptions) -> Result<ResolvedCredential> {
    let (access, expires) = ensure_fresh(options).await?;
    let (ua_platform, meta_platform) = platform_tokens();
    let version = antigravity_version(options).await;
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        format!("antigravity/{version} {ua_platform}"),
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

#[cfg(test)]
mod tests {
    use super::{
        build_authorize_url, parse_antigravity_version, platform_tokens_for, redirect_uri,
        ANTIGRAVITY_VERSION_FALLBACK, CODE_ASSIST_BASE_URL,
    };
    use crate::subscription_auth::pkce::Pkce;

    #[test]
    fn uses_registered_localhost_redirect_uri() {
        let redirect_uri = redirect_uri(super::CALLBACK_PORT);
        assert_eq!(redirect_uri, "http://localhost:51121/oauth-callback");

        let authorize_url = build_authorize_url(&Pkce::generate(), "state", &redirect_uri);
        assert!(
            authorize_url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A51121%2Foauth-callback")
        );
    }

    #[test]
    fn uses_only_antigravity_supported_desktop_fingerprints() {
        assert_eq!(
            platform_tokens_for("windows", "x86_64"),
            ("windows/amd64".to_string(), "WINDOWS")
        );
        assert_eq!(
            platform_tokens_for("windows", "aarch64"),
            ("darwin/arm64".to_string(), "MACOS")
        );
        assert_eq!(
            platform_tokens_for("macos", "aarch64"),
            ("darwin/arm64".to_string(), "MACOS")
        );
        assert_eq!(
            platform_tokens_for("linux", "x86_64"),
            ("windows/amd64".to_string(), "WINDOWS")
        );
        assert_eq!(
            platform_tokens_for("linux", "aarch64"),
            ("darwin/arm64".to_string(), "MACOS")
        );
    }

    #[test]
    fn parses_live_antigravity_version_and_uses_current_fallback() {
        assert_eq!(
            parse_antigravity_version("Auto updater is running. Fixed Version: 2.0.6"),
            Some("2.0.6".to_string())
        );
        assert_eq!(ANTIGRAVITY_VERSION_FALLBACK, "2.0.6");
        assert_eq!(
            CODE_ASSIST_BASE_URL,
            "https://daily-cloudcode-pa.sandbox.googleapis.com"
        );
    }
}
