//! CLI account login and device-routing (RPC control) support.
//!
//! This module lets the CLI log in to a BitFun relay account and then become
//! RPC-controllable by other devices on the same account.
//!
//! Incoming `HostInvoke` / `DeviceEvent` messages are handled by
//! `crate::peer_host` (Peer Device Mode host). Other remote-connect commands
//! still go through `RemoteServer`.
//!
//! The master key lives in memory only and is lost when the CLI exits.

use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::{Notify, RwLock};

use bitfun_core::service::remote_connect::{
    self, encryption, relay_client::RelayClient, relay_client::RelayEvent, session_store,
    validate_relay_base_url, AccountClient, AccountSession, DeviceIdentity, RemoteServer,
};

#[derive(Clone)]
struct AccountContextState {
    session: AccountSession,
    relay_url: String,
}

/// Session and relay URL are one atomic account context so concurrent login,
/// logout, routing and sync cannot observe a torn pair.
static ACCOUNT_CONTEXT: OnceLock<Arc<RwLock<Option<AccountContextState>>>> = OnceLock::new();
static ACCOUNT_CONTEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static ACCOUNT_CONTEXT_TRANSITIONS: AtomicUsize = AtomicUsize::new(0);
static ACCOUNT_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Serializes candidate credential verification without hiding or stopping the
/// currently active account. Only a fully authenticated candidate may enter
/// the account transition that replaces it.
static ACCOUNT_LOGIN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ACCOUNT_CONTEXT_TRANSITION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ACCOUNT_SYNC_CANCEL: OnceLock<Notify> = OnceLock::new();
/// At most one delayed daemon-exit recovery poller may own a generation.
/// A newer generation supersedes an older poller without accumulating tasks.
static ROUTING_RECOVERY_GENERATION: AtomicU64 = AtomicU64::new(0);
/// Read leases cover one routing event through its side effects and response.
/// Account transitions and routing-client ownership changes take the write
/// lease, so a new owner cannot be published while an old handler is active.
static DEVICE_ROUTING_LIFECYCLE: RwLock<()> = RwLock::const_new(());

pub(crate) fn account_context_generation() -> u64 {
    ACCOUNT_CONTEXT_GENERATION.load(Ordering::Acquire)
}

pub(crate) fn account_context_is_current(generation: u64) -> bool {
    ACCOUNT_CONTEXT_TRANSITIONS.load(Ordering::Acquire) == 0
        && account_context_generation() == generation
}

struct AccountContextTransitionPermit;

impl AccountContextTransitionPermit {
    fn begin() -> Self {
        ACCOUNT_CONTEXT_TRANSITIONS.fetch_add(1, Ordering::AcqRel);
        ACCOUNT_CONTEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        account_sync_cancel().notify_waiters();
        Self
    }
}

impl Drop for AccountContextTransitionPermit {
    fn drop(&mut self) {
        // Reject work queued during the transition before exposing the newly
        // installed (or cleared) account context.
        ACCOUNT_CONTEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        ACCOUNT_CONTEXT_TRANSITIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

struct AccountContextTransitionGuard {
    sync_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
    transition: Option<AccountContextTransitionPermit>,
    routing_guard: Option<tokio::sync::RwLockWriteGuard<'static, ()>>,
    transition_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
}

impl AccountContextTransitionGuard {
    fn finish(mut self) -> u64 {
        drop(self.sync_guard.take());
        drop(self.transition.take());
        let generation = account_context_generation();
        drop(self.routing_guard.take());
        drop(self.transition_guard.take());
        generation
    }
}

impl Drop for AccountContextTransitionGuard {
    fn drop(&mut self) {
        drop(self.sync_guard.take());
        drop(self.transition.take());
        drop(self.routing_guard.take());
        drop(self.transition_guard.take());
    }
}

pub(crate) async fn lock_account_sync(
    generation: u64,
) -> Result<tokio::sync::MutexGuard<'static, ()>> {
    let guard = ACCOUNT_SYNC_LOCK.lock().await;
    if !account_context_is_current(generation) {
        return Err(anyhow!("account sync cancelled"));
    }
    Ok(guard)
}

fn account_sync_cancel() -> &'static Notify {
    ACCOUNT_SYNC_CANCEL.get_or_init(Notify::new)
}

pub(crate) async fn await_account_sync_current<F, T>(generation: u64, future: F) -> Result<T>
where
    F: Future<Output = T>,
{
    let mut cancelled = Box::pin(account_sync_cancel().notified());
    cancelled.as_mut().enable();
    if !account_context_is_current(generation) {
        return Err(anyhow!("account sync cancelled"));
    }
    tokio::select! {
        _ = &mut cancelled => Err(anyhow!("account sync cancelled")),
        result = future => {
            if !account_context_is_current(generation) {
                Err(anyhow!("account sync cancelled"))
            } else {
                Ok(result)
            }
        }
    }
}

async fn invalidate_and_wait_for_account_sync() -> AccountContextTransitionGuard {
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    let transition = AccountContextTransitionPermit::begin();
    let sync_guard = ACCOUNT_SYNC_LOCK.lock().await;
    bitfun_core::service::remote_connect::settings_sync::wait_for_sync_operations_idle().await;
    let routing_guard = DEVICE_ROUTING_LIFECYCLE.write().await;
    AccountContextTransitionGuard {
        sync_guard: Some(sync_guard),
        transition: Some(transition),
        routing_guard: Some(routing_guard),
        transition_guard: Some(transition_guard),
    }
}

async fn invalidate_and_wait_if_account_current(
    expected_generation: u64,
) -> Option<AccountContextTransitionGuard> {
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    if !account_context_is_current(expected_generation) {
        return None;
    }
    let transition = AccountContextTransitionPermit::begin();
    let sync_guard = ACCOUNT_SYNC_LOCK.lock().await;
    bitfun_core::service::remote_connect::settings_sync::wait_for_sync_operations_idle().await;
    let routing_guard = DEVICE_ROUTING_LIFECYCLE.write().await;
    Some(AccountContextTransitionGuard {
        sync_guard: Some(sync_guard),
        transition: Some(transition),
        routing_guard: Some(routing_guard),
        transition_guard: Some(transition_guard),
    })
}

/// The background device-routing relay client. Holding this keeps the WS
/// connection alive (the internal read/write tasks own the socket). Dropping it
/// tears the connection down.
static DEVICE_RELAY_CLIENT: OnceLock<RwLock<Option<Arc<RelayClient>>>> = OnceLock::new();

/// Set when the relay returns an auth error (token expired or invalid).
/// The chat loop checks this via `is_token_expired()` and prompts the user.
static TOKEN_EXPIRED: AtomicBool = AtomicBool::new(false);

/// True while credentials succeeded but the user has not yet chosen
/// cloud-vs-local settings. Session is held in memory only; a process kill
/// must not restore a logged-in state (same contract as desktop
/// `account_login` / `account_finalize_login`).
static PENDING_SYNC_CHOICE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutomaticAccountSyncPolicy {
    pub(crate) background_engine: bool,
    pub(crate) management_push: bool,
}

fn automatic_account_sync_policy_for_pending(
    pending_sync_choice: bool,
) -> AutomaticAccountSyncPolicy {
    let allowed = !pending_sync_choice;
    AutomaticAccountSyncPolicy {
        background_engine: allowed,
        management_push: allowed,
    }
}

/// Automatic sync must remain idle while an authenticated account is waiting
/// for the user to choose whether cloud or local settings should win. Explicit
/// first-login sync is intentionally not governed by this policy.
pub(crate) fn automatic_account_sync_policy() -> AutomaticAccountSyncPolicy {
    automatic_account_sync_policy_for_pending(PENDING_SYNC_CHOICE.load(Ordering::Acquire))
}

fn account_context() -> &'static Arc<RwLock<Option<AccountContextState>>> {
    ACCOUNT_CONTEXT.get_or_init(|| Arc::new(RwLock::new(None)))
}

fn device_relay_client() -> &'static RwLock<Option<Arc<RelayClient>>> {
    DEVICE_RELAY_CLIENT.get_or_init(|| RwLock::new(None))
}

/// Read both the session and relay URL, returning owned clones to avoid holding
/// locks across awaits.
pub(crate) async fn read_account_context() -> Result<(AccountSession, String)> {
    let generation = account_context_generation();
    read_account_context_for_generation(generation).await
}

async fn read_account_context_raw() -> Result<(AccountSession, String)> {
    account_context()
        .read()
        .await
        .clone()
        .map(|context| (context.session, context.relay_url))
        .ok_or_else(|| anyhow!("not logged in"))
}

pub(crate) async fn read_account_context_for_generation(
    generation: u64,
) -> Result<(AccountSession, String)> {
    if !account_context_is_current(generation) {
        return Err(anyhow!("account context changed"));
    }
    let context = read_account_context_raw().await?;
    if !account_context_is_current(generation) {
        return Err(anyhow!("account context changed"));
    }
    Ok(context)
}

/// Whether an account session is currently held and login is finalized.
/// Matches desktop `account_status`: pending cloud/local sync choice is not
/// treated as logged in.
pub(crate) async fn is_logged_in() -> bool {
    if PENDING_SYNC_CHOICE.load(Ordering::Acquire) {
        return false;
    }
    read_account_context().await.is_ok()
}

fn normalize_relay_url(relay_url: &str) -> Result<String> {
    let parsed = validate_relay_base_url(relay_url.trim())?;
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

/// Attempt to restore a persisted session from disk.  Called at startup.
/// Returns `Some(user_id)` if a session was restored.
pub(crate) async fn try_restore_session() -> Option<String> {
    let _sync_guard = invalidate_and_wait_for_account_sync().await;
    match session_store::load_session_detailed() {
        Ok(Some(loaded)) => {
            let relay_url = match normalize_relay_url(&loaded.relay_url) {
                Ok(url) => url,
                Err(error) => {
                    tracing::warn!("Ignoring invalid persisted relay URL: {error}");
                    session_store::clear_session();
                    return None;
                }
            };
            let user_id = loaded.user_id.clone();
            if let Some(device_id) = loaded.device_id.as_deref() {
                if let Err(e) = DeviceIdentity::adopt_account_device_id(device_id) {
                    tracing::warn!("Failed to adopt restored session device_id: {e}");
                }
            }
            let session = AccountSession {
                token: loaded.token,
                user_id: user_id.clone(),
                master_key: loaded.master_key,
            };
            *account_context().write().await = Some(AccountContextState { session, relay_url });
            tracing::info!("Restored account session for user {user_id}");
            Some(user_id)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("Failed to load persisted session: {e}");
            None
        }
    }
}

/// Whether the relay has reported the account token as expired/invalid.
/// The TUI prompts re-login via this; the daemon exits on it.
pub(crate) fn is_token_expired() -> bool {
    TOKEN_EXPIRED.load(Ordering::Relaxed)
}

/// Mark the account token as rejected by the relay (expired / revoked).
/// Called by the settings sync engine when a sync request gets a 401.
pub(crate) fn mark_token_expired() {
    TOKEN_EXPIRED.store(true, Ordering::Relaxed);
}

/// Resolve the current device identity (machine-based).
fn current_device_identity() -> Result<DeviceIdentity> {
    DeviceIdentity::from_current_machine().map_err(|e| anyhow!("detect device: {e}"))
}

/// Structured result of a successful credential login.
#[derive(Debug, Clone)]
pub(crate) struct LoginResult {
    pub user_id: String,
    pub relay_url: String,
    /// True when the relay already has a settings blob (Desktop overwrite prompt).
    pub has_cloud_settings: bool,
    pub status_message: String,
}

/// Log in with credentials collected by the Login TUI form.
///
/// Same fields as Desktop Account Login: Auth Server (relay URL), Username,
/// Password. Persists encrypted session + non-secret hint, then starts device
/// routing so this CLI becomes a Peer Device Mode host.
pub(crate) async fn login_with_credentials(
    relay_url: &str,
    username: &str,
    password: &str,
) -> Result<LoginResult> {
    let _login_guard = ACCOUNT_LOGIN_LOCK.lock().await;
    let relay_url_input = relay_url.trim();
    let username = username.trim();
    if relay_url_input.is_empty() {
        return Err(anyhow!("Auth Server is required"));
    }
    if username.is_empty() {
        return Err(anyhow!("Username is required"));
    }
    if password.is_empty() {
        return Err(anyhow!("Password is required"));
    }
    let relay_url = normalize_relay_url(relay_url_input)?;
    let expected_generation = account_context_generation();
    if !account_context_is_current(expected_generation) {
        return Err(anyhow!("account context changed"));
    }

    let device = current_device_identity()?;
    let client = AccountClient::new();
    let session = client
        .login(&relay_url, username, password, &device)
        .await
        .map_err(|e| anyhow!("login failed: {e}"))?;

    let has_cloud_settings =
        match resolve_cloud_settings_probe(client.fetch_settings(&relay_url, &session).await) {
            Ok(has_cloud_settings) => has_cloud_settings,
            Err(error) => {
                revoke_rejected_login_candidate(&client, &relay_url, &session).await;
                return Err(error);
            }
        };

    // A daemon is a separate process with its own in-memory session and WebSocket.
    // Retire it after candidate authentication but before beginning the local
    // generation transition. If retirement fails, the old local owner keeps
    // its original generation and remains usable. A clean daemon exit is not
    // auto-restarted by the generated launchd/systemd service definitions.
    // Snapshot the old owner before the guarded replacement. A generation race
    // rejects the transition below, in which case this snapshot is never used.
    let previous_account_context = account_context().read().await.clone();
    let (retired_daemon, transition_guard) = match begin_candidate_account_transition(
        expected_generation,
        retire_running_daemon_for_account_switch().await,
    )
    .await
    {
        Ok(transition) => transition,
        Err(CandidateAccountTransitionError::DaemonRetirement(failure)) => {
            let recovery_message = if failure.daemon_may_exit {
                schedule_routing_recovery_after_daemon_exit(
                    expected_generation,
                    device.device_name.clone(),
                );
                "; this CLI will restore local routing if the daemon exits"
            } else {
                ""
            };
            revoke_rejected_login_candidate(&client, &relay_url, &session).await;
            return Err(anyhow!(
                "{}; the old account context and generation were preserved{}",
                failure.error,
                recovery_message
            ));
        }
        Err(CandidateAccountTransitionError::AccountContextChanged) => {
            revoke_rejected_login_candidate(&client, &relay_url, &session).await;
            return Err(anyhow!("account context changed"));
        }
    };

    // The transition owns the routing lifecycle write lease. Retire any
    // in-process owner before making the candidate context observable.
    clear_replaced_persisted_session();
    stop_device_routing_locked().await;

    let user_id = session.user_id.clone();
    let device_name = device.device_name.clone();
    let token = session.token.clone();
    let master_key = session.master_key;
    *account_context().write().await = Some(AccountContextState {
        session,
        relay_url: relay_url.clone(),
    });
    session_store::save_credential_hint(username, &relay_url);
    TOKEN_EXPIRED.store(false, Ordering::Relaxed);

    if has_cloud_settings {
        // Defer disk persist until the sync choice is accepted. Killing the
        // process during the choice panel must not restore a logged-in session.
        PENDING_SYNC_CHOICE.store(true, Ordering::Release);
        transition_guard.finish();
        revoke_replaced_account_context(&client, previous_account_context, &relay_url, &token)
            .await;
        return Ok(LoginResult {
            user_id: user_id.clone(),
            relay_url: relay_url.clone(),
            has_cloud_settings,
            status_message: format!(
                "Authenticated as user {} on {}. Choose cloud or local settings to finish login.{}",
                user_id,
                relay_url,
                if retired_daemon {
                    " The previous CLI daemon was stopped; routing will resume after the sync choice."
                } else {
                    ""
                }
            ),
        });
    }

    PENDING_SYNC_CHOICE.store(false, Ordering::Release);
    if let Err(e) = session_store::save_session_with_device(
        &token,
        &user_id,
        &master_key,
        &relay_url,
        Some(device.device_id.as_str()),
    ) {
        tracing::warn!("Failed to persist session: {e}");
    }

    let generation = transition_guard.finish();
    let routing_msg = match spawn_device_routing(&relay_url, &device_name, generation).await {
        Ok(()) if retired_daemon => " The previous CLI daemon was stopped and routing is connected in this CLI process. Restart `bitfun daemon run` to restore always-on routing.".to_string(),
        Ok(()) => " Device routing connected (Peer Host ready). Tip: `bitfun daemon install` keeps this device reachable after exit or reboot.".to_string(),
        Err(e) if retired_daemon => format!(" (Warning: the previous CLI daemon was stopped, but replacement routing failed: {e})"),
        Err(e) => format!(" (Warning: device routing failed: {e})"),
    };
    revoke_replaced_account_context(&client, previous_account_context, &relay_url, &token).await;

    Ok(LoginResult {
        user_id: user_id.clone(),
        relay_url: relay_url.clone(),
        has_cloud_settings,
        status_message: format!(
            "Logged in as user {} on {}.{}",
            user_id, relay_url, routing_msg
        ),
    })
}

async fn revoke_rejected_login_candidate(
    client: &AccountClient,
    relay_url: &str,
    session: &AccountSession,
) {
    if let Err(error) = client.revoke_token(relay_url, session).await {
        tracing::warn!("Failed to revoke rejected login candidate token: {error}");
    }
}

fn clear_replaced_persisted_session() {
    // Once this candidate has won the transition, the old account must never
    // be restored after a crash. A finalized replacement is persisted below;
    // a pending cloud-sync choice intentionally leaves no restorable session.
    session_store::clear_session();
}

fn replaced_account_revocation_target(
    previous: Option<AccountContextState>,
    replacement_relay_url: &str,
    replacement_token: &str,
) -> Option<AccountContextState> {
    previous.filter(|context| {
        context.relay_url != replacement_relay_url || context.session.token != replacement_token
    })
}

async fn revoke_replaced_account_context(
    client: &AccountClient,
    previous: Option<AccountContextState>,
    replacement_relay_url: &str,
    replacement_token: &str,
) {
    let Some(previous) =
        replaced_account_revocation_target(previous, replacement_relay_url, replacement_token)
    else {
        return;
    };
    if let Err(error) = client
        .revoke_token(&previous.relay_url, &previous.session)
        .await
    {
        // B is already the committed in-memory owner. Relay cleanup of A is
        // best-effort and must never roll the replacement back.
        tracing::warn!("Failed to revoke replaced account token: {error}");
    }
}

fn resolve_cloud_settings_probe(result: Result<Option<String>>) -> Result<bool> {
    result.map(|settings| settings.is_some()).map_err(|error| {
        anyhow!("could not check cloud settings: {error}; the current account remains active")
    })
}

struct DaemonRetirementFailure {
    error: anyhow::Error,
    daemon_may_exit: bool,
}

enum CandidateAccountTransitionError {
    DaemonRetirement(DaemonRetirementFailure),
    AccountContextChanged,
}

async fn begin_candidate_account_transition(
    expected_generation: u64,
    daemon_retirement: std::result::Result<bool, DaemonRetirementFailure>,
) -> std::result::Result<(bool, AccountContextTransitionGuard), CandidateAccountTransitionError> {
    let retired_daemon =
        daemon_retirement.map_err(CandidateAccountTransitionError::DaemonRetirement)?;
    let transition_guard = invalidate_and_wait_if_account_current(expected_generation)
        .await
        .ok_or(CandidateAccountTransitionError::AccountContextChanged)?;
    Ok((retired_daemon, transition_guard))
}

async fn retire_running_daemon_for_account_switch(
) -> std::result::Result<bool, DaemonRetirementFailure> {
    if !crate::daemon::is_daemon_running() {
        return Ok(false);
    }
    if !crate::daemon::request_daemon_shutdown() {
        return Err(DaemonRetirementFailure {
            error: anyhow!("could not stop the CLI daemon; the current account remains active"),
            daemon_may_exit: false,
        });
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while crate::daemon::is_daemon_running() {
        if tokio::time::Instant::now() >= deadline {
            return Err(DaemonRetirementFailure {
                error: anyhow!(
                    "CLI daemon did not stop in time; the current account remains active"
                ),
                daemon_may_exit: true,
            });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(true)
}

fn schedule_routing_recovery_after_daemon_exit(expected_generation: u64, device_name: String) {
    if !account_context_is_current(expected_generation)
        || ROUTING_RECOVERY_GENERATION.swap(expected_generation, Ordering::AcqRel)
            == expected_generation
    {
        return;
    }
    tokio::spawn(async move {
        while ROUTING_RECOVERY_GENERATION.load(Ordering::Acquire) == expected_generation
            && account_context_is_current(expected_generation)
            && crate::daemon::is_daemon_running()
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if ROUTING_RECOVERY_GENERATION.load(Ordering::Acquire) == expected_generation
            && account_context_is_current(expected_generation)
            && !crate::daemon::is_daemon_running()
        {
            if let Err(error) = restore_device_routing(&device_name).await {
                tracing::warn!(
                    "Failed to restore old account routing after delayed daemon exit: {error}"
                );
            }
        }
        let _ = ROUTING_RECOVERY_GENERATION.compare_exchange(
            expected_generation,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    });
}

/// Persist the in-memory session after the user accepts the sync choice, then
/// start device routing (same as a first login with no cloud settings).
pub(crate) async fn finalize_login_after_sync_choice() -> Result<()> {
    let generation = account_context_generation();
    let sync_guard = lock_account_sync(generation).await?;
    let device = current_device_identity()?;
    let (session, relay_url) = read_account_context().await?;
    let retired_daemon = retire_running_daemon_for_account_switch()
        .await
        .map_err(|failure| failure.error)?;
    session_store::save_session_with_device(
        &session.token,
        &session.user_id,
        &session.master_key,
        &relay_url,
        Some(device.device_id.as_str()),
    )
    .map_err(|e| anyhow!("persist session: {e}"))?;
    PENDING_SYNC_CHOICE.store(false, Ordering::Release);

    if retired_daemon {
        tracing::info!(
            "Stopped the previous CLI daemon before finalizing replacement account routing"
        );
    }
    drop(sync_guard);
    spawn_device_routing(&relay_url, &device.device_name, generation)
        .await
        .map_err(|e| anyhow!("device routing failed: {e}"))
}

/// Snapshot of the logged-in account for the Account status page.
#[derive(Debug, Clone)]
pub(crate) struct AccountInfo {
    pub user_id: String,
    pub relay_url: String,
    pub device_id: String,
    pub device_name: String,
}

pub(crate) async fn account_info() -> Result<AccountInfo> {
    let (session, relay_url) = read_account_context().await?;
    let device = current_device_identity()?;
    Ok(AccountInfo {
        user_id: session.user_id,
        relay_url,
        device_id: device.device_id,
        device_name: device.device_name,
    })
}

/// Public wrapper for restoring device routing after session restore at startup.
pub(crate) async fn restore_device_routing(device_name: &str) -> Result<()> {
    let generation = account_context_generation();
    let (_, relay_url) = read_account_context().await?;
    spawn_device_routing(&relay_url, device_name, generation).await
}

/// Connect to the account relay for device-to-device routing and spawn the
/// background task that handles incoming RPC commands.
async fn spawn_device_routing(
    relay_url: &str,
    device_name: &str,
    account_generation: u64,
) -> Result<()> {
    let _sync_guard = lock_account_sync(account_generation).await?;
    let relay_url = normalize_relay_url(relay_url)?;
    // Tear down any previous connection first.
    stop_device_routing().await;

    let (session, current_relay_url) = read_account_context().await?;
    if current_relay_url != relay_url {
        return Err(anyhow!("account context changed"));
    }

    let ws_url = format!(
        "{}/ws",
        relay_url
            .replace("https://", "wss://")
            .replace("http://", "ws://")
    );

    let (client, mut event_rx) = RelayClient::new();
    client.connect(&ws_url).await?;
    client
        .connect_authenticated(&session.token, device_name)
        .await?;
    let client_arc = Arc::new(client);
    {
        let _routing_guard = DEVICE_ROUTING_LIFECYCLE.write().await;
        let mut current_client = device_relay_client().write().await;
        if !account_context_is_current(account_generation) {
            drop(current_client);
            client_arc.disconnect().await;
            return Err(anyhow!("account context changed"));
        }
        *current_client = Some(client_arc.clone());
    }

    let account_context = account_context().clone();
    let relay_client_arc = client_arc.clone();
    tokio::spawn(async move {
        loop {
            if !routing_loop_is_current(account_generation, &relay_client_arc).await {
                tracing::debug!("Stopping stale device routing event loop");
                break;
            }
            let Some(event) = event_rx.recv().await else {
                break;
            };
            if !routing_loop_is_current(account_generation, &relay_client_arc).await {
                tracing::debug!("Stopping stale device routing event loop");
                break;
            }
            handle_relay_event(
                event,
                &account_context,
                &relay_client_arc,
                account_generation,
                &session.token,
            )
            .await;
        }
        retire_routing_client_if_same(&relay_client_arc).await;
        tracing::info!("Device routing event loop exited");
    });

    Ok(())
}

/// Disconnect the device-routing connection (if any).
pub(crate) async fn stop_device_routing() {
    let _routing_guard = DEVICE_ROUTING_LIFECYCLE.write().await;
    stop_device_routing_locked().await;
}

/// Stop routing while the caller holds the lifecycle write lease.
async fn stop_device_routing_locked() {
    let client = { device_relay_client().write().await.take() };
    if let Some(client) = client {
        client.disconnect().await;
    }
    crate::peer_host::update_controller_presence(Vec::new()).await;
}

async fn is_current_routing_client(client: &Arc<RelayClient>) -> bool {
    same_routing_client(device_relay_client().read().await.as_ref(), client)
}

fn same_routing_client<T>(current: Option<&Arc<T>>, expected: &Arc<T>) -> bool {
    current.is_some_and(|client| Arc::ptr_eq(client, expected))
}

fn take_routing_client_if_same<T>(current: &mut Option<Arc<T>>, expected: &Arc<T>) -> bool {
    if !same_routing_client(current.as_ref(), expected) {
        return false;
    }
    current.take();
    true
}

/// Validate both halves of a routing-loop lease. The generation is checked
/// again after awaiting the client slot so a concurrent account transition
/// cannot make the pre-lock snapshot look current.
async fn routing_loop_is_current(account_generation: u64, relay_client: &Arc<RelayClient>) -> bool {
    if !account_context_is_current(account_generation) {
        return false;
    }
    let matches = is_current_routing_client(relay_client).await;
    matches && account_context_is_current(account_generation)
}

/// Retire only the client owned by this loop. Keep the lifecycle write lease
/// while clearing controller presence so a replacement cannot publish its
/// presence and then have it erased by the old loop's cleanup.
async fn retire_routing_client_if_same(relay_client: &Arc<RelayClient>) -> bool {
    let _routing_guard = DEVICE_ROUTING_LIFECYCLE.write().await;
    let mut current = device_relay_client().write().await;
    if !take_routing_client_if_same(&mut current, relay_client) {
        return false;
    }
    drop(current);
    crate::peer_host::update_controller_presence(Vec::new()).await;
    true
}

/// Log out: tear down routing, revoke the token (best-effort), clear state.
pub(crate) async fn logout() -> Result<()> {
    let _sync_guard = invalidate_and_wait_for_account_sync().await;
    stop_device_routing_locked().await;
    // Take the always-on daemon down with the account: the token is revoked
    // below, so leaving the daemon connected would keep this device online
    // with a doomed token until its next reconnect fails.
    if crate::daemon::request_daemon_shutdown() {
        tracing::info!("Signalled the CLI daemon to shut down after logout");
    }
    let result = read_account_context_raw().await;
    if let Ok((session, relay_url)) = result {
        let _ = AccountClient::new()
            .revoke_token(&relay_url, &session)
            .await;
    }
    *account_context().write().await = None;
    PENDING_SYNC_CHOICE.store(false, Ordering::Release);
    session_store::clear_session();
    session_store::clear_credential_hint();
    TOKEN_EXPIRED.store(false, Ordering::Relaxed);
    Ok(())
}

/// Handle a single relay event for the device-routing loop.
async fn handle_relay_event(
    event: RelayEvent,
    account_context: &Arc<RwLock<Option<AccountContextState>>>,
    relay_client: &Arc<RelayClient>,
    account_generation: u64,
    expected_token: &str,
) {
    let event = match event {
        RelayEvent::AuthError { message } => {
            handle_relay_auth_error(
                message,
                account_context,
                relay_client,
                account_generation,
                expected_token,
            )
            .await;
            return;
        }
        event => event,
    };

    let _routing_lease = DEVICE_ROUTING_LIFECYCLE.read().await;
    if !routing_loop_is_current(account_generation, relay_client).await {
        tracing::debug!("Ignoring event from a stale device routing client");
        return;
    }
    let fanout_owner = PeerFanoutOwner {
        account_generation,
        account_token: expected_token.to_string(),
        relay_client: Arc::clone(relay_client),
    };
    ACTIVE_PEER_FANOUT_OWNER
        .scope(fanout_owner, async {
            match event {
                RelayEvent::AuthOk { user_id, device_id } => {
                    tracing::info!("Device routing auth ok: user={user_id} device={device_id}");
                    if let Err(e) = DeviceIdentity::adopt_account_device_id(&device_id) {
                        tracing::warn!("Failed to adopt AuthOk device_id: {e}");
                    } else if let Some(context) = account_context.read().await.clone() {
                        if routing_loop_is_current(account_generation, relay_client).await
                            && context.session.token == expected_token
                        {
                            if let Err(e) = session_store::save_session_with_device(
                                &context.session.token,
                                &context.session.user_id,
                                &context.session.master_key,
                                &context.relay_url,
                                Some(device_id.as_str()),
                            ) {
                                tracing::warn!(
                                    "Failed to persist AuthOk device_id into session: {e}"
                                );
                            }
                        }
                    }
                }
                RelayEvent::AuthError { .. } => {
                    unreachable!("AuthError handled before routing read lease")
                }
                RelayEvent::DevicePresence { devices } => {
                    tracing::info!("Device presence updated: {} online", devices.len());
                    if !routing_loop_is_current(account_generation, relay_client).await {
                        return;
                    }
                    crate::peer_host::update_controller_presence(
                        devices.into_iter().map(|device| device.device_id).collect(),
                    )
                    .await;
                    if !routing_loop_is_current(account_generation, relay_client).await {
                        tracing::debug!("Account changed while applying device presence");
                    }
                }
                RelayEvent::DeviceMessageReceived {
                    source_device_id,
                    correlation_id,
                    encrypted_data,
                    nonce,
                } => {
                    let context = account_context.read().await.clone();
                    if !routing_loop_is_current(account_generation, relay_client).await {
                        return;
                    }
                    let Some(context) = context else {
                        return;
                    };
                    if context.session.token != expected_token {
                        return;
                    }
                    let plaintext = match encryption::decrypt_from_base64(
                        &context.session.master_key,
                        &encrypted_data,
                        &nonce,
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!("Failed to decrypt device message: {e}");
                            return;
                        }
                    };
                    use remote_connect::remote_server::{RemoteCommand, RemoteResponse};
                    let cmd: RemoteCommand = match serde_json::from_str(&plaintext) {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("Could not parse device command: {e}");
                            return;
                        }
                    };
                    tracing::info!(
                        "Device command from {source_device_id}: {cmd:?} corr={correlation_id}"
                    );

                    if !routing_loop_is_current(account_generation, relay_client).await {
                        return;
                    }
                    let response = match &cmd {
                        RemoteCommand::HostInvoke { command, args } => {
                            let response =
                                crate::peer_host::handle_host_invoke(command, args.clone()).await;
                            if !routing_loop_is_current(account_generation, relay_client).await {
                                return;
                            }
                            response
                        }
                        RemoteCommand::DeviceEvent { .. } => {
                            crate::peer_host::handle_device_event_command()
                        }
                        other => {
                            let server = RemoteServer::new(context.session.master_key);
                            let response = server.dispatch(other).await;
                            if !routing_loop_is_current(account_generation, relay_client).await {
                                return;
                            }
                            response
                        }
                    };

                    let resp_json = match serde_json::to_string(&response) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to serialize RPC response: {e}");
                            serde_json::to_string(&RemoteResponse::Error {
                                message: format!("failed to serialize RPC response: {e}"),
                            })
                            .unwrap_or_else(|_| {
                                r#"{"resp":"error","message":"serialize failed"}"#.to_string()
                            })
                        }
                    };

                    match encryption::encrypt_to_base64(&context.session.master_key, &resp_json) {
                        Ok((enc_resp, resp_nonce)) => {
                            // HTTP RPC bridge expects replies targeted at "rpc".
                            let reply_target = if source_device_id == "rpc" {
                                "rpc"
                            } else {
                                source_device_id.as_str()
                            };
                            if !routing_loop_is_current(account_generation, relay_client).await {
                                return;
                            }
                            let send_result = relay_client
                                .send_device_message(
                                    reply_target,
                                    &correlation_id,
                                    &enc_resp,
                                    &resp_nonce,
                                )
                                .await;
                            if !routing_loop_is_current(account_generation, relay_client).await {
                                return;
                            }
                            if let Err(e) = send_result {
                                tracing::warn!("Failed to send RPC response: {e}");
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to encrypt RPC response: {e}");
                        }
                    }
                }
                RelayEvent::Disconnected => {
                    tracing::info!("Device routing disconnected");
                    if !routing_loop_is_current(account_generation, relay_client).await {
                        return;
                    }
                    crate::peer_host::update_controller_presence(Vec::new()).await;
                    if !routing_loop_is_current(account_generation, relay_client).await {
                        tracing::debug!("Account changed while clearing device presence");
                    }
                }
                RelayEvent::Reconnected => {
                    tracing::info!("Device routing reconnected");
                }
                RelayEvent::Error { message } => {
                    tracing::warn!("Device routing error: {message}");
                }
                _ => {}
            }
        })
        .await;
}

/// Auth failure starts an account transition, which owns the lifecycle write
/// lease. It cannot be handled under the ordinary event read lease because
/// upgrading a Tokio `RwLock` would deadlock.
async fn handle_relay_auth_error(
    message: String,
    account_context: &Arc<RwLock<Option<AccountContextState>>>,
    relay_client: &Arc<RelayClient>,
    account_generation: u64,
    expected_token: &str,
) {
    tracing::warn!("Device routing auth error: {message}");
    let Some(_transition_guard) = invalidate_and_wait_if_account_current(account_generation).await
    else {
        tracing::debug!("Ignoring auth error from a stale account generation");
        return;
    };
    if !is_current_routing_client(relay_client).await {
        tracing::debug!("Ignoring auth error from a replaced routing client");
        return;
    }
    let token_matches = account_context
        .read()
        .await
        .as_ref()
        .is_some_and(|context| context.session.token == expected_token);
    if !token_matches || !is_current_routing_client(relay_client).await {
        tracing::debug!("Ignoring auth error from a replaced routing client");
        return;
    }

    // Keep CLI/daemon semantics aligned with Desktop: a relay-rejected token
    // is no longer a usable local login and must not be restored again on the
    // next process start. Preserve the non-secret hint for the re-login form.
    relay_client.disconnect().await;
    if !is_current_routing_client(relay_client).await {
        tracing::debug!("Ignoring auth error cleanup for a replaced routing client");
        return;
    }

    let mut current_client = device_relay_client().write().await;
    if !same_routing_client(current_client.as_ref(), relay_client) {
        tracing::debug!("Ignoring auth error cleanup for a replaced routing client");
        return;
    }
    let mut current_context = account_context.write().await;
    if !current_context
        .as_ref()
        .is_some_and(|context| context.session.token == expected_token)
    {
        tracing::debug!("Ignoring auth error cleanup for a replaced account");
        return;
    }
    take_routing_client_if_same(&mut current_client, relay_client);
    *current_context = None;
    drop(current_context);
    drop(current_client);

    TOKEN_EXPIRED.store(true, Ordering::Relaxed);
    PENDING_SYNC_CHOICE.store(false, Ordering::Release);
    session_store::clear_session();
    crate::peer_host::update_controller_presence(Vec::new()).await;
}

/// Immutable routing owner captured when a Peer DeviceEvent enters the bounded
/// delivery queue. It prevents an event from account A being encrypted or sent
/// through account B after waiting behind older events.
#[derive(Clone)]
pub(crate) struct PeerFanoutOwner {
    account_generation: u64,
    account_token: String,
    relay_client: Arc<RelayClient>,
}

tokio::task_local! {
    static ACTIVE_PEER_FANOUT_OWNER: PeerFanoutOwner;
}

pub(crate) fn inherited_peer_fanout_owner() -> Option<PeerFanoutOwner> {
    ACTIVE_PEER_FANOUT_OWNER
        .try_with(PeerFanoutOwner::clone)
        .ok()
}

impl PeerFanoutOwner {
    fn matches(&self, generation: u64, token: &str, relay_client: &Arc<RelayClient>) -> bool {
        self.account_generation == generation
            && self.account_token == token
            && Arc::ptr_eq(&self.relay_client, relay_client)
    }

    #[cfg(test)]
    pub(crate) fn for_test(account_generation: u64, account_token: &str) -> Self {
        let (relay_client, _) = RelayClient::new();
        Self {
            account_generation,
            account_token: account_token.to_string(),
            relay_client: Arc::new(relay_client),
        }
    }

    #[cfg(test)]
    pub(crate) fn generation_for_test(&self) -> u64 {
        self.account_generation
    }
}

/// Stable fan-out context. The read lease is intentionally retained through
/// encryption and all target sends; account replacement takes the write lease.
pub(crate) struct PeerFanoutLease {
    pub(crate) session: AccountSession,
    pub(crate) relay_client: Arc<RelayClient>,
    _routing_lease: tokio::sync::RwLockReadGuard<'static, ()>,
}

pub(crate) async fn capture_peer_fanout_owner() -> Result<PeerFanoutOwner> {
    let generation = account_context_generation();
    let _routing_lease = DEVICE_ROUTING_LIFECYCLE.read().await;
    let (session, _) = read_account_context_for_generation(generation).await?;
    let client = device_relay_client()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("device routing not connected"))?;
    if !account_context_is_current(generation) || !is_current_routing_client(&client).await {
        return Err(anyhow!("account context changed"));
    }
    Ok(PeerFanoutOwner {
        account_generation: generation,
        account_token: session.token,
        relay_client: client,
    })
}

pub(crate) async fn acquire_peer_fanout_lease(owner: &PeerFanoutOwner) -> Result<PeerFanoutLease> {
    let routing_lease = DEVICE_ROUTING_LIFECYCLE.read().await;
    if !account_context_is_current(owner.account_generation) {
        return Err(anyhow!("queued Peer event account changed"));
    }
    let context = account_context()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("not logged in"))?;
    let client = device_relay_client()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("device routing not connected"))?;
    if !account_context_is_current(owner.account_generation)
        || !owner.matches(
            account_context_generation(),
            &context.session.token,
            &client,
        )
    {
        return Err(anyhow!("queued Peer event routing owner changed"));
    }
    Ok(PeerFanoutLease {
        session: context.session,
        relay_client: client,
        _routing_lease: routing_lease,
    })
}

/// A textual device listing entry for display.
pub(crate) struct AccountDevice {
    pub(crate) device_id: String,
    pub(crate) device_name: String,
    pub(crate) online: bool,
}

/// List all devices in the account.
pub(crate) async fn list_devices() -> Result<Vec<AccountDevice>> {
    let (session, relay_url) = read_account_context().await?;
    let devices = AccountClient::new()
        .list_devices(&relay_url, &session)
        .await?;
    Ok(devices
        .into_iter()
        .map(|d| AccountDevice {
            device_id: d.device_id,
            device_name: d.device_name,
            online: d.online,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        account_context_generation, automatic_account_sync_policy_for_pending,
        begin_candidate_account_transition, clear_replaced_persisted_session,
        inherited_peer_fanout_owner, login_with_credentials, replaced_account_revocation_target,
        resolve_cloud_settings_probe, take_routing_client_if_same, AccountContextState,
        CandidateAccountTransitionError, DaemonRetirementFailure, PeerFanoutOwner,
        ACCOUNT_LOGIN_LOCK, ACTIVE_PEER_FANOUT_OWNER, DEVICE_ROUTING_LIFECYCLE,
    };

    #[test]
    fn stale_routing_loop_cannot_clear_replacement_client() {
        let stale = Arc::new("stale");
        let replacement = Arc::new("replacement");
        let mut current = Some(Arc::clone(&replacement));

        assert!(!take_routing_client_if_same(&mut current, &stale));
        assert!(current
            .as_ref()
            .is_some_and(|client| Arc::ptr_eq(client, &replacement)));
    }

    #[test]
    fn routing_loop_can_clear_only_its_own_client() {
        let owned = Arc::new("owned");
        let mut current = Some(Arc::clone(&owned));

        assert!(take_routing_client_if_same(&mut current, &owned));
        assert!(current.is_none());
    }

    #[tokio::test]
    async fn routing_replacement_waits_for_in_flight_event_lease() {
        let event_lease = DEVICE_ROUTING_LIFECYCLE.read().await;
        let (attempting_tx, attempting_rx) = tokio::sync::oneshot::channel();
        let replacement = tokio::spawn(async move {
            let _ = attempting_tx.send(());
            let _replacement_lease = DEVICE_ROUTING_LIFECYCLE.write().await;
        });

        attempting_rx.await.expect("replacement task started");
        tokio::task::yield_now().await;
        assert!(!replacement.is_finished());

        drop(event_lease);
        tokio::time::timeout(Duration::from_secs(1), replacement)
            .await
            .expect("replacement should acquire the lifecycle after event completion")
            .expect("replacement task should finish");
    }

    #[tokio::test]
    async fn inherited_fanout_owner_does_not_reacquire_routing_read_lease() {
        let event_lease = DEVICE_ROUTING_LIFECYCLE.read().await;
        let (attempting_tx, attempting_rx) = tokio::sync::oneshot::channel();
        let replacement = tokio::spawn(async move {
            let _ = attempting_tx.send(());
            let _replacement_lease = DEVICE_ROUTING_LIFECYCLE.write().await;
        });

        attempting_rx.await.expect("replacement task started");
        tokio::task::yield_now().await;
        assert!(!replacement.is_finished());

        let owner = PeerFanoutOwner::for_test(21, "token-a");
        let inherited = tokio::time::timeout(
            Duration::from_millis(100),
            ACTIVE_PEER_FANOUT_OWNER.scope(owner, async { inherited_peer_fanout_owner() }),
        )
        .await
        .expect("inherited owner lookup must not wait behind the queued writer")
        .expect("task-local owner should be visible");
        assert_eq!(inherited.generation_for_test(), 21);
        assert!(!replacement.is_finished());

        drop(event_lease);
        tokio::time::timeout(Duration::from_secs(1), replacement)
            .await
            .expect("replacement should proceed after the outer event lease is released")
            .expect("replacement task should finish");
    }

    #[tokio::test]
    async fn invalid_login_does_not_invalidate_the_current_account() {
        let generation = account_context_generation();

        let error = login_with_credentials("", "user", "password")
            .await
            .expect_err("empty relay URL must be rejected");

        assert!(error.to_string().contains("Auth Server is required"));
        assert_eq!(account_context_generation(), generation);
    }

    #[test]
    fn cloud_settings_probe_errors_are_not_treated_as_missing_settings() {
        assert!(!resolve_cloud_settings_probe(Ok(None)).expect("missing settings is valid"));
        assert!(
            resolve_cloud_settings_probe(Ok(Some("encrypted settings".to_string())))
                .expect("existing settings is valid")
        );

        let error = resolve_cloud_settings_probe(Err(anyhow::anyhow!("relay unavailable")))
            .expect_err("probe failure must reject the candidate login");
        assert!(error.to_string().contains("could not check cloud settings"));
        assert!(error.to_string().contains("relay unavailable"));
    }

    #[test]
    fn pending_sync_choice_blocks_automatic_pull_and_push_until_finalized() {
        let pending = automatic_account_sync_policy_for_pending(true);
        assert!(!pending.background_engine);
        assert!(!pending.management_push);

        let finalized = automatic_account_sync_policy_for_pending(false);
        assert!(finalized.background_engine);
        assert!(finalized.management_push);
    }

    #[tokio::test]
    async fn daemon_retirement_failure_does_not_begin_account_transition() {
        let generation = account_context_generation();
        let result = begin_candidate_account_transition(
            generation,
            Err(DaemonRetirementFailure {
                error: anyhow::anyhow!("daemon stayed alive"),
                daemon_may_exit: true,
            }),
        )
        .await;

        assert!(matches!(
            result,
            Err(CandidateAccountTransitionError::DaemonRetirement(_))
        ));
        assert_eq!(account_context_generation(), generation);
    }

    #[test]
    fn pending_replacement_cannot_restore_the_previous_persisted_account() {
        let directory = std::env::temp_dir().join(format!(
            "bitfun-cli-account-session-{}",
            uuid::Uuid::new_v4()
        ));
        bitfun_core::service::remote_connect::session_store::set_session_store_directory_for_test(
            directory,
        );
        let old_master_key = [7_u8; 32];
        bitfun_core::service::remote_connect::session_store::save_session_with_device(
            "account-a-token",
            "account-a",
            &old_master_key,
            "https://relay-a.example",
            Some("device-a"),
        )
        .expect("persist account A");
        assert_eq!(
            bitfun_core::service::remote_connect::session_store::load_session_detailed()
                .expect("load account A")
                .expect("account A should be persisted")
                .token,
            "account-a-token"
        );

        // This is the disk step used after candidate B wins the transition and
        // before B is exposed as awaiting its cloud/local sync choice.
        clear_replaced_persisted_session();

        assert!(
            bitfun_core::service::remote_connect::session_store::load_session_detailed()
                .expect("load after candidate B becomes pending")
                .is_none()
        );
    }

    #[test]
    fn replacement_revokes_only_the_previous_distinct_token() {
        let previous = AccountContextState {
            session: bitfun_core::service::remote_connect::AccountSession {
                token: "old-token".to_string(),
                user_id: "same-account".to_string(),
                master_key: [3_u8; 32],
            },
            relay_url: "https://relay.example".to_string(),
        };

        let target = replaced_account_revocation_target(
            Some(previous.clone()),
            "https://relay.example",
            "new-token",
        )
        .expect("a new token for the same account must retire the old bearer");
        assert_eq!(target.session.token, "old-token");
        assert_eq!(target.session.user_id, "same-account");
        assert_eq!(target.relay_url, "https://relay.example");

        assert!(replaced_account_revocation_target(
            Some(previous.clone()),
            "https://relay.example",
            "old-token"
        )
        .is_none());
        assert!(replaced_account_revocation_target(
            Some(previous),
            "https://other-relay.example",
            "old-token"
        )
        .is_some());
        assert!(
            replaced_account_revocation_target(None, "https://relay.example", "new-token")
                .is_none()
        );
    }

    #[tokio::test]
    async fn candidate_login_attempts_are_serialized() {
        let first_candidate = ACCOUNT_LOGIN_LOCK.lock().await;
        let (attempting_tx, attempting_rx) = tokio::sync::oneshot::channel();
        let second_candidate = tokio::spawn(async move {
            let _ = attempting_tx.send(());
            let _guard = ACCOUNT_LOGIN_LOCK.lock().await;
        });

        attempting_rx.await.expect("second candidate started");
        tokio::task::yield_now().await;
        assert!(!second_candidate.is_finished());

        drop(first_candidate);
        tokio::time::timeout(Duration::from_secs(1), second_candidate)
            .await
            .expect("second candidate should proceed after the first")
            .expect("second candidate task should finish");
    }

    #[test]
    fn queued_fanout_owner_requires_generation_token_and_client_identity() {
        let owner = PeerFanoutOwner::for_test(11, "token-a");
        let owned_client = Arc::clone(&owner.relay_client);
        let replacement = PeerFanoutOwner::for_test(12, "token-b");

        assert!(owner.matches(11, "token-a", &owned_client));
        assert!(!owner.matches(12, "token-a", &owned_client));
        assert!(!owner.matches(11, "token-b", &owned_client));
        assert!(!owner.matches(11, "token-a", &replacement.relay_client));
    }
}
