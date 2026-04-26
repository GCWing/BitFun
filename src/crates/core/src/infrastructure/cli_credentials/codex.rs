//! Codex CLI credential resolver.
//!
//! File layout (`~/.codex/auth.json`):
//! ```json
//! {
//!   "auth_mode": "chatgpt" | "apikey",
//!   "OPENAI_API_KEY": "sk-..." | null,
//!   "tokens": {
//!     "id_token":      "eyJhbGciOi...",
//!     "access_token":  "eyJhbGciOi...",
//!     "refresh_token": "rt_...",
//!     "account_id":    "..."
//!   },
//!   "last_refresh": "2026-04-17T15:00:00Z"
//! }
//! ```
//!
//! In ChatGPT mode the access token is short-lived (~28 days) and we refresh it
//! against `https://auth.openai.com/oauth/token`. The access token is then sent
//! as a Bearer to `https://chatgpt.com/backend-api/codex/responses` (an
//! OpenAI-Responses-compatible endpoint) along with `chatgpt-account-id` /
//! `originator` / `OpenAI-Beta` / `session_id` headers.

use super::{
    CliCredentialKind, CliCredentialMode, CredentialResolver, DiscoveredCredential,
    ResolvedCredential,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const CODEX_CHATGPT_REQUEST_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CODEX_DEFAULT_MODEL_CHATGPT: &str = "gpt-5-codex";
const CODEX_DEFAULT_MODEL_APIKEY: &str = "gpt-5";
const REFRESH_LEEWAY_SECONDS: i64 = 5 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CodexAuthFile {
    #[serde(default, rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    #[serde(default)]
    auth_mode: Option<String>,
    #[serde(default)]
    tokens: Option<CodexTokens>,
    #[serde(default)]
    last_refresh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CodexTokens {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
}

fn auth_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".codex").join("auth.json"))
}

async fn load_auth_file() -> Result<Option<CodexAuthFile>> {
    let Some(path) = auth_file_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let bytes = tokio::fs::read(&path)
        .await
        .with_context(|| format!("read codex auth.json at {}", path.display()))?;
    let file: CodexAuthFile = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse codex auth.json at {}", path.display()))?;
    Ok(Some(file))
}

async fn save_auth_file(file: &CodexAuthFile) -> Result<()> {
    let Some(path) = auth_file_path() else {
        return Err(anyhow!("home directory not available"));
    };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let bytes = serde_json::to_vec_pretty(file)?;
    tokio::fs::write(&path, &bytes).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = tokio::fs::metadata(&path).await {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = tokio::fs::set_permissions(&path, perms).await;
        }
    }
    Ok(())
}

/// Decode the JWT payload (no signature verification – we trust the local file).
fn decode_jwt_claims(token: &str) -> Option<Value> {
    let mut parts = token.splitn(3, '.');
    let _h = parts.next()?;
    let payload = parts.next()?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

fn jwt_exp(token: &str) -> Option<i64> {
    decode_jwt_claims(token)?.get("exp")?.as_i64()
}

fn jwt_email(token: &str) -> Option<String> {
    decode_jwt_claims(token)?
        .get("email")?
        .as_str()
        .map(|s| s.to_string())
}

fn jwt_chatgpt_account_id(token: &str) -> Option<String> {
    decode_jwt_claims(token)?
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(|s| s.to_string())
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

async fn resolve_codex_cli_version() -> Option<String> {
    let check = crate::service::system::check_command("codex");
    let command = check.path.as_deref()?;
    let args = vec!["--version".to_string()];
    let output = crate::service::system::run_command(command, &args, None, None)
        .await
        .ok()?;
    if !output.success {
        log::debug!(
            "codex CLI version command failed: exit_code={}, stderr={}",
            output.exit_code,
            output.stderr.trim()
        );
        return None;
    }
    parse_codex_cli_version(&output.stdout).or_else(|| parse_codex_cli_version(&output.stderr))
}

fn detect_mode(file: &CodexAuthFile) -> Option<CliCredentialMode> {
    let declared = file.auth_mode.as_deref().unwrap_or("");
    if declared.eq_ignore_ascii_case("chatgpt") {
        return Some(CliCredentialMode::ChatGpt);
    }
    if declared.eq_ignore_ascii_case("apikey")
        && file
            .openai_api_key
            .as_deref()
            .map(|k| !k.is_empty())
            .unwrap_or(false)
    {
        return Some(CliCredentialMode::ApiKey);
    }
    if file.tokens.as_ref().is_some_and(|t| {
        t.access_token
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }) {
        return Some(CliCredentialMode::ChatGpt);
    }
    if file
        .openai_api_key
        .as_deref()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
    {
        return Some(CliCredentialMode::ApiKey);
    }
    None
}

pub async fn discover() -> Result<Option<DiscoveredCredential>> {
    let Some(file) = load_auth_file().await? else {
        return Ok(None);
    };
    let Some(path) = auth_file_path() else {
        return Ok(None);
    };
    let Some(mode) = detect_mode(&file) else {
        return Ok(None);
    };

    let (label, account, expires_at, format, base_url, model) = match mode {
        CliCredentialMode::ApiKey => (
            "Codex CLI · API Key".to_string(),
            None,
            None,
            "responses".to_string(),
            "https://api.openai.com/v1".to_string(),
            CODEX_DEFAULT_MODEL_APIKEY.to_string(),
        ),
        CliCredentialMode::ChatGpt => {
            let id_token = file.tokens.as_ref().and_then(|t| t.id_token.clone());
            let email = id_token.as_deref().and_then(jwt_email);
            let access_token = file.tokens.as_ref().and_then(|t| t.access_token.clone());
            let exp = access_token.as_deref().and_then(jwt_exp);
            (
                "Codex CLI · ChatGPT Login".to_string(),
                email,
                exp,
                "responses".to_string(),
                CODEX_CHATGPT_BASE_URL.to_string(),
                CODEX_DEFAULT_MODEL_CHATGPT.to_string(),
            )
        }
        CliCredentialMode::OauthPersonal => unreachable!("Codex never uses OauthPersonal"),
    };

    Ok(Some(DiscoveredCredential {
        kind: CliCredentialKind::Codex,
        mode,
        display_label: label,
        account,
        expires_at,
        source_path: path.display().to_string(),
        suggested_format: format,
        suggested_base_url: base_url,
        suggested_model: model,
    }))
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

async fn refresh_codex_tokens(refresh_token: &str) -> Result<OAuthTokenResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client
        .post(CODEX_TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "client_id": CODEX_CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "scope": "openid profile email",
        }))
        .send()
        .await
        .context("call codex oauth token endpoint")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("codex token refresh failed: HTTP {status}: {body}"));
    }
    let parsed: OAuthTokenResponse = resp
        .json()
        .await
        .context("parse codex oauth token response")?;
    Ok(parsed)
}

async fn ensure_fresh_tokens(file: &mut CodexAuthFile) -> Result<()> {
    let access_exp = file
        .tokens
        .as_ref()
        .and_then(|t| t.access_token.as_deref())
        .and_then(jwt_exp);
    let now = chrono::Utc::now().timestamp();
    let needs_refresh = match access_exp {
        Some(exp) => exp - now <= REFRESH_LEEWAY_SECONDS,
        None => true,
    };
    if !needs_refresh {
        return Ok(());
    }
    let refresh_token = file
        .tokens
        .as_ref()
        .and_then(|t| t.refresh_token.clone())
        .ok_or_else(|| anyhow!("codex refresh_token missing; please re-login via `codex login`"))?;
    let refreshed = refresh_codex_tokens(&refresh_token).await?;
    let tokens = file.tokens.get_or_insert_with(Default::default);
    if let Some(id) = refreshed.id_token {
        tokens.id_token = Some(id);
    }
    if let Some(at) = refreshed.access_token {
        tokens.access_token = Some(at);
    }
    if let Some(rt) = refreshed.refresh_token {
        tokens.refresh_token = Some(rt);
    }
    file.last_refresh = Some(chrono::Utc::now().to_rfc3339());
    save_auth_file(file).await?;
    log::info!("codex CLI tokens refreshed");
    Ok(())
}

pub struct CodexResolver;

#[async_trait]
impl CredentialResolver for CodexResolver {
    async fn resolve(&self) -> Result<ResolvedCredential> {
        let mut file = load_auth_file()
            .await?
            .ok_or_else(|| anyhow!("~/.codex/auth.json not found; run `codex login` first"))?;
        let mode = detect_mode(&file)
            .ok_or_else(|| anyhow!("codex auth.json present but no usable credentials"))?;

        match mode {
            CliCredentialMode::ApiKey => {
                let key = file
                    .openai_api_key
                    .clone()
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| anyhow!("codex apikey mode but OPENAI_API_KEY empty"))?;
                Ok(ResolvedCredential {
                    api_key: key,
                    base_url: Some("https://api.openai.com/v1".to_string()),
                    request_url: Some("https://api.openai.com/v1/responses".to_string()),
                    format: Some("responses".to_string()),
                    extra_headers: HashMap::new(),
                    expires_at: None,
                })
            }
            CliCredentialMode::ChatGpt => {
                ensure_fresh_tokens(&mut file).await?;
                let tokens = file
                    .tokens
                    .as_ref()
                    .ok_or_else(|| anyhow!("codex tokens missing after refresh"))?;
                let access_token = tokens
                    .access_token
                    .clone()
                    .ok_or_else(|| anyhow!("codex access_token missing"))?;
                let mut headers = HashMap::new();
                let account_id = tokens
                    .account_id
                    .clone()
                    .or_else(|| tokens.id_token.as_deref().and_then(jwt_chatgpt_account_id));
                if let Some(account) = account_id {
                    headers.insert("chatgpt-account-id".to_string(), account);
                }
                headers.insert("originator".to_string(), "codex_cli_rs".to_string());
                headers.insert(
                    "OpenAI-Beta".to_string(),
                    "responses=experimental".to_string(),
                );
                headers.insert("session_id".to_string(), Uuid::new_v4().to_string());
                // Codex backend (`chatgpt.com/backend-api/codex`) gates `/models`
                // on a Codex-shaped User-Agent. Mirror the CLI's format
                // (`codex_cli_rs/<ver>`) so requests aren't silently rejected
                // / served an empty list.
                let codex_cli_version = resolve_codex_cli_version().await;
                let user_agent = codex_cli_version
                    .as_deref()
                    .map(|version| format!("codex_cli_rs/{version}"))
                    .unwrap_or_else(|| {
                        log::warn!(
                            "Unable to detect codex CLI version; using BitFun user agent for Codex backend requests"
                        );
                        format!("BitFun/{}", env!("CARGO_PKG_VERSION"))
                    });
                headers.insert("User-Agent".to_string(), user_agent);
                let exp = tokens.access_token.as_deref().and_then(jwt_exp);
                Ok(ResolvedCredential {
                    api_key: access_token,
                    base_url: Some(CODEX_CHATGPT_BASE_URL.to_string()),
                    request_url: Some(CODEX_CHATGPT_REQUEST_URL.to_string()),
                    format: Some("responses".to_string()),
                    extra_headers: headers,
                    expires_at: exp,
                })
            }
            CliCredentialMode::OauthPersonal => Err(anyhow!("codex never uses OauthPersonal mode")),
        }
    }
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
        assert_eq!(
            parse_codex_cli_version("codex v0.125.1-beta.2").as_deref(),
            Some("0.125.1-beta.2")
        );
    }

    #[test]
    fn ignores_output_without_semver_like_token() {
        assert_eq!(parse_codex_cli_version("codex-cli unknown"), None);
    }
}
