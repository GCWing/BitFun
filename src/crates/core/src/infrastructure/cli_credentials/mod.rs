//! CLI credential discovery and resolution.
//!
//! Lets BitFun reuse already-authenticated Codex CLI / Gemini CLI sessions on
//! the local machine instead of asking the user to paste an API key. Each
//! provider exposes a [`CredentialResolver`] that:
//!   * inspects well-known config files in the user's home directory,
//!   * refreshes OAuth tokens if they are expired or close to expiry,
//!   * returns a [`ResolvedCredential`] that the AI client factory uses to
//!     override `api_key` / `base_url` / `request_url` / `format` /
//!     `custom_headers` before constructing an `AIClient`.

pub mod codex;
pub mod gemini;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Source kind of a discovered CLI credential. Persisted on `AIModelConfig.auth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliCredentialKind {
    Codex,
    Gemini,
}

/// Concrete sub-mode of a credential, auto-detected from the on-disk file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliCredentialMode {
    /// Codex CLI in `OPENAI_API_KEY` mode, or Gemini CLI in `GEMINI_API_KEY` mode.
    ApiKey,
    /// Codex CLI in ChatGPT login mode (uses chatgpt.com backend with OAuth tokens).
    ChatGpt,
    /// Gemini CLI in personal Google OAuth mode (uses Cloud Code Assist endpoint).
    OauthPersonal,
}

/// What `discover_cli_credentials` returns to the UI for each detected source.
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredCredential {
    pub kind: CliCredentialKind,
    pub mode: CliCredentialMode,
    pub display_label: String,
    pub account: Option<String>,
    pub expires_at: Option<i64>,
    pub source_path: String,
    /// Suggested provider format (`responses`, `gemini`, `gemini-code-assist`, `openai`).
    pub suggested_format: String,
    /// Suggested base URL to seed the model entry with.
    pub suggested_base_url: String,
    /// Suggested model name to seed the entry with.
    pub suggested_model: String,
}

/// Final, runtime-resolved credential that overrides fields in `AIConfig`.
#[derive(Debug, Clone)]
pub struct ResolvedCredential {
    pub api_key: String,
    pub base_url: Option<String>,
    pub request_url: Option<String>,
    pub format: Option<String>,
    pub extra_headers: HashMap<String, String>,
    /// Unix seconds when this credential expires; `None` means non-expiring.
    pub expires_at: Option<i64>,
}

#[async_trait]
pub trait CredentialResolver: Send + Sync {
    async fn resolve(&self) -> anyhow::Result<ResolvedCredential>;
}

/// Discover all CLI credentials on the local machine. Errors per-source are
/// swallowed (logged) so that a broken Codex install doesn't hide a working
/// Gemini install.
pub async fn discover_all() -> Vec<DiscoveredCredential> {
    let mut out = Vec::new();
    match codex::discover().await {
        Ok(Some(item)) => out.push(item),
        Ok(None) => {}
        Err(err) => log::debug!("codex credential discovery failed: {err:#}"),
    }
    match gemini::discover().await {
        Ok(Some(item)) => out.push(item),
        Ok(None) => {}
        Err(err) => log::debug!("gemini credential discovery failed: {err:#}"),
    }
    out
}
