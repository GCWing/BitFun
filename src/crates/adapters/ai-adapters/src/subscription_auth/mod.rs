//! In-app subscription authentication.
//!
//! Lets BitFun sign in to another product's subscription (Codex/ChatGPT,
//! Antigravity/Google, OpenCode Zen) with an OpenCode-style in-app OAuth flow,
//! and use the resulting tokens to authenticate AI requests. Secret material
//! is stored in the operating-system credential vault; the local JSON file
//! contains non-secret account metadata only.
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
    /// The account was known previously, but its secret is absent from the
    /// system credential vault. The UI should ask the user to sign in again.
    #[serde(default)]
    pub reauthentication_required: bool,
    /// The system credential vault is currently locked or unavailable. Unlike
    /// a missing entry, this is retryable and should not request re-login.
    #[serde(default)]
    pub vault_unavailable: bool,
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

/// Runs the externally cancellable authorization/polling phase, then commits
/// the resulting credential without cancellation. Dropping a credential-vault
/// write can leave an orphan secret because blocking platform keyring calls
/// continue running after their Rust future is dropped.
pub(crate) async fn authorize_then_persist<T, Authorize, Persist, PersistFuture>(
    provider: SubscriptionProvider,
    cancel: CancellationToken,
    authorize: Authorize,
    persist: Persist,
) -> Result<()>
where
    Authorize: std::future::Future<Output = Result<T>>,
    Persist: FnOnce(T) -> PersistFuture,
    PersistFuture: std::future::Future<Output = Result<()>>,
{
    let credential = tokio::select! {
        _ = cancel.cancelled() => return Err(anyhow!("login cancelled")),
        result = tokio::time::timeout(LOGIN_TIMEOUT, authorize) => match result {
            Ok(result) => result?,
            Err(_) => return Err(anyhow!("Login timed out")),
        },
    };
    // Logout/re-login cancels the generation before waiting on this same
    // provider lock. Whichever side reaches the lock boundary first wins:
    // an already-started commit finishes before logout deletes it, while a
    // cancelled commit waiting on the lock is discarded before writing.
    let _guard = store_lock(provider).lock().await;
    if cancel.is_cancelled() {
        return Err(anyhow!("login cancelled"));
    }
    persist(credential).await
}

fn build_account(
    provider: SubscriptionProvider,
    entry: Option<&StoredCredential>,
    reauthentication_required: bool,
    vault_unavailable: bool,
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
        reauthentication_required,
        vault_unavailable,
        suggested_format: format.to_string(),
        suggested_base_url: base_url.to_string(),
        suggested_model: model.to_string(),
    }
}

async fn account_snapshot(provider: SubscriptionProvider) -> SubscriptionAccount {
    let state = store::load_with_state().await.unwrap_or_else(|error| {
        log::warn!("load subscription credential state failed: {error:#}");
        store::LoadState {
            credentials: store::Store::new(),
            requires_reauthentication: std::collections::HashSet::new(),
            vault_unavailable: std::collections::HashSet::new(),
        }
    });
    build_account(
        provider,
        state.credentials.get(provider.key()),
        state.requires_reauthentication.contains(provider.key()),
        state.vault_unavailable.contains(provider.key()),
    )
}

/// Lists all providers with their current connection state.
pub async fn list_accounts() -> Vec<SubscriptionAccount> {
    let state = store::load_with_state().await.unwrap_or_else(|error| {
        log::warn!("load subscription credential state failed: {error:#}");
        store::LoadState {
            credentials: store::Store::new(),
            requires_reauthentication: std::collections::HashSet::new(),
            vault_unavailable: std::collections::HashSet::new(),
        }
    });
    SubscriptionProvider::ALL
        .iter()
        .map(|provider| {
            build_account(
                *provider,
                state.credentials.get(provider.key()),
                state.requires_reauthentication.contains(provider.key()),
                state.vault_unavailable.contains(provider.key()),
            )
        })
        .collect()
}

/// Starts a login session, cancelling any existing pending session for the
/// same provider. Returns immediately with the authorization URL / user code.
pub async fn start_login(provider: SubscriptionProvider) -> Result<LoginStartResult> {
    let cancel = CancellationToken::new();
    let generation = next_generation();
    let previous = {
        let mut map = sessions()
            .lock()
            .map_err(|_| anyhow!("subscription login session lock poisoned"))?;
        map.insert(
            provider,
            SessionState {
                status: LoginStatus::Pending,
                authorization_url: None,
                user_code: None,
                instructions: None,
                error: None,
                account: None,
                cancel: cancel.clone(),
                generation,
            },
        )
    };
    if let Some(previous) = previous {
        previous.cancel.cancel();
    }

    // The placeholder above makes cancellation visible even while a provider
    // is still binding its callback listener or requesting a device code.
    let begin = async {
        match provider {
            SubscriptionProvider::Codex => codex::begin_login(cancel.clone()).await,
            SubscriptionProvider::Antigravity => antigravity::begin_login(cancel.clone()).await,
            SubscriptionProvider::Opencode => opencode::begin_login(cancel.clone()).await,
        }
    };
    let started_result = tokio::select! {
        _ = cancel.cancelled() => Err(anyhow!("login cancelled")),
        result = begin => result,
    };
    let started = match started_result {
        Ok(started) if !cancel.is_cancelled() => started,
        Ok(_) => return Err(anyhow!("login cancelled")),
        Err(error) => {
            if let Ok(mut map) = sessions().lock() {
                if let Some(state) = map
                    .get_mut(&provider)
                    .filter(|state| state.generation == generation)
                {
                    state.status = if cancel.is_cancelled() {
                        LoginStatus::Cancelled
                    } else {
                        LoginStatus::Failed
                    };
                    state.error = Some(format!("{error:#}"));
                }
            }
            return Err(error);
        }
    };

    let authorization_url = started.authorization_url.clone();
    // Desktop opener rejects relative URLs ("Not allowed to open url /...").
    // Every provider must return an absolute http(s) authorization URL.
    if !(authorization_url.starts_with("https://") || authorization_url.starts_with("http://")) {
        cancel.cancel();
        if let Ok(mut map) = sessions().lock() {
            if let Some(state) = map
                .get_mut(&provider)
                .filter(|state| state.generation == generation)
            {
                state.status = LoginStatus::Failed;
                state.error = Some(
                    "Subscription login returned a non-absolute authorization URL".to_string(),
                );
            }
        }
        return Err(anyhow!(
            "subscription login returned a non-absolute authorization URL: {authorization_url}"
        ));
    }
    let user_code = started.user_code.clone();
    let instructions = started.instructions.clone();
    {
        let mut map = sessions()
            .lock()
            .map_err(|_| anyhow!("subscription login session lock poisoned"))?;
        let Some(state) = map
            .get_mut(&provider)
            .filter(|state| state.generation == generation && !state.cancel.is_cancelled())
        else {
            cancel.cancel();
            return Err(anyhow!("login cancelled"));
        };
        state.authorization_url = Some(authorization_url.clone());
        state.user_code = user_code.clone();
        state.instructions = Some(instructions.clone());
    }

    let runner = started.runner;
    tokio::spawn(async move {
        // Authorization timeout lives inside `authorize_then_persist`; once
        // persistence begins it must not be dropped by a surrounding timeout.
        let outcome: Result<Result<()>, tokio::time::error::Elapsed> = Ok(runner.await);
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

    update_session_if_current(provider, generation, status, error, account);
}

fn update_session_if_current(
    provider: SubscriptionProvider,
    generation: u64,
    status: LoginStatus,
    error: Option<String>,
    account: Option<SubscriptionAccount>,
) {
    if let Ok(mut map) = sessions().lock() {
        if let Some(state) = map
            .get_mut(&provider)
            .filter(|state| state.generation == generation)
        {
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
    // Act as a completion barrier for the commit phase. If cancellation wins
    // the provider lock, the runner observes the cancelled token and skips its
    // write. If persistence already owns the lock, let that atomic commit
    // finish before reporting cancellation back to the UI.
    let _guard = store_lock(provider).lock().await;
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
    store::remove(provider.key()).await?;
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

        let metadata_file = std::fs::read_to_string(store_path_override_for_assertion()).unwrap();
        assert!(!metadata_file.contains("refresh-token"));
        assert!(!metadata_file.contains("access-token"));
        assert!(metadata_file.contains("user@example.com"));

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
        assert!(!codex.reauthentication_required);
    }

    fn store_path_override_for_assertion() -> std::path::PathBuf {
        super::store::store_path_for_test_assertion()
    }

    #[tokio::test]
    async fn legacy_plaintext_store_is_migrated_and_scrubbed() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = temp_store_path();
        store::set_store_path_for_test(path.clone());
        let legacy = serde_json::json!({
            "opencode": {
                "type": "oauth",
                "refresh": "legacy-refresh-secret",
                "access": "legacy-access-secret",
                "expires": 1_900_000_000_000_i64,
                "metadata": { "email": "legacy@example.com" }
            }
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let loaded = store::load().await.unwrap();
        assert!(loaded.contains_key("opencode"));

        let migrated = std::fs::read_to_string(&path).unwrap();
        assert!(migrated.contains("\"version\": 2"));
        assert!(migrated.contains("legacy@example.com"));
        assert!(!migrated.contains("legacy-refresh-secret"));
        assert!(!migrated.contains("legacy-access-secret"));
    }

    #[tokio::test]
    async fn concurrent_provider_upserts_preserve_both_metadata_entries() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = temp_store_path();
        store::set_store_path_for_test(path.clone());

        let codex = store::upsert(
            "codex",
            StoredCredential::Oauth {
                refresh: "codex-refresh".to_string(),
                access: "codex-access".to_string(),
                expires: 1_900_000_000_000,
                account_id: Some("codex-account".to_string()),
                metadata: None,
            },
        );
        let opencode = store::upsert(
            "opencode",
            StoredCredential::Oauth {
                refresh: "opencode-refresh".to_string(),
                access: "opencode-access".to_string(),
                expires: 1_900_000_000_000,
                account_id: None,
                metadata: Some(serde_json::json!({ "email": "zen@example.com" })),
            },
        );
        let (codex_result, opencode_result) = tokio::join!(codex, opencode);
        codex_result.unwrap();
        opencode_result.unwrap();

        let loaded = store::load().await.unwrap();
        assert!(loaded.contains_key("codex"));
        assert!(loaded.contains_key("opencode"));
        let metadata = std::fs::read_to_string(path).unwrap();
        assert!(metadata.contains("\"codex\""));
        assert!(metadata.contains("\"opencode\""));
        assert!(!metadata.contains("codex-access"));
        assert!(!metadata.contains("opencode-access"));
    }

    #[tokio::test]
    async fn repeated_upsert_replaces_existing_metadata_file() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = temp_store_path();
        store::set_store_path_for_test(path.clone());

        store::upsert(
            "codex",
            StoredCredential::Oauth {
                refresh: "old-refresh".to_string(),
                access: "old-access".to_string(),
                expires: 1_800_000_000_000,
                account_id: None,
                metadata: None,
            },
        )
        .await
        .unwrap();
        store::upsert(
            "codex",
            StoredCredential::Oauth {
                refresh: "new-refresh".to_string(),
                access: "new-access".to_string(),
                expires: 1_900_000_000_000,
                account_id: Some("updated-account".to_string()),
                metadata: None,
            },
        )
        .await
        .unwrap();

        let loaded = store::load_entry("codex").await.unwrap().unwrap();
        match loaded {
            StoredCredential::Oauth {
                refresh,
                access,
                expires,
                account_id,
                ..
            } => {
                assert_eq!(refresh, "new-refresh");
                assert_eq!(access, "new-access");
                assert_eq!(expires, 1_900_000_000_000);
                assert_eq!(account_id.as_deref(), Some("updated-account"));
            }
            _ => panic!("expected oauth credential"),
        }
        let metadata = std::fs::read_to_string(path).unwrap();
        assert!(!metadata.contains("old-access"));
        assert!(!metadata.contains("new-access"));
        assert!(metadata.contains("updated-account"));
    }

    #[tokio::test]
    async fn long_tokens_are_split_below_the_windows_vault_limit() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = temp_store_path();
        store::set_store_path_for_test(path.clone());
        let refresh = "r".repeat(5_000);
        let access = "a".repeat(9_000);

        store::upsert(
            "codex",
            StoredCredential::Oauth {
                refresh: refresh.clone(),
                access: access.clone(),
                expires: 1_900_000_000_000,
                account_id: None,
                metadata: None,
            },
        )
        .await
        .unwrap();

        let entries = store::test_vault_entries_for_assertion();
        assert!(entries.len() > 2, "long tokens must use multiple entries");
        assert!(entries.keys().all(|name| name != "codex"));
        assert!(entries.values().all(|part| part.len() <= 2_048));
        let loaded = store::load_entry("codex").await.unwrap().unwrap();
        match loaded {
            StoredCredential::Oauth {
                refresh: loaded_refresh,
                access: loaded_access,
                ..
            } => {
                assert_eq!(loaded_refresh, refresh);
                assert_eq!(loaded_access, access);
            }
            _ => panic!("expected oauth credential"),
        }
        let metadata = std::fs::read_to_string(path).unwrap();
        assert!(!metadata.contains(&refresh));
        assert!(!metadata.contains(&access));
    }

    #[tokio::test]
    async fn unavailable_vault_is_retryable_not_missing_credential() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        store::set_store_path_for_test(temp_store_path());
        store::upsert(
            "opencode",
            StoredCredential::Api {
                key: "sk-present-but-locked".to_string(),
                metadata: None,
            },
        )
        .await
        .unwrap();

        store::set_test_vault_unavailable(true);
        let state = store::load_with_state().await.unwrap();
        assert!(state.credentials.get("opencode").is_none());
        assert!(!state.requires_reauthentication.contains("opencode"));
        assert!(state.vault_unavailable.contains("opencode"));
        let error = store::load_entry("opencode").await.unwrap_err();
        assert!(error.to_string().contains("locked or unavailable"));
        store::set_test_vault_unavailable(false);

        let restored = store::load_entry("opencode").await.unwrap();
        assert!(restored.is_some());
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
    async fn failed_logout_metadata_commit_preserves_usable_credential() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        store::set_store_path_for_test(temp_store_path());
        store::upsert(
            "opencode",
            StoredCredential::Api {
                key: "sk-still-usable".to_string(),
                metadata: None,
            },
        )
        .await
        .unwrap();
        let entries_before = store::test_vault_entries_for_assertion();

        store::set_test_metadata_write_failure(true);
        let error = store::remove("opencode").await.unwrap_err();
        assert!(error.to_string().contains("injected"));
        store::set_test_metadata_write_failure(false);

        assert_eq!(store::test_vault_entries_for_assertion(), entries_before);
        let loaded = store::load_entry("opencode").await.unwrap().unwrap();
        match loaded {
            StoredCredential::Api { key, .. } => assert_eq!(key, "sk-still-usable"),
            _ => panic!("expected api credential"),
        }
    }

    #[tokio::test]
    async fn finalize_ignores_superseded_session() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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

    #[tokio::test]
    async fn cancellation_does_not_drop_started_credential_persistence() {
        let cancel = CancellationToken::new();
        let (persist_started_tx, persist_started_rx) = tokio::sync::oneshot::channel();
        let (allow_persist_tx, allow_persist_rx) = tokio::sync::oneshot::channel();

        let task = tokio::spawn(authorize_then_persist(
            SubscriptionProvider::Codex,
            cancel.clone(),
            async { Ok::<_, anyhow::Error>("authorized-token") },
            move |token| async move {
                assert_eq!(token, "authorized-token");
                persist_started_tx.send(()).unwrap();
                allow_persist_rx.await.unwrap();
                Ok(())
            },
        ));

        persist_started_rx.await.unwrap();
        cancel.cancel();
        tokio::task::yield_now().await;
        assert!(!task.is_finished());

        allow_persist_tx.send(()).unwrap();
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn cancellation_before_authorization_skips_persistence() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let persisted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let persisted_for_task = persisted.clone();

        let result = authorize_then_persist(
            SubscriptionProvider::Opencode,
            cancel,
            std::future::pending::<Result<()>>(),
            move |_| async move {
                persisted_for_task.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .await;

        assert!(result.is_err());
        assert!(!persisted.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cancelled_commit_waiting_for_provider_lock_does_not_persist() {
        let provider = SubscriptionProvider::Antigravity;
        let store_guard = store_lock(provider).lock().await;
        let cancel = CancellationToken::new();
        let (authorized_tx, authorized_rx) = tokio::sync::oneshot::channel();
        let persisted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let persisted_for_task = persisted.clone();

        let task = tokio::spawn(authorize_then_persist(
            provider,
            cancel.clone(),
            async move {
                authorized_tx.send(()).unwrap();
                Ok::<_, anyhow::Error>("authorized-token")
            },
            move |_| async move {
                persisted_for_task.store(true, Ordering::SeqCst);
                Ok(())
            },
        ));

        authorized_rx.await.unwrap();
        cancel.cancel();
        drop(store_guard);

        assert!(task.await.unwrap().is_err());
        assert!(!persisted.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn cancel_command_waits_for_the_commit_boundary() {
        let provider = SubscriptionProvider::Opencode;
        let store_guard = store_lock(provider).lock().await;
        let cancel = CancellationToken::new();
        let generation = next_generation();
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
                    cancel: cancel.clone(),
                    generation,
                },
            );
        }

        let task = tokio::spawn(cancel_login(provider));
        tokio::task::yield_now().await;
        assert!(cancel.is_cancelled());
        assert!(!task.is_finished());

        drop(store_guard);
        task.await.unwrap();
        let status = sessions()
            .lock()
            .unwrap()
            .remove(&provider)
            .map(|state| state.status);
        assert_eq!(status, Some(LoginStatus::Cancelled));
    }

    #[test]
    fn final_state_update_rechecks_generation_after_async_work() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let provider = SubscriptionProvider::Codex;
        let old_generation = next_generation();
        let new_generation = next_generation();
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
                    generation: new_generation,
                },
            );
        }

        update_session_if_current(
            provider,
            old_generation,
            LoginStatus::Authorized,
            None,
            None,
        );

        let status = {
            let mut map = sessions().lock().unwrap();
            let status = map.get(&provider).map(|state| state.status);
            map.remove(&provider);
            status
        };
        assert_eq!(status, Some(LoginStatus::Pending));
    }
}
