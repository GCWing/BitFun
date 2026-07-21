//! Codex (ChatGPT) subscription login and credential resolution.
//!
//! Aligned with OpenCode's `plugin/openai/codex.ts`: browser PKCE login against
//! `auth.openai.com` on the fixed loopback port `1455`, then Bearer access to
//! `chatgpt.com/backend-api/codex/responses`.

use super::store::{self, StoredCredential};
use super::{jwt, oauth_server, pkce::Pkce, ResolvedCredential, StartedLogin};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ISSUER: &str = "https://auth.openai.com";
const CALLBACK_PATH: &str = "/auth/callback";
const CALLBACK_PORT: u16 = 1455;
const SCOPE: &str = "openid profile email offline_access";
const CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const CHATGPT_REQUEST_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const DEFAULT_MODEL: &str = "gpt-5-codex";
const REFRESH_LEEWAY_MS: i64 = 5 * 60 * 1000;
const STORE_KEY: &str = "codex";

fn redirect_uri() -> String {
    oauth_server::loopback_redirect_uri(CALLBACK_PORT, CALLBACK_PATH)
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

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build codex http client")
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<TokenResponse> {
    let client = http_client()?;
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

async fn refresh(refresh_token: &str) -> Result<TokenResponse> {
    let client = http_client()?;
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

async fn persist_tokens(tokens: TokenResponse) -> Result<()> {
    let _guard = super::store_lock(super::SubscriptionProvider::Codex)
        .lock()
        .await;
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
    let mut store = store::load().await.unwrap_or_default();
    store.insert(
        STORE_KEY.to_string(),
        StoredCredential::Oauth {
            refresh,
            access,
            expires,
            account_id,
            metadata,
        },
    );
    store::save(&store).await?;
    log::info!("codex subscription tokens saved");
    Ok(())
}

/// Starts the browser PKCE login flow, binding the loopback callback server.
pub(crate) async fn begin_login(cancel: CancellationToken) -> Result<StartedLogin> {
    let pkce = Pkce::generate();
    let state = super::pkce::random_state();
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
                    .ok_or_else(|| anyhow!("codex callback missing code"))?;
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
/// the current `(access, account_id, expires_ms)`.
async fn ensure_fresh() -> Result<(String, Option<String>, i64)> {
    let _guard = super::store_lock(super::SubscriptionProvider::Codex)
        .lock()
        .await;
    let mut store = store::load().await.unwrap_or_default();
    let entry = store
        .get(STORE_KEY)
        .cloned()
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

    let refreshed = refresh(&refresh_token).await?;
    let new_access = refreshed
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("codex refresh response missing access_token"))?;
    let new_refresh = refreshed.refresh_token.clone().unwrap_or(refresh_token);
    let new_expires = now_ms() + refreshed.expires_in.unwrap_or(3600) * 1000;
    let new_account_id = account_id_from(&refreshed).or(account_id);
    let new_metadata = metadata_from(&refreshed).or(metadata);
    store.insert(
        STORE_KEY.to_string(),
        StoredCredential::Oauth {
            refresh: new_refresh,
            access: new_access.clone(),
            expires: new_expires,
            account_id: new_account_id.clone(),
            metadata: new_metadata,
        },
    );
    store::save(&store).await?;
    log::info!("codex subscription tokens refreshed");
    Ok((new_access, new_account_id, new_expires))
}

async fn resolve_codex_cli_version() -> Option<String> {
    let check = bitfun_services_core::system::check_command("codex");
    let command = check.path.as_deref()?;
    let args = vec!["--version".to_string()];
    let output = bitfun_services_core::system::run_command(command, &args, None, None)
        .await
        .ok()?;
    if !output.success {
        return None;
    }
    parse_codex_cli_version(&output.stdout).or_else(|| parse_codex_cli_version(&output.stderr))
}

fn parse_codex_cli_version(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let version = token
            .trim()
            .trim_start_matches('v')
            .trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '('));
        if version.chars().next().is_some_and(|ch| ch.is_ascii_digit())
            && version.contains('.')
            && version
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'))
        {
            Some(version.to_string())
        } else {
            None
        }
    })
}

/// Resolves the runtime credential (refreshing tokens if required).
pub(crate) async fn resolve() -> Result<ResolvedCredential> {
    let (access, account_id, expires) = ensure_fresh().await?;
    let mut headers = HashMap::new();
    if let Some(account) = account_id {
        headers.insert("ChatGPT-Account-ID".to_string(), account);
    }
    headers.insert("originator".to_string(), "codex_cli_rs".to_string());
    headers.insert(
        "OpenAI-Beta".to_string(),
        "responses=experimental".to_string(),
    );
    headers.insert("session_id".to_string(), Uuid::new_v4().to_string());
    let user_agent = resolve_codex_cli_version()
        .await
        .map(|version| format!("codex_cli_rs/{version}"))
        .unwrap_or_else(|| {
            log::warn!(
                "Unable to detect codex CLI version; using codex-compatible user agent for Codex backend requests"
            );
            "codex_cli_rs/0.0.0".to_string()
        });
    headers.insert("User-Agent".to_string(), user_agent);

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
    use super::parse_codex_cli_version;

    #[test]
    fn parses_codex_cli_version_output() {
        assert_eq!(
            parse_codex_cli_version("codex-cli 0.124.0\n").as_deref(),
            Some("0.124.0")
        );
    }
}
