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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;

use bitfun_core::service::remote_connect::{
    self, encryption, relay_client::RelayClient, relay_client::RelayEvent, session_store,
    AccountClient, AccountSession, DeviceIdentity, RemoteServer,
};

/// In-memory account session (token + master key). Lost on restart.
static ACCOUNT_SESSION: OnceLock<Arc<RwLock<Option<AccountSession>>>> = OnceLock::new();

/// The relay URL associated with the current account session.
static ACCOUNT_RELAY_URL: OnceLock<Arc<RwLock<Option<String>>>> = OnceLock::new();

/// The background device-routing relay client. Holding this keeps the WS
/// connection alive (the internal read/write tasks own the socket). Dropping it
/// tears the connection down.
static DEVICE_RELAY_CLIENT: OnceLock<RwLock<Option<Arc<RelayClient>>>> = OnceLock::new();

/// Set when the relay returns an auth error (token expired or invalid).
/// The chat loop checks this via `is_token_expired()` and prompts the user.
static TOKEN_EXPIRED: AtomicBool = AtomicBool::new(false);

fn account_session() -> &'static Arc<RwLock<Option<AccountSession>>> {
    ACCOUNT_SESSION.get_or_init(|| Arc::new(RwLock::new(None)))
}

fn account_relay_url() -> &'static Arc<RwLock<Option<String>>> {
    ACCOUNT_RELAY_URL.get_or_init(|| Arc::new(RwLock::new(None)))
}

fn device_relay_client() -> &'static RwLock<Option<Arc<RelayClient>>> {
    DEVICE_RELAY_CLIENT.get_or_init(|| RwLock::new(None))
}

/// Read both the session and relay URL, returning owned clones to avoid holding
/// locks across awaits.
pub(crate) async fn read_account_context() -> Result<(AccountSession, String)> {
    let session = account_session().read().await.clone();
    let relay_url = account_relay_url().read().await.clone();
    match (session, relay_url) {
        (Some(s), Some(u)) => Ok((s, u)),
        _ => Err(anyhow!("not logged in")),
    }
}

/// Whether an account session is currently held.
pub(crate) async fn is_logged_in() -> bool {
    account_session().read().await.is_some()
}

/// Attempt to restore a persisted session from disk.  Called at startup.
/// Returns `Some(user_id)` if a session was restored.
pub(crate) async fn try_restore_session() -> Option<String> {
    match session_store::load_session() {
        Ok(Some((token, user_id, master_key, relay_url))) => {
            let session = AccountSession {
                token,
                user_id: user_id.clone(),
                master_key,
            };
            *account_session().write().await = Some(session);
            *account_relay_url().write().await = Some(relay_url);
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
/// Intended for the TUI to prompt re-login when account pages open.
#[allow(dead_code)]
pub(crate) fn is_token_expired() -> bool {
    TOKEN_EXPIRED.load(Ordering::Relaxed)
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
    let relay_url = relay_url.trim();
    let username = username.trim();
    if relay_url.is_empty() {
        return Err(anyhow!("Auth Server is required"));
    }
    if username.is_empty() {
        return Err(anyhow!("Username is required"));
    }
    if password.is_empty() {
        return Err(anyhow!("Password is required"));
    }

    let device = current_device_identity()?;
    let client = AccountClient::new();
    let session = client
        .login(relay_url, username, password, &device)
        .await
        .map_err(|e| anyhow!("login failed: {e}"))?;

    let has_cloud_settings = client
        .fetch_settings(relay_url, &session)
        .await
        .unwrap_or(None)
        .is_some();

    let user_id = session.user_id.clone();
    let device_name = device.device_name.clone();
    let token = session.token.clone();
    let master_key = session.master_key;
    *account_session().write().await = Some(session);
    *account_relay_url().write().await = Some(relay_url.to_string());

    if let Err(e) = session_store::save_session(&token, &user_id, &master_key, relay_url) {
        tracing::warn!("Failed to persist session: {e}");
    }
    session_store::save_credential_hint(username, relay_url);

    TOKEN_EXPIRED.store(false, Ordering::Relaxed);

    let connect_result = spawn_device_routing(relay_url, &device_name).await;
    let routing_msg = match connect_result {
        Ok(()) => " Device routing connected (Peer Host ready).".to_string(),
        Err(e) => format!(" (Warning: device routing failed: {e})"),
    };

    Ok(LoginResult {
        user_id: user_id.clone(),
        relay_url: relay_url.to_string(),
        has_cloud_settings,
        status_message: format!(
            "Logged in as user {} on {}.{}",
            user_id, relay_url, routing_msg
        ),
    })
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
    let relay_url = account_relay_url()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("not logged in"))?;
    spawn_device_routing(&relay_url, device_name).await
}

/// Connect to the account relay for device-to-device routing and spawn the
/// background task that handles incoming RPC commands.
async fn spawn_device_routing(relay_url: &str, device_name: &str) -> Result<()> {
    // Tear down any previous connection first.
    stop_device_routing().await;

    let session_guard = account_session().read().await;
    let session = session_guard
        .as_ref()
        .ok_or_else(|| anyhow!("not logged in"))?
        .clone();
    drop(session_guard);

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
    *device_relay_client().write().await = Some(client_arc.clone());

    let session_arc = account_session().clone();
    let relay_client_arc = client_arc.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_relay_event(event, &session_arc, &relay_client_arc).await;
        }
        if is_current_routing_client(&relay_client_arc).await {
            crate::peer_host::update_controller_presence(Vec::new()).await;
        }
        tracing::info!("Device routing event loop exited");
    });

    Ok(())
}

/// Disconnect the device-routing connection (if any).
pub(crate) async fn stop_device_routing() {
    let client = { device_relay_client().write().await.take() };
    if let Some(client) = client {
        client.disconnect().await;
    }
    crate::peer_host::update_controller_presence(Vec::new()).await;
}

async fn is_current_routing_client(client: &Arc<RelayClient>) -> bool {
    device_relay_client()
        .read()
        .await
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, client))
}

/// Log out: tear down routing, revoke the token (best-effort), clear state.
pub(crate) async fn logout() -> Result<()> {
    stop_device_routing().await;
    let result = read_account_context().await;
    if let Ok((session, relay_url)) = result {
        let _ = AccountClient::new()
            .revoke_token(&relay_url, &session)
            .await;
    }
    *account_session().write().await = None;
    *account_relay_url().write().await = None;
    session_store::clear_session();
    session_store::clear_credential_hint();
    TOKEN_EXPIRED.store(false, Ordering::Relaxed);
    Ok(())
}

/// Handle a single relay event for the device-routing loop.
async fn handle_relay_event(
    event: RelayEvent,
    session_arc: &Arc<RwLock<Option<AccountSession>>>,
    relay_client: &Arc<RelayClient>,
) {
    if !is_current_routing_client(relay_client).await {
        tracing::debug!("Ignoring event from a stale device routing client");
        return;
    }
    match event {
        RelayEvent::AuthOk { user_id, device_id } => {
            tracing::info!("Device routing auth ok: user={user_id} device={device_id}");
        }
        RelayEvent::AuthError { message } => {
            tracing::warn!("Device routing auth error: {message}");
            TOKEN_EXPIRED.store(true, Ordering::Relaxed);
            crate::peer_host::update_controller_presence(Vec::new()).await;
        }
        RelayEvent::DevicePresence { devices } => {
            tracing::info!("Device presence updated: {} online", devices.len());
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
            let session_guard = session_arc.read().await.clone();
            let Some(session) = session_guard else {
                return;
            };
            let plaintext =
                match encryption::decrypt_from_base64(&session.master_key, &encrypted_data, &nonce)
                {
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
            tracing::info!("Device command from {source_device_id}: {cmd:?} corr={correlation_id}");

            let response = match &cmd {
                RemoteCommand::HostInvoke { command, args } => {
                    crate::peer_host::handle_host_invoke(command, args.clone()).await
                }
                RemoteCommand::DeviceEvent { .. } => {
                    crate::peer_host::handle_device_event_command()
                }
                other => {
                    let server = RemoteServer::new(session.master_key);
                    server.dispatch(other).await
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

            match encryption::encrypt_to_base64(&session.master_key, &resp_json) {
                Ok((enc_resp, resp_nonce)) => {
                    // HTTP RPC bridge expects replies targeted at "rpc".
                    let reply_target = if source_device_id == "rpc" {
                        "rpc"
                    } else {
                        source_device_id.as_str()
                    };
                    if let Err(e) = relay_client
                        .send_device_message(reply_target, &correlation_id, &enc_resp, &resp_nonce)
                        .await
                    {
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
            crate::peer_host::update_controller_presence(Vec::new()).await;
        }
        RelayEvent::Reconnected => {
            tracing::info!("Device routing reconnected");
        }
        RelayEvent::Error { message } => {
            tracing::warn!("Device routing error: {message}");
        }
        _ => {}
    }
}

/// Context needed by Peer Host DeviceEvent fan-out.
pub(crate) async fn peer_fanout_context() -> Result<(AccountSession, Arc<RelayClient>)> {
    let session = account_session()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("not logged in"))?;
    let client = device_relay_client()
        .read()
        .await
        .clone()
        .ok_or_else(|| anyhow!("device routing not connected"))?;
    Ok((session, client))
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
