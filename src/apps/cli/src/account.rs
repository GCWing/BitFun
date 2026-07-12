//! CLI account login and device-routing (RPC control) support.
//!
//! This module lets the CLI log in to a BitFun relay account and then become
//! RPC-controllable by other devices on the same account. The flow mirrors the
//! desktop implementation but is kept minimal: a single global AccountSession,
//! a background WS task that decrypts incoming device messages and dispatches
//! them through the shared `RemoteServer`, and simple text-based listing.
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
async fn read_account_context() -> Result<(AccountSession, String)> {
    let session = account_session().read().await.clone();
    let relay_url = account_relay_url().read().await.clone();
    match (session, relay_url) {
        (Some(s), Some(u)) => Ok((s, u)),
        _ => Err(anyhow!("not logged in")),
    }
}

/// Whether an account session is currently held.
pub async fn is_logged_in() -> bool {
    account_session().read().await.is_some()
}

/// Attempt to restore a persisted session from disk.  Called at startup.
/// Returns `Some(user_id)` if a session was restored.
pub async fn try_restore_session() -> Option<String> {
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
/// The chat loop can call this to prompt the user to re-login.
pub fn is_token_expired() -> bool {
    TOKEN_EXPIRED.load(Ordering::Relaxed)
}

/// Resolve the current device identity (machine-based).
fn current_device_identity() -> Result<DeviceIdentity> {
    DeviceIdentity::from_current_machine().map_err(|e| anyhow!("detect device: {e}"))
}

/// Prompt for a line of input from stdin with the terminal in cooked mode.
///
/// `secret=true` disables local echo (best-effort — used for passwords). The
/// caller must have already left raw mode so that line editing works normally.
fn prompt_line(prompt: &str, secret: bool) -> Result<String> {
    use std::io::{self, Write};
    print!("{}", prompt);
    io::stdout().flush()?;

    let value = if secret {
        read_password()?
    } else {
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        buf
    };
    Ok(value.trim_end_matches(['\r', '\n']).to_string())
}

/// Read a password from stdin with echo disabled. Falls back to plain
/// `read_line` if terminal echo control is unavailable.
fn read_password() -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let stdin = std::io::stdin();
        let fd = stdin.as_raw_fd();
        // Attempt to disable ECHO; if it fails, fall back to normal read.
        let disabled = disable_echo(fd).is_ok();
        let result = (|| {
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;
            Ok(buf)
        })();
        if disabled {
            let _ = restore_echo(fd);
            // Print a newline so the next prompt starts on a fresh line.
            println!();
        }
        return result;
    }
    #[cfg(not(unix))]
    {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
        Ok(buf)
    }
}

#[cfg(unix)]
fn disable_echo(fd: std::os::unix::io::RawFd) -> std::io::Result<()> {
    // Minimal termios binding to avoid pulling in another crate. Only the
    // fields needed to toggle ECHO are touched.
    use std::mem::MaybeUninit;
    extern "C" {
        fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }
    const ECHO: u32 = 0o000010;
    const ECHONL: u32 = 0o000100;
    const TCSANOW: i32 = 0;
    unsafe {
        let mut t: MaybeUninit<Termios> = MaybeUninit::uninit();
        if tcgetattr(fd, t.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut t = t.assume_init();
        t.c_lflag &= !(ECHO);
        t.c_lflag |= ECHONL;
        if tcsetattr(fd, TCSANOW, &t) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn restore_echo(fd: std::os::unix::io::RawFd) -> std::io::Result<()> {
    use std::mem::MaybeUninit;
    extern "C" {
        fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }
    const ECHO: u32 = 0o000010;
    const ECHONL: u32 = 0o000100;
    const TCSANOW: i32 = 0;
    unsafe {
        let mut t: MaybeUninit<Termios> = MaybeUninit::uninit();
        if tcgetattr(fd, t.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut t = t.assume_init();
        t.c_lflag |= ECHO;
        t.c_lflag &= !ECHONL;
        if tcsetattr(fd, TCSANOW, &t) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Default relay URL used when the user presses Enter at the prompt.
const DEFAULT_RELAY_URL: &str = "https://remote.openbitfun.com/relay";

/// Run the interactive login flow.
///
/// `read_input` is called for each prompt while the terminal is in cooked
/// mode. Returns a short status message for the chat view. The caller is
/// responsible for leaving raw mode before calling this and re-enabling it
/// afterwards.
pub async fn login_interactive() -> Result<String> {
    let relay_url = prompt_line(&format!("Relay URL [{}]: ", DEFAULT_RELAY_URL), false)?;
    let relay_url = if relay_url.is_empty() {
        DEFAULT_RELAY_URL.to_string()
    } else {
        relay_url
    };
    let username = prompt_line("Username: ", false)?;
    if username.is_empty() {
        return Err(anyhow!("username is required"));
    }
    let password = prompt_line("Password: ", true)?;
    if password.is_empty() {
        return Err(anyhow!("password is required"));
    }

    let device = current_device_identity()?;
    let client = AccountClient::new();
    let session = client
        .login(&relay_url, &username, &password, &device)
        .await
        .map_err(|e| anyhow!("login failed: {e}"))?;

    let user_id = session.user_id.clone();
    let device_name = device.device_name.clone();
    let token = session.token.clone();
    let master_key = session.master_key;
    *account_session().write().await = Some(session);
    *account_relay_url().write().await = Some(relay_url.clone());

    // Persist session for restart recovery.
    if let Err(e) = session_store::save_session(&token, &user_id, &master_key, &relay_url) {
        tracing::warn!("Failed to persist session: {e}");
    }

    TOKEN_EXPIRED.store(false, Ordering::Relaxed);

    // Establish device routing so the CLI becomes RPC-controllable.
    let connect_result = spawn_device_routing(&relay_url, &device_name).await;
    let routing_msg = match connect_result {
        Ok(()) => " Device routing connected.".to_string(),
        Err(e) => format!(" (Warning: device routing failed: {e})"),
    };

    Ok(format!(
        "Logged in as user {} on {}.{}",
        user_id, relay_url, routing_msg
    ))
}

/// Public wrapper for restoring device routing after session restore at startup.
pub async fn restore_device_routing(device_name: &str) -> Result<()> {
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
        tracing::info!("Device routing event loop exited");
    });

    Ok(())
}

/// Disconnect the device-routing connection (if any).
pub async fn stop_device_routing() {
    if let Some(client) = device_relay_client().write().await.take() {
        client.disconnect().await;
    }
}

/// Log out: tear down routing, revoke the token (best-effort), clear state.
pub async fn logout() -> Result<()> {
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
    TOKEN_EXPIRED.store(false, Ordering::Relaxed);
    Ok(())
}

/// Handle a single relay event for the device-routing loop.
async fn handle_relay_event(
    event: RelayEvent,
    session_arc: &Arc<RwLock<Option<AccountSession>>>,
    relay_client: &Arc<RelayClient>,
) {
    match event {
        RelayEvent::AuthOk { user_id, device_id } => {
            tracing::info!("Device routing auth ok: user={user_id} device={device_id}");
        }
        RelayEvent::AuthError { message } => {
            tracing::warn!("Device routing auth error: {message}");
            TOKEN_EXPIRED.store(true, Ordering::Relaxed);
        }
        RelayEvent::DevicePresence { devices } => {
            tracing::info!("Device presence updated: {} online", devices.len());
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
            use remote_connect::remote_server::RemoteCommand;
            let cmd: RemoteCommand = match serde_json::from_str(&plaintext) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Could not parse device command: {e}");
                    return;
                }
            };
            tracing::info!("Device command from {source_device_id}: {cmd:?} corr={correlation_id}");

            let server = RemoteServer::new(session.master_key);
            let response = server.dispatch(&cmd).await;
            match server.encrypt_response(&response, None) {
                Ok((enc_resp, resp_nonce)) => {
                    let _ = relay_client
                        .send_device_message(
                            &source_device_id,
                            &correlation_id,
                            &enc_resp,
                            &resp_nonce,
                        )
                        .await;
                }
                Err(e) => {
                    tracing::warn!("Failed to encrypt RPC response: {e}");
                }
            }
        }
        RelayEvent::Disconnected => {
            tracing::info!("Device routing disconnected");
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

/// A textual device listing entry for display.
pub struct AccountDevice {
    pub device_id: String,
    pub device_name: String,
    pub online: bool,
    pub last_seen_at: Option<i64>,
}

/// List all devices in the account.
pub async fn list_devices() -> Result<Vec<AccountDevice>> {
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
            last_seen_at: d.last_seen_at,
        })
        .collect())
}

/// Build a simple text report of account devices.
pub fn format_devices(devices: &[AccountDevice]) -> String {
    if devices.is_empty() {
        return "No devices found on this account.".to_string();
    }
    let mut lines = Vec::new();
    lines.push(format!("Account devices ({}):", devices.len()));
    for d in devices {
        let status = if d.online { "online" } else { "offline" };
        let last = match d.last_seen_at {
            Some(ts) => format!(", last seen {}", format_ts(ts)),
            None => String::new(),
        };
        lines.push(format!(
            "  • {} — {} [{}]{}",
            d.device_name, d.device_id, status, last
        ));
    }
    lines.join("\n")
}

fn format_ts(ts: i64) -> String {
    use chrono::{DateTime, Utc};
    match DateTime::<Utc>::from_timestamp(ts, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => ts.to_string(),
    }
}
