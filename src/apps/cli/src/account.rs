//! CLI adapter for account-backed device routing.
//!
//! Shared account identity, persistence, synchronization, and transitions are
//! owned by [`AccountRuntime`]. This module contains only CLI Host effects:
//! daemon retirement, Relay routing, and Peer Device Mode fan-out fencing.

use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use openbitfun_product_domains::account::{
    AccountDevice, AccountInfo, AccountSnapshotProjection, SettingsSyncProgress, SettingsSyncStatus,
};
use tokio::sync::RwLock;

use openbitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use openbitfun_core::service::remote_connect::account::{
    ensure_relay_session_history_exportable, AccountSession,
};
use openbitfun_core::service::remote_connect::account_runtime::{
    build_session_backup, AccountRoutingStartRequest, AccountRuntime, AccountRuntimeHost,
    AccountSessionBackup, AccountSessionBackupPort, BackgroundRoutingOwnerRetirementError,
};
use openbitfun_core::service::remote_connect::{
    self, encryption, relay_client::RelayClient, relay_client::RelayEvent, session_store,
    DeviceIdentity, RemoteServer,
};

pub(crate) struct CliAccountRuntimeParts {
    pub(crate) runtime: Arc<AccountRuntime>,
    pub(crate) routing: Arc<CliAccountRoutingHost>,
}

pub(crate) fn build_account_runtime(
    compatibility: CoreAgentRuntimeCompatibility,
) -> CliAccountRuntimeParts {
    build_account_runtime_with_backup(Arc::new(CliAccountSessionBackupPort { compatibility }))
}

pub(crate) fn build_management_account_runtime() -> Arc<AccountRuntime> {
    build_account_runtime_with_backup(Arc::new(UnavailableSessionBackup)).runtime
}

pub(crate) fn account_snapshot_projection(
    snapshot: openbitfun_core::service::remote_connect::account_runtime::AccountSnapshot,
) -> AccountSnapshotProjection {
    AccountSnapshotProjection {
        logged_in: snapshot.logged_in,
        pending_sync_choice: snapshot.pending_sync_choice,
        info: snapshot.info.map(|info| AccountInfo {
            user_id: info.user_id,
            relay_url: info.relay_url,
            device_id: info.device_id,
            device_name: info.device_name,
        }),
        devices: snapshot
            .devices
            .into_iter()
            .map(|device| AccountDevice {
                device_id: device.device_id,
                device_name: device.device_name,
                online: device.online,
            })
            .collect(),
        sync: settings_sync_progress(snapshot.sync),
    }
}

pub(crate) fn settings_sync_progress(
    progress: openbitfun_core::service::remote_connect::account_runtime::AccountSyncProgress,
) -> SettingsSyncProgress {
    settings_sync_progress_from_core(progress)
}

fn settings_sync_progress_from_core(
    progress: openbitfun_core::service::remote_connect::account_runtime::AccountSyncProgress,
) -> SettingsSyncProgress {
    SettingsSyncProgress {
        operation_id: progress.operation_id,
        status: match progress.status {
            openbitfun_core::service::remote_connect::account_runtime::AccountSyncStatus::Idle => {
                SettingsSyncStatus::Idle
            }
            openbitfun_core::service::remote_connect::account_runtime::AccountSyncStatus::Syncing => {
                SettingsSyncStatus::Syncing
            }
            openbitfun_core::service::remote_connect::account_runtime::AccountSyncStatus::Done => {
                SettingsSyncStatus::Done
            }
            openbitfun_core::service::remote_connect::account_runtime::AccountSyncStatus::Failed => {
                SettingsSyncStatus::Failed
            }
            openbitfun_core::service::remote_connect::account_runtime::AccountSyncStatus::Cancelled => {
                SettingsSyncStatus::Cancelled
            }
        },
        phase: progress.phase,
        percent: progress.percent,
        current: progress.current,
        total: progress.total,
        detail: progress.detail,
        error: progress.error,
        settings_synced: progress.settings_synced,
        sessions_exported: progress.sessions_exported,
    }
}

pub(crate) fn account_login_status_message(
    result: &openbitfun_core::service::remote_connect::account_runtime::AccountLoginResult,
) -> String {
    if result.has_cloud_settings {
        return format!(
            "Authenticated as user {} on {}. Choose cloud or local settings to finish login.",
            result.user_id, result.relay_url
        );
    }
    if result.routing_connected {
        format!(
            "Logged in as user {} on {}. Device routing connected.",
            result.user_id, result.relay_url
        )
    } else if let Some(error) = &result.routing_error {
        format!(
            "Logged in as user {} on {}. Device routing failed: {}",
            result.user_id,
            result.relay_url,
            bounded_account_error(error)
        )
    } else {
        format!(
            "Logged in as user {} on {}.",
            result.user_id, result.relay_url
        )
    }
}

pub(crate) fn redact_login_error(error: anyhow::Error, secrets: [&str; 3]) -> anyhow::Error {
    let mut message = error.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            message = message.replace(secret, "<redacted>");
        }
    }
    anyhow!(bounded_account_error(&message))
}

fn bounded_account_error(message: &str) -> String {
    message
        .chars()
        .filter(|character| !character.is_control())
        .take(500)
        .collect()
}

fn build_account_runtime_with_backup(
    backup: Arc<dyn AccountSessionBackupPort>,
) -> CliAccountRuntimeParts {
    let routing = CliAccountRoutingHost::new();
    let runtime = AccountRuntime::new(routing.clone(), backup);
    routing.bind_runtime(Arc::downgrade(&runtime));
    CliAccountRuntimeParts { runtime, routing }
}

struct UnavailableSessionBackup;

#[async_trait]
impl AccountSessionBackupPort for UnavailableSessionBackup {
    async fn list_session_backups(
        &self,
        _workspace_path: &std::path::Path,
    ) -> Result<Vec<AccountSessionBackup>> {
        Err(anyhow!(
            "Session backup is unavailable in a short-lived management command"
        ))
    }
}

struct CliAccountSessionBackupPort {
    compatibility: CoreAgentRuntimeCompatibility,
}

#[async_trait]
impl AccountSessionBackupPort for CliAccountSessionBackupPort {
    async fn list_session_backups(
        &self,
        workspace_path: &std::path::Path,
    ) -> Result<Vec<AccountSessionBackup>> {
        let metadata = self
            .compatibility
            .list_persisted_sessions(workspace_path)
            .await
            .map_err(|error| anyhow!("list sessions: {error}"))?;
        let mut backups = Vec::new();
        for item in &metadata {
            if let Err(error) = ensure_relay_session_history_exportable(item) {
                tracing::debug!("Skipping CLI account session export: {error}");
                continue;
            }
            let turns = self
                .compatibility
                .load_persisted_session_turns(workspace_path, &item.session_id, None)
                .await
                .map_err(|error| anyhow!("load turns: {error}"))?;
            backups.push(build_session_backup(item, &turns)?);
        }
        Ok(backups)
    }
}

/// CLI-owned routing effects injected into the shared Account Runtime.
pub(crate) struct CliAccountRoutingHost {
    self_ref: Weak<CliAccountRoutingHost>,
    runtime: OnceLock<Weak<AccountRuntime>>,
    relay_client: RwLock<Option<Arc<RelayClient>>>,
    /// Read leases cover one routing event through its response. Routing owner
    /// changes take the write lease, so old events cannot escape through a new
    /// account's Relay client.
    lifecycle: Arc<RwLock<()>>,
}

impl CliAccountRoutingHost {
    fn new() -> Arc<Self> {
        Arc::new_cyclic(|self_ref| Self {
            self_ref: self_ref.clone(),
            runtime: OnceLock::new(),
            relay_client: RwLock::new(None),
            lifecycle: Arc::new(RwLock::new(())),
        })
    }

    fn bind_runtime(&self, runtime: Weak<AccountRuntime>) {
        self.runtime
            .set(runtime)
            .unwrap_or_else(|_| panic!("CLI account routing runtime was bound twice"));
    }

    fn runtime(&self) -> Result<Arc<AccountRuntime>> {
        self.runtime
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| anyhow!("account runtime is unavailable"))
    }

    async fn start_routing(&self, request: AccountRoutingStartRequest) -> Result<()> {
        let runtime = self.runtime()?;
        if !runtime.account_context_is_current(request.account_generation) {
            return Err(anyhow!("account context changed"));
        }
        self.stop_routing().await;

        let ws_url = format!(
            "{}/ws",
            request
                .relay_url
                .replace("https://", "wss://")
                .replace("http://", "ws://")
        );
        let (client, mut event_rx) = RelayClient::new();
        client.connect(&ws_url).await?;
        client
            .connect_authenticated(&request.session.token, &request.device_name)
            .await?;
        let client = Arc::new(client);
        {
            let _routing_guard = self.lifecycle.write().await;
            if !runtime.account_context_is_current(request.account_generation) {
                client.disconnect().await;
                return Err(anyhow!("account context changed"));
            }
            *self.relay_client.write().await = Some(client.clone());
        }
        if !runtime.account_context_is_current(request.account_generation) {
            self.retire_routing_client_if_same(&client).await;
            client.disconnect().await;
            return Err(anyhow!("account context changed"));
        }

        let routing = self
            .self_ref
            .upgrade()
            .ok_or_else(|| anyhow!("account routing is unavailable"))?;
        let expected_token = request.session.token;
        let generation = request.account_generation;
        tokio::spawn(async move {
            loop {
                if !routing.routing_loop_is_current(generation, &client).await {
                    tracing::debug!("Stopping stale device routing event loop");
                    break;
                }
                let Some(event) = event_rx.recv().await else {
                    break;
                };
                if !routing.routing_loop_is_current(generation, &client).await {
                    tracing::debug!("Stopping stale device routing event loop");
                    break;
                }
                routing
                    .handle_relay_event(event, &client, generation, &expected_token)
                    .await;
            }
            routing.retire_routing_client_if_same(&client).await;
            tracing::info!("Device routing event loop exited");
        });
        Ok(())
    }

    async fn stop_routing(&self) {
        let _routing_guard = self.lifecycle.write().await;
        self.stop_routing_locked().await;
    }

    pub(crate) async fn stop_device_routing(&self) {
        self.stop_routing().await;
    }

    async fn stop_routing_locked(&self) {
        if let Some(client) = self.relay_client.write().await.take() {
            client.disconnect().await;
        }
        crate::peer_host::update_controller_presence(Vec::new()).await;
    }

    async fn is_current_routing_client(&self, client: &Arc<RelayClient>) -> bool {
        same_routing_client(self.relay_client.read().await.as_ref(), client)
    }

    async fn routing_loop_is_current(
        &self,
        account_generation: u64,
        client: &Arc<RelayClient>,
    ) -> bool {
        let Ok(runtime) = self.runtime() else {
            return false;
        };
        if !runtime.account_context_is_current(account_generation) {
            return false;
        }
        let matches = self.is_current_routing_client(client).await;
        matches && runtime.account_context_is_current(account_generation)
    }

    async fn retire_routing_client_if_same(&self, client: &Arc<RelayClient>) -> bool {
        let _routing_guard = self.lifecycle.write().await;
        let mut current = self.relay_client.write().await;
        if !take_routing_client_if_same(&mut current, client) {
            return false;
        }
        drop(current);
        crate::peer_host::update_controller_presence(Vec::new()).await;
        true
    }

    async fn handle_relay_event(
        self: &Arc<Self>,
        event: RelayEvent,
        relay_client: &Arc<RelayClient>,
        account_generation: u64,
        expected_token: &str,
    ) {
        if let RelayEvent::AuthError { message } = event {
            self.handle_relay_auth_error(message, relay_client, account_generation, expected_token)
                .await;
            return;
        }

        let _routing_lease = self.lifecycle.read().await;
        if !self
            .routing_loop_is_current(account_generation, relay_client)
            .await
        {
            tracing::debug!("Ignoring event from a stale device routing client");
            return;
        }
        let fanout_owner = PeerFanoutOwner {
            account_generation,
            account_token: expected_token.to_string(),
            relay_client: Arc::clone(relay_client),
            runtime: Arc::downgrade(&self.runtime().expect("bound account runtime")),
            routing: Arc::downgrade(self),
        };
        ACTIVE_PEER_FANOUT_OWNER
            .scope(fanout_owner, async {
                self.handle_current_relay_event(
                    event,
                    relay_client,
                    account_generation,
                    expected_token,
                )
                .await;
            })
            .await;
    }

    async fn handle_current_relay_event(
        &self,
        event: RelayEvent,
        relay_client: &Arc<RelayClient>,
        account_generation: u64,
        expected_token: &str,
    ) {
        let runtime = match self.runtime() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        match event {
            RelayEvent::AuthOk { user_id, device_id } => {
                tracing::info!("Device routing auth ok: user={user_id} device={device_id}");
                if let Err(error) = DeviceIdentity::adopt_account_device_id(&device_id) {
                    tracing::warn!("Failed to adopt AuthOk device_id: {error}");
                    return;
                }
                if let Ok((session, relay_url)) = runtime
                    .read_account_context_for_generation(account_generation)
                    .await
                {
                    if session.token == expected_token
                        && self
                            .routing_loop_is_current(account_generation, relay_client)
                            .await
                    {
                        if let Err(error) = session_store::save_session_with_device(
                            &session.token,
                            &session.user_id,
                            &session.master_key,
                            &relay_url,
                            Some(device_id.as_str()),
                        ) {
                            tracing::warn!("Failed to persist AuthOk device_id: {error}");
                        }
                    }
                }
            }
            RelayEvent::DevicePresence { devices } => {
                tracing::info!("Device presence updated: {} online", devices.len());
                if !self
                    .routing_loop_is_current(account_generation, relay_client)
                    .await
                {
                    return;
                }
                crate::peer_host::update_controller_presence(
                    devices.into_iter().map(|device| device.device_id).collect(),
                )
                .await;
            }
            RelayEvent::DeviceMessageReceived {
                source_device_id,
                correlation_id,
                encrypted_data,
                nonce,
            } => {
                let Ok((session, _)) = runtime
                    .read_account_context_for_generation(account_generation)
                    .await
                else {
                    return;
                };
                if session.token != expected_token
                    || !self
                        .routing_loop_is_current(account_generation, relay_client)
                        .await
                {
                    return;
                }
                let plaintext = match encryption::decrypt_from_base64(
                    &session.master_key,
                    &encrypted_data,
                    &nonce,
                ) {
                    Ok(plaintext) => plaintext,
                    Err(error) => {
                        tracing::warn!("Failed to decrypt device message: {error}");
                        return;
                    }
                };
                use remote_connect::remote_server::{RemoteCommand, RemoteResponse};
                let command: RemoteCommand = match serde_json::from_str(&plaintext) {
                    Ok(command) => command,
                    Err(error) => {
                        tracing::warn!("Could not parse device command: {error}");
                        return;
                    }
                };
                tracing::info!(
                    "Device command from {source_device_id}: {command:?} corr={correlation_id}"
                );
                let response = match &command {
                    RemoteCommand::HostInvoke { command, args } => {
                        crate::peer_host::handle_host_invoke(command, args.clone()).await
                    }
                    RemoteCommand::DeviceEvent { .. } => {
                        crate::peer_host::handle_device_event_command()
                    }
                    other => RemoteServer::new(session.master_key).dispatch(other).await,
                };
                if !self
                    .routing_loop_is_current(account_generation, relay_client)
                    .await
                {
                    return;
                }
                let response_json = serde_json::to_string(&response).unwrap_or_else(|error| {
                    serde_json::to_string(&RemoteResponse::Error {
                        message: format!("failed to serialize RPC response: {error}"),
                    })
                    .unwrap_or_else(|_| {
                        r#"{"resp":"error","message":"serialize failed"}"#.to_string()
                    })
                });
                let Ok((encrypted_response, response_nonce)) =
                    encryption::encrypt_to_base64(&session.master_key, &response_json)
                else {
                    tracing::warn!("Failed to encrypt RPC response");
                    return;
                };
                let reply_target = if source_device_id == "rpc" {
                    "rpc"
                } else {
                    source_device_id.as_str()
                };
                if let Err(error) = relay_client
                    .send_device_message(
                        reply_target,
                        &correlation_id,
                        &encrypted_response,
                        &response_nonce,
                    )
                    .await
                {
                    tracing::warn!("Failed to send RPC response: {error}");
                }
            }
            RelayEvent::Disconnected => {
                tracing::info!("Device routing disconnected");
                crate::peer_host::update_controller_presence(Vec::new()).await;
            }
            RelayEvent::Reconnected => tracing::info!("Device routing reconnected"),
            RelayEvent::Error { message } => {
                tracing::warn!("Device routing error: {message}")
            }
            RelayEvent::AuthError { .. } => unreachable!("AuthError handled before routing lease"),
            _ => {}
        }
    }

    async fn handle_relay_auth_error(
        &self,
        message: String,
        relay_client: &Arc<RelayClient>,
        account_generation: u64,
        expected_token: &str,
    ) {
        tracing::warn!("Device routing auth error: {message}");
        {
            let _routing_guard = self.lifecycle.write().await;
            let mut current = self.relay_client.write().await;
            if !take_routing_client_if_same(&mut current, relay_client) {
                tracing::debug!("Ignoring auth error from a replaced routing client");
                return;
            }
            drop(current);
            relay_client.disconnect().await;
        }
        let Ok(runtime) = self.runtime() else {
            return;
        };
        if runtime
            .expire_rejected_context(account_generation, expected_token)
            .await
        {
            crate::peer_host::update_controller_presence(Vec::new()).await;
        }
    }

    pub(crate) async fn capture_peer_fanout_owner(&self) -> Result<PeerFanoutOwner> {
        let runtime = self.runtime()?;
        let generation = runtime.account_context_generation();
        let _routing_lease = self.lifecycle.read().await;
        let (session, _) = runtime
            .read_account_context_for_generation(generation)
            .await?;
        let relay_client = self
            .relay_client
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("device routing not connected"))?;
        if !self
            .routing_loop_is_current(generation, &relay_client)
            .await
        {
            return Err(anyhow!("account context changed"));
        }
        Ok(PeerFanoutOwner {
            account_generation: generation,
            account_token: session.token,
            relay_client,
            runtime: Arc::downgrade(&runtime),
            routing: self.self_ref.clone(),
        })
    }
}

#[async_trait]
impl AccountRuntimeHost for CliAccountRoutingHost {
    async fn retire_background_routing_owner(
        &self,
    ) -> std::result::Result<bool, BackgroundRoutingOwnerRetirementError> {
        if !crate::daemon::is_daemon_running() {
            return Ok(false);
        }
        if !crate::daemon::request_daemon_shutdown() {
            return Err(BackgroundRoutingOwnerRetirementError {
                error: anyhow!("could not stop the CLI daemon; the current account remains active"),
                owner_may_exit: false,
            });
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while crate::daemon::is_daemon_running() {
            if tokio::time::Instant::now() >= deadline {
                return Err(BackgroundRoutingOwnerRetirementError {
                    error: anyhow!(
                        "CLI daemon did not stop in time; the current account remains active"
                    ),
                    owner_may_exit: true,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(true)
    }

    fn background_routing_owner_is_running(&self) -> bool {
        crate::daemon::is_daemon_running()
    }

    fn request_background_routing_owner_shutdown(&self) -> bool {
        crate::daemon::request_daemon_shutdown()
    }

    async fn start_device_routing(&self, request: AccountRoutingStartRequest) -> Result<()> {
        self.start_routing(request).await
    }

    async fn stop_device_routing(&self) {
        self.stop_routing().await;
    }

    fn notify_controllers_settings_changed(&self) {
        crate::peer_host::notify_controllers_settings_changed();
    }
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

/// Immutable routing owner captured when a Peer DeviceEvent enters the queue.
#[derive(Clone)]
pub(crate) struct PeerFanoutOwner {
    account_generation: u64,
    account_token: String,
    relay_client: Arc<RelayClient>,
    runtime: Weak<AccountRuntime>,
    routing: Weak<CliAccountRoutingHost>,
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
            runtime: Weak::new(),
            routing: Weak::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn generation_for_test(&self) -> u64 {
        self.account_generation
    }
}

pub(crate) struct PeerFanoutLease {
    pub(crate) session: AccountSession,
    pub(crate) relay_client: Arc<RelayClient>,
    _routing_lease: tokio::sync::OwnedRwLockReadGuard<()>,
}

pub(crate) async fn acquire_peer_fanout_lease(owner: &PeerFanoutOwner) -> Result<PeerFanoutLease> {
    let runtime = owner
        .runtime
        .upgrade()
        .ok_or_else(|| anyhow!("account runtime stopped"))?;
    let routing = owner
        .routing
        .upgrade()
        .ok_or_else(|| anyhow!("account routing stopped"))?;
    let routing_lease = routing.lifecycle.clone().read_owned().await;
    if !runtime.account_context_is_current(owner.account_generation) {
        return Err(anyhow!("queued Peer event account changed"));
    }
    let (session, _) = runtime
        .read_account_context_for_generation(owner.account_generation)
        .await?;
    let client = routing
        .relay_client
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("device routing not connected"))?;
    if !owner.matches(
        runtime.account_context_generation(),
        &session.token,
        &client,
    ) {
        return Err(anyhow!("queued Peer event routing owner changed"));
    }
    Ok(PeerFanoutLease {
        session,
        relay_client: client,
        _routing_lease: routing_lease,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
    fn queued_fanout_owner_requires_generation_token_and_client_identity() {
        let owner = PeerFanoutOwner::for_test(11, "token-a");
        let owned_client = Arc::clone(&owner.relay_client);
        let replacement = PeerFanoutOwner::for_test(12, "token-b");
        assert!(owner.matches(11, "token-a", &owned_client));
        assert!(!owner.matches(12, "token-a", &owned_client));
        assert!(!owner.matches(11, "token-b", &owned_client));
        assert!(!owner.matches(11, "token-a", &replacement.relay_client));
    }

    #[tokio::test]
    async fn routing_replacement_waits_for_an_in_flight_event_lease() {
        let routing = CliAccountRoutingHost::new();
        let event_lease = routing.lifecycle.read().await;
        let lifecycle = routing.lifecycle.clone();
        let (attempting_tx, attempting_rx) = tokio::sync::oneshot::channel();
        let replacement = tokio::spawn(async move {
            let _ = attempting_tx.send(());
            let _replacement_lease = lifecycle.write().await;
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
}
