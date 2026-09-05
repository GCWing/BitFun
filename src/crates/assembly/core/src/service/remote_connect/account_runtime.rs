//! Shared account runtime owner for product Hosts.
//!
//! The runtime owns account identity transitions, persisted credentials,
//! settings synchronization, and account-backed Session backup. Product Hosts
//! inject device-routing and background-owner lifecycle effects without
//! exposing App Server wire DTOs to this owner.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, MutexGuard, Notify, RwLock};

use openbitfun_services_integrations::remote_connect::account::{
    ensure_relay_session_history_exportable, relay_session_export_metadata, AccountClient,
    AccountSession,
};
use openbitfun_services_integrations::remote_connect::{session_store, sync_state, DeviceIdentity};

use super::{settings_sync, validate_relay_base_url};

const UPLOAD_CONCURRENCY_CHUNK: usize = 5;

#[derive(Debug, Clone)]
struct AccountContextState {
    session: AccountSession,
    relay_url: String,
}

#[derive(Debug, Clone)]
pub struct AccountRoutingStartRequest {
    pub session: AccountSession,
    pub relay_url: String,
    pub device_name: String,
    pub account_generation: u64,
}

#[derive(Debug)]
pub struct BackgroundRoutingOwnerRetirementError {
    pub error: anyhow::Error,
    pub owner_may_exit: bool,
}

#[async_trait]
pub trait AccountRuntimeHost: Send + Sync {
    async fn retire_background_routing_owner(
        &self,
    ) -> std::result::Result<bool, BackgroundRoutingOwnerRetirementError>;

    fn background_routing_owner_is_running(&self) -> bool;

    fn request_background_routing_owner_shutdown(&self) -> bool;

    async fn start_device_routing(&self, request: AccountRoutingStartRequest) -> Result<()>;

    async fn stop_device_routing(&self);

    fn notify_controllers_settings_changed(&self);
}

#[derive(Debug, Clone)]
pub struct AccountSessionBackup {
    pub session_id: String,
    pub metadata: serde_json::Value,
    pub turns: Vec<serde_json::Value>,
}

#[async_trait]
pub trait AccountSessionBackupPort: Send + Sync {
    async fn list_session_backups(
        &self,
        workspace_path: &Path,
    ) -> Result<Vec<AccountSessionBackup>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomaticAccountSyncPolicy {
    pub background_engine: bool,
    pub management_push: bool,
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

#[derive(Debug, Clone)]
pub struct AccountLoginResult {
    pub user_id: String,
    pub relay_url: String,
    pub has_cloud_settings: bool,
    pub routing_owner_replaced: bool,
    pub routing_connected: bool,
    pub routing_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub user_id: String,
    pub relay_url: String,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Debug, Clone)]
pub struct AccountDevice {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountSyncStatus {
    #[default]
    Idle,
    Syncing,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct AccountSyncProgress {
    pub operation_id: Option<String>,
    pub status: AccountSyncStatus,
    pub phase: String,
    pub percent: u8,
    pub current: Option<usize>,
    pub total: Option<usize>,
    pub detail: Option<String>,
    pub error: Option<String>,
    pub settings_synced: bool,
    pub sessions_exported: usize,
}

impl Default for AccountSyncProgress {
    fn default() -> Self {
        Self {
            operation_id: None,
            status: AccountSyncStatus::Idle,
            phase: String::new(),
            percent: 0,
            current: None,
            total: None,
            detail: None,
            error: None,
            settings_synced: false,
            sessions_exported: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
    pub logged_in: bool,
    pub pending_sync_choice: bool,
    pub info: Option<AccountInfo>,
    pub devices: Vec<AccountDevice>,
    pub sync: AccountSyncProgress,
}

#[derive(Debug, Clone)]
struct AutoSyncResult {
    settings_synced: bool,
    sessions_exported: usize,
}

#[derive(Serialize, Deserialize)]
struct SessionBundle {
    session_id: String,
    metadata: serde_json::Value,
    turns: Vec<serde_json::Value>,
    source_device_id: Option<String>,
    source_device_name: Option<String>,
}

pub struct AccountRuntime {
    host: Arc<dyn AccountRuntimeHost>,
    session_backup: Arc<dyn AccountSessionBackupPort>,
    account_context: RwLock<Option<AccountContextState>>,
    account_context_generation: AtomicU64,
    account_context_transitions: AtomicUsize,
    account_sync_lock: Mutex<()>,
    account_login_lock: Mutex<()>,
    account_context_transition_lock: Mutex<()>,
    account_sync_cancel: Notify,
    routing_recovery_generation: AtomicU64,
    token_expired: AtomicBool,
    pending_sync_choice: AtomicBool,
    sync_progress: RwLock<AccountSyncProgress>,
    auto_sync_in_flight: AtomicBool,
}

impl AccountRuntime {
    pub fn new(
        host: Arc<dyn AccountRuntimeHost>,
        session_backup: Arc<dyn AccountSessionBackupPort>,
    ) -> Arc<Self> {
        Arc::new(Self {
            host,
            session_backup,
            account_context: RwLock::new(None),
            account_context_generation: AtomicU64::new(1),
            account_context_transitions: AtomicUsize::new(0),
            account_sync_lock: Mutex::new(()),
            account_login_lock: Mutex::new(()),
            account_context_transition_lock: Mutex::new(()),
            account_sync_cancel: Notify::new(),
            routing_recovery_generation: AtomicU64::new(0),
            token_expired: AtomicBool::new(false),
            pending_sync_choice: AtomicBool::new(false),
            sync_progress: RwLock::new(AccountSyncProgress::default()),
            auto_sync_in_flight: AtomicBool::new(false),
        })
    }

    pub fn account_context_generation(&self) -> u64 {
        self.account_context_generation.load(Ordering::Acquire)
    }

    pub fn account_context_is_current(&self, generation: u64) -> bool {
        self.account_context_transitions.load(Ordering::Acquire) == 0
            && self.account_context_generation() == generation
    }

    pub fn automatic_account_sync_policy(&self) -> AutomaticAccountSyncPolicy {
        automatic_account_sync_policy_for_pending(self.pending_sync_choice.load(Ordering::Acquire))
    }

    pub fn pending_sync_choice(&self) -> bool {
        self.pending_sync_choice.load(Ordering::Acquire)
    }

    pub fn is_token_expired(&self) -> bool {
        self.token_expired.load(Ordering::Relaxed)
    }

    pub fn mark_token_expired(&self) {
        self.token_expired.store(true, Ordering::Relaxed);
    }

    async fn lock_account_sync(&self, generation: u64) -> Result<MutexGuard<'_, ()>> {
        let guard = self.account_sync_lock.lock().await;
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account sync cancelled"));
        }
        Ok(guard)
    }

    async fn await_account_sync_current<F, T>(&self, generation: u64, future: F) -> Result<T>
    where
        F: Future<Output = T>,
    {
        let mut cancelled = Box::pin(self.account_sync_cancel.notified());
        cancelled.as_mut().enable();
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account sync cancelled"));
        }
        tokio::select! {
            _ = &mut cancelled => Err(anyhow!("account sync cancelled")),
            result = future => {
                if !self.account_context_is_current(generation) {
                    Err(anyhow!("account sync cancelled"))
                } else {
                    Ok(result)
                }
            }
        }
    }

    async fn begin_account_transition(&self) -> AccountContextTransitionGuard<'_> {
        let transition_guard = self.account_context_transition_lock.lock().await;
        self.account_context_transitions
            .fetch_add(1, Ordering::AcqRel);
        self.account_context_generation
            .fetch_add(1, Ordering::AcqRel);
        self.account_sync_cancel.notify_waiters();
        let sync_guard = self.account_sync_lock.lock().await;
        settings_sync::wait_for_sync_operations_idle().await;
        AccountContextTransitionGuard {
            runtime: self,
            sync_guard: Some(sync_guard),
            transition_guard: Some(transition_guard),
            active: true,
        }
    }

    async fn begin_account_transition_if_current(
        &self,
        expected_generation: u64,
    ) -> Option<AccountContextTransitionGuard<'_>> {
        let transition_guard = self.account_context_transition_lock.lock().await;
        if !self.account_context_is_current(expected_generation) {
            return None;
        }
        self.account_context_transitions
            .fetch_add(1, Ordering::AcqRel);
        self.account_context_generation
            .fetch_add(1, Ordering::AcqRel);
        self.account_sync_cancel.notify_waiters();
        let sync_guard = self.account_sync_lock.lock().await;
        settings_sync::wait_for_sync_operations_idle().await;
        Some(AccountContextTransitionGuard {
            runtime: self,
            sync_guard: Some(sync_guard),
            transition_guard: Some(transition_guard),
            active: true,
        })
    }

    async fn read_account_context_raw(&self) -> Result<(AccountSession, String)> {
        self.account_context
            .read()
            .await
            .clone()
            .map(|context| (context.session, context.relay_url))
            .ok_or_else(|| anyhow!("not logged in"))
    }

    pub async fn read_account_context(&self) -> Result<(AccountSession, String)> {
        let generation = self.account_context_generation();
        self.read_account_context_for_generation(generation).await
    }

    pub async fn read_account_context_for_generation(
        &self,
        generation: u64,
    ) -> Result<(AccountSession, String)> {
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account context changed"));
        }
        let context = self.read_account_context_raw().await?;
        if !self.account_context_is_current(generation) {
            return Err(anyhow!("account context changed"));
        }
        Ok(context)
    }

    pub async fn is_logged_in(&self) -> bool {
        if self.pending_sync_choice.load(Ordering::Acquire) {
            return false;
        }
        self.read_account_context().await.is_ok()
    }

    pub async fn try_restore_session(&self) -> Option<String> {
        let transition = self.begin_account_transition().await;
        self.host.stop_device_routing().await;
        let restored = match session_store::load_session_detailed() {
            Ok(Some(loaded)) => {
                let relay_url = match normalize_relay_url(&loaded.relay_url) {
                    Ok(url) => url,
                    Err(error) => {
                        log::warn!("Ignoring invalid persisted relay URL: {error}");
                        session_store::clear_session();
                        transition.finish();
                        return None;
                    }
                };
                let user_id = loaded.user_id.clone();
                if let Some(device_id) = loaded.device_id.as_deref() {
                    if let Err(error) = DeviceIdentity::adopt_account_device_id(device_id) {
                        log::warn!("Failed to adopt restored session device_id: {error}");
                    }
                }
                let session = AccountSession {
                    token: loaded.token,
                    user_id: user_id.clone(),
                    master_key: loaded.master_key,
                };
                *self.account_context.write().await =
                    Some(AccountContextState { session, relay_url });
                log::info!("Restored account session for user {user_id}");
                Some(user_id)
            }
            Ok(None) => None,
            Err(error) => {
                log::warn!("Failed to load persisted session: {error}");
                None
            }
        };
        transition.finish();
        restored
    }

    pub async fn login_with_credentials(
        self: &Arc<Self>,
        relay_url: &str,
        username: &str,
        password: &str,
    ) -> Result<AccountLoginResult> {
        let _login_guard = self.account_login_lock.lock().await;
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
        let expected_generation = self.account_context_generation();
        if !self.account_context_is_current(expected_generation) {
            return Err(anyhow!("account context changed"));
        }

        let device = current_device_identity()?;
        let client = AccountClient::new();
        let session = client
            .login(&relay_url, username, password, &device)
            .await
            .map_err(|error| anyhow!("login failed: {error}"))?;
        let has_cloud_settings =
            match resolve_cloud_settings_probe(client.fetch_settings(&relay_url, &session).await) {
                Ok(value) => value,
                Err(error) => {
                    revoke_rejected_login_candidate(&client, &relay_url, &session).await;
                    return Err(error);
                }
            };

        let previous_account_context = self.account_context.read().await.clone();
        let retired_background_owner = match self.host.retire_background_routing_owner().await {
            Ok(retired) => retired,
            Err(failure) => {
                if failure.owner_may_exit {
                    self.schedule_routing_recovery_after_background_owner_exit(
                        expected_generation,
                        device.device_name.clone(),
                    );
                }
                revoke_rejected_login_candidate(&client, &relay_url, &session).await;
                return Err(failure.error);
            }
        };
        let Some(transition) = self
            .begin_account_transition_if_current(expected_generation)
            .await
        else {
            revoke_rejected_login_candidate(&client, &relay_url, &session).await;
            return Err(anyhow!("account context changed"));
        };
        self.host.stop_device_routing().await;
        session_store::clear_session();

        let user_id = session.user_id.clone();
        let token = session.token.clone();
        let master_key = session.master_key;
        *self.account_context.write().await = Some(AccountContextState {
            session: session.clone(),
            relay_url: relay_url.clone(),
        });
        session_store::save_credential_hint(username, &relay_url);
        self.token_expired.store(false, Ordering::Relaxed);

        if has_cloud_settings {
            self.pending_sync_choice.store(true, Ordering::Release);
            transition.finish();
            revoke_replaced_account_context(&client, previous_account_context, &relay_url, &token)
                .await;
            return Ok(AccountLoginResult {
                user_id,
                relay_url,
                has_cloud_settings,
                routing_owner_replaced: retired_background_owner,
                routing_connected: false,
                routing_error: None,
            });
        }

        self.pending_sync_choice.store(false, Ordering::Release);
        if let Err(error) = session_store::save_session_with_device(
            &token,
            &user_id,
            &master_key,
            &relay_url,
            Some(device.device_id.as_str()),
        ) {
            log::warn!("Failed to persist session: {error}");
        }
        let generation = transition.finish();
        let routing = self
            .host
            .start_device_routing(AccountRoutingStartRequest {
                session,
                relay_url: relay_url.clone(),
                device_name: device.device_name,
                account_generation: generation,
            })
            .await;
        revoke_replaced_account_context(&client, previous_account_context, &relay_url, &token)
            .await;

        Ok(AccountLoginResult {
            user_id,
            relay_url,
            has_cloud_settings,
            routing_owner_replaced: retired_background_owner,
            routing_connected: routing.is_ok(),
            routing_error: routing.err().map(|error| error.to_string()),
        })
    }

    pub async fn finalize_login_after_sync_choice(self: &Arc<Self>) -> Result<()> {
        let generation = self.account_context_generation();
        let sync_guard = self.lock_account_sync(generation).await?;
        let device = current_device_identity()?;
        let (session, relay_url) = self.read_account_context().await?;
        let retired_background_owner = self
            .host
            .retire_background_routing_owner()
            .await
            .map_err(|failure| failure.error)?;
        session_store::save_session_with_device(
            &session.token,
            &session.user_id,
            &session.master_key,
            &relay_url,
            Some(device.device_id.as_str()),
        )
        .map_err(|error| anyhow!("persist session: {error}"))?;
        self.pending_sync_choice.store(false, Ordering::Release);
        if retired_background_owner {
            log::info!("Stopped the previous background account routing owner");
        }
        drop(sync_guard);
        self.host
            .start_device_routing(AccountRoutingStartRequest {
                session,
                relay_url,
                device_name: device.device_name,
                account_generation: generation,
            })
            .await
            .map_err(|error| anyhow!("device routing failed: {error}"))
    }

    pub async fn restore_device_routing(self: &Arc<Self>, device_name: &str) -> Result<()> {
        let generation = self.account_context_generation();
        let (session, relay_url) = self.read_account_context_for_generation(generation).await?;
        self.host
            .start_device_routing(AccountRoutingStartRequest {
                session,
                relay_url,
                device_name: device_name.to_string(),
                account_generation: generation,
            })
            .await
    }

    pub async fn logout(&self) -> Result<()> {
        let transition = self.begin_account_transition().await;
        self.host.stop_device_routing().await;
        if self.host.request_background_routing_owner_shutdown() {
            log::info!("Signalled the background account routing owner to shut down");
        }
        if let Ok((session, relay_url)) = self.read_account_context_raw().await {
            let _ = AccountClient::new()
                .revoke_token(&relay_url, &session)
                .await;
        }
        *self.account_context.write().await = None;
        self.pending_sync_choice.store(false, Ordering::Release);
        session_store::clear_session();
        session_store::clear_credential_hint();
        self.token_expired.store(false, Ordering::Relaxed);
        transition.finish();
        Ok(())
    }

    pub async fn expire_rejected_context(
        &self,
        account_generation: u64,
        expected_token: &str,
    ) -> bool {
        let Some(transition) = self
            .begin_account_transition_if_current(account_generation)
            .await
        else {
            return false;
        };
        self.host.stop_device_routing().await;
        let mut context = self.account_context.write().await;
        if context
            .as_ref()
            .is_none_or(|context| context.session.token != expected_token)
        {
            transition.finish();
            return false;
        }
        *context = None;
        drop(context);
        self.token_expired.store(true, Ordering::Relaxed);
        self.pending_sync_choice.store(false, Ordering::Release);
        session_store::clear_session();
        transition.finish();
        true
    }

    pub async fn account_info(&self) -> Result<AccountInfo> {
        let (session, relay_url) = self.read_account_context().await?;
        let device = current_device_identity()?;
        Ok(AccountInfo {
            user_id: session.user_id,
            relay_url,
            device_id: device.device_id,
            device_name: device.device_name,
        })
    }

    pub async fn list_devices(&self) -> Result<Vec<AccountDevice>> {
        let (session, relay_url) = self.read_account_context().await?;
        let devices = AccountClient::new()
            .list_devices(&relay_url, &session)
            .await?;
        Ok(devices
            .into_iter()
            .map(|device| AccountDevice {
                device_id: device.device_id,
                device_name: device.device_name,
                online: device.online,
            })
            .collect())
    }

    pub async fn snapshot(&self) -> AccountSnapshot {
        let logged_in = self.is_logged_in().await;
        let info = if logged_in {
            self.account_info().await.ok()
        } else {
            None
        };
        let devices = if logged_in {
            self.list_devices().await.unwrap_or_default()
        } else {
            Vec::new()
        };
        AccountSnapshot {
            logged_in,
            pending_sync_choice: self.pending_sync_choice(),
            info,
            devices,
            sync: self.current_sync_progress().await,
        }
    }

    pub fn start_settings_sync_loop(self: &Arc<Self>) {
        let weak_runtime = Arc::downgrade(self);
        let context_runtime = weak_runtime.clone();
        let current_runtime = weak_runtime.clone();
        let settings_runtime = weak_runtime.clone();
        let pushed_runtime = weak_runtime.clone();
        let expired_runtime = weak_runtime;
        settings_sync::start_settings_sync_engine(settings_sync::SettingsSyncHooks {
            account_context: Some(Arc::new(move || {
                let runtime = context_runtime.clone();
                Box::pin(async move {
                    let runtime = runtime
                        .upgrade()
                        .ok_or_else(|| anyhow!("account runtime stopped"))?;
                    if !runtime.automatic_account_sync_policy().background_engine {
                        return Err(anyhow!("account login is awaiting a sync choice"));
                    }
                    let generation = runtime.account_context_generation();
                    let (account, relay_url) = runtime
                        .read_account_context_for_generation(generation)
                        .await?;
                    if !runtime.automatic_account_sync_policy().background_engine {
                        return Err(anyhow!("account login is awaiting a sync choice"));
                    }
                    Ok((account, relay_url, generation))
                })
            })),
            is_account_context_current: Some(Arc::new(move |generation| {
                current_runtime
                    .upgrade()
                    .is_some_and(|runtime| runtime.account_context_is_current(generation))
            })),
            on_settings_applied: Some(Arc::new(move || {
                if let Some(runtime) = settings_runtime.upgrade() {
                    runtime.host.notify_controllers_settings_changed();
                }
            })),
            on_settings_pushed: Some(Arc::new(move || {
                if let Some(runtime) = pushed_runtime.upgrade() {
                    runtime.host.notify_controllers_settings_changed();
                }
            })),
            on_token_expired: Some(Arc::new(move || {
                if let Some(runtime) = expired_runtime.upgrade() {
                    runtime.mark_token_expired();
                }
            })),
            ..Default::default()
        });
    }

    pub fn notify_local_settings_changed(&self) {
        settings_sync::notify_settings_changed();
    }

    pub async fn push_settings_after_local_change(&self) {
        if !self.automatic_account_sync_policy().management_push {
            return;
        }
        if self.read_account_context().await.is_err() {
            self.try_restore_session().await;
        }
        let generation = self.account_context_generation();
        let Ok(_sync_guard) = self.lock_account_sync(generation).await else {
            return;
        };
        if !self.automatic_account_sync_policy().management_push {
            return;
        }
        let Ok((account, relay_url)) = self.read_account_context().await else {
            return;
        };
        match settings_sync::push_settings_now(&account, &relay_url).await {
            Ok(true) => log::info!("Settings pushed to account cloud"),
            Ok(false) => {}
            Err(error) => log::warn!("Settings push failed: {error}"),
        }
    }

    pub async fn current_sync_progress(&self) -> AccountSyncProgress {
        self.sync_progress.read().await.clone()
    }

    async fn set_progress(&self, mut update: impl FnMut(&mut AccountSyncProgress)) {
        let mut progress = self.sync_progress.write().await;
        update(&mut progress);
    }

    async fn emit_progress(
        &self,
        phase: &str,
        percent: u8,
        current: Option<usize>,
        total: Option<usize>,
        detail: Option<&str>,
    ) {
        self.set_progress(|progress| {
            progress.status = AccountSyncStatus::Syncing;
            progress.phase = phase.to_string();
            progress.percent = percent;
            progress.current = current;
            progress.total = total;
            progress.detail = detail.map(str::to_string);
            progress.error = None;
        })
        .await;
    }

    pub async fn start_auto_sync_background(
        self: &Arc<Self>,
        operation_id: String,
        is_first_login: bool,
        workspace_path: PathBuf,
    ) -> bool {
        if self.auto_sync_in_flight.swap(true, Ordering::SeqCst) {
            log::warn!("Account auto-sync already in flight; skipping duplicate start");
            return false;
        }
        self.set_progress(|progress| {
            progress.operation_id = Some(operation_id.clone());
        })
        .await;
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            let result = runtime.run_auto_sync(is_first_login, &workspace_path).await;
            runtime.auto_sync_in_flight.store(false, Ordering::SeqCst);
            match result {
                Ok(result) => {
                    runtime
                        .set_progress(|progress| {
                            if progress.operation_id.as_deref() != Some(operation_id.as_str()) {
                                return;
                            }
                            if progress.status == AccountSyncStatus::Cancelled {
                                return;
                            }
                            progress.status = AccountSyncStatus::Done;
                            progress.phase = "done".to_string();
                            progress.percent = 100;
                            progress.settings_synced = result.settings_synced;
                            progress.sessions_exported = result.sessions_exported;
                            progress.error = None;
                        })
                        .await;
                }
                Err(error) => {
                    runtime
                        .set_progress(|progress| {
                            if progress.operation_id.as_deref() != Some(operation_id.as_str()) {
                                return;
                            }
                            if progress.status == AccountSyncStatus::Cancelled {
                                return;
                            }
                            progress.status = AccountSyncStatus::Failed;
                            progress.error = Some(error.to_string());
                        })
                        .await;
                    log::warn!("Account auto-sync failed: {error}");
                }
            }
        });
        true
    }

    pub async fn mark_sync_cancelled(&self, operation_id: String) {
        self.set_progress(|progress| {
            progress.operation_id = Some(operation_id.clone());
            progress.status = AccountSyncStatus::Cancelled;
            progress.phase = "cancelled".to_string();
            progress.error = None;
        })
        .await;
    }

    pub async fn cancel_sync(&self, operation_id: String) -> Result<AccountSyncProgress> {
        self.logout().await?;
        self.mark_sync_cancelled(operation_id).await;
        Ok(self.current_sync_progress().await)
    }

    async fn run_auto_sync(
        self: &Arc<Self>,
        is_first_login: bool,
        workspace_path: &Path,
    ) -> Result<AutoSyncResult> {
        let generation = self.account_context_generation();
        let _sync_guard = self.lock_account_sync(generation).await?;
        self.set_progress(|progress| {
            *progress = AccountSyncProgress {
                operation_id: progress.operation_id.clone(),
                status: AccountSyncStatus::Syncing,
                phase: "starting".to_string(),
                percent: 1,
                ..AccountSyncProgress::default()
            };
        })
        .await;

        let (account, relay_url) = self.read_account_context().await?;
        let client = AccountClient::new();
        let settings_synced = if is_first_login {
            self.emit_progress("uploading_settings", 5, None, None, None)
                .await;
            let config_service = crate::service::config::get_global_config_service()
                .await
                .map_err(|error| anyhow!("config service: {error}"))?;
            let exported = config_service
                .export_config()
                .await
                .map_err(|error| anyhow!("export config: {error}"))?;
            let config_json = serde_json::to_string(&exported)
                .map_err(|error| anyhow!("serialize config: {error}"))?;
            self.await_account_sync_current(
                generation,
                settings_sync::upload_settings_payload(&account, &relay_url, &config_json),
            )
            .await??;
            self.emit_progress("settings_done", 15, None, None, None)
                .await;
            true
        } else {
            self.emit_progress("downloading_settings", 5, None, None, None)
                .await;
            let cloud = self
                .await_account_sync_current(
                    generation,
                    client.fetch_settings_with_version(&relay_url, &account),
                )
                .await??;
            if let Some(blob) = cloud {
                self.emit_progress("applying_settings", 10, None, None, None)
                    .await;
                self.await_account_sync_current(
                    generation,
                    settings_sync::apply_settings_blob(&account, &blob, true),
                )
                .await??;
                self.emit_progress("settings_done", 15, None, None, None)
                    .await;
                true
            } else {
                self.emit_progress("settings_done", 15, None, None, None)
                    .await;
                false
            }
        };

        self.emit_progress("listing_sessions", 18, None, None, None)
            .await;
        let local_sessions = self
            .await_account_sync_current(
                generation,
                self.session_backup.list_session_backups(workspace_path),
            )
            .await??;
        self.emit_progress(
            "exporting_sessions",
            20,
            Some(0),
            Some(local_sessions.len()),
            None,
        )
        .await;

        let mut local_sync_state = sync_state::load(&account.user_id);
        let mut pending_uploads = Vec::new();
        for backup in local_sessions {
            if !self.account_context_is_current(generation) {
                return Err(anyhow!("account sync cancelled"));
            }
            let bundle = SessionBundle {
                session_id: backup.session_id.clone(),
                metadata: backup.metadata,
                turns: backup.turns,
                source_device_id: None,
                source_device_name: None,
            };
            let bundle_json = serde_json::to_string(&bundle)
                .map_err(|error| anyhow!("serialize bundle: {error}"))?;
            let hash = sync_state::content_hash(&bundle_json);
            if local_sync_state.uploaded_hash(&backup.session_id) == Some(hash.as_str()) {
                continue;
            }
            pending_uploads.push((backup.session_id, bundle_json, hash));
        }

        let upload_total = pending_uploads.len();
        self.emit_progress("exporting_sessions", 20, Some(0), Some(upload_total), None)
            .await;
        let mut uploaded = Vec::new();
        let mut upload_errors = Vec::new();
        for chunk in pending_uploads.chunks(UPLOAD_CONCURRENCY_CHUNK) {
            let mut handles = Vec::new();
            for (session_id, bundle_json, hash) in chunk {
                let runtime = Arc::clone(self);
                let client = AccountClient::new();
                let relay_url = relay_url.clone();
                let account = account.clone();
                let session_id = session_id.clone();
                let bundle_json = bundle_json.clone();
                let hash = hash.clone();
                handles.push(tokio::spawn(async move {
                    let result = runtime
                        .await_account_sync_current(
                            generation,
                            client.upload_session(&relay_url, &account, &session_id, &bundle_json),
                        )
                        .await;
                    (session_id, hash, result)
                }));
            }
            for handle in handles {
                match handle.await {
                    Ok((session_id, hash, Ok(Ok(version)))) => {
                        uploaded.push((session_id.clone(), hash, version));
                        let done = uploaded.len();
                        let percent = if upload_total == 0 {
                            95
                        } else {
                            20 + ((75 * done) / upload_total) as u8
                        };
                        self.emit_progress(
                            "exporting_sessions",
                            percent.min(95),
                            Some(done),
                            Some(upload_total),
                            Some(&session_id),
                        )
                        .await;
                    }
                    Ok((session_id, _, Ok(Err(error)))) => {
                        log::warn!("Auto-sync upload {session_id} failed: {error}");
                        upload_errors.push(format!("{session_id}: {error}"));
                    }
                    Ok((_, _, Err(error))) => return Err(error),
                    Err(error) => {
                        log::warn!("Auto-sync upload task join failed: {error}");
                        upload_errors.push(format!("upload task join failed: {error}"));
                    }
                }
            }
            if !self.account_context_is_current(generation) {
                return Err(anyhow!("account sync cancelled"));
            }
        }

        let exported = uploaded.len();
        let mut max_uploaded_version = local_sync_state.last_session_since;
        for (session_id, hash, version) in uploaded {
            local_sync_state.set_uploaded_hash(&session_id, hash);
            max_uploaded_version = max_uploaded_version.max(version);
        }
        if max_uploaded_version > local_sync_state.last_session_since {
            local_sync_state.last_session_since = max_uploaded_version;
        }
        let _ = sync_state::save(&account.user_id, &local_sync_state);
        ensure_session_backup_complete(upload_total, exported, &upload_errors)?;
        log::info!("Auto-sync: settings={settings_synced} exported={exported} imported=0");
        self.emit_progress("done", 100, Some(exported), Some(0), None)
            .await;
        Ok(AutoSyncResult {
            settings_synced,
            sessions_exported: exported,
        })
    }

    fn schedule_routing_recovery_after_background_owner_exit(
        self: &Arc<Self>,
        expected_generation: u64,
        device_name: String,
    ) {
        if !self.account_context_is_current(expected_generation)
            || self
                .routing_recovery_generation
                .swap(expected_generation, Ordering::AcqRel)
                == expected_generation
        {
            return;
        }
        let runtime = Arc::clone(self);
        tokio::spawn(async move {
            while runtime.routing_recovery_generation.load(Ordering::Acquire) == expected_generation
                && runtime.account_context_is_current(expected_generation)
                && runtime.host.background_routing_owner_is_running()
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if runtime.routing_recovery_generation.load(Ordering::Acquire) == expected_generation
                && runtime.account_context_is_current(expected_generation)
                && !runtime.host.background_routing_owner_is_running()
            {
                if let Err(error) = runtime.restore_device_routing(&device_name).await {
                    log::warn!(
                        "Failed to restore account routing after background owner exit: {error}"
                    );
                }
            }
            let _ = runtime.routing_recovery_generation.compare_exchange(
                expected_generation,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        });
    }
}

struct AccountContextTransitionGuard<'a> {
    runtime: &'a AccountRuntime,
    sync_guard: Option<MutexGuard<'a, ()>>,
    transition_guard: Option<MutexGuard<'a, ()>>,
    active: bool,
}

impl AccountContextTransitionGuard<'_> {
    fn finish(mut self) -> u64 {
        self.release()
    }

    fn release(&mut self) -> u64 {
        drop(self.sync_guard.take());
        if self.active {
            self.runtime
                .account_context_generation
                .fetch_add(1, Ordering::AcqRel);
            self.runtime
                .account_context_transitions
                .fetch_sub(1, Ordering::AcqRel);
            self.active = false;
        }
        let generation = self.runtime.account_context_generation();
        drop(self.transition_guard.take());
        generation
    }
}

impl Drop for AccountContextTransitionGuard<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

fn normalize_relay_url(relay_url: &str) -> Result<String> {
    let parsed = validate_relay_base_url(relay_url.trim())?;
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn current_device_identity() -> Result<DeviceIdentity> {
    DeviceIdentity::from_current_machine().map_err(|error| anyhow!("detect device: {error}"))
}

async fn revoke_rejected_login_candidate(
    client: &AccountClient,
    relay_url: &str,
    session: &AccountSession,
) {
    if let Err(error) = client.revoke_token(relay_url, session).await {
        log::warn!("Failed to revoke rejected login candidate token: {error}");
    }
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
        log::warn!("Failed to revoke replaced account token: {error}");
    }
}

fn resolve_cloud_settings_probe(result: Result<Option<String>>) -> Result<bool> {
    result.map(|settings| settings.is_some()).map_err(|error| {
        anyhow!("could not check cloud settings: {error}; the current account remains active")
    })
}

fn ensure_session_backup_complete(
    total: usize,
    uploaded: usize,
    upload_errors: &[String],
) -> Result<()> {
    if uploaded == total {
        return Ok(());
    }
    let detail = upload_errors
        .first()
        .map(String::as_str)
        .unwrap_or("retry will resume remaining sessions");
    Err(anyhow!(
        "session backup incomplete: uploaded {uploaded} of {total}; {detail}"
    ))
}

pub fn build_session_backup(
    metadata: &openbitfun_services_core::session::SessionMetadata,
    turns: &[openbitfun_services_core::session::DialogTurnData],
) -> Result<AccountSessionBackup> {
    ensure_relay_session_history_exportable(metadata).map_err(anyhow::Error::msg)?;
    let metadata = relay_session_export_metadata(metadata, turns.len());
    Ok(AccountSessionBackup {
        session_id: metadata.session_id.clone(),
        metadata: serde_json::to_value(metadata)
            .map_err(|error| anyhow!("serialize metadata: {error}"))?,
        turns: turns
            .iter()
            .map(|turn| serde_json::to_value(turn).unwrap_or(serde_json::Value::Null))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAccountRuntimeHost;

    #[async_trait]
    impl AccountRuntimeHost for TestAccountRuntimeHost {
        async fn retire_background_routing_owner(
            &self,
        ) -> std::result::Result<bool, BackgroundRoutingOwnerRetirementError> {
            Ok(false)
        }

        fn background_routing_owner_is_running(&self) -> bool {
            false
        }

        fn request_background_routing_owner_shutdown(&self) -> bool {
            false
        }

        async fn start_device_routing(&self, _request: AccountRoutingStartRequest) -> Result<()> {
            Ok(())
        }

        async fn stop_device_routing(&self) {}

        fn notify_controllers_settings_changed(&self) {}
    }

    struct EmptySessionBackup;

    #[async_trait]
    impl AccountSessionBackupPort for EmptySessionBackup {
        async fn list_session_backups(
            &self,
            _workspace_path: &Path,
        ) -> Result<Vec<AccountSessionBackup>> {
            Ok(Vec::new())
        }
    }

    fn test_runtime() -> Arc<AccountRuntime> {
        AccountRuntime::new(
            Arc::new(TestAccountRuntimeHost),
            Arc::new(EmptySessionBackup),
        )
    }

    #[test]
    fn pending_sync_choice_blocks_automatic_sync() {
        let pending = automatic_account_sync_policy_for_pending(true);
        assert!(!pending.background_engine);
        assert!(!pending.management_push);

        let finalized = automatic_account_sync_policy_for_pending(false);
        assert!(finalized.background_engine);
        assert!(finalized.management_push);
    }

    #[test]
    fn cloud_settings_probe_errors_are_not_treated_as_missing_settings() {
        assert!(!resolve_cloud_settings_probe(Ok(None)).expect("missing settings"));
        assert!(resolve_cloud_settings_probe(Ok(Some("settings".to_string()))).unwrap());
        assert!(resolve_cloud_settings_probe(Err(anyhow!("relay unavailable"))).is_err());
    }

    #[test]
    fn partial_session_backup_is_not_reported_as_success() {
        assert!(ensure_session_backup_complete(4, 4, &[]).is_ok());
        assert!(ensure_session_backup_complete(4, 1, &["quota full".to_string()]).is_err());
    }

    #[tokio::test]
    async fn invalid_login_does_not_advance_the_account_generation() {
        let runtime = test_runtime();
        let generation = runtime.account_context_generation();

        let error = runtime
            .login_with_credentials("", "user", "password")
            .await
            .expect_err("empty relay URL must be rejected");

        assert!(error.to_string().contains("Auth Server is required"));
        assert_eq!(runtime.account_context_generation(), generation);
    }
}
