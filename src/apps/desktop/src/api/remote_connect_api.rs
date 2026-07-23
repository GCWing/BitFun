//! Tauri commands for Remote Connect.

use crate::api::session_storage_path::desktop_effective_session_storage_path;
use crate::embedded_relay_host::DesktopEmbeddedRelayHost;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::agentic::tools::account_login_capability::set_account_login_available;
use bitfun_core::agentic::tools::page_deploy_host::set_page_deploy_handler;
use bitfun_core::agentic::tools::page_publish_host::set_page_publish_handler;
use bitfun_core::service::remote_connect::session_store::{
    clear_credential_hint, load_credential_hint, save_credential_hint, AccountHint,
};
use bitfun_core::service::remote_connect::{
    bot::{self, weixin, BotConfig},
    lan, session_store, sync_state, AccountClient, AccountPairingVerification, AccountSession,
    ConnectionMethod, ConnectionResult, DelegatedIdentityAuthorization, DeviceIdentity,
    PairingState, RemoteConnectConfig, RemoteConnectService,
};
use bitfun_core::service::session::{DialogTurnData, SessionMetadata};
use bitfun_services_integrations::remote_connect::account::{
    error_indicates_expired_token, validate_relay_base_url,
};
use bitfun_services_integrations::remote_connect::{
    deploy_page_version_on_relay, join_relay_url, list_pages_from_relay,
    publish_page_content_on_relay,
};
use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{Notify, RwLock};

static REMOTE_CONNECT_SERVICE: OnceLock<Arc<RwLock<Option<RemoteConnectService>>>> =
    OnceLock::new();

/// Session and relay URL must move together. Keeping them behind one lock
/// prevents a request from observing a token from one login and the URL from
/// another while an account transition is in progress.
#[derive(Clone)]
struct AccountContextState {
    session: AccountSession,
    relay_url: String,
}

static ACCOUNT_CONTEXT: OnceLock<Arc<RwLock<Option<AccountContextState>>>> = OnceLock::new();

/// Serializes explicit login-time syncs and lets logout/new login invalidate
/// the active operation before account state is changed.
static ACCOUNT_AUTO_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Serializes credential verification attempts without hiding or disconnecting
/// the currently active account. A successful candidate acquires the account
/// transition guard only after all login-time network requests complete.
static ACCOUNT_LOGIN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ACCOUNT_CONTEXT_TRANSITION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ACCOUNT_AUTO_SYNC_CANCEL: OnceLock<Notify> = OnceLock::new();
static ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID: AtomicU64 = AtomicU64::new(0);
static ACCOUNT_CONTEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static ACCOUNT_CONTEXT_TRANSITIONS: AtomicUsize = AtomicUsize::new(0);

/// Device-routing effects take a read lease; connection replacement and
/// teardown take the write lease. Together with `DeviceRoutingOwner`, this
/// prevents a retiring event loop from dispatching through a newer socket.
static DEVICE_ROUTING_LIFECYCLE_LOCK: tokio::sync::RwLock<()> = tokio::sync::RwLock::const_new(());
static DEVICE_ROUTING_CONNECTION_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeviceRoutingOwner {
    account_generation: u64,
    account_token: String,
    connection_id: u64,
    service_connection_id: u64,
}

#[derive(Default)]
struct DeviceRoutingState {
    owner: Option<DeviceRoutingOwner>,
    online_devices: Vec<OnlineDeviceInfo>,
}

static DEVICE_ROUTING_STATE: OnceLock<std::sync::Mutex<DeviceRoutingState>> = OnceLock::new();

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
        ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID.store(0, Ordering::Release);
        clear_last_finalized_pending_login();
        account_auto_sync_cancel().notify_waiters();
        Self
    }
}

impl Drop for AccountContextTransitionPermit {
    fn drop(&mut self) {
        // Invalidate work queued during the transition before making the newly
        // installed (or cleared) account context discoverable.
        ACCOUNT_CONTEXT_GENERATION.fetch_add(1, Ordering::AcqRel);
        ACCOUNT_CONTEXT_TRANSITIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

struct AccountContextTransitionGuard {
    sync_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
    transition: Option<AccountContextTransitionPermit>,
    transition_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
}

impl AccountContextTransitionGuard {
    /// Make the committed context observable while retaining the transition
    /// mutex. Login-state listeners can now probe `account_status`, while a
    /// competing logout or replacement remains blocked until publication ends.
    fn make_context_observable(&mut self) {
        drop(self.sync_guard.take());
        drop(self.transition.take());
    }
}

struct PendingLoginFinalizeGuard {
    sync_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
    transition_guard: Option<tokio::sync::MutexGuard<'static, ()>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FinalizedPendingLoginOwner {
    pending_login_id: String,
    account_generation: u64,
    account_token: String,
}

impl Drop for PendingLoginFinalizeGuard {
    fn drop(&mut self) {
        drop(self.sync_guard.take());
        drop(self.transition_guard.take());
    }
}

impl Drop for AccountContextTransitionGuard {
    fn drop(&mut self) {
        // Release the operation lock before reopening context discovery. A
        // queued operation that wins this handoff still fails on the gate.
        drop(self.sync_guard.take());
        drop(self.transition.take());
        drop(self.transition_guard.take());
    }
}

async fn lock_account_sync(
    generation: u64,
) -> Result<tokio::sync::MutexGuard<'static, ()>, String> {
    let guard = ACCOUNT_AUTO_SYNC_LOCK.lock().await;
    if !account_context_is_current(generation) {
        return Err("account sync cancelled".to_string());
    }
    Ok(guard)
}

fn ensure_account_auto_sync_current(operation_id: u64) -> Result<(), String> {
    if operation_id != 0
        && ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID.load(Ordering::Acquire) == operation_id
    {
        Ok(())
    } else {
        Err("account sync cancelled".to_string())
    }
}

fn account_auto_sync_cancel() -> &'static Notify {
    ACCOUNT_AUTO_SYNC_CANCEL.get_or_init(Notify::new)
}

async fn await_account_auto_sync<F, T>(operation_id: u64, future: F) -> Result<T, String>
where
    F: Future<Output = T>,
{
    let mut cancelled = Box::pin(account_auto_sync_cancel().notified());
    cancelled.as_mut().enable();
    ensure_account_auto_sync_current(operation_id)?;
    tokio::select! {
        _ = &mut cancelled => Err("account sync cancelled".to_string()),
        result = future => {
            ensure_account_auto_sync_current(operation_id)?;
            Ok(result)
        }
    }
}

async fn cancel_and_wait_for_account_auto_sync() -> AccountContextTransitionGuard {
    // Serialize transition creation so a stale invalidation can re-check its
    // generation before it makes the current account undiscoverable.
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    let transition = AccountContextTransitionPermit::begin();
    let sync_guard = ACCOUNT_AUTO_SYNC_LOCK.lock().await;
    bitfun_core::service::remote_connect::settings_sync::wait_for_sync_operations_idle().await;
    AccountContextTransitionGuard {
        sync_guard: Some(sync_guard),
        transition: Some(transition),
        transition_guard: Some(transition_guard),
    }
}

async fn cancel_and_wait_if_account_current(
    expected_generation: u64,
) -> Option<AccountContextTransitionGuard> {
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    if !account_context_is_current(expected_generation) {
        return None;
    }
    let transition = AccountContextTransitionPermit::begin();
    let sync_guard = ACCOUNT_AUTO_SYNC_LOCK.lock().await;
    bitfun_core::service::remote_connect::settings_sync::wait_for_sync_operations_idle().await;
    Some(AccountContextTransitionGuard {
        sync_guard: Some(sync_guard),
        transition: Some(transition),
        transition_guard: Some(transition_guard),
    })
}

fn pending_login_is_owned_by(expected_pending_login_id: &str) -> bool {
    if expected_pending_login_id.is_empty()
        || !PENDING_SYNC_CHOICE.load(std::sync::atomic::Ordering::Acquire)
    {
        return false;
    }
    PENDING_LOGIN_ID
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_deref()
        == Some(expected_pending_login_id)
}

fn set_pending_login_id(pending_login_id: Option<String>) {
    let mut current = PENDING_LOGIN_ID
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *current = pending_login_id;
    PENDING_SYNC_CHOICE.store(current.is_some(), std::sync::atomic::Ordering::Release);
}

fn background_account_sync_is_allowed() -> bool {
    !PENDING_SYNC_CHOICE.load(std::sync::atomic::Ordering::Acquire)
}

fn clear_last_finalized_pending_login() {
    *LAST_FINALIZED_PENDING_LOGIN
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn record_finalized_pending_login(owner: FinalizedPendingLoginOwner) {
    *LAST_FINALIZED_PENDING_LOGIN
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(owner);
}

async fn finalized_pending_login_is_current(pending_login_id: &str) -> bool {
    let owner = LAST_FINALIZED_PENDING_LOGIN
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let Some(owner) = owner else {
        return false;
    };
    owner.pending_login_id == pending_login_id
        && account_context_matches(owner.account_generation, &owner.account_token).await
}

async fn lock_pending_login_for_finalize(
    expected_pending_login_id: &str,
) -> Result<PendingLoginFinalizeGuard, String> {
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    let generation = account_context_generation();
    if !account_context_is_current(generation)
        || !pending_login_is_owned_by(expected_pending_login_id)
    {
        return Err("pending login changed".to_string());
    }
    let sync_guard = ACCOUNT_AUTO_SYNC_LOCK.lock().await;
    if !account_context_is_current(generation)
        || !pending_login_is_owned_by(expected_pending_login_id)
    {
        return Err("pending login changed".to_string());
    }
    Ok(PendingLoginFinalizeGuard {
        sync_guard: Some(sync_guard),
        transition_guard: Some(transition_guard),
    })
}

/// Global handle to the DialogScheduler, set during app startup. Used by the
/// device-routing background task to execute commands received from peer
/// devices (ExecuteOnDevice).
static DIALOG_SCHEDULER: OnceLock<Arc<bitfun_core::agentic::coordination::DialogScheduler>> =
    OnceLock::new();

/// Set the global scheduler handle. Called once during app startup.
pub fn set_dialog_scheduler(scheduler: Arc<bitfun_core::agentic::coordination::DialogScheduler>) {
    let _ = DIALOG_SCHEDULER.set(scheduler);
}

/// AppHandle used by the device-routing background task to emit UI events
/// (presence / settings-applied) without requiring a Tauri command context.
static ACCOUNT_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Store the app handle for account device-routing events. Called once during setup.
pub fn set_account_app_handle(app: AppHandle) {
    let _ = ACCOUNT_APP_HANDLE.set(app);
}

/// Shared AppHandle for account / peer-host bridges.
pub fn account_app_handle() -> Option<&'static AppHandle> {
    ACCOUNT_APP_HANDLE.get()
}

/// Current device id for peer capability probes.
pub fn current_device_id_for_peer() -> Result<String, String> {
    Ok(current_device_identity()?.device_id)
}

fn emit_account_event(event: &str, payload: serde_json::Value) {
    if let Some(app) = ACCOUNT_APP_HANDLE.get() {
        if let Err(e) = app.emit(event, payload) {
            log::warn!("Failed to emit {event}: {e}");
        }
    }
}

fn emit_device_presence(devices: &[(String, String)]) {
    let payload = serde_json::json!({
        "devices": devices
            .iter()
            .map(|(id, name)| serde_json::json!({
                "device_id": id,
                "device_name": name,
            }))
            .collect::<Vec<_>>(),
    });
    emit_account_event("account://device-presence", payload);
}

fn emit_settings_applied() {
    emit_account_event(
        "account://settings-applied",
        serde_json::json!({ "applied": true }),
    );
}

async fn disconnect_peer_controllers(reason: &'static str) {
    let request_ids = crate::api::peer_host_invoke::disconnect_controllers();
    if let Err(error) =
        crate::api::peer_host_invoke::fail_closed_permission_requests(request_ids, reason).await
    {
        log::warn!("Peer permission requests were not fully cancelled: {error}");
    }
    emit_device_presence(&[]);
}

/// Retire the active routing owner before touching the shared service. The
/// lifecycle write lease ensures no retiring event handler can cross this
/// boundary and dispatch through a subsequently installed connection.
async fn stop_and_clear_device_routing(reason: &'static str) {
    let _lifecycle = DEVICE_ROUTING_LIFECYCLE_LOCK.write().await;
    clear_device_routing_state();
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service.stop_device_connection().await;
    }
    disconnect_peer_controllers(reason).await;
}

async fn finish_device_routing_event_loop(owner: &DeviceRoutingOwner) {
    let _lifecycle = DEVICE_ROUTING_LIFECYCLE_LOCK.write().await;
    if !clear_device_routing_if_owner(owner) {
        return;
    }
    disconnect_peer_controllers("Peer device-routing stream closed").await;
}

/// Emit granular auto-sync progress for the account login / devices UI.
fn emit_sync_progress(
    operation_id: u64,
    phase: &str,
    percent: u8,
    current: Option<usize>,
    total: Option<usize>,
    detail: Option<&str>,
) {
    emit_account_event(
        "account://sync-progress",
        serde_json::json!({
            "operation_id": operation_id,
            "phase": phase,
            "percent": percent.min(100),
            "current": current,
            "total": total,
            "detail": detail,
        }),
    );
}

/// Push a UI event to all attached peer controllers.
///
/// Events are queued and sent **sequentially** so high-frequency streams
/// (especially `agentic://text-chunk`) keep emission order. Concurrent
/// `tokio::spawn` per chunk previously scrambled peer remote chat text.
pub fn fanout_peer_device_event(event: String, payload: serde_json::Value) {
    if crate::api::peer_host_invoke::attached_controllers().is_empty() {
        return;
    }
    let Some(routing_owner) = current_device_routing_owner_snapshot() else {
        return;
    };
    let tx = PEER_EVENT_FANOUT_TX.get_or_init(|| {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PeerEventFanoutItem>();
        tokio::spawn(async move {
            while let Some(item) = rx.recv().await {
                fanout_peer_device_event_once(item).await;
            }
        });
        tx
    });
    if let Err(e) = tx.send(PeerEventFanoutItem {
        routing_owner,
        event,
        payload,
    }) {
        log::debug!("peer event fanout queue closed: {e}");
    }
}

struct PeerEventFanoutItem {
    routing_owner: DeviceRoutingOwner,
    event: String,
    payload: serde_json::Value,
}

static PEER_EVENT_FANOUT_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<PeerEventFanoutItem>> =
    OnceLock::new();

async fn fanout_peer_device_event_once(item: PeerEventFanoutItem) {
    let Some(_routing_effect) = lock_current_device_routing(&item.routing_owner).await else {
        return;
    };
    let targets = crate::api::peer_host_invoke::attached_controllers();
    if targets.is_empty() {
        return;
    }
    let (session, _) =
        match read_account_context_for_generation(item.routing_owner.account_generation).await {
            Ok(ctx) => ctx,
            Err(e) => {
                log::debug!("peer event fanout skipped (no account): {e}");
                return;
            }
        };
    if session.token != item.routing_owner.account_token
        || !device_routing_owner_is_current(&item.routing_owner).await
    {
        return;
    }
    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
    let envelope = match serde_json::to_string(&RemoteCommand::DeviceEvent {
        event: item.event.clone(),
        payload: item.payload,
    }) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("peer event fanout serialize failed: {e}");
            return;
        }
    };
    let (encrypted_data, nonce) = match encrypt_to_base64(&session.master_key, &envelope) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("peer event fanout encrypt failed: {e}");
            return;
        }
    };
    for target in targets {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = send_device_message_with_routing_lease(
            &item.routing_owner,
            &target,
            &correlation_id,
            &encrypted_data,
            &nonce,
        )
        .await
        {
            log::debug!("peer event fanout to {target} failed: {e}");
        }
    }
}

fn should_fanout_peer_ui_event(event: &str) -> bool {
    matches!(
        event,
        "terminal_event"
            | "file-system-changed"
            | "lsp-event"
            | "backend-event-mcpinteractionrequest"
            | "backend-event-acppermissionrequest"
            | "backend-event-toolexecutionprogress"
            | "backend-event-toolterminalready"
            | "backend-event-backgroundcommandlifecycle"
            | "backend-event-toolexecutionstarted"
            | "backend-event-toolexecutioncompleted"
            | "backend-event-toolexecutionerror"
            | "backend-event-toolawaitinguserinput"
            | "backend-event-toolcallconfirmation"
            | "permission://event"
    )
}

/// Fan-out non-agentic UI events to attached Peer Mode controllers when needed.
pub fn maybe_fanout_peer_ui_event(event: &str, payload: serde_json::Value) {
    if !should_fanout_peer_ui_event(event) {
        return;
    }
    if crate::api::peer_host_invoke::attached_controllers().is_empty() {
        return;
    }
    fanout_peer_device_event(event.to_string(), payload);
}

/// EventEmitter wrapper that mirrors selected UI events to Peer Mode controllers.
pub struct PeerAwareEmitter {
    inner: Arc<dyn bitfun_core::infrastructure::events::EventEmitter>,
}

impl PeerAwareEmitter {
    pub fn new(inner: Arc<dyn bitfun_core::infrastructure::events::EventEmitter>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl bitfun_core::infrastructure::events::EventEmitter for PeerAwareEmitter {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) -> anyhow::Result<()> {
        self.inner.emit(event_name, payload.clone()).await?;
        maybe_fanout_peer_ui_event(event_name, payload);
        Ok(())
    }
}

pub fn wrap_peer_aware_emitter(
    inner: Arc<dyn bitfun_core::infrastructure::events::EventEmitter>,
) -> Arc<dyn bitfun_core::infrastructure::events::EventEmitter> {
    Arc::new(PeerAwareEmitter::new(inner))
}

async fn send_device_message_with_routing_lease(
    owner: &DeviceRoutingOwner,
    target_device_id: &str,
    correlation_id: &str,
    encrypted_data: &str,
    nonce: &str,
) -> Result<(), String> {
    if !device_routing_owner_is_current(owner).await {
        return Err("device routing changed".to_string());
    }
    let holder = get_service_holder().read().await;
    if !device_routing_owner_is_current(owner).await {
        return Err("device routing changed".to_string());
    }
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;
    let sent = service
        .send_device_message_if_connection(
            owner.service_connection_id,
            target_device_id,
            correlation_id,
            encrypted_data,
            nonce,
        )
        .await
        .map_err(|error| error.to_string())?;
    if !sent || !device_routing_owner_is_current(owner).await {
        return Err("device routing changed".to_string());
    }
    Ok(())
}

/// Encrypt and send an RPC response (or error) back to the HTTP caller via relay.
async fn send_rpc_envelope(
    owner: &DeviceRoutingOwner,
    session: &AccountSession,
    correlation_id: &str,
    resp_value: serde_json::Value,
) {
    if !device_routing_owner_is_current(owner).await {
        return;
    }
    let resp_json = match serde_json::to_string(&resp_value) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("RPC: serialize response failed: {e}");
            serde_json::json!({
                "resp": "error",
                "message": format!("failed to serialize RPC response: {e}"),
            })
            .to_string()
        }
    };
    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    match encrypt_to_base64(&session.master_key, &resp_json) {
        Ok((enc_resp, resp_nonce)) => {
            if let Err(e) = send_device_message_with_routing_lease(
                owner,
                "rpc",
                correlation_id,
                &enc_resp,
                &resp_nonce,
            )
            .await
            {
                log::warn!("RPC: send response failed: {e}");
            }
        }
        Err(e) => {
            log::warn!("RPC: encrypt response failed: {e}");
        }
    }
}

async fn send_rpc_error(
    owner: &DeviceRoutingOwner,
    session: &AccountSession,
    correlation_id: &str,
    message: impl Into<String>,
) {
    send_rpc_envelope(
        owner,
        session,
        correlation_id,
        serde_json::json!({
            "resp": "error",
            "message": message.into(),
        }),
    )
    .await;
}

/// Global flag set when the relay returns HTTP 401 (token expired). The
/// frontend checks this via `account_token_expired` and prompts re-login.
static TOKEN_EXPIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Check if the account token has been marked expired by the sync loop.
#[tauri::command]
pub async fn account_token_expired() -> bool {
    TOKEN_EXPIRED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Internal helper: check if an error message indicates HTTP 401.
fn is_token_expired_error(e: &anyhow::Error) -> bool {
    error_indicates_expired_token(&e.to_string())
}

/// Drop the local account session after the relay rejects the token, but only
/// if the response still belongs to the same account generation and token.
/// Keeps the username/relay hint so the login form can be prefilled.
async fn invalidate_local_account_session_if_current(
    expected_generation: u64,
    expected_token: &str,
    reason: &str,
) -> bool {
    let Some(_transition_guard) = cancel_and_wait_if_account_current(expected_generation).await
    else {
        log::info!("Ignored auth failure from a stale account generation");
        return false;
    };
    let token_matches = get_account_context()
        .read()
        .await
        .as_ref()
        .is_some_and(|context| context.session.token == expected_token);
    if !token_matches {
        log::info!("Ignored auth failure from a replaced account token");
        return false;
    }
    sync_account_login_capability(false);
    TOKEN_EXPIRED.store(true, std::sync::atomic::Ordering::Relaxed);
    stop_and_clear_device_routing("Account session expired").await;
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service.clear_account_pairing_context().await;
        service.clear_trusted_mobile_identity().await;
        service.clear_bot_delegated_identities().await;
    }
    *get_account_context().write().await = None;
    set_pending_login_id(None);
    session_store::clear_session();
    emit_account_event(
        "account://login-state",
        serde_json::json!({ "logged_in": false, "reason": "session_expired" }),
    );
    log::warn!("Invalidated local account session after relay auth failure: {reason}");
    true
}

fn sync_account_login_capability(logged_in: bool) {
    set_account_login_available(logged_in);
}

fn register_page_deploy_host() {
    set_page_deploy_handler(Arc::new(|slug, version_id| {
        Box::pin(async move {
            let generation = account_context_generation();
            let (session, relay_url) = read_account_context_for_generation(generation).await?;
            let page = list_pages_from_relay(&relay_url, &session.token)
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|page| page.slug == slug)
                .ok_or_else(|| {
                    "Page not found; refresh the Page list before deploying".to_string()
                })?;
            let info = deploy_page_version_on_relay(
                &relay_url,
                &session.token,
                &slug,
                &version_id,
                &page.generation,
            )
            .await
            .map_err(|e| e.to_string())?;
            if !account_context_is_current(generation) {
                return Err("account context changed".to_string());
            }
            let mut value = serde_json::to_value(info).map_err(|e| e.to_string())?;
            if let Some(obj) = value.as_object_mut() {
                let path = obj
                    .get("url_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let preview_path = obj
                    .get("preview_url_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                obj.insert(
                    "url".into(),
                    serde_json::Value::String(join_relay_url(&relay_url, &path)),
                );
                if !preview_path.is_empty() {
                    obj.insert(
                        "preview_url".into(),
                        serde_json::Value::String(join_relay_url(&relay_url, &preview_path)),
                    );
                }
            }
            Ok(value)
        })
    }));
}

fn register_page_publish_host() {
    set_page_publish_handler(Arc::new(|request| {
        Box::pin(async move {
            let generation = account_context_generation();
            let (session, relay_url) = read_account_context_for_generation(generation).await?;
            let result = publish_page_content_on_relay(
                &relay_url,
                &session.token,
                &request.slug,
                &request.visibility,
                request.title.as_deref(),
                request.note.as_deref(),
                request.deploy,
                request.directory.as_deref(),
                request.files.as_ref(),
            )
            .await
            .map_err(|e| e.to_string())?;
            if !account_context_is_current(generation) {
                return Err("account context changed".to_string());
            }
            serde_json::to_value(result).map_err(|e| e.to_string())
        })
    }));
}

fn get_account_context() -> &'static Arc<RwLock<Option<AccountContextState>>> {
    ACCOUNT_CONTEXT.get_or_init(|| Arc::new(RwLock::new(None)))
}

/// Read both the session and relay URL, returning owned clones to avoid
/// holding locks across awaits.
pub(crate) async fn read_account_context() -> Result<(AccountSession, String), String> {
    let generation = account_context_generation();
    read_account_context_for_generation(generation).await
}

async fn read_account_context_raw() -> Result<(AccountSession, String), String> {
    get_account_context()
        .read()
        .await
        .clone()
        .map(|context| (context.session, context.relay_url))
        .ok_or_else(|| "not logged in".to_string())
}

pub(crate) async fn read_account_context_for_generation(
    generation: u64,
) -> Result<(AccountSession, String), String> {
    if !account_context_is_current(generation) {
        return Err("account context changed".to_string());
    }
    let context = read_account_context_raw().await?;
    if !account_context_is_current(generation) {
        return Err("account context changed".to_string());
    }
    Ok(context)
}

async fn account_context_matches(generation: u64, token: &str) -> bool {
    if !account_context_is_current(generation) {
        return false;
    }
    let token_matches = get_account_context()
        .read()
        .await
        .as_ref()
        .is_some_and(|context| context.session.token == token);
    token_matches && account_context_is_current(generation)
}

fn get_device_routing_state() -> &'static std::sync::Mutex<DeviceRoutingState> {
    DEVICE_ROUTING_STATE.get_or_init(|| std::sync::Mutex::new(DeviceRoutingState::default()))
}

fn with_device_routing_state<T>(update: impl FnOnce(&mut DeviceRoutingState) -> T) -> T {
    let mut state = get_device_routing_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    update(&mut state)
}

fn new_device_routing_owner(
    account_generation: u64,
    account_token: &str,
    service_connection_id: u64,
) -> DeviceRoutingOwner {
    DeviceRoutingOwner {
        account_generation,
        account_token: account_token.to_string(),
        connection_id: DEVICE_ROUTING_CONNECTION_ID.fetch_add(1, Ordering::AcqRel) + 1,
        service_connection_id,
    }
}

fn install_device_routing_owner(owner: DeviceRoutingOwner) {
    with_device_routing_state(|state| {
        state.owner = Some(owner);
        state.online_devices.clear();
    });
}

fn device_routing_owner_is_registered(owner: &DeviceRoutingOwner) -> bool {
    with_device_routing_state(|state| state.owner.as_ref() == Some(owner))
}

fn device_routing_owner_for_account(
    account_generation: u64,
    account_token: &str,
) -> Option<DeviceRoutingOwner> {
    with_device_routing_state(|state| {
        let owner = state.owner.as_ref()?;
        if owner.account_generation != account_generation || owner.account_token != account_token {
            return None;
        }
        Some(owner.clone())
    })
}

fn current_device_routing_owner_snapshot() -> Option<DeviceRoutingOwner> {
    with_device_routing_state(|state| state.owner.clone()).filter(|owner| {
        account_context_is_current(owner.account_generation)
            && device_routing_owner_is_registered(owner)
    })
}

async fn device_routing_owner_is_current(owner: &DeviceRoutingOwner) -> bool {
    device_routing_owner_is_registered(owner)
        && account_context_matches(owner.account_generation, &owner.account_token).await
        && device_routing_owner_is_registered(owner)
}

async fn lock_current_device_routing(
    owner: &DeviceRoutingOwner,
) -> Option<tokio::sync::RwLockReadGuard<'static, ()>> {
    let guard = DEVICE_ROUTING_LIFECYCLE_LOCK.read().await;
    if device_routing_owner_is_current(owner).await {
        Some(guard)
    } else {
        None
    }
}

fn replace_device_presence_if_owner(
    owner: &DeviceRoutingOwner,
    devices: Vec<OnlineDeviceInfo>,
) -> bool {
    with_device_routing_state(|state| {
        if state.owner.as_ref() != Some(owner) {
            return false;
        }
        state.online_devices = devices;
        true
    })
}

fn device_presence_for_account(
    account_generation: u64,
    account_token: &str,
) -> Option<Vec<OnlineDeviceInfo>> {
    with_device_routing_state(|state| {
        let owner = state.owner.as_ref()?;
        if owner.account_generation != account_generation || owner.account_token != account_token {
            return None;
        }
        Some(state.online_devices.clone())
    })
}

fn clear_device_routing_if_owner(owner: &DeviceRoutingOwner) -> bool {
    with_device_routing_state(|state| {
        if state.owner.as_ref() != Some(owner) {
            return false;
        }
        state.owner = None;
        state.online_devices.clear();
        true
    })
}

fn clear_device_routing_state() -> bool {
    with_device_routing_state(|state| {
        let had_owner = state.owner.take().is_some();
        state.online_devices.clear();
        had_owner
    })
}

fn normalize_relay_url(relay_url: &str) -> Result<String, String> {
    let parsed = validate_relay_base_url(relay_url.trim()).map_err(|error| error.to_string())?;
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn cloud_settings_exist_from_probe<T, E>(result: Result<Option<T>, E>) -> Result<bool, E> {
    result.map(|settings| settings.is_some())
}

async fn revoke_login_candidate(
    client: &AccountClient,
    relay_url: &str,
    session: &AccountSession,
    reason: &str,
) {
    if let Err(error) = client.revoke_token(relay_url, session).await {
        log::warn!("Failed to revoke rejected login candidate after {reason}: {error}");
    }
}

fn select_replaced_account_for_revocation(
    previous: Option<AccountContextState>,
    replacement_relay_url: &str,
    replacement_token: &str,
) -> Option<AccountContextState> {
    previous.filter(|account| {
        account.relay_url != replacement_relay_url || account.session.token != replacement_token
    })
}

async fn revoke_replaced_account(client: &AccountClient, account: Option<AccountContextState>) {
    let Some(account) = account else {
        return;
    };
    if let Err(error) = client
        .revoke_token(&account.relay_url, &account.session)
        .await
    {
        // The replacement is already fully published. Revocation is
        // best-effort and must never roll the new account back.
        log::warn!("Failed to revoke replaced account token: {error}");
    }
}

// ── Credential persistence (non-secret: username + relay_url only) ──────
// Owned by shared `session_store` so Desktop and CLI use the same hint file.

/// Tauri command: get the persisted credential hint for pre-filling the login form.
#[tauri::command]
pub async fn account_get_credential_hint() -> Option<AccountHint> {
    load_credential_hint()
}

/// Tauri resource directory path for mobile-web, set during app setup.
static MOBILE_WEB_RESOURCE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn get_service_holder() -> &'static Arc<RwLock<Option<RemoteConnectService>>> {
    REMOTE_CONNECT_SERVICE.get_or_init(|| Arc::new(RwLock::new(None)))
}

/// Called from Tauri setup to register the resolved resource directory path
/// for the bundled mobile-web files.
pub fn set_mobile_web_resource_path(path: PathBuf) {
    log::info!("Registered mobile-web resource path: {}", path.display());
    let _ = MOBILE_WEB_RESOURCE_PATH.set(path);
}

/// Called from Tauri setup to eagerly initialize the remote connect service
/// and restore any previously paired bot connections.  Without this, bots
/// only start listening after the user first opens the Remote Connect dialog.
/// Register delegated identity providers for mobile-web (room channel) and
/// IM bots (global provider). Called after session is restored (startup) or
/// after fresh login.
async fn register_delegated_identity_providers() {
    // Room-channel provider for mobile-web.
    let account_context = get_account_context().clone();
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service
            .set_delegated_identity_provider(move || {
                let account_context = account_context.clone();
                Box::pin(async move {
                    let generation = account_context_generation();
                    if !account_context_is_current(generation) {
                        return None;
                    }
                    // Core calls this provider while holding the room lifecycle
                    // lease. Acquire the account lease second and return it with
                    // the credentials so account replacement cannot begin until
                    // Core has encrypted and sent the response.
                    let account_lease = lock_account_sync(generation).await.ok()?;
                    let context = account_context.read().await.clone()?;
                    if !account_context_matches(generation, &context.session.token).await {
                        return None;
                    }
                    match AccountClient::new()
                        .delegate_token(&context.relay_url, &context.session)
                        .await
                    {
                        Ok(delegated) => {
                            if delegated.user_id != context.session.user_id {
                                log::warn!(
                                    "Delegated identity user did not match the desktop account"
                                );
                                return None;
                            }
                            if !account_context_matches(generation, &context.session.token).await {
                                return None;
                            }
                            Some(DelegatedIdentityAuthorization::with_host_lease(
                                delegated.token,
                                delegated.user_id,
                                context.session.master_key,
                                account_lease,
                            ))
                        }
                        Err(e) => {
                            log::warn!("Delegate token failed: {e}");
                            None
                        }
                    }
                })
            })
            .await;

        // Account-mode mobile pairing: QR prefill + password verification.
        register_account_pairing_context(service).await;

        // Login/restore may switch accounts; drop any prior URL-bound mobile
        // identity so the next pair can bind to the current account user id.
        service.clear_trusted_mobile_identity().await;
        service.clear_bot_delegated_identities().await;
    }

    // Global provider for IM bots.
    let account_context = get_account_context().clone();
    bitfun_core::service::remote_connect::bot::set_delegated_identity_provider(move || {
        let account_context = account_context.clone();
        Box::pin(async move {
            let generation = account_context_generation();
            if !account_context_is_current(generation) {
                return None;
            }
            let context = account_context.read().await.clone()?;
            if !account_context_is_current(generation) {
                return None;
            }
            match AccountClient::new()
                .delegate_token(&context.relay_url, &context.session)
                .await
            {
                Ok(delegated) if account_context_is_current(generation) => Some((
                    context.relay_url,
                    delegated.token,
                    context.session.master_key.to_vec(),
                )),
                Ok(_) => None,
                Err(e) => {
                    log::warn!("Bot delegate token failed: {e}");
                    None
                }
            }
        })
    });
}

/// Wire QR account prefill + verify-only password check for mobile pairing.
async fn register_account_pairing_context(service: &RemoteConnectService) {
    // Always enable account mode when logged in; username prefill is best-effort.
    let username = load_credential_hint()
        .map(|hint| hint.username)
        .unwrap_or_default();
    service.set_account_pairing_username(Some(username)).await;

    let account_context = get_account_context().clone();
    let pairing_attempts = Arc::new(tokio::sync::Mutex::new((0_u32, None::<std::time::Instant>)));
    service
        .set_account_pairing_verifier(move |username, password| {
            let account_context = account_context.clone();
            let pairing_attempts = pairing_attempts.clone();
            async move {
                let generation = account_context_generation();
                if !account_context_is_current(generation) {
                    return Err("Desktop account is changing; scan again".to_string());
                }
                {
                    let mut attempts = pairing_attempts.lock().await;
                    if let Some(locked_until) = attempts.1 {
                        if locked_until > std::time::Instant::now() {
                            return Err(
                                "Too many pairing attempts. Wait one minute and scan again."
                                    .to_string(),
                            );
                        }
                        *attempts = (0, None);
                    }
                }
                let context = account_context
                    .read()
                    .await
                    .clone()
                    .ok_or_else(|| "Desktop is not logged into a BitFun account".to_string())?;
                if !account_context_is_current(generation) {
                    return Err("Desktop account is changing; scan again".to_string());
                }
                let account_lease = lock_account_sync(generation)
                    .await
                    .map_err(|_| "Desktop account is changing; scan again".to_string())?;
                if !account_context_matches(generation, &context.session.token).await {
                    return Err("Desktop account is changing; scan again".to_string());
                }
                let verification = AccountClient::new()
                    .verify_password_for_master_key(
                        &context.relay_url,
                        &username,
                        &password,
                        &context.session.master_key,
                    )
                    .await;
                if !account_context_matches(generation, &context.session.token).await {
                    return Err("Desktop account changed; scan again".to_string());
                }
                if let Err(error) = verification {
                    // Keep the real cause in desktop logs (network vs bad
                    // credentials); the mobile only gets the unified message.
                    log::warn!("Account pairing verification failed: {error}");
                    let mut attempts = pairing_attempts.lock().await;
                    attempts.0 = attempts.0.saturating_add(1);
                    if attempts.0 >= 5 {
                        attempts.1 =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
                        return Err("Too many pairing attempts. Wait one minute and scan again."
                            .to_string());
                    }
                    return Err("Invalid username or password".to_string());
                }
                *pairing_attempts.lock().await = (0, None);
                Ok(AccountPairingVerification::with_host_lease(
                    context.session.user_id,
                    account_lease,
                ))
            }
        })
        .await;
}

pub fn init_on_startup() {
    register_page_deploy_host();
    register_page_publish_host();
    tokio::spawn(async {
        let startup_generation = account_context_generation();
        // Restore persisted account session (if any) before anything else
        // so that auto-sync, device routing, and bot delegation work on restart.
        match session_store::load_session_detailed() {
            Ok(Some(loaded)) => {
                let user_id = loaded.user_id.clone();
                let relay_url = match normalize_relay_url(&loaded.relay_url) {
                    Ok(url) => url,
                    Err(error) => {
                        log::warn!("Ignoring invalid persisted relay URL: {error}");
                        session_store::clear_session();
                        sync_account_login_capability(false);
                        if let Err(error) = ensure_service().await {
                            log::warn!("Remote connect startup init failed: {error}");
                        }
                        return;
                    }
                };
                let Some(restore_guard) =
                    cancel_and_wait_if_account_current(startup_generation).await
                else {
                    log::info!(
                        "Skipped persisted session restore after a newer account transition"
                    );
                    if let Err(error) = ensure_service().await {
                        log::warn!("Remote connect startup init failed: {error}");
                    }
                    return;
                };
                if let Some(device_id) = loaded.device_id.as_deref() {
                    if let Err(e) = DeviceIdentity::adopt_account_device_id(device_id) {
                        log::warn!("Failed to adopt restored session device_id: {e}");
                    }
                }
                let session = AccountSession {
                    token: loaded.token,
                    user_id: user_id.clone(),
                    master_key: loaded.master_key,
                };
                *get_account_context().write().await = Some(AccountContextState {
                    session,
                    relay_url: relay_url.clone(),
                });
                sync_account_login_capability(true);
                // Keep the mirrored "Self-Hosted" server field in sync for
                // sessions restored from an older version without the mirror.
                set_self_hosted_form_url(Some(&relay_url));
                log::info!("Restored account session for user {user_id}");
                drop(restore_guard);

                // Initialize the remote-connect service if not yet ready.
                if let Err(e) = ensure_service().await {
                    log::warn!("Remote connect startup init failed: {e}");
                }

                // Re-register delegated identity providers for mobile-web / bots.
                register_delegated_identity_providers().await;

                // Best-effort: restore bot connections now that delegated
                // identity is available again.
                restore_saved_bots().await;

                // Re-establish device routing WebSocket in the background.
                // Uses the same Tauri command logic — fire and forget.
                tokio::spawn(async {
                    if let Err(e) = account_connect_devices().await {
                        log::warn!("Startup device connect failed: {e}");
                    }
                    log::info!("Device routing restored on startup");
                });
            }
            Ok(None) => {
                sync_account_login_capability(false);
                // No persisted session — normal for first-time users.
                if let Err(e) = ensure_service().await {
                    log::warn!("Remote connect startup init failed: {e}");
                }
            }
            Err(e) => {
                sync_account_login_capability(false);
                log::warn!("Failed to load persisted session: {e}");
                if let Err(e) = ensure_service().await {
                    log::warn!("Remote connect startup init failed: {e}");
                }
            }
        }
    });
}

/// Synchronous cleanup called when the application exits.
pub fn cleanup_on_exit() {
    bitfun_core::service::remote_connect::ngrok::cleanup_all_ngrok();
    log::info!("Remote connect cleanup completed on exit");
}

async fn ensure_service() -> Result<(), String> {
    let holder = get_service_holder();
    let guard = holder.read().await;
    if guard.is_some() {
        return Ok(());
    }
    drop(guard);

    let config = RemoteConnectConfig {
        mobile_web_dir: detect_mobile_web_dir(),
        ..RemoteConnectConfig::default()
    };
    let service =
        new_remote_connect_service(config).map_err(|e| format!("init remote connect: {e}"))?;
    *holder.write().await = Some(service);

    // Auto-restore previously paired bots
    restore_saved_bots().await;

    Ok(())
}

fn new_remote_connect_service(config: RemoteConnectConfig) -> anyhow::Result<RemoteConnectService> {
    RemoteConnectService::new(config, Arc::new(DesktopEmbeddedRelayHost::default()))
}

/// Restore any bot connections that were previously saved to disk.
async fn restore_saved_bots() {
    use bitfun_core::service::remote_connect::bot;

    let data = bot::load_bot_persistence();
    if data.connections.is_empty() {
        return;
    }

    let holder = get_service_holder();
    let guard = holder.read().await;
    let Some(service) = guard.as_ref() else {
        return;
    };

    for conn in &data.connections {
        if !conn.chat_state.paired {
            continue;
        }
        log::info!(
            "Restoring {} bot connection for chat_id={}",
            conn.bot_type,
            conn.chat_id
        );
        let result = service.restore_bot(conn).await;
        if let Err(e) = result {
            log::warn!("Failed to restore {} bot: {e}", conn.bot_type);
        }
    }
}

/// Auto-detect the mobile-web build output directory.
fn detect_mobile_web_dir() -> Option<String> {
    if let Ok(dir) = std::env::var("BITFUN_MOBILE_WEB_DIR") {
        let p = std::path::Path::new(&dir);
        if p.join("index.html").exists() {
            log::info!("Using BITFUN_MOBILE_WEB_DIR: {dir}");
            return Some(dir);
        }
        log::warn!("BITFUN_MOBILE_WEB_DIR set but index.html not found: {dir}");
    }

    if let Some(resource_path) = MOBILE_WEB_RESOURCE_PATH.get() {
        if is_valid_mobile_web_dir(resource_path) {
            let dir = resource_path.to_string_lossy().into_owned();
            log::info!("Using Tauri bundled mobile-web: {dir}");
            return Some(dir);
        }
        log::debug!(
            "Tauri resource path registered but not a valid mobile-web dir: {}",
            resource_path.display()
        );
    }

    if let Some(dir) = detect_from_exe() {
        return Some(dir);
    }

    if let Some(dir) = detect_from_cwd() {
        return Some(dir);
    }

    log::warn!("mobile-web dist directory not found; LAN/Ngrok modes will not serve static files");
    None
}

fn detect_from_exe() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let mut candidates: Vec<PathBuf> = Vec::new();

    if cfg!(target_os = "macos") {
        // Primary: tauri.conf.json maps dist -> mobile-web/dist in Resources
        candidates.push(exe_dir.join("../Resources/mobile-web/dist"));
        // Fallback: legacy layout without dist subdirectory
        candidates.push(exe_dir.join("../Resources/mobile-web"));
        // Fallback: array-format bundling may place files at Resources/dist directly
        candidates.push(exe_dir.join("../Resources/dist"));
    }
    candidates.push(exe_dir.join("mobile-web/dist"));
    candidates.push(exe_dir.join("mobile-web"));
    candidates.push(exe_dir.join("resources/mobile-web/dist"));
    candidates.push(exe_dir.join("resources/mobile-web"));

    if cfg!(target_os = "linux") {
        candidates.push(exe_dir.join("../lib/bitfun/mobile-web/dist"));
        candidates.push(exe_dir.join("../lib/bitfun/mobile-web"));
        candidates.push(exe_dir.join("../share/bitfun/mobile-web/dist"));
        candidates.push(exe_dir.join("../share/bitfun/mobile-web"));
        candidates.push(exe_dir.join("../share/com.bitfun.desktop/mobile-web/dist"));
        candidates.push(exe_dir.join("../share/com.bitfun.desktop/mobile-web"));
    }

    check_candidates(&candidates, "exe-relative")
}

fn detect_from_cwd() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join("src/mobile-web/dist"),
        cwd.join("../../mobile-web/dist"),
        cwd.join("../mobile-web/dist"),
    ];

    check_candidates(&candidates, "cwd-relative")
}

fn check_candidates(candidates: &[PathBuf], source: &str) -> Option<String> {
    for candidate in candidates {
        if is_valid_mobile_web_dir(candidate) {
            if let Ok(abs) = candidate.canonicalize() {
                log::info!("Detected mobile-web dir ({}): {}", source, abs.display());
                return Some(abs.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn is_valid_mobile_web_dir(dir: &std::path::Path) -> bool {
    dir.join("index.html").exists() && dir.join("assets").is_dir()
}

// ── Request / Response DTOs ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct StartRemoteConnectRequest {
    pub method: String,
    pub custom_server_url: Option<String>,
    pub lan_ip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RemoteConnectStatusResponse {
    pub is_connected: bool,
    pub pairing_state: PairingState,
    pub active_method: Option<String>,
    pub peer_device_name: Option<String>,
    pub peer_user_id: Option<String>,
    /// Independent bot connection info — e.g. "Telegram(7096812005)".
    /// Present when a bot is active, regardless of relay pairing state.
    pub bot_connected: Option<String>,
    /// Bot verbose mode setting — when true, intermediate progress is sent to users.
    pub bot_verbose_mode: bool,
}

#[derive(Debug, Serialize)]
pub struct ConnectionMethodInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub mac_address: String,
}

#[derive(Debug, Serialize)]
pub struct LanNetworkInterface {
    pub interface_name: String,
    pub ip: String,
    pub gateway_ip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LanNetworkInfo {
    pub local_ip: String,
    pub gateway_ip: Option<String>,
    pub available_ips: Vec<LanNetworkInterface>,
}

fn detect_default_gateway_ip() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = bitfun_core::util::process_manager::create_command("route")
            .args(["-n", "get", "default"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"(?m)^\s*gateway:\s*([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\s*$").ok()?;
        return re
            .captures(&stdout)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    }

    #[cfg(target_os = "linux")]
    {
        let output = bitfun_core::util::process_manager::create_command("ip")
            .args(["route", "show", "default"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"(?m)^default\s+via\s+([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\b").ok()?;
        return re
            .captures(&stdout)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    }

    #[cfg(target_os = "windows")]
    {
        let output = bitfun_core::util::process_manager::create_command("route")
            .args(["print", "-4"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let re =
            Regex::new(r"(?m)^\s*0\.0\.0\.0\s+0\.0\.0\.0\s+([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\s+")
                .ok()?;
        return re
            .captures(&stdout)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
    }

    #[allow(unreachable_code)]
    None
}

/// Detect per-interface gateway IPs by parsing the system routing table.
///
/// Returns a map keyed by interface identifier (interface name on macOS/Linux,
/// interface IP on Windows) → gateway IP.  Only interfaces that have a default
/// route entry appear in the map.
fn detect_interface_gateways() -> HashMap<String, String> {
    let mut map = HashMap::new();

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = bitfun_core::util::process_manager::create_command("netstat")
            .args(["-rn", "-f", "inet"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Lines look like:
                //   default            192.168.1.1       UGScg    en0
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 && parts[0] == "default" {
                        let gateway = parts[1];
                        let netif = parts[3];
                        if is_ipv4(gateway) {
                            map.insert(netif.to_string(), gateway.to_string());
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = bitfun_core::util::process_manager::create_command("ip")
            .args(["route", "show", "default"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Lines look like:
                //   default via 192.168.1.1 dev eth0 proto dhcp metric 100
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let mut via = None;
                    let mut dev = None;
                    for i in 0..parts.len() {
                        match parts[i] {
                            "via" if i + 1 < parts.len() => via = Some(parts[i + 1]),
                            "dev" if i + 1 < parts.len() => dev = Some(parts[i + 1]),
                            _ => {}
                        }
                    }
                    if let (Some(gw), Some(iface)) = (via, dev) {
                        if is_ipv4(gw) {
                            map.insert(iface.to_string(), gw.to_string());
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = bitfun_core::util::process_manager::create_command("route")
            .args(["print", "-4"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Lines look like:
                //   0.0.0.0  0.0.0.0  192.168.1.1  192.168.1.2  25
                // Column 3 = gateway, column 4 = interface IP
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4
                        && parts[0] == "0.0.0.0"
                        && parts[1] == "0.0.0.0"
                        && is_ipv4(parts[2])
                        && is_ipv4(parts[3])
                    {
                        // Key by interface IP so it can be matched later
                        map.insert(parts[3].to_string(), parts[2].to_string());
                    }
                }
            }
        }
    }

    map
}

/// Quick check whether a string looks like an IPv4 address.
fn is_ipv4(s: &str) -> bool {
    s.split('.').count() == 4 && s.split('.').all(|p| p.parse::<u8>().is_ok())
}

#[tauri::command]
pub async fn remote_connect_get_device_info() -> Result<DeviceInfo, String> {
    ensure_service().await?;
    // Always read the persisted/account-adopted identity — not a stale copy
    // captured when RemoteConnectService was first constructed.
    let id = DeviceIdentity::from_current_machine().map_err(|e| format!("detect device: {e}"))?;
    Ok(DeviceInfo {
        device_id: id.device_id,
        device_name: id.device_name,
        mac_address: id.mac_address,
    })
}

#[tauri::command]
pub async fn remote_connect_get_lan_ip() -> Result<String, String> {
    lan::get_local_ip().map_err(|e| format!("get local ip: {e}"))
}

#[tauri::command]
pub async fn remote_connect_get_lan_network_info() -> Result<LanNetworkInfo, String> {
    let interfaces = lan::list_local_ips().map_err(|e| format!("list local ips: {e}"))?;
    let local_ip = interfaces
        .first()
        .map(|e| e.ip.clone())
        .ok_or_else(|| "no local IPv4 addresses found".to_string())?;
    let gateway_ip = detect_default_gateway_ip();
    // Build per-interface gateway map once from the routing table.
    let gateway_map = detect_interface_gateways();
    let available_ips = interfaces
        .into_iter()
        .map(|e| {
            // Look up by interface name (macOS/Linux) or by IP (Windows).
            let gw = gateway_map
                .get(&e.interface_name)
                .or_else(|| gateway_map.get(&e.ip))
                .cloned();
            LanNetworkInterface {
                gateway_ip: gw,
                interface_name: e.interface_name,
                ip: e.ip,
            }
        })
        .collect();
    Ok(LanNetworkInfo {
        local_ip,
        gateway_ip,
        available_ips,
    })
}

#[tauri::command]
pub async fn remote_connect_get_methods() -> Result<Vec<ConnectionMethodInfo>, String> {
    ensure_service().await?;
    let holder = get_service_holder();
    let guard = holder.read().await;
    let service = guard.as_ref().ok_or("service not initialized")?;
    let methods = service.available_methods().await;

    let infos = methods
        .into_iter()
        .map(|m| match m {
            ConnectionMethod::Lan { .. } => ConnectionMethodInfo {
                id: "lan".into(),
                name: "LAN".into(),
                available: true,
                description: "Same local network".into(),
            },
            ConnectionMethod::Ngrok => ConnectionMethodInfo {
                id: "ngrok".into(),
                name: "ngrok".into(),
                available: true,
                description: "Internet via ngrok tunnel".into(),
            },
            ConnectionMethod::BitfunServer => ConnectionMethodInfo {
                id: "bitfun_server".into(),
                name: "BitFun Server".into(),
                available: true,
                description: "Official BitFun relay".into(),
            },
            ConnectionMethod::CustomServer { url } => ConnectionMethodInfo {
                id: "custom_server".into(),
                name: "Custom Server".into(),
                available: true,
                description: format!("Self-hosted: {url}"),
            },
            ConnectionMethod::BotFeishu => ConnectionMethodInfo {
                id: "bot_feishu".into(),
                name: "Feishu Bot".into(),
                available: true,
                description: "Via Feishu messenger".into(),
            },
            ConnectionMethod::BotTelegram => ConnectionMethodInfo {
                id: "bot_telegram".into(),
                name: "Telegram Bot".into(),
                available: true,
                description: "Via Telegram".into(),
            },
            ConnectionMethod::BotWeixin => ConnectionMethodInfo {
                id: "bot_weixin".into(),
                name: "WeChat (Weixin)".into(),
                available: true,
                description: "Via WeChat iLink bot".into(),
            },
        })
        .collect();

    Ok(infos)
}

fn parse_connection_method(
    method: &str,
    custom_url: Option<String>,
    lan_ip: Option<String>,
) -> Result<ConnectionMethod, String> {
    match method {
        "lan" => Ok(ConnectionMethod::Lan {
            ip: lan_ip.filter(|s| !s.is_empty()),
        }),
        "ngrok" => Ok(ConnectionMethod::Ngrok),
        "bitfun_server" => Ok(ConnectionMethod::BitfunServer),
        "custom_server" => Ok(ConnectionMethod::CustomServer {
            url: custom_url.unwrap_or_default(),
        }),
        "bot_feishu" => Ok(ConnectionMethod::BotFeishu),
        "bot_telegram" => Ok(ConnectionMethod::BotTelegram),
        "bot_weixin" => Ok(ConnectionMethod::BotWeixin),
        _ => Err(format!("unknown connection method: {method}")),
    }
}

#[tauri::command]
pub async fn remote_connect_start(
    request: StartRemoteConnectRequest,
) -> Result<ConnectionResult, String> {
    ensure_service().await?;
    let method =
        parse_connection_method(&request.method, request.custom_server_url, request.lan_ip)?;

    let holder = get_service_holder();
    let guard = holder.read().await;
    let service = guard.as_ref().ok_or("service not initialized")?;
    // Refresh account pairing context so a newly logged-in session is reflected
    // in the QR (`auth=account&user=...`) before the room is created.
    if read_account_context().await.is_ok() {
        register_account_pairing_context(service).await;
    } else {
        service.clear_account_pairing_context().await;
    }
    service
        .start(method)
        .await
        .map_err(|e| format!("start remote connect: {e}"))
}

#[tauri::command]
pub async fn remote_connect_stop() -> Result<(), String> {
    let holder = get_service_holder();
    let guard = holder.read().await;
    if let Some(service) = guard.as_ref() {
        service.stop_relay().await;
    }
    Ok(())
}

#[tauri::command]
pub async fn remote_connect_stop_bot() -> Result<(), String> {
    let holder = get_service_holder();
    let guard = holder.read().await;
    if let Some(service) = guard.as_ref() {
        service.stop_bots().await;
    }
    // Remove persistence so the bot is not auto-restored
    bot::update_bot_persistence(|data| data.connections.clear());
    Ok(())
}

#[tauri::command]
pub async fn remote_connect_status() -> Result<RemoteConnectStatusResponse, String> {
    ensure_service().await?;
    let holder = get_service_holder();
    let guard = holder.read().await;
    let service = guard.as_ref().ok_or("service not initialized")?;

    let state = service.pairing_state().await;
    let method = service.active_method().await;
    let peer = service.peer_device_name().await;
    let peer_user_id = service.trusted_mobile_user_id().await;
    let bot_connected = service.bot_connected_info().await;
    let bot_verbose_mode = bot::load_bot_persistence().verbose_mode;

    Ok(RemoteConnectStatusResponse {
        is_connected: state == PairingState::Connected,
        pairing_state: state,
        active_method: method.map(|m| format!("{m:?}")),
        peer_device_name: peer,
        peer_user_id,
        bot_connected,
        bot_verbose_mode,
    })
}

#[tauri::command]
pub async fn remote_connect_get_form_state() -> Result<bot::RemoteConnectFormState, String> {
    Ok(bot::load_bot_persistence().form_state)
}

#[tauri::command]
pub async fn remote_connect_set_form_state(
    request: bot::RemoteConnectFormState,
) -> Result<(), String> {
    bot::update_bot_persistence(|data| data.form_state = request);
    Ok(())
}

#[tauri::command]
pub async fn remote_connect_configure_custom_server(url: String) -> Result<(), String> {
    let holder = get_service_holder();
    let mut guard = holder.write().await;
    if guard.is_none() {
        let config = RemoteConnectConfig {
            custom_server_url: Some(url),
            ..RemoteConnectConfig::default()
        };
        let service = new_remote_connect_service(config).map_err(|e| format!("init: {e}"))?;
        *guard = Some(service);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ConfigureBotRequest {
    pub bot_type: String,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub bot_token: Option<String>,
    pub weixin_ilink_token: Option<String>,
    pub weixin_base_url: Option<String>,
    pub weixin_bot_account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WeixinQrStartRequest {
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WeixinQrPollRequest {
    pub session_key: String,
    pub base_url: Option<String>,
}

#[tauri::command]
pub async fn remote_connect_configure_bot(request: ConfigureBotRequest) -> Result<(), String> {
    let holder = get_service_holder();
    let mut guard = holder.write().await;

    let bot_config = match request.bot_type.as_str() {
        "feishu" => BotConfig::Feishu {
            app_id: request.app_id.unwrap_or_default(),
            app_secret: request.app_secret.unwrap_or_default(),
        },
        "telegram" => BotConfig::Telegram {
            bot_token: request.bot_token.unwrap_or_default(),
        },
        "weixin" => BotConfig::Weixin {
            ilink_token: request.weixin_ilink_token.unwrap_or_default(),
            base_url: request.weixin_base_url.unwrap_or_default(),
            bot_account_id: request.weixin_bot_account_id.unwrap_or_default(),
        },
        _ => return Err(format!("unknown bot type: {}", request.bot_type)),
    };

    if guard.is_none() {
        let config = match bot_config {
            BotConfig::Feishu { .. } => RemoteConnectConfig {
                mobile_web_dir: detect_mobile_web_dir(),
                bot_feishu: Some(bot_config),
                ..RemoteConnectConfig::default()
            },
            BotConfig::Telegram { .. } => RemoteConnectConfig {
                mobile_web_dir: detect_mobile_web_dir(),
                bot_telegram: Some(bot_config),
                ..RemoteConnectConfig::default()
            },
            BotConfig::Weixin { .. } => RemoteConnectConfig {
                mobile_web_dir: detect_mobile_web_dir(),
                bot_weixin: Some(bot_config),
                ..RemoteConnectConfig::default()
            },
        };
        let service = new_remote_connect_service(config).map_err(|e| format!("init: {e}"))?;
        *guard = Some(service);
    } else if let Some(service) = guard.as_mut() {
        service.update_bot_config(bot_config);
    }

    Ok(())
}

#[tauri::command]
pub async fn remote_connect_weixin_qr_start(
    request: WeixinQrStartRequest,
) -> Result<weixin::WeixinQrStartResponse, String> {
    weixin::weixin_qr_start(request.base_url)
        .await
        .map_err(|e| format!("weixin qr start: {e}"))
}

#[tauri::command]
pub async fn remote_connect_weixin_qr_poll(
    request: WeixinQrPollRequest,
) -> Result<weixin::WeixinQrPollResponse, String> {
    weixin::weixin_qr_poll(&request.session_key, request.base_url)
        .await
        .map_err(|e| format!("weixin qr poll: {e}"))
}

#[tauri::command]
pub async fn remote_connect_get_bot_verbose_mode() -> Result<bool, String> {
    let data = bot::load_bot_persistence();
    Ok(data.verbose_mode)
}

#[tauri::command]
pub async fn remote_connect_set_bot_verbose_mode(verbose: bool) -> Result<(), String> {
    log::info!(
        "remote_connect_set_bot_verbose_mode called with verbose={}",
        verbose
    );
    bot::update_bot_persistence(|data| data.verbose_mode = verbose);
    log::info!("Saved bot verbose_mode={} to persistence", verbose);
    Ok(())
}

// ── Account commands ────────────────────────────────────────────────────

/// Result returned to the frontend after a successful register/login.
/// The master key is deliberately NOT included — it stays in Rust memory.
#[derive(Serialize, Deserialize, Clone)]
pub struct AccountLoginResult {
    pub user_id: String,
    /// Opaque owner for the pending cloud/local decision. This is never an
    /// account bearer token and is present only when a choice is required.
    pub pending_login_id: Option<String>,
    /// Whether the relay already has a cloud settings blob for this account.
    /// `true` = non-first login → the frontend should prompt the user before
    /// overwriting local settings. `false` = first login → auto-upload local.
    pub has_cloud_settings: bool,
}

/// Current account login status (no secrets exposed).
#[derive(Serialize, Deserialize)]
pub struct AccountStatus {
    pub logged_in: bool,
    pub user_id: Option<String>,
}

/// Request payload for register/login (matches the frontend `request` wrapper).
#[derive(Deserialize)]
pub struct AccountAuthRequest {
    pub relay_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct PendingAccountLoginRequest {
    pub pending_login_id: String,
}

fn current_device_identity() -> Result<DeviceIdentity, String> {
    DeviceIdentity::from_current_machine().map_err(|e| format!("detect device: {e}"))
}

/// True while credentials succeeded but the user has not yet chosen
/// cloud-vs-local settings. Session is held in memory only; a process kill
/// must not restore a logged-in state.
static PENDING_SYNC_CHOICE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static PENDING_LOGIN_ID: OnceLock<std::sync::Mutex<Option<String>>> = OnceLock::new();
static LAST_FINALIZED_PENDING_LOGIN: OnceLock<
    std::sync::Mutex<Option<FinalizedPendingLoginOwner>>,
> = OnceLock::new();

/// Persist the in-memory account session so restart restores login.
async fn persist_account_session(device_id: Option<&str>) -> Result<(), String> {
    // Login installs the context while its transition permit is still held;
    // this internal persistence step intentionally reads that staged value.
    let (session, relay_url) = read_account_context_raw().await?;
    session_store::save_session_with_device(
        &session.token,
        &session.user_id,
        &session.master_key,
        &relay_url,
        device_id,
    )
    .map_err(|e| format!("persist session: {e}"))
}

/// Finish a login that was waiting on the cloud/local settings choice:
/// persist session, register providers, and emit the logged-in event.
///
/// Pair with `PENDING_SYNC_CHOICE` / `account_login`: never persist or emit
/// logged-in before this runs when `has_cloud_settings` was true. Closing the
/// overwrite UI must conditionally cancel its opaque pending owner instead of
/// leaving a memory-only session.
#[tauri::command]
pub async fn account_finalize_login(request: PendingAccountLoginRequest) -> Result<(), String> {
    if finalized_pending_login_is_current(&request.pending_login_id).await {
        return Ok(());
    }
    let _pending_guard = match lock_pending_login_for_finalize(&request.pending_login_id).await {
        Ok(guard) => guard,
        Err(error) => {
            // A concurrent/retried call may arrive after the first invocation
            // committed but before its transport response reached the UI.
            if finalized_pending_login_is_current(&request.pending_login_id).await {
                return Ok(());
            }
            return Err(error);
        }
    };
    let account_generation = account_context_generation();
    let (session, _) = read_account_context_for_generation(account_generation).await?;
    let finalized_owner = FinalizedPendingLoginOwner {
        pending_login_id: request.pending_login_id,
        account_generation,
        account_token: session.token,
    };
    let device = current_device_identity()?;
    persist_account_session(Some(device.device_id.as_str())).await?;
    set_pending_login_id(None);
    TOKEN_EXPIRED.store(false, std::sync::atomic::Ordering::Relaxed);
    sync_account_login_capability(true);

    register_delegated_identity_providers().await;

    let relay_url = read_account_context()
        .await
        .ok()
        .map(|(_, relay_url)| relay_url);
    emit_account_event(
        "account://login-state",
        serde_json::json!({
            "logged_in": true,
            "relay_url": relay_url,
        }),
    );
    record_finalized_pending_login(finalized_owner);
    log::info!("Account login finalized (sync choice accepted)");
    Ok(())
}

/// Abandon only the pending login identified by `pending_login_id`. A stale
/// component cleanup is a no-op and, importantly, does not begin an account
/// transition or increment the context generation.
#[tauri::command]
pub async fn account_cancel_pending_login(
    request: PendingAccountLoginRequest,
) -> Result<bool, String> {
    let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
    let generation = account_context_generation();
    if !account_context_is_current(generation)
        || !pending_login_is_owned_by(&request.pending_login_id)
    {
        return Ok(false);
    }

    let transition = AccountContextTransitionPermit::begin();
    let sync_guard = ACCOUNT_AUTO_SYNC_LOCK.lock().await;
    bitfun_core::service::remote_connect::settings_sync::wait_for_sync_operations_idle().await;
    let _transition_guard = AccountContextTransitionGuard {
        sync_guard: Some(sync_guard),
        transition: Some(transition),
        transition_guard: Some(transition_guard),
    };
    if !pending_login_is_owned_by(&request.pending_login_id) {
        return Ok(false);
    }
    clear_account_login_state(true).await;
    log::info!("Pending account login cancelled");
    Ok(true)
}

#[tauri::command]
pub async fn account_login(request: AccountAuthRequest) -> Result<AccountLoginResult, String> {
    // Keep the old account fully usable while credentials are verified. Only a
    // successful candidate is allowed to begin the protected replacement
    // transition and retire the old account's runtime state.
    let _login_guard = ACCOUNT_LOGIN_LOCK.lock().await;
    let expected_generation = account_context_generation();
    if !account_context_is_current(expected_generation) {
        return Err("account context changed".to_string());
    }
    let relay_url = normalize_relay_url(&request.relay_url)?;
    let device = current_device_identity()?;
    let client = AccountClient::new();
    let session = client
        .login(&relay_url, &request.username, &request.password, &device)
        .await
        .map_err(|e| format!("{e}"))?;

    // Check whether the relay already has a cloud settings blob for this
    // account.  This tells the frontend whether to prompt before overwriting.
    let has_cloud_settings =
        match cloud_settings_exist_from_probe(client.fetch_settings(&relay_url, &session).await) {
            Ok(has_cloud_settings) => has_cloud_settings,
            Err(error) => {
                let message = error.to_string();
                revoke_login_candidate(&client, &relay_url, &session, "settings probe failure")
                    .await;
                return Err(message);
            }
        };

    let Some(mut transition_guard) = cancel_and_wait_if_account_current(expected_generation).await
    else {
        revoke_login_candidate(&client, &relay_url, &session, "account replacement race").await;
        return Err("account context changed".to_string());
    };
    // The old account is now hidden, so account-backed tools must be hidden as
    // well. A committed no-cloud login re-enables them after publication.
    sync_account_login_capability(false);
    let replaced_account = select_replaced_account_for_revocation(
        get_account_context().read().await.clone(),
        &relay_url,
        &session.token,
    );

    // The candidate is authenticated and the old context is now hidden. Clear
    // its device socket, presence, controllers, and account-pairing callbacks
    // before publishing the replacement context.
    stop_and_clear_device_routing("Account changed").await;
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service.clear_account_pairing_context().await;
        service.clear_trusted_mobile_identity().await;
        service.clear_bot_delegated_identities().await;
    }
    // A replacement that still needs a sync choice must remain memory-only;
    // never leave the prior account's persisted session restorable on crash.
    session_store::clear_session();

    let pending_login_id = has_cloud_settings.then(|| uuid::Uuid::new_v4().to_string());
    let result = AccountLoginResult {
        user_id: session.user_id.clone(),
        pending_login_id: pending_login_id.clone(),
        has_cloud_settings,
    };
    *get_account_context().write().await = Some(AccountContextState {
        session,
        relay_url: relay_url.clone(),
    });
    // Persist non-secret credentials for next startup pre-fill
    save_credential_hint(&request.username, &relay_url);
    // Mirror the relay URL into the Remote Connect "Self-Hosted" server field
    // so phone pairing can ride the same relay the account is logged into.
    set_self_hosted_form_url(Some(&relay_url));
    // Reset the token-expired flag on fresh login
    TOKEN_EXPIRED.store(false, std::sync::atomic::Ordering::Relaxed);

    if has_cloud_settings {
        // Hold the session in memory only until the user picks cloud vs local.
        // Persisting here would restore "logged in" after a kill with no choice.
        set_pending_login_id(pending_login_id);
    } else {
        set_pending_login_id(None);
        if let Err(e) = persist_account_session(Some(device.device_id.as_str())).await {
            log::warn!("Failed to persist session: {e}");
        }

        register_delegated_identity_providers().await;
    }

    // AccountClient revocation is transport-only and does not re-enter host
    // lifecycle locks. Keep the transition lease until it finishes so no
    // caller can replace B and then observe this command returning B's result.
    revoke_replaced_account(&client, replaced_account).await;
    // End context hiding before notifying listeners. Keep the transition mutex
    // until after the event so a listener's immediate account-status probe sees
    // this committed account instead of a transient logged-out state.
    transition_guard.make_context_observable();
    if !has_cloud_settings {
        sync_account_login_capability(true);
        emit_account_event(
            "account://login-state",
            serde_json::json!({
                "logged_in": true,
                "relay_url": relay_url,
            }),
        );
    }
    if has_cloud_settings {
        log::info!(
            "Account authenticated pending sync choice: {} (has_cloud_settings=true)",
            result.user_id
        );
    } else {
        log::info!(
            "Account logged in: {} (has_cloud_settings=false)",
            result.user_id
        );
    }
    Ok(result)
}

#[tauri::command]
pub async fn account_status() -> Result<AccountStatus, String> {
    let pending = PENDING_SYNC_CHOICE.load(std::sync::atomic::Ordering::Relaxed);
    let context = read_account_context().await.ok();
    let logged_in = context.is_some() && !pending;
    Ok(AccountStatus {
        logged_in,
        user_id: if logged_in {
            context.map(|(session, _)| session.user_id)
        } else {
            None
        },
    })
}

/// Stop account-backed runtime services and clear all local login state.
///
/// `revoke_relay_token` is false after deleting this device because the relay
/// deletion already revoked the current token along with the device row.
async fn clear_account_login(revoke_relay_token: bool) {
    // Invalidate the active operation first, then wait for it to observe the
    // cancellation and release its guard. This ensures no settings apply or
    // progress event can happen after logout completes.
    let _sync_guard = cancel_and_wait_for_account_auto_sync().await;
    clear_account_login_state(revoke_relay_token).await;
}

async fn clear_account_login_if_current(
    expected_generation: u64,
    expected_token: &str,
    revoke_relay_token: bool,
) -> bool {
    let Some(_transition_guard) = cancel_and_wait_if_account_current(expected_generation).await
    else {
        return false;
    };
    let token_matches = get_account_context()
        .read()
        .await
        .as_ref()
        .is_some_and(|context| context.session.token == expected_token);
    if !token_matches {
        return false;
    }
    clear_account_login_state(revoke_relay_token).await;
    true
}

async fn clear_account_login_state(revoke_relay_token: bool) {
    // Account transitions hide the context immediately; hide account-backed
    // tools at the same boundary rather than after network cleanup completes.
    sync_account_login_capability(false);
    // Disconnect device routing before clearing the session.
    stop_and_clear_device_routing("Account logged out").await;
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service.clear_account_pairing_context().await;
        service.clear_trusted_mobile_identity().await;
        service.clear_bot_delegated_identities().await;
    }
    if revoke_relay_token {
        // Best-effort relay revocation must not prevent local logout.
        if let Ok((session, relay_url)) = read_account_context_raw().await {
            let _ = AccountClient::new()
                .revoke_token(&relay_url, &session)
                .await;
        }
    }
    *get_account_context().write().await = None;
    set_pending_login_id(None);
    clear_credential_hint();
    session_store::clear_session();
    // Clear the mirrored "Self-Hosted" server field on logout.
    set_self_hosted_form_url(None);
    TOKEN_EXPIRED.store(false, std::sync::atomic::Ordering::Relaxed);
    emit_account_event(
        "account://login-state",
        serde_json::json!({ "logged_in": false }),
    );
}

#[tauri::command]
pub async fn account_logout() -> Result<(), String> {
    clear_account_login(true).await;
    log::info!("Account logged out");
    Ok(())
}

/// Persist (or clear) the account relay URL in the Remote Connect
/// "Self-Hosted" form field so the pairing UI follows account login state.
fn set_self_hosted_form_url(url: Option<&str>) {
    let value = url.unwrap_or_default();
    bot::update_bot_persistence(|data| {
        if data.form_state.custom_server_url != value {
            data.form_state.custom_server_url = value.to_string();
        }
    });
}

// ── P2: Device routing commands ──────────────────────────────────────────

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OnlineDeviceInfo {
    pub device_id: String,
    pub device_name: String,
}

/// Connect to the account relay for device-to-device routing. Must be called
/// after `account_login`. The event receiver is consumed in a background task
/// that logs presence updates; device messages are forwarded to the RemoteConnectService.
#[tauri::command]
pub async fn account_connect_devices() -> Result<Vec<OnlineDeviceInfo>, String> {
    let account_generation = account_context_generation();
    let sync_guard = lock_account_sync(account_generation).await?;
    let (session, relay_url) = read_account_context_for_generation(account_generation).await?;
    let identity = current_device_identity()?;
    let device_name = identity.device_name.clone();
    let holder = get_service_holder().read().await;
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;

    let routing_lifecycle = DEVICE_ROUTING_LIFECYCLE_LOCK.write().await;

    // Reuse is allowed only when the active socket is explicitly owned by the
    // current account generation and token. A service-level connected flag by
    // itself may still describe the account that was just replaced.
    if let Some(devices) = device_presence_for_account(account_generation, &session.token) {
        let is_connected = service.is_device_connected().await;
        if !account_context_matches(account_generation, &session.token).await {
            return Err("account context changed".to_string());
        }
        if is_connected {
            let local_id = DeviceIdentity::from_current_machine()
                .ok()
                .map(|d| d.device_id);
            let local_known = local_id
                .as_ref()
                .is_some_and(|id| devices.iter().any(|d| d.device_id == *id));
            if local_known {
                return Ok(devices);
            }
            log::info!(
            "Device WS connected but local device_id not in online set; reconnecting to heal identity"
        );
        }
    }

    // Invalidate the prior loop before `start_device_connection` swaps the
    // service client. Its compare-and-clear exit path must not touch this new
    // connection's controllers or presence.
    clear_device_routing_state();
    disconnect_peer_controllers("Device routing reconnecting").await;

    let (mut event_rx, auth_device_id, service_connection_id) = match service
        .start_device_connection(&relay_url, &session.token, &device_name)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            let msg = format!("{e}");
            clear_device_routing_state();
            // Token invalidation re-enters the account transition path, so the
            // current account-operation lease must be released first.
            drop(routing_lifecycle);
            drop(sync_guard);
            drop(holder);
            if error_indicates_expired_token(&msg) {
                invalidate_local_account_session_if_current(
                    account_generation,
                    &session.token,
                    &msg,
                )
                .await;
            }
            return Err(msg);
        }
    };

    if !account_context_matches(account_generation, &session.token).await {
        service.stop_device_connection().await;
        clear_device_routing_state();
        return Err("account context changed".to_string());
    }
    let routing_owner =
        new_device_routing_owner(account_generation, &session.token, service_connection_id);
    install_device_routing_owner(routing_owner.clone());

    if let Err(e) = session_store::save_session_with_device(
        &session.token,
        &session.user_id,
        &session.master_key,
        &relay_url,
        Some(auth_device_id.as_str()),
    ) {
        log::warn!("Failed to persist AuthOk device_id into session: {e}");
    }

    // Background task: consume events (presence / device messages / auth errors)
    // Note: AuthOk is consumed inside start_device_connection (adopt happens there).
    let event_session = session.clone();
    let event_owner = routing_owner.clone();
    tokio::spawn(async move {
        use bitfun_core::service::remote_connect::relay_client::RelayEvent;
        'routing_events: while let Some(event) = event_rx.recv().await {
            if !device_routing_owner_is_current(&event_owner).await {
                break;
            }
            match event {
                RelayEvent::AuthOk { user_id, device_id } => {
                    // Should not normally arrive — start_device_connection consumes AuthOk.
                    log::info!(
                        "Device routing auth ok (forwarded unexpectedly): user={user_id} device={device_id}"
                    );
                }
                RelayEvent::AuthError { message } => {
                    log::warn!("Device routing auth error: {message}");
                    // The socket can be rejected while the account panel is
                    // closed. Fully invalidate local state here so desktop
                    // capabilities and every UI surface agree immediately.
                    invalidate_local_account_session_if_current(
                        account_generation,
                        &event_session.token,
                        &message,
                    )
                    .await;
                    break;
                }
                RelayEvent::DevicePresence { devices } => {
                    let Some(_routing_effect) = lock_current_device_routing(&event_owner).await
                    else {
                        break 'routing_events;
                    };
                    let presence = devices
                        .iter()
                        .map(|device| OnlineDeviceInfo {
                            device_id: device.device_id.clone(),
                            device_name: device.device_name.clone(),
                        })
                        .collect();
                    if !replace_device_presence_if_owner(&event_owner, presence) {
                        break 'routing_events;
                    }
                    log::info!("Device presence updated: {} online", devices.len());
                    let online_device_ids = devices
                        .iter()
                        .map(|device| device.device_id.clone())
                        .collect::<std::collections::HashSet<_>>();
                    let request_ids =
                        crate::api::peer_host_invoke::retain_online_controllers(&online_device_ids);
                    if let Err(error) =
                        crate::api::peer_host_invoke::fail_closed_permission_requests(
                            request_ids,
                            "Last Peer controller went offline",
                        )
                        .await
                    {
                        log::warn!("Peer permission requests were not fully cancelled: {error}");
                    }
                    if !device_routing_owner_is_current(&event_owner).await {
                        break 'routing_events;
                    }
                    let pairs: Vec<(String, String)> = devices
                        .iter()
                        .map(|d| (d.device_id.clone(), d.device_name.clone()))
                        .collect();
                    emit_device_presence(&pairs);
                    // Another device came online — pull cloud settings if needed.
                    if devices.len() > 1 {
                        tokio::spawn(async move {
                            pull_and_reconcile(account_generation).await;
                        });
                    }
                }
                RelayEvent::DeviceMessageReceived {
                    source_device_id,
                    correlation_id,
                    encrypted_data,
                    nonce,
                } => {
                    use bitfun_core::service::remote_connect::encryption::decrypt_from_base64;
                    match decrypt_from_base64(&event_session.master_key, &encrypted_data, &nonce) {
                        Ok(plaintext) => {
                            use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
                            match serde_json::from_str::<RemoteCommand>(&plaintext) {
                                Ok(RemoteCommand::DeviceEvent { event, payload }) => {
                                    let Some(_routing_effect) =
                                        lock_current_device_routing(&event_owner).await
                                    else {
                                        break 'routing_events;
                                    };
                                    // Controller receiving peer UI events — re-emit locally
                                    // under the same event name so PeerDeviceTransport listen works.
                                    log::debug!("DeviceEvent from {source_device_id}: {event}");
                                    emit_account_event(&event, payload);
                                }
                                Ok(RemoteCommand::ExecuteOnDevice {
                                    session_id,
                                    content,
                                    agent_type,
                                    workspace_path: _,
                                }) => {
                                    let Some(_routing_effect) =
                                        lock_current_device_routing(&event_owner).await
                                    else {
                                        break 'routing_events;
                                    };
                                    log::info!(
                                        "ExecuteOnDevice from {source_device_id}: \
                                         session={:?} content_len={}",
                                        session_id,
                                        content.len()
                                    );
                                    if let Some(scheduler) = DIALOG_SCHEDULER.get() {
                                        use bitfun_core::agentic::coordination::{
                                            DialogSubmissionPolicy, DialogTriggerSource,
                                        };
                                        let session_id = session_id
                                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                        let policy = DialogSubmissionPolicy::for_source(
                                            DialogTriggerSource::RemoteRelay,
                                        );
                                        let wp = resolve_local_workspace_path();
                                        let agent =
                                            agent_type.unwrap_or_else(|| "agentic".to_string());
                                        if let Err(e) = scheduler
                                            .submit(
                                                session_id,
                                                content,
                                                None,
                                                None,
                                                agent,
                                                Some(wp),
                                                None,
                                                None,
                                                policy,
                                                None,
                                                None,
                                                None,
                                            )
                                            .await
                                        {
                                            log::warn!("ExecuteOnDevice failed: {e}");
                                        }
                                    } else {
                                        log::warn!(
                                            "DialogScheduler not available for ExecuteOnDevice"
                                        );
                                    }
                                    if !device_routing_owner_is_current(&event_owner).await {
                                        break 'routing_events;
                                    }
                                }
                                Ok(RemoteCommand::SendSessionToDevice {
                                    session_data,
                                    session_id,
                                    session_name: _,
                                }) => {
                                    log::info!(
                                        "SendSessionToDevice from {source_device_id}: \
                                         session={session_id} bytes={}",
                                        session_data.len()
                                    );
                                    let import_result =
                                        import_session_bundle(&session_data, account_generation)
                                            .await;
                                    if !device_routing_owner_is_current(&event_owner).await {
                                        break 'routing_events;
                                    }
                                    match import_result {
                                        Ok(()) => {
                                            log::info!("Session {session_id} imported from device {source_device_id}");
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Failed to import session {session_id}: {e}"
                                            );
                                        }
                                    }
                                }
                                Ok(cmd) if source_device_id == "rpc" => {
                                    let Some(_routing_effect) =
                                        lock_current_device_routing(&event_owner).await
                                    else {
                                        break 'routing_events;
                                    };
                                    // HTTP RPC request from another device via relay.
                                    // Execute the command locally and send back
                                    // the encrypted response (including errors).
                                    log::info!(
                                        "RPC request received from relay: corr={correlation_id}"
                                    );
                                    let execution = execute_local_remote_command(&cmd).await;
                                    if !device_routing_owner_is_current(&event_owner).await {
                                        break 'routing_events;
                                    }
                                    match execution {
                                        Ok(resp_value) => {
                                            send_rpc_envelope(
                                                &event_owner,
                                                &event_session,
                                                &correlation_id,
                                                resp_value,
                                            )
                                            .await;
                                        }
                                        Err(e) => {
                                            log::warn!("RPC: execute command failed: {e}");
                                            send_rpc_error(
                                                &event_owner,
                                                &event_session,
                                                &correlation_id,
                                                format!("RPC execute failed: {e}"),
                                            )
                                            .await;
                                        }
                                    }
                                    if !device_routing_owner_is_current(&event_owner).await {
                                        break 'routing_events;
                                    }
                                }
                                Ok(cmd) => {
                                    let _ = cmd;
                                    log::info!("Received device command");
                                }
                                Err(e) => {
                                    log::warn!("Could not parse device command: {e}");
                                    if source_device_id == "rpc" {
                                        let Some(_routing_effect) =
                                            lock_current_device_routing(&event_owner).await
                                        else {
                                            break 'routing_events;
                                        };
                                        send_rpc_error(
                                            &event_owner,
                                            &event_session,
                                            &correlation_id,
                                            format!("invalid RPC command: {e}"),
                                        )
                                        .await;
                                        if !device_routing_owner_is_current(&event_owner).await {
                                            break 'routing_events;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to decrypt device message: {e}");
                            if source_device_id == "rpc" {
                                let Some(_routing_effect) =
                                    lock_current_device_routing(&event_owner).await
                                else {
                                    break 'routing_events;
                                };
                                send_rpc_error(
                                    &event_owner,
                                    &event_session,
                                    &correlation_id,
                                    format!("failed to decrypt RPC request: {e}"),
                                )
                                .await;
                                if !device_routing_owner_is_current(&event_owner).await {
                                    break 'routing_events;
                                }
                            }
                        }
                    }
                }
                RelayEvent::Disconnected => {
                    let Some(_routing_effect) = lock_current_device_routing(&event_owner).await
                    else {
                        break 'routing_events;
                    };
                    if !replace_device_presence_if_owner(&event_owner, Vec::new()) {
                        break 'routing_events;
                    }
                    log::info!("Device routing disconnected");
                    let request_ids =
                        crate::api::peer_host_invoke::take_tracked_permission_requests();
                    if let Err(error) =
                        crate::api::peer_host_invoke::fail_closed_permission_requests(
                            request_ids,
                            "Peer device-routing connection lost",
                        )
                        .await
                    {
                        log::warn!("Peer permission requests were not fully cancelled: {error}");
                    }
                    if !device_routing_owner_is_current(&event_owner).await {
                        break 'routing_events;
                    }
                    emit_device_presence(&[]);
                }
                RelayEvent::Reconnected => {
                    let Some(_routing_effect) = lock_current_device_routing(&event_owner).await
                    else {
                        break 'routing_events;
                    };
                    log::info!("Device routing reconnected — AuthConnect re-sent by transport");
                }
                _ => {}
            }
        }
        finish_device_routing_event_loop(&event_owner).await;
    });

    drop(routing_lifecycle);
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    Ok(device_presence_for_account(account_generation, &session.token).unwrap_or_default())
}

/// Get the current online device list.
#[tauri::command]
pub async fn account_online_devices() -> Result<Vec<OnlineDeviceInfo>, String> {
    let account_generation = account_context_generation();
    let (session, _) = read_account_context_for_generation(account_generation).await?;
    let _lifecycle = DEVICE_ROUTING_LIFECYCLE_LOCK.read().await;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    Ok(device_presence_for_account(account_generation, &session.token).unwrap_or_default())
}

/// Send an encrypted session to a peer device. The `session_json` is encrypted
/// with the master key before being sent over the relay.
#[tauri::command]
pub async fn account_send_session_to_device(
    target_device_id: String,
    session_id: String,
    session_json: String,
) -> Result<(), String> {
    let account_generation = account_context_generation();
    let (session, _) = read_account_context_for_generation(account_generation).await?;

    // Wrap the raw session JSON in a SendSessionToDevice command envelope so the
    // receiving device knows what to do with the payload.
    use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
    let envelope = serde_json::to_string(&RemoteCommand::SendSessionToDevice {
        session_data: session_json,
        session_id: session_id.clone(),
        session_name: None,
    })
    .map_err(|e| format!("serialize envelope: {e}"))?;

    let _routing_effect = DEVICE_ROUTING_LIFECYCLE_LOCK.read().await;
    let routing_owner = device_routing_owner_for_account(account_generation, &session.token)
        .ok_or_else(|| "device routing not connected for current account".to_string())?;
    if !device_routing_owner_is_current(&routing_owner).await {
        return Err("device routing changed".to_string());
    }
    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    let (encrypted_data, nonce) =
        encrypt_to_base64(&session.master_key, &envelope).map_err(|e| format!("{e}"))?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    send_device_message_with_routing_lease(
        &routing_owner,
        &target_device_id,
        &correlation_id,
        &encrypted_data,
        &nonce,
    )
    .await
}

// ── P4: Session / settings sync commands ─────────────────────────────────

/// Upload a single session blob (encrypted client-side with the master key).
#[tauri::command]
pub async fn account_sync_session(session_id: String, session_json: String) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .upload_session(&relay_url, &session, &session_id, &session_json)
        .await
        .map(|_| ())
        .map_err(|e| format!("{e}"))
}

/// Fetch all synced session blobs (decrypted client-side).
#[derive(Serialize)]
pub struct SyncedSession {
    pub session_id: String,
    pub session_json: String,
}

#[tauri::command]
pub async fn account_fetch_synced_sessions() -> Result<Vec<SyncedSession>, String> {
    let (session, relay_url) = read_account_context().await?;
    let sessions = AccountClient::new()
        .fetch_sessions(&relay_url, &session, 0)
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(sessions
        .into_iter()
        .map(|s| SyncedSession {
            session_id: s.session_id,
            session_json: s.plaintext,
        })
        .collect())
}

/// Delete a synced session blob from the relay.
#[tauri::command]
pub async fn account_delete_synced_session(session_id: String) -> Result<(), String> {
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .delete_session(&relay_url, &session, &session_id)
        .await
        .map_err(|e| format!("{e}"))?;
    let mut state = sync_state::load(&session.user_id);
    state.clear_uploaded_hash(&session_id);
    let _ = sync_state::save(&session.user_id, &state);
    Ok(())
}

/// Upload settings blob (encrypted client-side with the master key).
#[tauri::command]
pub async fn account_sync_settings(settings_json: String) -> Result<(), String> {
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    let (session, relay_url) = read_account_context().await?;
    bitfun_core::service::remote_connect::settings_sync::upload_settings_payload(
        &session,
        &relay_url,
        &settings_json,
    )
    .await
    .map_err(|e| format!("{e}"))?;
    Ok(())
}

/// Fetch and decrypt the settings blob. Returns null if none exists.
#[tauri::command]
pub async fn account_fetch_settings() -> Result<Option<String>, String> {
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .fetch_settings(&relay_url, &session)
        .await
        .map_err(|e| format!("{e}"))
}

// ── High-level session sync (export / import / auto-sync) ─────────────────

/// Max concurrent session blob POSTs during multi-session upload.
const UPLOAD_CONCURRENCY: usize = 5;

/// A serializable session bundle: metadata + all dialog turns.
/// This is the unit of cross-device sync — encrypted with the master key
/// before upload to the relay.
#[derive(Serialize, Deserialize)]
pub struct SessionBundle {
    pub session_id: String,
    pub metadata: serde_json::Value,
    pub turns: Vec<serde_json::Value>,
    pub source_device_id: Option<String>,
    pub source_device_name: Option<String>,
}

const RELAY_TURNS_IMPORT_STATE_KEY: &str = "relayTurnsImportState";
const RELAY_TURNS_IMPORT_PENDING: &str = "pending";
const RELAY_TURNS_IMPORT_COMPLETE: &str = "complete";

fn relay_turns_import_state(metadata: &SessionMetadata) -> Option<&str> {
    metadata
        .custom_metadata
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .and_then(|custom| custom.get(RELAY_TURNS_IMPORT_STATE_KEY))
        .and_then(serde_json::Value::as_str)
}

fn relay_turns_import_is_complete(metadata: &SessionMetadata, local_turn_count: usize) -> bool {
    metadata.turn_count == local_turn_count
        && relay_turns_import_state(metadata) == Some(RELAY_TURNS_IMPORT_COMPLETE)
}

fn set_relay_turns_import_state(metadata: &mut SessionMetadata, state: &str) {
    let mut custom = metadata
        .custom_metadata
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    custom.insert(
        RELAY_TURNS_IMPORT_STATE_KEY.to_string(),
        serde_json::Value::String(state.to_string()),
    );
    metadata.custom_metadata = Some(serde_json::Value::Object(custom));
}

fn mark_relay_turns_import_complete(metadata: &mut SessionMetadata) {
    set_relay_turns_import_state(metadata, RELAY_TURNS_IMPORT_COMPLETE);
}

/// Export a single local session as an encrypted blob and upload it to the relay.
/// Uses the workspace + session_id to load metadata and turns from disk.
#[tauri::command]
pub async fn account_export_local_session(
    session_id: String,
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<(), String> {
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    let (acct_session, relay_url) = read_account_context().await?;

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;

    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    // Load metadata
    let metadata = manager
        .load_session_metadata(&storage_path, &session_id)
        .await
        .map_err(|e| format!("load metadata: {e}"))?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    // Load all turns
    let turns = manager
        .load_session_turns(&storage_path, &session_id)
        .await
        .map_err(|e| format!("load turns: {e}"))?;

    // Serialize to bundle
    let metadata_json =
        serde_json::to_value(&metadata).map_err(|e| format!("serialize metadata: {e}"))?;
    let turns_json: Vec<serde_json::Value> = turns
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
        .collect();

    let device = current_device_identity()?;
    let bundle = SessionBundle {
        session_id: session_id.clone(),
        metadata: metadata_json,
        turns: turns_json,
        source_device_id: Some(device.device_id.clone()),
        source_device_name: Some(device.device_name.clone()),
    };

    let bundle_json =
        serde_json::to_string(&bundle).map_err(|e| format!("serialize bundle: {e}"))?;

    let hash = sync_state::content_hash(&bundle_json);
    AccountClient::new()
        .upload_session(&relay_url, &acct_session, &session_id, &bundle_json)
        .await
        .map_err(|e| format!("{e}"))?;
    let mut state = sync_state::load(&acct_session.user_id);
    state.set_uploaded_hash(&session_id, hash);
    let _ = sync_state::save(&acct_session.user_id, &state);
    Ok(())
}

/// Export all local sessions for a workspace and upload them to the relay.
/// Returns the number of sessions synced.
#[tauri::command]
pub async fn account_export_all_sessions(
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<usize, String> {
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    let (acct_session, relay_url) = read_account_context().await?;

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;

    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    let sessions = manager
        .list_session_metadata(&storage_path)
        .await
        .map_err(|e| format!("list sessions: {e}"))?;

    let mut state = sync_state::load(&acct_session.user_id);
    let mut pending: Vec<(String, String, String)> = Vec::new();
    for meta in &sessions {
        let turns = manager
            .load_session_turns(&storage_path, &meta.session_id)
            .await
            .map_err(|e| format!("load turns for {}: {e}", meta.session_id))?;

        let metadata_json =
            serde_json::to_value(meta).map_err(|e| format!("serialize metadata: {e}"))?;
        let turns_json: Vec<serde_json::Value> = turns
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
            .collect();

        let bundle = SessionBundle {
            session_id: meta.session_id.clone(),
            metadata: metadata_json,
            turns: turns_json,
            source_device_id: None,
            source_device_name: None,
        };

        let bundle_json =
            serde_json::to_string(&bundle).map_err(|e| format!("serialize bundle: {e}"))?;
        let hash = sync_state::content_hash(&bundle_json);
        if state.uploaded_hash(&meta.session_id) == Some(hash.as_str()) {
            continue;
        }
        pending.push((meta.session_id.clone(), bundle_json, hash));
    }

    let uploaded: Vec<(String, String)> = stream::iter(pending)
        .map(|(session_id, bundle_json, hash)| {
            let client = AccountClient::new();
            let relay_url = relay_url.clone();
            let acct_session = acct_session.clone();
            async move {
                match client
                    .upload_session(&relay_url, &acct_session, &session_id, &bundle_json)
                    .await
                {
                    Ok(_version) => Some((session_id, hash)),
                    Err(e) => {
                        log::warn!("Export session {session_id} failed: {e}");
                        None
                    }
                }
            }
        })
        .buffer_unordered(UPLOAD_CONCURRENCY)
        .filter_map(|r| async move { r })
        .collect()
        .await;

    let count = uploaded.len();
    for (session_id, hash) in uploaded {
        state.set_uploaded_hash(&session_id, hash);
    }
    let _ = sync_state::save(&acct_session.user_id, &state);
    log::info!("Exported {count} sessions to relay");
    Ok(count)
}

/// Import all synced sessions from the relay into local storage.
/// Sessions that already exist locally are skipped (no overwrite).
/// Returns the number of newly imported sessions.
#[tauri::command]
pub async fn account_import_remote_sessions(
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<Vec<String>, String> {
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    let (acct_session, relay_url) = read_account_context().await?;

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;

    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    let remote_sessions = AccountClient::new()
        .fetch_sessions(&relay_url, &acct_session, 0)
        .await
        .map_err(|e| format!("{e}"))?;

    let mut imported = Vec::new();
    for fetched in remote_sessions {
        let session_id = fetched.session_id;
        let bundle_json = fetched.plaintext;
        // Deserialize the bundle and write metadata as-is. The source device's
        // workspace_path is preserved for display (read-only history). Tasks
        // are always executed on the receiving device's own workspace, so
        // cross-platform path differences don't affect execution.
        let bundle: SessionBundle =
            serde_json::from_str(&bundle_json).map_err(|e| format!("deserialize bundle: {e}"))?;

        let mut metadata: SessionMetadata = serde_json::from_value(bundle.metadata)
            .map_err(|e| format!("deserialize metadata: {e}"))?;
        if metadata.session_id != session_id {
            log::warn!(
                "Skipping remote session bundle with mismatched metadata identity: expected_session_id={}, metadata_session_id={}",
                session_id,
                metadata.session_id
            );
            continue;
        }
        // Only write metadata — turns are lazy-loaded when the user opens
        // the session (see `account_fetch_session_turns`).
        set_relay_turns_import_state(&mut metadata, RELAY_TURNS_IMPORT_PENDING);
        if !manager
            .create_session_metadata_if_absent(&storage_path, &metadata)
            .await
            .map_err(|error| {
                format!("persist imported metadata for session {session_id}: {error}")
            })?
        {
            continue;
        }

        imported.push(session_id);
    }

    log::info!("Imported {} remote sessions", imported.len());
    Ok(imported)
}

/// Lazy-load a session's turns from the relay on first open.
///
/// When the periodic pull imports a remote session, it writes only metadata
/// (no turns) to keep the pull lightweight. When the user clicks into that
/// session, the frontend calls this command to fetch the full session bundle
/// from the relay and persist the turns locally. Subsequent opens read from
/// local disk without hitting the relay.
///
/// Returns `true` if turns were fetched and written, `false` if the session
/// already had local turns (no relay fetch needed).
#[tauri::command]
pub async fn account_fetch_session_turns(
    session_id: String,
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<bool, String> {
    let generation = account_context_generation();
    // Soft-skip before any disk IO so accidental callers cannot fail-closed
    // Peer hydrate on metadata load errors. History comes from the peer host.
    if crate::api::peer_host_invoke::is_peer_controller_active() {
        log::info!("Skipping cloud session turn fetch in Peer Device Mode (session={session_id})");
        return Ok(false);
    }

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    // Ordinary local sessions carry no relay marker and return without an
    // account or network lookup. A non-empty turn prefix is not proof that an
    // import completed, so pending or inconsistent imports are retried.
    let Some(metadata) = manager
        .load_session_metadata(&storage_path, &session_id)
        .await
        .map_err(|error| format!("load imported metadata: {error}"))?
    else {
        return Ok(false);
    };
    if relay_turns_import_state(&metadata).is_none() {
        return Ok(false);
    }

    let local_turns = manager
        .load_session_turns(&storage_path, &session_id)
        .await
        .map_err(|error| format!("load imported turns: {error}"))?;
    if relay_turns_import_is_complete(&metadata, local_turns.len()) {
        return Ok(false);
    }

    // Fetch the full bundle from the relay (which includes turns). Keep the
    // account lease through the local commit so an account switch cannot write
    // a stale account's history after it completes.
    let _sync_guard = lock_account_sync(generation).await?;
    let (acct_session, relay_url) = read_account_context().await?;
    let fetched = AccountClient::new()
        .fetch_session(&relay_url, &acct_session, &session_id)
        .await
        .map_err(|e| format!("{e}"))?
        .ok_or_else(|| "session not found on relay".to_string())?;

    let bundle: SessionBundle =
        serde_json::from_str(&fetched.plaintext).map_err(|e| format!("deserialize bundle: {e}"))?;

    let metadata: SessionMetadata = serde_json::from_value(bundle.metadata.clone())
        .map_err(|e| format!("deserialize metadata: {e}"))?;
    if metadata.session_id != session_id {
        return Err("relay session metadata identity does not match request".to_string());
    }
    let turns = bundle
        .turns
        .iter()
        .map(|turn| {
            serde_json::from_value::<DialogTurnData>(turn.clone())
                .map_err(|error| format!("deserialize turn: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if turns.iter().any(|turn| turn.session_id != session_id) {
        return Err("relay session turn identity does not match request".to_string());
    }

    manager
        .create_session_metadata_if_absent(&storage_path, &metadata)
        .await
        .map_err(|e| format!("persist imported metadata: {e}"))?;

    // Each turn save refreshes counts through an owner-side metadata RMW.
    for turn in &turns {
        manager
            .save_dialog_turn(&storage_path, turn)
            .await
            .map_err(|e| format!("persist imported turn: {e}"))?;
    }
    manager
        .update_session_metadata(&storage_path, &session_id, |metadata| {
            mark_relay_turns_import_complete(metadata);
        })
        .await
        .map_err(|e| format!("mark imported turns complete: {e}"))?;

    log::info!(
        "Lazy-loaded {} turns for session {session_id}",
        bundle.turns.len()
    );
    Ok(true)
}

/// Execute a task on a remote device — sends an ExecuteOnDevice command
/// over the device-messaging WS pathway.
#[tauri::command]
pub async fn account_execute_on_device(
    target_device_id: String,
    session_id: Option<String>,
    content: String,
    agent_type: Option<String>,
    workspace_path: Option<String>,
) -> Result<(), String> {
    let account_generation = account_context_generation();
    let (session, _) = read_account_context_for_generation(account_generation).await?;

    use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
    let envelope = serde_json::to_string(&RemoteCommand::ExecuteOnDevice {
        session_id,
        content,
        agent_type,
        workspace_path,
    })
    .map_err(|e| format!("serialize envelope: {e}"))?;

    let _routing_effect = DEVICE_ROUTING_LIFECYCLE_LOCK.read().await;
    let routing_owner = device_routing_owner_for_account(account_generation, &session.token)
        .ok_or_else(|| "device routing not connected for current account".to_string())?;
    if !device_routing_owner_is_current(&routing_owner).await {
        return Err("device routing changed".to_string());
    }
    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    let (encrypted_data, nonce) =
        encrypt_to_base64(&session.master_key, &envelope).map_err(|e| format!("{e}"))?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    send_device_message_with_routing_lease(
        &routing_owner,
        &target_device_id,
        &correlation_id,
        &encrypted_data,
        &nonce,
    )
    .await
}

/// List all online devices in the account via the relay HTTP API.
/// Returns `(device_id, device_name)` pairs.
#[derive(Serialize)]
pub struct AccountDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

#[tauri::command]
pub async fn account_list_devices() -> Result<Vec<AccountDeviceInfo>, String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let client = AccountClient::new();
    let devices = match client.list_devices(&relay_url, &session).await {
        Ok(devices) => devices,
        Err(e) => {
            let msg = format!("{e}");
            if error_indicates_expired_token(&msg) {
                invalidate_local_account_session_if_current(generation, &session.token, &msg).await;
            }
            return Err(msg);
        }
    };
    if !account_context_is_current(generation) {
        return Err("account context changed".to_string());
    }
    Ok(devices
        .into_iter()
        .map(|d| AccountDeviceInfo {
            device_id: d.device_id,
            device_name: d.device_name,
            online: d.online,
            last_seen_at: d.last_seen_at,
        })
        .collect())
}

/// Remove a device from the account.
#[tauri::command]
pub async fn account_delete_device(targetDeviceId: String) -> Result<(), String> {
    let generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(generation).await?;
    let is_current_device = current_device_identity()?.device_id == targetDeviceId;
    if let Err(error) = AccountClient::new()
        .delete_device(&relay_url, &session, &targetDeviceId)
        .await
    {
        let message = error.to_string();
        if error_indicates_expired_token(&message) {
            invalidate_local_account_session_if_current(generation, &session.token, &message).await;
        }
        return Err(message);
    }
    if !account_context_is_current(generation) {
        return Err("account context changed".to_string());
    }
    log::info!("Device {targetDeviceId} removed from account");
    if is_current_device {
        if clear_account_login_if_current(generation, &session.token, false).await {
            log::info!("Current device removed; local account session cleared");
        } else {
            return Err("account context changed".to_string());
        }
    }
    Ok(())
}

/// Send any RemoteCommand to a target device via HTTP RPC.
/// The command is encrypted with the master_key, sent to the relay,
/// which routes it to the target device's WS. The target executes it
/// and the response is returned (decrypted).
/// Returns the decrypted response JSON.
const ACCOUNT_DEVICE_RPC_DEFAULT_TIMEOUT_MS: u64 = 120_000;
const ACCOUNT_DEVICE_RPC_MIN_TIMEOUT_MS: u64 = 1_000;

fn account_device_rpc_timeout_ms(requested: Option<u64>) -> u64 {
    requested
        .unwrap_or(ACCOUNT_DEVICE_RPC_DEFAULT_TIMEOUT_MS)
        .clamp(
            ACCOUNT_DEVICE_RPC_MIN_TIMEOUT_MS,
            ACCOUNT_DEVICE_RPC_DEFAULT_TIMEOUT_MS,
        )
}

#[tauri::command]
pub async fn account_device_rpc(
    target_device_id: String,
    command_json: String,
    timeout_ms: Option<u64>,
) -> Result<String, String> {
    let account_generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(account_generation).await?;
    let client = AccountClient::new();
    let timeout_ms = account_device_rpc_timeout_ms(timeout_ms);
    let response = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        client.device_rpc(&relay_url, &session, &target_device_id, &command_json),
    )
    .await
    .map_err(|_| format!("device RPC timed out after {timeout_ms}ms"))?
    .map_err(|e| format!("{e}"))?;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    Ok(response)
}

/// Delegate the account identity to a paired mobile-web/IM client.
/// Called by the frontend after pairing succeeds.
#[tauri::command]
pub async fn account_delegate_to_paired(correlation_id: String) -> Result<String, String> {
    let account_generation = account_context_generation();
    let (session, relay_url) = read_account_context_for_generation(account_generation).await?;
    let client = AccountClient::new();

    // Capture the room owner before requesting a token. A later secret check
    // rejects a pairing that changed while the relay request was in flight.
    let holder = get_service_holder().read().await;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;
    let pairing_secret = service
        .pairing_shared_secret()
        .await
        .ok_or_else(|| "no paired device".to_string())?;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }

    // 1. Get a delegated token from the relay
    let delegated = client
        .delegate_token(&relay_url, &session)
        .await
        .map_err(|e| format!("{e}"))?;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    if delegated.user_id != session.user_id {
        return Err("delegated identity does not match the current account".to_string());
    }

    let current_pairing_secret = service.pairing_shared_secret().await;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    if current_pairing_secret.as_ref() != Some(&pairing_secret) {
        return Err("paired device changed".to_string());
    }

    // 2. Build the delegated identity JSON (master_key as base64)
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let device_id = current_device_identity()?.device_id;
    let identity_json = serde_json::json!({
        "resp": "delegate_identity",
        "token": delegated.token,
        "user_id": delegated.user_id,
        "master_key": B64.encode(&session.master_key),
        "device_id": device_id,
    });
    let identity_str =
        serde_json::to_string(&identity_json).map_err(|e| format!("serialize identity: {e}"))?;

    // 3. Encrypt with the captured room secret and atomically verify that the
    // service still owns that pairing before sending.
    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    let (enc, nonce) = encrypt_to_base64(&pairing_secret, &identity_str)
        .map_err(|e| format!("encrypt delegated identity: {e}"))?;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    let expected_token = session.token.clone();
    let sent = service
        .send_room_response_if_pairing_secret_authorized(
            &pairing_secret,
            &correlation_id,
            &enc,
            &nonce,
            || async move {
                let account_lease = lock_account_sync(account_generation).await?;
                if !account_context_matches(account_generation, &expected_token).await {
                    return Err("account context changed".to_string());
                }
                Ok(account_lease)
            },
        )
        .await
        .map_err(|e| format!("send delegated identity: {e}"))?;
    if !account_context_matches(account_generation, &session.token).await {
        return Err("account context changed".to_string());
    }
    if !sent {
        return Err("paired device changed".to_string());
    }
    log::info!("Delegated identity sent to paired device (corr={correlation_id})");

    Ok(identity_str)
}

/// Result of an auto-sync operation, returned to the frontend.
#[derive(Serialize)]
pub struct AutoSyncResult {
    pub settings_synced: bool,
    pub sessions_exported: usize,
    pub sessions_imported: usize,
}

/// Perform the full auto-sync flow. Called by the frontend after login
/// (first login) or after the user confirms cloud-settings overwrite
/// (non-first login).
#[tauri::command]
pub async fn account_auto_sync(
    is_first_login: bool,
    workspace_path: String,
    config_json: String,
    sync_operation_id: u64,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<AutoSyncResult, String> {
    if sync_operation_id == 0 {
        return Err("sync operation id must be non-zero".to_string());
    }
    // Capture the account generation before queueing. A logout or replacement
    // login that wins the lock invalidates this call instead of letting a stale
    // request start against the newly installed account.
    let generation = account_context_generation();
    let _sync_guard = lock_account_sync(generation).await?;
    ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID.store(sync_operation_id, Ordering::Release);
    let result = account_auto_sync_inner(
        is_first_login,
        workspace_path,
        config_json,
        sync_operation_id,
        app_state,
        path_manager,
    )
    .await;
    let _ = ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID.compare_exchange(
        sync_operation_id,
        0,
        Ordering::AcqRel,
        Ordering::Acquire,
    );
    result
}

async fn account_auto_sync_inner(
    is_first_login: bool,
    workspace_path: String,
    config_json: String,
    sync_operation_id: u64,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<AutoSyncResult, String> {
    // Soft no-op while controlling a peer: cloud sync would rewrite the
    // controller's local disk mid-remote. Match account_fetch_session_turns.
    if crate::api::peer_host_invoke::is_peer_controller_active() {
        log::info!("Skipping account auto-sync while Peer Device Mode is active");
        return Ok(AutoSyncResult {
            settings_synced: false,
            sessions_exported: 0,
            sessions_imported: 0,
        });
    }
    ensure_account_auto_sync_current(sync_operation_id)?;
    let (acct_session, relay_url) = read_account_context().await?;
    let client = AccountClient::new();
    use bitfun_core::service::remote_connect::settings_sync;

    // 1. Settings sync
    let settings_synced = if is_first_login {
        emit_sync_progress(sync_operation_id, "uploading_settings", 5, None, None, None);
        await_account_auto_sync(
            sync_operation_id,
            settings_sync::upload_settings_payload(&acct_session, &relay_url, &config_json),
        )
        .await?
        .map_err(|e| format!("upload settings: {e}"))?;
        ensure_account_auto_sync_current(sync_operation_id)?;
        log::info!("First login: uploaded local settings to cloud");
        emit_sync_progress(sync_operation_id, "settings_done", 15, None, None, None);
        true
    } else {
        emit_sync_progress(
            sync_operation_id,
            "downloading_settings",
            5,
            None,
            None,
            None,
        );
        let cloud = await_account_auto_sync(
            sync_operation_id,
            client.fetch_settings_with_version(&relay_url, &acct_session),
        )
        .await?
        .map_err(|e| format!("fetch settings: {e}"))?;
        ensure_account_auto_sync_current(sync_operation_id)?;
        if let Some(blob) = cloud {
            emit_sync_progress(sync_operation_id, "applying_settings", 10, None, None, None);
            // Explicit user choice — always apply, even when the cursor says
            // this device already has this version. Applies into the global
            // config service, invalidates the AI client cache, reloads, and
            // emits `account://settings-applied`.
            await_account_auto_sync(
                sync_operation_id,
                settings_sync::apply_settings_blob(&acct_session, &blob, true),
            )
            .await?
            .map_err(|e| format!("apply cloud config: {e}"))?;
            ensure_account_auto_sync_current(sync_operation_id)?;
            log::info!(
                "Applied cloud settings to local device (version={})",
                blob.version
            );
            emit_sync_progress(sync_operation_id, "settings_done", 15, None, None, None);
            true
        } else {
            emit_sync_progress(sync_operation_id, "settings_done", 15, None, None, None);
            false
        }
    };

    // 2. Session sync: upload local sessions only (backup). Do NOT import cloud
    // sessions into local disk — Remote peer mode reads the peer's live disk.
    ensure_account_auto_sync_current(sync_operation_id)?;
    emit_sync_progress(sync_operation_id, "listing_sessions", 18, None, None, None);
    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;
    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    let local_sessions = manager
        .list_session_metadata(&storage_path)
        .await
        .map_err(|e| format!("list sessions: {e}"))?;

    let export_candidates = local_sessions.len();
    emit_sync_progress(
        sync_operation_id,
        "exporting_sessions",
        20,
        Some(0),
        Some(export_candidates),
        None,
    );

    let mut sync_state_local = sync_state::load(&acct_session.user_id);
    let mut pending_uploads: Vec<(String, String, String)> = Vec::new();
    for meta in local_sessions.iter() {
        ensure_account_auto_sync_current(sync_operation_id)?;
        let turns = manager
            .load_session_turns(&storage_path, &meta.session_id)
            .await
            .map_err(|e| format!("load turns: {e}"))?;
        let metadata_json =
            serde_json::to_value(meta).map_err(|e| format!("serialize metadata: {e}"))?;
        let turns_json: Vec<serde_json::Value> = turns
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
            .collect();
        let bundle = SessionBundle {
            session_id: meta.session_id.clone(),
            metadata: metadata_json,
            turns: turns_json,
            source_device_id: None,
            source_device_name: None,
        };
        let bundle_json =
            serde_json::to_string(&bundle).map_err(|e| format!("serialize bundle: {e}"))?;
        let hash = sync_state::content_hash(&bundle_json);
        if sync_state_local.uploaded_hash(&meta.session_id) == Some(hash.as_str()) {
            continue;
        }
        pending_uploads.push((meta.session_id.clone(), bundle_json, hash));
    }

    let upload_total = pending_uploads.len();
    emit_sync_progress(
        sync_operation_id,
        "exporting_sessions",
        20,
        Some(0),
        Some(upload_total),
        None,
    );

    let completed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let upload_outcomes: Vec<Result<(String, String, i64), String>> = stream::iter(pending_uploads)
        .map(|(session_id, bundle_json, hash)| {
            let client = AccountClient::new();
            let relay_url = relay_url.clone();
            let acct_session = acct_session.clone();
            let completed = completed.clone();
            async move {
                if ensure_account_auto_sync_current(sync_operation_id).is_err() {
                    return Err("account sync cancelled".to_string());
                }
                let result = match await_account_auto_sync(
                    sync_operation_id,
                    client.upload_session(&relay_url, &acct_session, &session_id, &bundle_json),
                )
                .await
                {
                    Ok(result) => result,
                    Err(e) => return Err(e),
                };
                match result {
                    Ok(version) => {
                        let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        let percent = if upload_total == 0 {
                            95u8
                        } else {
                            20 + ((75 * done) / upload_total) as u8
                        };
                        if ensure_account_auto_sync_current(sync_operation_id).is_err() {
                            return Err("account sync cancelled".to_string());
                        }
                        emit_sync_progress(
                            sync_operation_id,
                            "exporting_sessions",
                            percent.min(95),
                            Some(done),
                            Some(upload_total),
                            Some(session_id.as_str()),
                        );
                        Ok((session_id, hash, version))
                    }
                    Err(e) => {
                        log::warn!("Auto-sync upload {session_id} failed: {e}");
                        Err(format!("{session_id}: {e}"))
                    }
                }
            }
        })
        .buffer_unordered(UPLOAD_CONCURRENCY)
        .collect()
        .await;

    ensure_account_auto_sync_current(sync_operation_id)?;

    let mut uploaded = Vec::new();
    let mut upload_errors = Vec::new();
    for outcome in upload_outcomes {
        match outcome {
            Ok(item) => uploaded.push(item),
            Err(err) => upload_errors.push(err),
        }
    }

    let exported = uploaded.len();
    let mut max_uploaded_version = sync_state_local.last_session_since;
    for (session_id, hash, version) in uploaded {
        sync_state_local.set_uploaded_hash(&session_id, hash);
        if version > max_uploaded_version {
            max_uploaded_version = version;
        }
    }
    if max_uploaded_version > sync_state_local.last_session_since {
        sync_state_local.last_session_since = max_uploaded_version;
    }
    let _ = sync_state::save(&acct_session.user_id, &sync_state_local);

    ensure_session_backup_complete(upload_total, exported, &upload_errors)?;

    log::info!("Auto-sync: settings={settings_synced} exported={exported} imported=0");
    emit_sync_progress(
        sync_operation_id,
        "done",
        100,
        Some(exported),
        Some(0),
        None,
    );
    Ok(AutoSyncResult {
        settings_synced,
        sessions_exported: exported,
        sessions_imported: 0,
    })
}

fn ensure_session_backup_complete(
    total: usize,
    uploaded: usize,
    upload_errors: &[String],
) -> Result<(), String> {
    if uploaded == total {
        return Ok(());
    }
    let detail = upload_errors
        .first()
        .map(|err| err.as_str())
        .unwrap_or("retry will resume remaining sessions");
    Err(format!(
        "session backup incomplete: uploaded {uploaded} of {total}; {detail}"
    ))
}

// ── Auto-sync: debounced upload on session changes ─────────────────────────
//
// Settings sync (debounced push + 30s pull) is owned by the shared engine in
// `bitfun_core::service::remote_connect::settings_sync`; this module only
// keeps the desktop-specific session backup loop and wires engine hooks.

use std::time::Duration;
use tokio::sync::mpsc;

/// What to sync. Each variant maps to a single relay operation.
#[derive(Debug, Clone)]
enum SyncRequest {
    /// Upload (or replace) a session blob — fired on create/turn-save/metadata/rename.
    SessionUpsert {
        session_id: String,
        workspace_path: String,
    },
    /// Tombstone a session on the relay — fired on delete. Prevents re-import.
    SessionDelete { session_id: String },
}

/// Global channel for notifying the sync background task.
static SYNC_TX: OnceLock<mpsc::UnboundedSender<SyncRequest>> = OnceLock::new();

/// Called once at app startup to start the settings sync engine and the
/// debounced session sync background task.
pub fn init_auto_sync() {
    start_settings_sync_engine();
    let (tx, rx) = mpsc::unbounded_channel::<SyncRequest>();
    let _ = SYNC_TX.set(tx);
    tokio::spawn(sync_background_loop(rx));
}

/// Start the shared settings sync engine with desktop hooks.
fn start_settings_sync_engine() {
    use bitfun_core::service::remote_connect::settings_sync;
    let hooks = settings_sync::SettingsSyncHooks {
        account_context: Some(std::sync::Arc::new(|| {
            Box::pin(async {
                if !background_account_sync_is_allowed() {
                    return Err(anyhow::anyhow!(
                        "account login is waiting for a settings choice"
                    ));
                }
                let generation = account_context_generation();
                if !account_context_is_current(generation) {
                    return Err(anyhow::anyhow!("account context is transitioning"));
                }
                let (account, relay_url) =
                    read_account_context().await.map_err(anyhow::Error::msg)?;
                if !account_context_is_current(generation) {
                    return Err(anyhow::anyhow!("account context changed while reading"));
                }
                if !background_account_sync_is_allowed() {
                    return Err(anyhow::anyhow!(
                        "account login is waiting for a settings choice"
                    ));
                }
                Ok((account, relay_url, generation))
            })
        })),
        is_account_context_current: Some(std::sync::Arc::new(account_context_is_current)),
        should_pause: Some(std::sync::Arc::new(|| {
            crate::api::peer_host_invoke::is_peer_controller_active()
        })),
        on_settings_applied: Some(std::sync::Arc::new(|| {
            emit_settings_applied();
            fanout_peer_device_event(
                "account://settings-applied".to_string(),
                serde_json::json!({ "applied": true }),
            );
        })),
        on_settings_pushed: Some(std::sync::Arc::new(|| {
            fanout_peer_device_event(
                "account://settings-applied".to_string(),
                serde_json::json!({ "applied": true }),
            );
        })),
        on_token_expired: Some(std::sync::Arc::new(|| {
            TOKEN_EXPIRED.store(true, std::sync::atomic::Ordering::Relaxed);
        })),
        ..Default::default()
    };
    settings_sync::start_settings_sync_engine(hooks);
}

/// Non-blocking notification that a session was created/modified. Called from
/// `save_session_turn`, `save_session_metadata`, `create_session`,
/// `update_session_title` Tauri commands.
pub fn notify_session_changed(session_id: &str, workspace_path: &str) {
    if let Some(tx) = SYNC_TX.get() {
        let _ = tx.send(SyncRequest::SessionUpsert {
            session_id: session_id.to_string(),
            workspace_path: workspace_path.to_string(),
        });
    }
}

/// Non-blocking notification that a session was deleted. Called from
/// `delete_session` and `delete_persisted_session` Tauri commands. Sends a
/// tombstone to the relay so the deleted session is not re-imported.
pub fn notify_session_deleted(session_id: &str) {
    if let Some(tx) = SYNC_TX.get() {
        let _ = tx.send(SyncRequest::SessionDelete {
            session_id: session_id.to_string(),
        });
    }
}

/// Non-blocking notification that config was changed. Called from `set_config`.
/// Forwards to the shared settings sync engine (debounced + hash-deduped).
pub fn notify_settings_changed() {
    bitfun_core::service::remote_connect::settings_sync::notify_settings_changed();
}

/// Background loop: collects session sync requests, debounces 5 seconds,
/// then uploads. Settings push/pull is handled by the settings sync engine.
async fn sync_background_loop(mut rx: mpsc::UnboundedReceiver<SyncRequest>) {
    let debounce = Duration::from_secs(5);
    loop {
        // Wait for the next session sync request, then drain during the
        // debounce window.
        let Some(first) = rx.recv().await else {
            return;
        };
        let mut pending_upserts: HashMap<String, String> = HashMap::new();
        let mut pending_deletes: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        match first {
            SyncRequest::SessionUpsert {
                session_id,
                workspace_path,
            } => {
                pending_upserts.insert(session_id, workspace_path);
            }
            SyncRequest::SessionDelete { session_id } => {
                pending_deletes.insert(session_id);
            }
        }

        let deadline = tokio::time::sleep(debounce);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                Some(req) = rx.recv() => {
                    match req {
                        SyncRequest::SessionUpsert { session_id, workspace_path } => {
                            pending_deletes.remove(&session_id);
                            pending_upserts.insert(session_id, workspace_path);
                        }
                        SyncRequest::SessionDelete { session_id } => {
                            pending_upserts.remove(&session_id);
                            pending_deletes.insert(session_id);
                        }
                    }
                }
            }
        }

        execute_debounced_sync(pending_upserts, pending_deletes).await;
    }
}

/// Execute the debounced sync: upload changed sessions, tombstone deleted
/// sessions.
async fn execute_debounced_sync(
    upserts: HashMap<String, String>,
    deletes: std::collections::HashSet<String>,
) {
    if !background_account_sync_is_allowed() {
        log::debug!("Debounced sync skipped while account login awaits a settings choice");
        return;
    }
    if crate::api::peer_host_invoke::is_peer_controller_active() {
        log::debug!("Debounced sync skipped while peer controller mode is active");
        return;
    }
    let generation = account_context_generation();
    let Ok(_sync_guard) = lock_account_sync(generation).await else {
        return;
    };
    if !background_account_sync_is_allowed() {
        return;
    }
    // Need to be logged in
    let (acct_session, relay_url) = match read_account_context().await {
        Ok(ctx) => ctx,
        Err(_) => return, // not logged in — silently skip
    };
    let client = AccountClient::new();

    // Tombstone deleted sessions on the relay
    let mut sync_state_local = sync_state::load(&acct_session.user_id);
    for session_id in &deletes {
        if let Err(e) = client
            .delete_session(&relay_url, &acct_session, session_id)
            .await
        {
            if is_token_expired_error(&e) {
                TOKEN_EXPIRED.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            log::warn!("Auto-sync delete {session_id} failed: {e}");
        } else {
            sync_state_local.clear_uploaded_hash(session_id);
            log::debug!("Auto-synced tombstone for session {session_id}");
        }
    }

    // Upload changed sessions (hash-skip + concurrency)
    let upsert_list: Vec<(String, String)> = upserts.into_iter().collect();
    let upload_results: Vec<(String, Option<String>)> = stream::iter(upsert_list)
        .map(|(session_id, workspace_path)| {
            let client = AccountClient::new();
            let relay_url = relay_url.clone();
            let acct_session = acct_session.clone();
            let known_hash = sync_state_local
                .uploaded_hash(&session_id)
                .map(str::to_string);
            async move {
                match export_and_upload_session(
                    &client,
                    &acct_session,
                    &relay_url,
                    &session_id,
                    &workspace_path,
                    known_hash.as_deref(),
                )
                .await
                {
                    Ok(Some(hash)) => {
                        log::debug!("Auto-synced session {session_id}");
                        (session_id, Some(hash))
                    }
                    Ok(None) => {
                        log::debug!("Auto-sync skip unchanged session {session_id}");
                        (session_id, None)
                    }
                    Err(e) => {
                        if is_token_expired_error(&e) {
                            TOKEN_EXPIRED.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        log::warn!("Auto-sync session {session_id} failed: {e}");
                        (session_id, None)
                    }
                }
            }
        })
        .buffer_unordered(UPLOAD_CONCURRENCY)
        .collect()
        .await;

    for (session_id, hash) in upload_results {
        if let Some(hash) = hash {
            sync_state_local.set_uploaded_hash(&session_id, hash);
        }
    }
    let _ = sync_state::save(&acct_session.user_id, &sync_state_local);
}

/// Load a single session from disk, serialize to bundle, and upload.
/// Returns `Some(hash)` when a POST succeeded, `None` when content was unchanged.
async fn export_and_upload_session(
    client: &AccountClient,
    acct_session: &AccountSession,
    relay_url: &str,
    session_id: &str,
    workspace_path: &str,
    known_hash: Option<&str>,
) -> anyhow::Result<Option<String>> {
    // Resolve storage path — we need app_state for desktop_effective_session_storage_path
    // but in this background context we don't have it. Use the path_manager approach.
    let path_manager = std::sync::Arc::new(
        bitfun_core::infrastructure::PathManager::new()
            .map_err(|e| anyhow::anyhow!("create path manager: {e}"))?,
    );
    let storage_path =
        bitfun_core::service::remote_ssh::workspace_state::get_effective_session_path(
            workspace_path,
            None,
            None,
        )
        .await;

    let manager = PersistenceManager::new(path_manager)
        .map_err(|e| anyhow::anyhow!("create persistence manager: {e}"))?;

    let metadata = manager
        .load_session_metadata(&storage_path, session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;

    let turns = manager
        .load_session_turns(&storage_path, session_id)
        .await?;

    let metadata_json = serde_json::to_value(&metadata)?;
    let turns_json: Vec<serde_json::Value> = turns
        .iter()
        .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
        .collect();

    let bundle = SessionBundle {
        session_id: session_id.to_string(),
        metadata: metadata_json,
        turns: turns_json,
        source_device_id: None,
        source_device_name: None,
    };
    let bundle_json = serde_json::to_string(&bundle)?;
    let hash = sync_state::content_hash(&bundle_json);
    if known_hash == Some(hash.as_str()) {
        return Ok(None);
    }
    client
        .upload_session(relay_url, acct_session, session_id, &bundle_json)
        .await?;
    Ok(Some(hash))
}

/// Resolve the local workspace path for task execution. Returns the first
/// project directory found under ~/.bitfun/projects/, or "/" as fallback.
fn resolve_local_workspace_path() -> String {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
    let projects = home.join(".bitfun").join("projects");
    if let Ok(entries) = std::fs::read_dir(&projects) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                // Return the workspace slug dir path as a string
                return entry.path().to_string_lossy().to_string();
            }
        }
    }
    "/".to_string()
}

/// Execute a RemoteCommand locally (for RPC requests from other devices).
/// Returns the RemoteResponse serialized as JSON to be encrypted and sent back.
async fn execute_local_remote_command(
    cmd: &bitfun_core::service::remote_connect::remote_server::RemoteCommand,
) -> anyhow::Result<serde_json::Value> {
    use bitfun_core::service::remote_connect::remote_server::{RemoteCommand, RemoteResponse};

    match cmd {
        RemoteCommand::HostInvoke { command, args } => {
            // Control-plane peer attach/detach/ping can run without webview bridge.
            if command == "peer_control_attach" {
                let controller_id = args
                    .get("controllerDeviceId")
                    .or_else(|| args.get("controller_device_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                crate::api::peer_host_invoke::attach_controller(controller_id);
                return serde_json::to_value(RemoteResponse::HostInvokeResult {
                    ok: true,
                    value: Some(serde_json::json!({ "attached": true })),
                    error: None,
                })
                .map_err(|e| anyhow::anyhow!("serialize response: {e}"));
            }
            if command == "peer_control_detach" {
                let controller_id = args
                    .get("controllerDeviceId")
                    .or_else(|| args.get("controller_device_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let request_ids = crate::api::peer_host_invoke::detach_controller(controller_id);
                crate::api::peer_host_invoke::fail_closed_permission_requests(
                    request_ids,
                    "Last Peer controller detached",
                )
                .await
                .map_err(anyhow::Error::msg)?;
                return serde_json::to_value(RemoteResponse::HostInvokeResult {
                    ok: true,
                    value: Some(serde_json::json!({ "detached": true })),
                    error: None,
                })
                .map_err(|e| anyhow::anyhow!("serialize response: {e}"));
            }
            if command == "peer_mode_ping" {
                let value = crate::api::peer_host_invoke::peer_mode_ping()
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                return serde_json::to_value(RemoteResponse::HostInvokeResult {
                    ok: true,
                    value: Some(value),
                    error: None,
                })
                .map_err(|e| anyhow::anyhow!("serialize response: {e}"));
            }

            let result = crate::api::peer_host_invoke::dispatch(command, args.clone()).await;
            serde_json::to_value(RemoteResponse::HostInvokeResult {
                ok: result.ok,
                value: result.value,
                error: result.error,
            })
            .map_err(|e| anyhow::anyhow!("serialize response: {e}"))
        }
        RemoteCommand::DeviceEvent { event, payload } => {
            // Peer→controller events are handled on the controller; on peer this is a no-op ack.
            let _ = (event, payload);
            serde_json::to_value(RemoteResponse::DeviceEventAccepted)
                .map_err(|e| anyhow::anyhow!("serialize response: {e}"))
        }
        other => {
            // RemoteServer uses the global dispatcher internally — no need for
            // manual coordinator access. The dummy shared secret is irrelevant
            // because we call dispatch() directly (encryption is handled at the
            // RPC envelope level, not here).
            let server = bitfun_core::service::remote_connect::RemoteServer::new([0u8; 32]);
            let response = server.dispatch(other).await;
            serde_json::to_value(&response).map_err(|e| anyhow::anyhow!("serialize response: {e}"))
        }
    }
}

/// Import a SessionBundle JSON into local storage. Tries all workspace session
/// directories and writes to the first one found (or creates one if none exist).
async fn import_session_bundle(bundle_json: &str, account_generation: u64) -> anyhow::Result<()> {
    let _sync_guard = lock_account_sync(account_generation)
        .await
        .map_err(anyhow::Error::msg)?;
    // A queued event from a disconnected account must not write into a new
    // account's local session view even if its encrypted payload was already
    // received before the socket closed.
    read_account_context().await.map_err(anyhow::Error::msg)?;
    let bundle: SessionBundle = serde_json::from_str(bundle_json)?;

    let path_manager = std::sync::Arc::new(bitfun_core::infrastructure::PathManager::new()?);
    let manager = PersistenceManager::new(path_manager.clone())?;

    // Find the first workspace sessions dir that exists
    let projects_root = path_manager.projects_root();
    let entries = std::fs::read_dir(&projects_root)?;
    let mut target_dir: Option<std::path::PathBuf> = None;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let sessions = entry.path().join("sessions");
        if sessions.is_dir() {
            target_dir = Some(sessions);
            break;
        }
    }

    // If no workspace sessions dir exists, create one under a "synced" workspace
    let target_dir = target_dir.unwrap_or_else(|| {
        let dir = projects_root.join("synced").join("sessions");
        let _ = std::fs::create_dir_all(&dir);
        dir
    });

    let mut metadata: SessionMetadata = serde_json::from_value(bundle.metadata.clone())?;
    if metadata.session_id != bundle.session_id {
        return Err(anyhow::anyhow!(
            "relay session metadata identity does not match bundle"
        ));
    }

    // Only write metadata — turns are lazy-loaded when the user opens the
    // session. This keeps the import fast and avoids writing potentially
    // large turn data that may never be read.
    set_relay_turns_import_state(&mut metadata, RELAY_TURNS_IMPORT_PENDING);
    manager
        .create_session_metadata_if_absent(&target_dir, &metadata)
        .await
        .map_err(|e| anyhow::anyhow!("save metadata: {e}"))?;

    Ok(())
}

/// One-shot cloud settings pull, triggered when another same-account device
/// comes online. The periodic pull lives in the shared settings sync engine.
async fn pull_and_reconcile(account_generation: u64) {
    if !background_account_sync_is_allowed() {
        log::debug!("Pull: skip while account login awaits a settings choice");
        return;
    }
    if crate::api::peer_host_invoke::is_peer_controller_active() {
        log::debug!("Pull: skip while peer controller mode is active");
        return;
    }
    let Ok(_sync_guard) = lock_account_sync(account_generation).await else {
        return;
    };
    if !background_account_sync_is_allowed() {
        return;
    }
    let Ok((acct_session, relay_url)) = read_account_context().await else {
        return;
    };
    use bitfun_core::service::remote_connect::settings_sync;
    if let Err(e) = settings_sync::pull_and_apply_settings(&acct_session, &relay_url).await {
        if is_token_expired_error(&e) {
            TOKEN_EXPIRED.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        log::debug!("Pull: fetch_settings failed: {e}");
    }
}

#[cfg(test)]
mod sync_state_tests {
    use super::*;

    static ACCOUNT_CONTEXT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn relay_url_normalization_removes_all_trailing_slashes() {
        assert_eq!(
            normalize_relay_url("https://relay.example.com///").unwrap(),
            "https://relay.example.com"
        );
    }

    #[test]
    fn settings_probe_errors_are_not_treated_as_an_empty_cloud() {
        assert!(!cloud_settings_exist_from_probe::<u8, &str>(Ok(None)).unwrap());
        assert!(cloud_settings_exist_from_probe::<u8, &str>(Ok(Some(1))).unwrap());
        assert_eq!(
            cloud_settings_exist_from_probe::<u8, &str>(Err("relay unavailable")),
            Err("relay unavailable")
        );
    }

    #[test]
    fn device_rpc_timeout_is_bounded_for_peer_requests() {
        assert_eq!(
            account_device_rpc_timeout_ms(None),
            ACCOUNT_DEVICE_RPC_DEFAULT_TIMEOUT_MS
        );
        assert_eq!(account_device_rpc_timeout_ms(Some(10_000)), 10_000);
        assert_eq!(
            account_device_rpc_timeout_ms(Some(100)),
            ACCOUNT_DEVICE_RPC_MIN_TIMEOUT_MS
        );
        assert_eq!(
            account_device_rpc_timeout_ms(Some(u64::MAX)),
            ACCOUNT_DEVICE_RPC_DEFAULT_TIMEOUT_MS
        );
    }

    #[test]
    fn login_result_exposes_only_an_opaque_pending_owner() {
        let value = serde_json::to_value(AccountLoginResult {
            user_id: "user-a".to_string(),
            pending_login_id: Some("pending-a".to_string()),
            has_cloud_settings: true,
        })
        .unwrap();

        assert_eq!(value["pending_login_id"], "pending-a");
        assert!(value.get("token").is_none());
    }

    #[test]
    fn background_sync_is_fail_closed_while_login_choice_is_pending() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_pending_login_id(Some("pending-a".to_string()));
        assert!(!background_account_sync_is_allowed());
        set_pending_login_id(None);
        assert!(background_account_sync_is_allowed());
    }

    #[test]
    fn replaced_token_revocation_never_selects_the_published_credential() {
        let account = |token: &str, relay_url: &str| AccountContextState {
            session: AccountSession {
                token: token.to_string(),
                user_id: "user-a".to_string(),
                master_key: [7; 32],
            },
            relay_url: relay_url.to_string(),
        };

        assert!(select_replaced_account_for_revocation(
            Some(account("token-b", "https://relay.example.com")),
            "https://relay.example.com",
            "token-b",
        )
        .is_none());
        let same_account_old_token = select_replaced_account_for_revocation(
            Some(account("token-a", "https://relay.example.com")),
            "https://relay.example.com",
            "token-b",
        )
        .expect("same-account relogin must revoke the old token");
        assert_eq!(same_account_old_token.session.token, "token-a");
        let other_relay = select_replaced_account_for_revocation(
            Some(account("same-token", "https://relay-a.example.com")),
            "https://relay-b.example.com",
            "same-token",
        )
        .expect("the same token text on a different relay is a distinct credential");
        assert_eq!(other_relay.relay_url, "https://relay-a.example.com");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn account_transition_cancels_an_in_flight_auto_sync_future() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let operation_id = u64::MAX - 41;
        ACTIVE_ACCOUNT_AUTO_SYNC_OPERATION_ID.store(operation_id, Ordering::Release);
        let waiter = tokio::spawn(async move {
            await_account_auto_sync(operation_id, std::future::pending::<()>()).await
        });
        tokio::task::yield_now().await;

        let permit = AccountContextTransitionPermit::begin();
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sync cancellation should not wait for the network timeout")
            .expect("cancellation task should join");
        drop(permit);
        assert_eq!(result.unwrap_err(), "account sync cancelled");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn external_account_reads_are_hidden_during_transition() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *get_account_context().write().await = Some(AccountContextState {
            session: AccountSession {
                token: "token-a".to_string(),
                user_id: "user-a".to_string(),
                master_key: [7; 32],
            },
            relay_url: "https://relay.example.com".to_string(),
        });
        assert!(read_account_context().await.is_ok());

        let permit = AccountContextTransitionPermit::begin();
        assert!(read_account_context().await.is_err());
        assert!(read_account_context_raw().await.is_ok());
        drop(permit);

        *get_account_context().write().await = None;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_event_probe_sees_context_before_transition_mutex_is_released() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let transition_guard = ACCOUNT_CONTEXT_TRANSITION_LOCK.lock().await;
        let transition = AccountContextTransitionPermit::begin();
        let mut guard = AccountContextTransitionGuard {
            sync_guard: None,
            transition: Some(transition),
            transition_guard: Some(transition_guard),
        };

        assert!(!account_context_is_current(account_context_generation()));
        guard.make_context_observable();
        assert!(account_context_is_current(account_context_generation()));
        assert!(ACCOUNT_CONTEXT_TRANSITION_LOCK.try_lock().is_err());

        drop(guard);
        assert!(ACCOUNT_CONTEXT_TRANSITION_LOCK.try_lock().is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_pending_login_id_cannot_finalize_or_cancel_replacement() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *get_account_context().write().await = Some(AccountContextState {
            session: AccountSession {
                token: "token-b".to_string(),
                user_id: "user-b".to_string(),
                master_key: [9; 32],
            },
            relay_url: "https://relay.example.com".to_string(),
        });
        set_pending_login_id(Some("pending-b".to_string()));
        let generation_before = account_context_generation();

        assert!(lock_pending_login_for_finalize("pending-a").await.is_err());
        assert!(!account_cancel_pending_login(PendingAccountLoginRequest {
            pending_login_id: "pending-a".to_string(),
        })
        .await
        .unwrap());
        assert_eq!(account_context_generation(), generation_before);
        assert!(pending_login_is_owned_by("pending-b"));

        set_pending_login_id(None);
        assert!(!PENDING_SYNC_CHOICE.load(std::sync::atomic::Ordering::Acquire));
        assert!(!pending_login_is_owned_by("pending-b"));
        *get_account_context().write().await = None;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalize_retry_after_commit_is_idempotent_only_for_the_same_account_owner() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *get_account_context().write().await = Some(AccountContextState {
            session: AccountSession {
                token: "token-a".to_string(),
                user_id: "user-a".to_string(),
                master_key: [7; 32],
            },
            relay_url: "https://relay.example.com".to_string(),
        });
        let generation = account_context_generation();
        record_finalized_pending_login(FinalizedPendingLoginOwner {
            pending_login_id: "pending-a".to_string(),
            account_generation: generation,
            account_token: "token-a".to_string(),
        });

        assert!(finalized_pending_login_is_current("pending-a").await);
        assert!(!finalized_pending_login_is_current("pending-b").await);

        let transition = AccountContextTransitionPermit::begin();
        assert!(!finalized_pending_login_is_current("pending-a").await);
        drop(transition);
        *get_account_context().write().await = None;
        clear_last_finalized_pending_login();
    }

    #[test]
    fn stale_routing_owner_cannot_clear_or_update_replacement() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_device_routing_state();
        let owner_a = DeviceRoutingOwner {
            account_generation: 10,
            account_token: "token-a".to_string(),
            connection_id: 1,
            service_connection_id: 101,
        };
        let owner_b = DeviceRoutingOwner {
            account_generation: 12,
            account_token: "token-b".to_string(),
            connection_id: 2,
            service_connection_id: 102,
        };
        install_device_routing_owner(owner_a.clone());
        assert!(replace_device_presence_if_owner(
            &owner_a,
            vec![OnlineDeviceInfo {
                device_id: "a-device".to_string(),
                device_name: "A".to_string(),
            }],
        ));

        install_device_routing_owner(owner_b.clone());
        assert!(!replace_device_presence_if_owner(
            &owner_a,
            vec![OnlineDeviceInfo {
                device_id: "late-a-device".to_string(),
                device_name: "Late A".to_string(),
            }],
        ));
        assert!(!clear_device_routing_if_owner(&owner_a));
        assert_eq!(
            device_presence_for_account(owner_b.account_generation, &owner_b.account_token),
            Some(Vec::new())
        );
        assert!(clear_device_routing_if_owner(&owner_b));
    }

    #[test]
    fn routing_presence_is_bound_to_account_generation_and_token() {
        let _test_guard = ACCOUNT_CONTEXT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_device_routing_state();
        let owner = DeviceRoutingOwner {
            account_generation: 20,
            account_token: "token-current".to_string(),
            connection_id: 3,
            service_connection_id: 103,
        };
        install_device_routing_owner(owner.clone());
        assert!(replace_device_presence_if_owner(
            &owner,
            vec![OnlineDeviceInfo {
                device_id: "current-device".to_string(),
                device_name: "Current".to_string(),
            }],
        ));

        assert!(device_presence_for_account(20, "token-current").is_some());
        assert!(device_presence_for_account(21, "token-current").is_none());
        assert!(device_presence_for_account(20, "token-replaced").is_none());
        clear_device_routing_state();
    }

    #[test]
    fn partial_session_backup_is_not_reported_as_success() {
        assert!(ensure_session_backup_complete(3, 3, &[]).is_ok());
        let error = ensure_session_backup_complete(
            3,
            2,
            &["s1: relay returned HTTP 507 Insufficient Storage".into()],
        )
        .unwrap_err();
        assert!(error.contains("uploaded 2 of 3"));
        assert!(error.contains("HTTP 507"));
    }

    #[test]
    fn content_hash_is_stable() {
        let a = sync_state::content_hash(r#"{"session_id":"x"}"#);
        let b = sync_state::content_hash(r#"{"session_id":"x"}"#);
        let c = sync_state::content_hash(r#"{"session_id":"y"}"#);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn advance_session_since_takes_max() {
        let mut state = sync_state::AccountSyncState::default();
        state.advance_session_since([1, 5, 3]);
        assert_eq!(state.last_session_since, 5);
        state.advance_session_since([4]);
        assert_eq!(state.last_session_since, 5);
        state.advance_session_since([9]);
        assert_eq!(state.last_session_since, 9);
    }

    #[test]
    fn relay_turn_import_requires_an_explicit_complete_marker_and_exact_count() {
        let mut metadata = SessionMetadata::new(
            "session".to_string(),
            "Session".to_string(),
            "agentic".to_string(),
            "auto".to_string(),
        );
        metadata.turn_count = 2;

        assert_eq!(relay_turns_import_state(&metadata), None);
        assert!(!relay_turns_import_is_complete(&metadata, 1));
        assert!(!relay_turns_import_is_complete(&metadata, 2));
        set_relay_turns_import_state(&mut metadata, RELAY_TURNS_IMPORT_PENDING);
        assert_eq!(
            relay_turns_import_state(&metadata),
            Some(RELAY_TURNS_IMPORT_PENDING)
        );
        assert!(!relay_turns_import_is_complete(&metadata, 2));
        mark_relay_turns_import_complete(&mut metadata);
        assert!(!relay_turns_import_is_complete(&metadata, 1));
        assert!(relay_turns_import_is_complete(&metadata, 2));
    }
}

#[cfg(test)]
mod peer_event_tests {
    use super::should_fanout_peer_ui_event;

    #[test]
    fn permission_events_are_fanned_out_to_peer_controllers() {
        assert!(should_fanout_peer_ui_event("permission://event"));
        assert!(!should_fanout_peer_ui_event("permission://internal"));
    }
}
