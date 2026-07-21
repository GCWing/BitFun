//! In-app subscription authentication.
//!
//! Lets BitFun sign in to another product's subscription (Codex/ChatGPT,
//! Antigravity/Google, OpenCode Zen) with an OpenCode-style in-app OAuth flow,
//! and use the resulting tokens to authenticate AI requests. Tokens are stored
//! locally in `subscription_auth.json` (mode 0600) and refreshed on resolve.
//!
//! There is no upgrade path for the previous Codex/Gemini CLI disk-scan import.

mod antigravity;
mod codex;
mod jwt;
mod oauth_server;
mod opencode;
mod pkce;
pub mod store;

pub use store::{set_store_path_for_test, StoredCredential};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Maximum lifetime of a pending login session (matches OpenCode).
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// One of the subscription providers BitFun can sign in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionProvider {
    Codex,
    Antigravity,
    Opencode,
}

impl SubscriptionProvider {
    /// All providers, in display order.
    pub const ALL: [SubscriptionProvider; 3] = [Self::Codex, Self::Antigravity, Self::Opencode];

    /// Stable store key / serde tag for this provider.
    pub fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::Opencode => "opencode",
        }
    }

    /// Parses a provider from its stable key.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "codex" => Some(Self::Codex),
            "antigravity" => Some(Self::Antigravity),
            "opencode" => Some(Self::Opencode),
            _ => None,
        }
    }

    fn display_label(self) -> String {
        match self {
            Self::Codex => "Codex (ChatGPT)",
            Self::Antigravity => "Antigravity (Google)",
            Self::Opencode => "OpenCode Zen",
        }
        .to_string()
    }

    fn suggested(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Codex => codex::suggested(),
            Self::Antigravity => antigravity::suggested(),
            Self::Opencode => opencode::suggested(),
        }
    }
}

/// A subscription account entry surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionAccount {
    pub provider: SubscriptionProvider,
    pub display_label: String,
    pub account: Option<String>,
    /// Unix seconds when the current credential expires (for UI display).
    pub expires_at: Option<i64>,
    pub connected: bool,
    pub suggested_format: String,
    pub suggested_base_url: String,
    pub suggested_model: String,
}

/// Runtime-resolved credential that overrides fields in the AI client config.
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

/// Returned by `start_login`; contains what the UI needs to guide the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginStartResult {
    pub provider: SubscriptionProvider,
    pub authorization_url: String,
    pub user_code: Option<String>,
    pub instructions: String,
}

/// Lifecycle state of a login session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginStatus {
    Pending,
    Authorized,
    Failed,
    Cancelled,
}

/// Snapshot of a login session, polled by the UI.
#[derive(Debug, Clone, Serialize)]
pub struct LoginSessionSnapshot {
    pub provider: SubscriptionProvider,
    pub status: LoginStatus,
    pub authorization_url: Option<String>,
    pub user_code: Option<String>,
    pub instructions: Option<String>,
    pub error: Option<String>,
    pub account: Option<SubscriptionAccount>,
}

/// Internal handle returned by each provider's `begin_login`.
pub(crate) struct StartedLogin {
    pub authorization_url: String,
    pub user_code: Option<String>,
    pub instructions: String,
    pub runner: Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>,
}

struct SessionState {
    status: LoginStatus,
    authorization_url: Option<String>,
    user_code: Option<String>,
    instructions: Option<String>,
    error: Option<String>,
    account: Option<SubscriptionAccount>,
    cancel: CancellationToken,
    /// Monotonic id distinguishing successive logins for the same provider.
    generation: u64,
}

impl SessionState {
    fn snapshot(&self, provider: SubscriptionProvider) -> LoginSessionSnapshot {
        LoginSessionSnapshot {
            provider,
            status: self.status,
            authorization_url: self.authorization_url.clone(),
            user_code: self.user_code.clone(),
            instructions: self.instructions.clone(),
            error: self.error.clone(),
            account: self.account.clone(),
        }
    }
}

fn sessions() -> &'static Mutex<HashMap<SubscriptionProvider, SessionState>> {
    static SESSIONS: OnceLock<Mutex<HashMap<SubscriptionProvider, SessionState>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_generation() -> u64 {
    static GENERATION: AtomicU64 = AtomicU64::new(1);
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Per-provider lock serializing credential-store read-modify-write cycles.
/// Token refresh persists a rotated refresh token, so a concurrent refresh or
/// logout must not interleave and overwrite the newer credentials.
pub(crate) fn store_lock(provider: SubscriptionProvider) -> &'static tokio::sync::Mutex<()> {
    static CODEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static ANTIGRAVITY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    static OPENCODE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    match provider {
        SubscriptionProvider::Codex => &CODEX,
        SubscriptionProvider::Antigravity => &ANTIGRAVITY,
        SubscriptionProvider::Opencode => &OPENCODE,
    }
}

fn build_account(
    provider: SubscriptionProvider,
    entry: Option<&StoredCredential>,
) -> SubscriptionAccount {
    let (format, base_url, model) = provider.suggested();
    let (connected, account, expires_at) = match entry {
        None => (false, None, None),
        Some(StoredCredential::Api { .. }) => (true, None, None),
        Some(StoredCredential::Oauth {
            expires,
            account_id,
            metadata,
            ..
        }) => {
            let email = metadata
                .as_ref()
                .and_then(|value| value.get("email"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let account = email.or_else(|| account_id.clone());
            (true, account, Some(expires / 1000))
        }
    };
    SubscriptionAccount {
        provider,
        display_label: provider.display_label(),
        account,
        expires_at,
        connected,
        suggested_format: format.to_string(),
        suggested_base_url: base_url.to_string(),
        suggested_model: model.to_string(),
    }
}

async fn account_snapshot(provider: SubscriptionProvider) -> SubscriptionAccount {
    let store = store::load().await.unwrap_or_default();
    build_account(provider, store.get(provider.key()))
}

/// Lists all providers with their current connection state.
pub async fn list_accounts() -> Vec<SubscriptionAccount> {
    let store = store::load().await.unwrap_or_default();
    SubscriptionProvider::ALL
        .iter()
        .map(|provider| build_account(*provider, store.get(provider.key())))
        .collect()
}

/// Starts a login session, cancelling any existing pending session for the
/// same provider. Returns immediately with the authorization URL / user code.
pub async fn start_login(provider: SubscriptionProvider) -> Result<LoginStartResult> {
    if let Some(previous) = {
        let mut map = sessions()
            .lock()
            .map_err(|_| anyhow!("subscription login session lock poisoned"))?;
        map.remove(&provider)
    } {
        previous.cancel.cancel();
    }

    let cancel = CancellationToken::new();
    let started = match provider {
        SubscriptionProvider::Codex => codex::begin_login(cancel.clone()).await,
        SubscriptionProvider::Antigravity => antigravity::begin_login(cancel.clone()).await,
        SubscriptionProvider::Opencode => opencode::begin_login(cancel.clone()).await,
    }?;

    let authorization_url = started.authorization_url.clone();
    // Desktop opener rejects relative URLs ("Not allowed to open url /...").
    // Every provider must return an absolute http(s) authorization URL.
    if !(authorization_url.starts_with("https://") || authorization_url.starts_with("http://")) {
        cancel.cancel();
        return Err(anyhow!(
            "subscription login returned a non-absolute authorization URL: {authorization_url}"
        ));
    }
    let user_code = started.user_code.clone();
    let instructions = started.instructions.clone();
    let generation = next_generation();

    {
        let mut map = sessions()
            .lock()
            .map_err(|_| anyhow!("subscription login session lock poisoned"))?;
        map.insert(
            provider,
            SessionState {
                status: LoginStatus::Pending,
                authorization_url: Some(authorization_url.clone()),
                user_code: user_code.clone(),
                instructions: Some(instructions.clone()),
                error: None,
                account: None,
                cancel: cancel.clone(),
                generation,
            },
        );
    }

    let runner = started.runner;
    tokio::spawn(async move {
        let outcome = tokio::time::timeout(LOGIN_TIMEOUT, runner).await;
        finalize_session(provider, generation, &cancel, outcome).await;
    });

    Ok(LoginStartResult {
        provider,
        authorization_url,
        user_code,
        instructions,
    })
}

async fn finalize_session(
    provider: SubscriptionProvider,
    generation: u64,
    cancel: &CancellationToken,
    outcome: Result<Result<()>, tokio::time::error::Elapsed>,
) {
    // A newer login for the same provider has already replaced this session;
    // the stale runner must not overwrite its state.
    let is_current = sessions()
        .lock()
        .map(|map| {
            map.get(&provider)
                .is_some_and(|state| state.generation == generation)
        })
        .unwrap_or(false);
    if !is_current {
        return;
    }

    let (status, error, account) = match outcome {
        Err(_) => (
            LoginStatus::Failed,
            Some("Login timed out".to_string()),
            None,
        ),
        Ok(Ok(())) => {
            let account = account_snapshot(provider).await;
            (LoginStatus::Authorized, None, Some(account))
        }
        Ok(Err(err)) => {
            if cancel.is_cancelled() {
                (
                    LoginStatus::Cancelled,
                    Some("Login cancelled".to_string()),
                    None,
                )
            } else {
                (LoginStatus::Failed, Some(format!("{err:#}")), None)
            }
        }
    };

    if let Ok(mut map) = sessions().lock() {
        if let Some(state) = map.get_mut(&provider) {
            state.status = status;
            state.error = error;
            if account.is_some() {
                state.account = account;
            }
        }
    }
}

/// Returns the current login session snapshot for a provider.
pub async fn login_status(provider: SubscriptionProvider) -> LoginSessionSnapshot {
    if let Ok(map) = sessions().lock() {
        if let Some(state) = map.get(&provider) {
            return state.snapshot(provider);
        }
    }

    let account = account_snapshot(provider).await;
    let status = if account.connected {
        LoginStatus::Authorized
    } else {
        LoginStatus::Failed
    };
    LoginSessionSnapshot {
        provider,
        status,
        authorization_url: None,
        user_code: None,
        instructions: None,
        error: None,
        account: account.connected.then_some(account),
    }
}

/// Cancels an in-flight login session for a provider.
pub async fn cancel_login(provider: SubscriptionProvider) {
    if let Ok(mut map) = sessions().lock() {
        if let Some(state) = map.get_mut(&provider) {
            state.cancel.cancel();
            state.status = LoginStatus::Cancelled;
            state.error = Some("Login cancelled".to_string());
        }
    }
}

/// Removes the stored credential for a provider.
pub async fn logout(provider: SubscriptionProvider) -> Result<()> {
    // Cancel any in-flight login first so its runner cannot persist fresh
    // tokens after the logout completes.
    if let Ok(mut map) = sessions().lock() {
        if let Some(state) = map.remove(&provider) {
            state.cancel.cancel();
        }
    }
    let _guard = store_lock(provider).lock().await;
    let mut store = store::load().await.unwrap_or_default();
    store.remove(provider.key());
    store::save(&store).await?;
    drop(_guard);
    log::info!("subscription provider {} logged out", provider.key());
    Ok(())
}

/// Resolves a runtime credential for a provider, refreshing tokens if needed.
pub async fn resolve(provider: SubscriptionProvider) -> Result<ResolvedCredential> {
    match provider {
        SubscriptionProvider::Codex => codex::resolve().await,
        SubscriptionProvider::Antigravity => antigravity::resolve().await,
        SubscriptionProvider::Opencode => opencode::resolve().await,
    }
}

/// Forces a resolve (which refreshes and saves), then returns the account entry.
pub async fn refresh_account(provider: SubscriptionProvider) -> Result<SubscriptionAccount> {
    resolve(provider).await?;
    Ok(account_snapshot(provider).await)
}

#[cfg(test)]
mod tests {
    use super::store::{self, StoredCredential};
    use super::*;

    /// Serializes tests that rely on the process-global store path override.
    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_store_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bitfun-subauth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("subscription_auth.json")
    }

    #[test]
    fn subscription_provider_serde_roundtrip() {
        assert_eq!(
            serde_json::to_value(SubscriptionProvider::Codex).unwrap(),
            serde_json::json!("codex")
        );
        assert_eq!(
            serde_json::to_value(SubscriptionProvider::Antigravity).unwrap(),
            serde_json::json!("antigravity")
        );
        let parsed: SubscriptionProvider =
            serde_json::from_value(serde_json::json!("opencode")).unwrap();
        assert_eq!(parsed, SubscriptionProvider::Opencode);
        assert_eq!(
            SubscriptionProvider::from_key("codex"),
            Some(SubscriptionProvider::Codex)
        );
        assert_eq!(SubscriptionProvider::from_key("unknown"), None);
    }

    #[tokio::test]
    async fn store_roundtrip_in_temp_dir() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        store::set_store_path_for_test(temp_store_path());
        let mut store = store::Store::new();
        store.insert(
            "codex".to_string(),
            StoredCredential::Oauth {
                refresh: "refresh-token".to_string(),
                access: "access-token".to_string(),
                expires: 1_800_000_000_000,
                account_id: Some("acct_1".to_string()),
                metadata: Some(serde_json::json!({ "email": "user@example.com" })),
            },
        );
        store::save(&store).await.unwrap();

        let loaded = store::load().await.unwrap();
        let entry = loaded.get("codex").expect("codex entry present");
        match entry {
            StoredCredential::Oauth {
                access, account_id, ..
            } => {
                assert_eq!(access, "access-token");
                assert_eq!(account_id.as_deref(), Some("acct_1"));
            }
            _ => panic!("expected oauth credential"),
        }

        let accounts = list_accounts().await;
        let codex = accounts
            .iter()
            .find(|a| a.provider == SubscriptionProvider::Codex)
            .unwrap();
        assert!(codex.connected);
        assert_eq!(codex.account.as_deref(), Some("user@example.com"));
        assert_eq!(codex.expires_at, Some(1_800_000_000));
    }

    #[tokio::test]
    async fn logout_clears_stored_credential() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        store::set_store_path_for_test(temp_store_path());
        let mut store = store::Store::new();
        store.insert(
            "opencode".to_string(),
            StoredCredential::Api {
                key: "sk-test".to_string(),
                metadata: None,
            },
        );
        store::save(&store).await.unwrap();

        logout(SubscriptionProvider::Opencode).await.unwrap();
        let loaded = store::load().await.unwrap();
        assert!(loaded.get("opencode").is_none());
    }

    #[tokio::test]
    async fn finalize_ignores_superseded_session() {
        let provider = SubscriptionProvider::Codex;
        let stale_generation = next_generation();
        {
            let mut map = sessions().lock().unwrap();
            map.insert(
                provider,
                SessionState {
                    status: LoginStatus::Pending,
                    authorization_url: None,
                    user_code: None,
                    instructions: None,
                    error: None,
                    account: None,
                    cancel: CancellationToken::new(),
                    generation: stale_generation + 1,
                },
            );
        }

        // The stale runner (previous generation) must not overwrite the newer
        // pending session when it finishes.
        finalize_session(
            provider,
            stale_generation,
            &CancellationToken::new(),
            Ok(Err(anyhow!("stale runner failed"))),
        )
        .await;

        let status = {
            let mut map = sessions().lock().unwrap();
            let status = map.get(&provider).map(|state| state.status);
            map.remove(&provider);
            status
        };
        assert_eq!(status, Some(LoginStatus::Pending));
    }
}
