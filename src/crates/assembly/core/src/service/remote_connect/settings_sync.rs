//! Account cloud settings sync engine, shared by Desktop and CLI.
//!
//! Owns the full settings sync lifecycle for one process:
//! - **Push**: `notify_changed()` marks local settings dirty; a 5s debounce
//!   later the engine exports the config and uploads it to the relay. Uploads
//!   are content-hash deduped so identical content is never re-uploaded.
//! - **Pull**: an immediate pull on start, then every 30s, fetches the cloud
//!   settings blob and applies it when the relay version differs from the
//!   last version this device uploaded or applied.
//! - **Apply**: import into the global config service, reload, invalidate the
//!   AI client cache, then fire `on_settings_applied` so the host app can
//!   refresh UI / notify peer controllers.
//!
//! The cursor (`version` + content `hash` of the last uploaded/applied blob)
//! is persisted in `~/.bitfun/account_sync/<user>.settings.json`, separate
//! from the session sync state, so restarts do not re-apply unchanged blobs
//! and co-located processes (e.g. CLI daemon + interactive CLI) share one
//! cursor without racing the session backup writer.
//!
//! Apps wire platform behavior through [`SettingsSyncHooks`]; the engine
//! itself is platform-agnostic.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use log::{debug, warn};
use tokio::sync::mpsc;

use bitfun_services_integrations::remote_connect::account::{
    error_indicates_expired_token, AccountClient, AccountSession, SettingsBlob,
};
use bitfun_services_integrations::remote_connect::sync_state;

/// How often the engine pulls cloud settings.
pub const SETTINGS_PULL_INTERVAL: Duration = Duration::from_secs(30);
/// Debounce window between the last local change and the settings upload.
pub const SETTINGS_PUSH_DEBOUNCE: Duration = Duration::from_secs(5);

/// Account context needed for every relay call: the session (token +
/// master_key) and the relay base URL.
pub type AccountContext = (AccountSession, String);

type AccountContextFn = dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<AccountContext>> + Send>>
    + Send
    + Sync;

/// Platform wiring for the settings sync engine. All hooks have no-op
/// defaults so apps only register what they need.
#[derive(Default)]
pub struct SettingsSyncHooks {
    /// Returns the current account session + relay URL, or an error when
    /// logged out. Required for the background loop; one-shot helpers take
    /// the context explicitly.
    pub account_context: Option<Arc<AccountContextFn>>,
    /// When true, push and pull are paused (Desktop: Peer controller mode).
    pub should_pause: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    /// Fired after cloud settings were applied to the local config.
    pub on_settings_applied: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Fired after local settings were uploaded to the cloud.
    pub on_settings_pushed: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Fired when the relay rejected the account token.
    pub on_token_expired: Option<Arc<dyn Fn() + Send + Sync>>,
}

static HOOKS: OnceLock<SettingsSyncHooks> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);
static PUSH_TX: OnceLock<mpsc::UnboundedSender<()>> = OnceLock::new();
/// Number of uploads/applies currently running (one-shot login sync, loop
/// push, or loop pull apply). The periodic pull skips while non-zero so it
/// never re-applies a blob that is mid-upload or fights an explicit
/// user-chosen direction. A counter (not a flag) so concurrent ops do not
/// clear each other's in-flight state on completion.
static SYNC_OPS_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// RAII guard that marks a sync upload/apply as in flight.
struct SyncOpGuard;
impl SyncOpGuard {
    fn begin() -> Self {
        SYNC_OPS_IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        Self
    }
}
impl Drop for SyncOpGuard {
    fn drop(&mut self) {
        SYNC_OPS_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
}

fn hooks() -> &'static SettingsSyncHooks {
    static DEFAULT: SettingsSyncHooks = SettingsSyncHooks {
        account_context: None,
        should_pause: None,
        on_settings_applied: None,
        on_settings_pushed: None,
        on_token_expired: None,
    };
    HOOKS.get().unwrap_or(&DEFAULT)
}

fn should_pause() -> bool {
    hooks().should_pause.as_ref().map(|f| f()).unwrap_or(false)
}

fn fire_settings_applied() {
    if let Some(f) = hooks().on_settings_applied.as_ref() {
        f();
    }
}

fn fire_settings_pushed() {
    if let Some(f) = hooks().on_settings_pushed.as_ref() {
        f();
    }
}

fn fire_token_expired() {
    if let Some(f) = hooks().on_token_expired.as_ref() {
        f();
    }
}

/// Start the background settings sync loop. Idempotent: later calls are
/// ignored. Safe to call before login — every cycle silently skips while the
/// account context is unavailable, and picks up once the user logs in.
pub fn start_settings_sync_engine(hooks: SettingsSyncHooks) {
    if STARTED.swap(true, Ordering::SeqCst) {
        debug!("Settings sync engine already started; ignoring duplicate start");
        return;
    }
    let _ = HOOKS.set(hooks);
    let (tx, rx) = mpsc::unbounded_channel::<()>();
    let _ = PUSH_TX.set(tx);
    tokio::spawn(settings_sync_loop(rx));
}

/// Notify the engine that local settings changed (config set / import /
/// reset). Cheap and non-blocking; the upload is debounced and deduped.
pub fn notify_settings_changed() {
    if let Some(tx) = PUSH_TX.get() {
        let _ = tx.send(());
    }
}

/// Extract the inner `config` object from a settings payload, tolerating both
/// the `ConfigExport` wrapper (`{ config, export_timestamp, version }`) and a
/// bare `GlobalConfig` JSON.
fn inner_config_value(payload: &str) -> Result<serde_json::Value> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|e| anyhow!("parse settings payload: {e}"))?;
    Ok(value.get("config").cloned().unwrap_or(value))
}

/// Hash the canonical config content of a settings payload, ignoring volatile
/// wrapper fields such as `export_timestamp`.
fn settings_content_hash(payload: &str) -> Result<String> {
    let inner = inner_config_value(payload)?;
    let canonical = serde_json::to_string(&inner)
        .map_err(|e| anyhow!("serialize settings for hashing: {e}"))?;
    Ok(sync_state::content_hash(&canonical))
}

/// Record the settings cursor after a successful upload or apply.
fn record_settings_cursor(user_id: &str, version: i64, hash: String) {
    let cursor = sync_state::SettingsCursor { version, hash };
    if let Err(e) = sync_state::save_settings_cursor(user_id, &cursor) {
        warn!("Settings sync: failed to persist settings cursor: {e}");
    }
}

fn note_relay_error(error: &anyhow::Error, context: &str) {
    if error_indicates_expired_token(&error.to_string()) {
        fire_token_expired();
    }
    warn!("Settings sync: {context} failed: {error}");
}

/// Upload a settings payload to the relay and record the cursor.
/// Returns the version assigned to the upload.
pub async fn upload_settings_payload(
    account: &AccountSession,
    relay_url: &str,
    payload: &str,
) -> Result<i64> {
    let _op = SyncOpGuard::begin();
    let client = AccountClient::new();
    // The version returned here is the exact one stored on the relay —
    // recording it keeps the next pull from re-applying our own upload.
    let version = client.upload_settings(relay_url, account, payload).await?;
    let hash = settings_content_hash(payload).unwrap_or_default();
    record_settings_cursor(&account.user_id, version, hash);
    debug!("Settings sync: uploaded settings (version={version})");
    fire_settings_pushed();
    Ok(version)
}

/// Export the current config and upload it when the content differs from the
/// last uploaded/applied blob. Returns `true` when an upload happened.
pub async fn push_settings_now(account: &AccountSession, relay_url: &str) -> Result<bool> {
    let config_service = crate::service::config::get_global_config_service()
        .await
        .map_err(|e| anyhow!("config service: {e}"))?;
    let exported = config_service
        .export_config()
        .await
        .map_err(|e| anyhow!("export config: {e}"))?;
    let payload = serde_json::to_string(&exported).map_err(|e| anyhow!("serialize config: {e}"))?;

    let hash = settings_content_hash(&payload)?;
    let known = sync_state::load_settings_cursor(&account.user_id);
    if known.hash == hash && known.version != 0 {
        debug!("Settings sync: push skipped, content unchanged");
        return Ok(false);
    }

    upload_settings_payload(account, relay_url, &payload).await?;
    Ok(true)
}

/// Apply a fetched cloud settings blob when its version is newer than the
/// recorded cursor. Pass `force = true` for explicit user choices (login
/// "use cloud") so the blob applies even when the version matches the cursor.
/// Returns `true` when the blob was applied.
pub async fn apply_settings_blob(
    account: &AccountSession,
    blob: &SettingsBlob,
    force: bool,
) -> Result<bool> {
    if !force {
        let known = sync_state::load_settings_cursor(&account.user_id);
        if blob.version == known.version {
            return Ok(false);
        }
    }
    let _op = SyncOpGuard::begin();

    let inner_config = inner_config_value(&blob.plaintext)?;
    let config_service = crate::service::config::get_global_config_service()
        .await
        .map_err(|e| anyhow!("config service: {e}"))?;
    let import_result = config_service
        .import_config_data(inner_config)
        .await
        .map_err(|e| anyhow!("import cloud config: {e}"))?;
    if !import_result.success {
        return Err(anyhow!(
            "import cloud config failed: {}",
            import_result.errors.join("; ")
        ));
    }
    if let Ok(factory) = crate::infrastructure::ai::AIClientFactory::get_global().await {
        factory.invalidate_cache();
    }
    // A failed reload leaves in-memory config stale; bail without recording
    // the cursor so the next pull retries the apply.
    config_service
        .reload()
        .await
        .map_err(|e| anyhow!("reload after config import: {e}"))?;

    let hash = settings_content_hash(&blob.plaintext).unwrap_or_default();
    record_settings_cursor(&account.user_id, blob.version, hash);
    debug!(
        "Settings sync: applied cloud settings (version={})",
        blob.version
    );
    fire_settings_applied();
    Ok(true)
}

/// Fetch the cloud settings blob and apply it when changed. Returns `true`
/// when new settings were applied; `Ok(false)` also when no cloud settings
/// exist yet.
pub async fn pull_and_apply_settings(account: &AccountSession, relay_url: &str) -> Result<bool> {
    let client = AccountClient::new();
    let Some(blob) = client
        .fetch_settings_with_version(relay_url, account)
        .await?
    else {
        return Ok(false);
    };
    apply_settings_blob(account, &blob, false).await
}

async fn account_context() -> Result<AccountContext> {
    let provider = hooks()
        .account_context
        .as_ref()
        .ok_or_else(|| anyhow!("account context provider not registered"))?;
    provider().await
}

async fn push_from_loop() {
    if should_pause() {
        debug!("Settings sync: push paused by host app");
        return;
    }
    let (account, relay_url) = match account_context().await {
        Ok(ctx) => ctx,
        Err(_) => return, // logged out — silently skip
    };
    if let Err(e) = push_settings_now(&account, &relay_url).await {
        note_relay_error(&e, "push");
    }
}

async fn pull_from_loop() {
    if should_pause() {
        debug!("Settings sync: pull paused by host app");
        return;
    }
    if SYNC_OPS_IN_FLIGHT.load(Ordering::SeqCst) > 0 {
        debug!("Settings sync: pull skipped while an upload/apply is in flight");
        return;
    }
    let (account, relay_url) = match account_context().await {
        Ok(ctx) => ctx,
        Err(_) => return, // logged out — silently skip
    };
    if let Err(e) = pull_and_apply_settings(&account, &relay_url).await {
        note_relay_error(&e, "pull");
    }
}

/// Background loop: debounced push on local change + periodic pull.
/// The first pull runs immediately so a long-running process (CLI daemon)
/// converges right after start instead of one interval later.
async fn settings_sync_loop(mut rx: mpsc::UnboundedReceiver<()>) {
    let mut next_pull = tokio::time::Instant::now();
    loop {
        let pull_deadline = tokio::time::sleep_until(next_pull);
        tokio::pin!(pull_deadline);

        tokio::select! {
            Some(()) = rx.recv() => {
                // Drain further notifications during the debounce window.
                let deadline = tokio::time::sleep(SETTINGS_PUSH_DEBOUNCE);
                tokio::pin!(deadline);
                loop {
                    tokio::select! {
                        _ = &mut deadline => break,
                        Some(()) = rx.recv() => {}
                    }
                }
                push_from_loop().await;
            }
            _ = &mut pull_deadline => {
                next_pull = tokio::time::Instant::now() + SETTINGS_PULL_INTERVAL;
                pull_from_loop().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_ignores_export_wrapper_fields() {
        let a = r#"{"config":{"ai":{"models":[{"id":"m1"}]}},"export_timestamp":"2026-01-01T00:00:00Z","version":"1"}"#;
        let b = r#"{"config":{"ai":{"models":[{"id":"m1"}]}},"export_timestamp":"2026-02-02T00:00:00Z","version":"2"}"#;
        assert_eq!(
            settings_content_hash(a).unwrap(),
            settings_content_hash(b).unwrap()
        );
    }

    #[test]
    fn content_hash_changes_with_config_content() {
        let a = r#"{"config":{"ai":{"models":[{"id":"m1"}]}}}"#;
        let b = r#"{"config":{"ai":{"models":[{"id":"m2"}]}}}"#;
        assert_ne!(
            settings_content_hash(a).unwrap(),
            settings_content_hash(b).unwrap()
        );
    }

    #[test]
    fn content_hash_accepts_bare_config_payload() {
        let wrapped = r#"{"config":{"ai":{"models":[{"id":"m1"}]}},"export_timestamp":"t"}"#;
        let bare = r#"{"ai":{"models":[{"id":"m1"}]}}"#;
        assert_eq!(
            settings_content_hash(wrapped).unwrap(),
            settings_content_hash(bare).unwrap()
        );
    }

    #[test]
    fn inner_config_unwraps_export_wrapper() {
        let payload = r#"{"config":{"a":1},"export_timestamp":"t","version":"v"}"#;
        let inner = inner_config_value(payload).unwrap();
        assert_eq!(inner, serde_json::json!({"a": 1}));
    }
}
