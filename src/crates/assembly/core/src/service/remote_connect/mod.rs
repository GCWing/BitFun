//! Remote Connect service module.
//!
//! Provides phone-to-desktop remote connection capabilities with E2E encryption.
//! Supports multiple connection methods: LAN, ngrok, relay server, and bots.
//!
//! Bot connections (Telegram / Feishu / Weixin) run independently of relay connections
//! (LAN / ngrok / BitFun Server / Custom Server).  Calling `stop()` only
//! tears down the relay side; bots keep running.  Use `stop_bot()` or
//! `stop_all()` to shut everything down.

pub mod bot;
pub mod embedded_relay_host;
pub mod lan;
pub mod ngrok;
pub mod remote_server;
pub mod settings_sync;

pub mod device {
    pub use bitfun_services_integrations::remote_connect::device::*;
}

pub mod encryption {
    pub use bitfun_services_integrations::remote_connect::encryption::*;
}

pub mod pairing {
    pub use bitfun_services_integrations::remote_connect::pairing::*;
}

pub mod qr_generator {
    pub use bitfun_services_integrations::remote_connect::qr_generator::*;
}

pub mod relay_client {
    pub use bitfun_services_integrations::remote_connect::relay_client::*;
}

pub mod account {
    pub use bitfun_services_integrations::remote_connect::account::*;
}

pub mod session_store {
    pub use bitfun_services_integrations::remote_connect::session_store::*;
}

pub mod sync_state {
    pub use bitfun_services_integrations::remote_connect::sync_state::*;
}

pub use account::{
    build_relay_websocket_url, validate_relay_base_url, AccountClient, AccountSession,
    DelegateToken, DelegatedIdentity, FetchedSession, KdfParams, ListedSessionEntry, SettingsBlob,
};
pub use device::DeviceIdentity;
pub use encryption::{decrypt_from_base64, encrypt_to_base64, KeyPair};
pub use pairing::{PairingProtocol, PairingState};
pub use qr_generator::QrGenerator;
pub use relay_client::ensure_rustls_crypto_provider;
pub use relay_client::RelayClient;
pub use remote_server::RemoteServer;

use anyhow::Result;
use bitfun_services_integrations::remote_connect::upload_mobile_web_to_relay;
use embedded_relay_host::EmbeddedRelayHost;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Supported connection methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMethod {
    Lan { ip: Option<String> },
    Ngrok,
    BitfunServer,
    CustomServer { url: String },
    BotFeishu,
    BotTelegram,
    BotWeixin,
}

/// Configuration for Remote Connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConnectConfig {
    pub lan_port: u16,
    pub bitfun_server_url: String,
    pub web_app_url: String,
    pub custom_server_url: Option<String>,
    pub bot_feishu: Option<bot::BotConfig>,
    pub bot_telegram: Option<bot::BotConfig>,
    pub bot_weixin: Option<bot::BotConfig>,
    pub mobile_web_dir: Option<String>,
}

impl Default for RemoteConnectConfig {
    fn default() -> Self {
        Self {
            lan_port: 9700,
            bitfun_server_url: "https://remote.openbitfun.com/relay".to_string(),
            web_app_url: "https://remote.openbitfun.com/relay".to_string(),
            custom_server_url: None,
            bot_feishu: None,
            bot_telegram: None,
            bot_weixin: None,
            mobile_web_dir: None,
        }
    }
}

/// Result of starting a remote connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResult {
    pub method: ConnectionMethod,
    pub qr_data: Option<String>,
    pub qr_svg: Option<String>,
    pub qr_url: Option<String>,
    pub bot_pairing_code: Option<String>,
    pub bot_link: Option<String>,
    pub pairing_state: PairingState,
}

/// Handle to a running bot (Telegram, Feishu, or Weixin).
struct BotHandle {
    stop_tx: tokio::sync::watch::Sender<bool>,
}

impl BotHandle {
    fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustedMobileIdentity {
    mobile_install_id: String,
    user_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoomConnectionOwner {
    generation: u64,
    room_id: String,
}

fn room_owner_is_current(
    active: &Option<RoomConnectionOwner>,
    expected: &RoomConnectionOwner,
) -> bool {
    active.as_ref() == Some(expected)
}

fn clear_room_owner_if_current(
    active: &mut Option<RoomConnectionOwner>,
    expected: &RoomConnectionOwner,
) -> bool {
    if !room_owner_is_current(active, expected) {
        return false;
    }
    *active = None;
    true
}

/// Successful account pairing verification together with an optional host
/// lease. The service keeps the lease alive until the trusted identity and
/// paired server are committed, so an account transition cannot clear state
/// and then be overwritten by a retiring verifier.
pub struct AccountPairingVerification {
    user_id: String,
    _host_lease: Option<Box<dyn Send>>,
}

impl std::fmt::Debug for AccountPairingVerification {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountPairingVerification")
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

impl AccountPairingVerification {
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            _host_lease: None,
        }
    }

    pub fn with_host_lease<L>(user_id: String, lease: L) -> Self
    where
        L: Send + 'static,
    {
        Self {
            user_id,
            _host_lease: Some(Box::new(lease)),
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

/// Delegated account credentials together with the host account lease that
/// authorized them. The lease is retained after the provider returns and is
/// released only after the encrypted room response has been sent.
pub struct DelegatedIdentityAuthorization {
    token: String,
    user_id: String,
    master_key: [u8; 32],
    host_lease: Option<Box<dyn Send>>,
}

impl DelegatedIdentityAuthorization {
    pub fn new(token: String, user_id: String, master_key: [u8; 32]) -> Self {
        Self {
            token,
            user_id,
            master_key,
            host_lease: None,
        }
    }

    pub fn with_host_lease<L>(
        token: String,
        user_id: String,
        master_key: [u8; 32],
        lease: L,
    ) -> Self
    where
        L: Send + 'static,
    {
        Self {
            token,
            user_id,
            master_key,
            host_lease: Some(Box::new(lease)),
        }
    }

    fn into_response(self, local_device_id: &str) -> DelegatedIdentityResolution {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let Self {
            token,
            user_id,
            master_key,
            host_lease,
        } = self;
        DelegatedIdentityResolution {
            response: remote_server::RemoteResponse::DelegateIdentity {
                token,
                user_id,
                master_key: B64.encode(master_key),
                device_id: local_device_id.to_string(),
            },
            _host_lease: host_lease,
        }
    }
}

struct DelegatedIdentityResolution {
    response: remote_server::RemoteResponse,
    _host_lease: Option<Box<dyn Send>>,
}

impl DelegatedIdentityResolution {
    fn error(message: impl Into<String>) -> Self {
        Self {
            response: remote_server::RemoteResponse::Error {
                message: message.into(),
            },
            _host_lease: None,
        }
    }
}

/// Unified Remote Connect service that orchestrates all connection methods.
pub struct RemoteConnectService {
    config: RemoteConnectConfig,
    device_identity: DeviceIdentity,
    pairing: Arc<RwLock<PairingProtocol>>,
    relay_client: Arc<RwLock<Option<RelayClient>>>,
    remote_server: Arc<RwLock<Option<RemoteServer>>>,
    active_method: Arc<RwLock<Option<ConnectionMethod>>>,
    ngrok_tunnel: Arc<RwLock<Option<ngrok::NgrokTunnel>>>,
    embedded_relay_host: Arc<dyn EmbeddedRelayHost>,
    relay_lifecycle: Arc<Mutex<()>>,
    room_connection_generation: AtomicU64,
    active_room_owner: Arc<RwLock<Option<RoomConnectionOwner>>>,
    // Bot handles live independently of relay connections
    bot_lifecycle: Arc<Mutex<()>>,
    bot_account_identity_epoch: Arc<AtomicU64>,
    bot_telegram_slot: Arc<bot::BotSlotFence>,
    bot_feishu_slot: Arc<bot::BotSlotFence>,
    bot_weixin_slot: Arc<bot::BotSlotFence>,
    bot_telegram_handle: Arc<RwLock<Option<BotHandle>>>,
    bot_feishu_handle: Arc<RwLock<Option<BotHandle>>>,
    bot_weixin_handle: Arc<RwLock<Option<BotHandle>>>,
    // Keep Arc references to bots for send_message etc.
    telegram_bot: Arc<RwLock<Option<Arc<bot::telegram::TelegramBot>>>>,
    feishu_bot: Arc<RwLock<Option<Arc<bot::feishu::FeishuBot>>>>,
    weixin_bot: Arc<RwLock<Option<Arc<bot::weixin::WeixinBot>>>>,
    /// Independent bot connection state — not tied to PairingProtocol.
    /// Stores the peer description (e.g. "Telegram(7096812005)") when a bot is active.
    bot_connected_info: Arc<RwLock<Option<String>>>,
    /// Trusted mobile identity for the current relay lifecycle only.
    trusted_mobile_identity: Arc<RwLock<Option<TrustedMobileIdentity>>>,
    /// Account-authenticated device-routing relay client (P2). Independent from
    /// the room-pairing relay_client above; connects after account login.
    device_relay_client: Arc<RwLock<Option<RelayClient>>>,
    device_relay_lifecycle: Arc<Mutex<()>>,
    device_connection_generation: AtomicU64,
    active_device_connection_id: Arc<RwLock<Option<u64>>>,
    /// Latest online-device presence for the account (P2).
    online_devices: Arc<RwLock<Vec<relay_client::DevicePresenceEntry>>>,
    /// Callback that provides a delegated identity and its host account lease
    /// for paired mobile/IM clients. Set by the desktop layer after account
    /// login. Resolved on demand when a paired client sends
    /// `get_delegated_identity` over the room channel.
    delegated_identity_fn: Arc<RwLock<Option<DelegatedIdentityFn>>>,
    /// Non-secret username embedded in the QR when the desktop is logged in.
    account_pairing_username: Arc<RwLock<Option<String>>>,
    /// When set, pairing requires BitFun account username+password and the
    /// verifier returns the canonical account `user_id` on success.
    account_pairing_verifier: Arc<RwLock<Option<AccountPairingVerifierFn>>>,
}

/// Provider returning authorized delegated credentials for the paired client.
type DelegatedIdentityFn = Arc<
    dyn Fn() -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Option<DelegatedIdentityAuthorization>>
                    + Send
                    + Sync,
            >,
        > + Send
        + Sync,
>;

/// Verifies mobile-submitted account credentials. Returns the canonical
/// account `user_id` when the credentials match the logged-in desktop account.
type AccountPairingVerifierFn = Arc<
    dyn Fn(
            String,
            String,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<AccountPairingVerification, String>>
                    + Send
                    + Sync,
            >,
        > + Send
        + Sync,
>;

impl RemoteConnectService {
    pub fn new(
        config: RemoteConnectConfig,
        embedded_relay_host: Arc<dyn EmbeddedRelayHost>,
    ) -> Result<Self> {
        let device_identity = DeviceIdentity::from_current_machine()?;
        let pairing = PairingProtocol::new(device_identity.clone());

        Ok(Self {
            config,
            device_identity,
            pairing: Arc::new(RwLock::new(pairing)),
            relay_client: Arc::new(RwLock::new(None)),
            remote_server: Arc::new(RwLock::new(None)),
            active_method: Arc::new(RwLock::new(None)),
            ngrok_tunnel: Arc::new(RwLock::new(None)),
            embedded_relay_host,
            relay_lifecycle: Arc::new(Mutex::new(())),
            room_connection_generation: AtomicU64::new(0),
            active_room_owner: Arc::new(RwLock::new(None)),
            bot_lifecycle: Arc::new(Mutex::new(())),
            bot_account_identity_epoch: Arc::new(AtomicU64::new(0)),
            bot_telegram_slot: Arc::new(bot::BotSlotFence::default()),
            bot_feishu_slot: Arc::new(bot::BotSlotFence::default()),
            bot_weixin_slot: Arc::new(bot::BotSlotFence::default()),
            bot_telegram_handle: Arc::new(RwLock::new(None)),
            bot_feishu_handle: Arc::new(RwLock::new(None)),
            bot_weixin_handle: Arc::new(RwLock::new(None)),
            telegram_bot: Arc::new(RwLock::new(None)),
            feishu_bot: Arc::new(RwLock::new(None)),
            weixin_bot: Arc::new(RwLock::new(None)),
            bot_connected_info: Arc::new(RwLock::new(None)),
            trusted_mobile_identity: Arc::new(RwLock::new(None)),
            device_relay_client: Arc::new(RwLock::new(None)),
            device_relay_lifecycle: Arc::new(Mutex::new(())),
            device_connection_generation: AtomicU64::new(0),
            active_device_connection_id: Arc::new(RwLock::new(None)),
            online_devices: Arc::new(RwLock::new(Vec::new())),
            delegated_identity_fn: Arc::new(RwLock::new(None)),
            account_pairing_username: Arc::new(RwLock::new(None)),
            account_pairing_verifier: Arc::new(RwLock::new(None)),
        })
    }

    /// Set the delegated identity provider (called by desktop after login).
    /// Returns delegated credentials with a host account lease for the paired client.
    pub async fn set_delegated_identity_provider<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Option<DelegatedIdentityAuthorization>>
            + Send
            + Sync
            + 'static,
    {
        *self.delegated_identity_fn.write().await = Some(Arc::new(move || Box::pin(f())));
    }

    /// Enable account-password pairing in the QR.
    /// `None` disables account mode; `Some(username)` enables it (username may
    /// be empty when only `auth=account` should be advertised without prefill).
    pub async fn set_account_pairing_username(&self, username: Option<String>) {
        *self.account_pairing_username.write().await = username.map(|u| u.trim().to_string());
    }

    /// Register account password verification for mobile pairing.
    pub async fn set_account_pairing_verifier<F, Fut>(&self, f: F)
    where
        F: Fn(String, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<AccountPairingVerification, String>>
            + Send
            + Sync
            + 'static,
    {
        *self.account_pairing_verifier.write().await = Some(Arc::new(move |username, password| {
            Box::pin(f(username, password))
        }));
    }

    /// Clear account pairing context (username + verifier) on logout.
    pub async fn clear_account_pairing_context(&self) {
        *self.account_pairing_username.write().await = None;
        *self.account_pairing_verifier.write().await = None;
    }

    /// Drop the URL-bound mobile identity. Call when the desktop account
    /// changes so a later pair can bind to the new account user id.
    pub async fn clear_trusted_mobile_identity(&self) {
        *self.trusted_mobile_identity.write().await = None;
    }

    /// Clear credentials and remote-device selections cached by long-lived IM
    /// bot chats. Bot connections intentionally survive account logout, but
    /// their delegated authority must not.
    pub async fn clear_bot_delegated_identities(&self) {
        // Increment before waiting for lifecycle ownership. In-flight bot work
        // can observe the epoch immediately and is forbidden from committing
        // account-bound state even if a provider request does not return.
        self.bot_account_identity_epoch
            .fetch_add(1, Ordering::AcqRel);
        let _lifecycle = self.bot_lifecycle.lock().await;
        let telegram = self.telegram_bot.read().await.clone();
        let feishu = self.feishu_bot.read().await.clone();
        let weixin = self.weixin_bot.read().await.clone();

        tokio::join!(
            async move {
                if let Some(bot) = telegram {
                    bot.clear_delegated_identities().await;
                }
            },
            async move {
                if let Some(bot) = feishu {
                    bot.clear_delegated_identities().await;
                }
            },
            async move {
                if let Some(bot) = weixin {
                    bot.clear_delegated_identities().await;
                }
            },
        );
        bitfun_services_integrations::remote_connect::bot::clear_persisted_bot_account_contexts();
    }

    pub fn device_identity(&self) -> &DeviceIdentity {
        &self.device_identity
    }

    async fn validate_mobile_identity(
        trusted_mobile_identity: &Arc<RwLock<Option<TrustedMobileIdentity>>>,
        mobile_install_id: &str,
        user_id: &str,
    ) -> std::result::Result<TrustedMobileIdentity, String> {
        let mobile_install_id = mobile_install_id.trim();
        let user_id = user_id.trim();
        if mobile_install_id.is_empty() {
            return Err("Missing mobile installation ID".to_string());
        }
        if user_id.is_empty() {
            return Err("Missing user ID".to_string());
        }

        let submitted = TrustedMobileIdentity {
            mobile_install_id: mobile_install_id.to_string(),
            user_id: user_id.to_string(),
        };

        let trusted = trusted_mobile_identity.read().await.clone();
        match trusted {
            Some(existing) if existing.mobile_install_id == submitted.mobile_install_id => {
                if existing.user_id != submitted.user_id {
                    Err("This mobile device must continue using the previously confirmed user ID".to_string())
                } else {
                    Ok(submitted)
                }
            }
            Some(existing) if existing.user_id != submitted.user_id => Err(
                "This remote URL is already protected. Enter the previously confirmed user ID to continue.".to_string(),
            ),
            _ => Ok(submitted),
        }
    }

    /// When the desktop is logged in, require and verify account credentials.
    /// Returns the canonical account `user_id` to bind as the trusted identity.
    async fn resolve_pairing_user_id(
        account_pairing_verifier: &Arc<RwLock<Option<AccountPairingVerifierFn>>>,
        response: &pairing::PairingResponse,
    ) -> std::result::Result<AccountPairingVerification, String> {
        let verifier = account_pairing_verifier.read().await.clone();
        let Some(verify) = verifier else {
            // The mobile submitted account credentials (QR advertised
            // auth=account) but the desktop logged out after generating the
            // code. Never downgrade to password-less pairing.
            if response
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                return Err(
                    "Desktop signed out of the BitFun account; sign in again and refresh the QR code"
                        .to_string(),
                );
            }
            let user_id = response
                .user_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Missing user ID".to_string())?;
            return Ok(AccountPairingVerification::new(user_id.to_string()));
        };

        let username = response
            .user_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Missing username".to_string())?;
        let password = response
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Missing password".to_string())?;

        verify(username.to_string(), password.to_string()).await
    }

    async fn persist_mobile_identity(
        trusted_mobile_identity: &Arc<RwLock<Option<TrustedMobileIdentity>>>,
        identity: TrustedMobileIdentity,
    ) {
        *trusted_mobile_identity.write().await = Some(identity);
    }

    /// Answer a paired client's `get_delegated_identity` request using the
    /// provider registered by the desktop layer after account login.
    async fn resolve_delegated_identity_response(
        delegated_identity_fn: &Arc<RwLock<Option<DelegatedIdentityFn>>>,
        trusted_mobile_identity: &Arc<RwLock<Option<TrustedMobileIdentity>>>,
        local_device_id: &str,
    ) -> DelegatedIdentityResolution {
        let trusted_identity = trusted_mobile_identity.read().await.clone();
        let Some(trusted_identity) = trusted_identity else {
            return DelegatedIdentityResolution::error(
                "Pairing authorization expired; scan a new QR code",
            );
        };
        let provider = delegated_identity_fn.read().await.clone();
        let Some(get_identity) = provider else {
            return DelegatedIdentityResolution::error(
                "Desktop is not logged into a BitFun account",
            );
        };
        let Some(authorization) = get_identity().await else {
            return DelegatedIdentityResolution::error(
                "Desktop is not logged into a BitFun account",
            );
        };
        if authorization.user_id != trusted_identity.user_id {
            return DelegatedIdentityResolution::error(
                "Paired mobile identity no longer matches the desktop account",
            );
        }
        info!("Delegated identity resolved for paired client");
        authorization.into_response(local_device_id)
    }

    async fn send_pairing_error_response(
        relay_arc: &Arc<RwLock<Option<RelayClient>>>,
        correlation_id: &str,
        shared_secret: &[u8; 32],
        message: String,
    ) {
        let server = RemoteServer::new(*shared_secret);
        if let Ok((enc, nonce)) =
            server.encrypt_response(&remote_server::RemoteResponse::Error { message }, None)
        {
            if let Some(ref client) = *relay_arc.read().await {
                let _ = client
                    .send_relay_response(correlation_id, &enc, &nonce)
                    .await;
            }
        }
    }

    pub fn update_bot_config(&mut self, bot_config: bot::BotConfig) {
        match bot_config {
            bot::BotConfig::Feishu { app_id, app_secret } => {
                self.config.bot_feishu = Some(bot::BotConfig::Feishu { app_id, app_secret });
            }
            bot::BotConfig::Telegram { bot_token } => {
                self.config.bot_telegram = Some(bot::BotConfig::Telegram { bot_token });
            }
            bot::BotConfig::Weixin {
                ilink_token,
                base_url,
                bot_account_id,
            } => {
                self.config.bot_weixin = Some(bot::BotConfig::Weixin {
                    ilink_token,
                    base_url,
                    bot_account_id,
                });
            }
        }
    }

    pub async fn available_methods(&self) -> Vec<ConnectionMethod> {
        vec![
            ConnectionMethod::Lan { ip: None },
            ConnectionMethod::Ngrok,
            ConnectionMethod::BitfunServer,
            ConnectionMethod::CustomServer {
                url: self.config.custom_server_url.clone().unwrap_or_default(),
            },
            ConnectionMethod::BotFeishu,
            ConnectionMethod::BotTelegram,
            ConnectionMethod::BotWeixin,
        ]
    }

    /// Start a remote connection with the given method.
    ///
    /// For relay methods (LAN / ngrok / BitFun Server / Custom Server) this
    /// tears down any existing relay and starts a new one.
    /// For bot methods, this starts the bot pairing flow without affecting
    /// any running relay connection.
    pub async fn start(&self, method: ConnectionMethod) -> Result<ConnectionResult> {
        info!("Starting remote connect: {method:?}");

        match &method {
            ConnectionMethod::BotFeishu
            | ConnectionMethod::BotTelegram
            | ConnectionMethod::BotWeixin => {
                return self.start_bot_connection(&method).await;
            }
            _ => {}
        }

        let _lifecycle = self.relay_lifecycle.lock().await;

        // Relay methods: clean up previous relay (but leave bots alone)
        self.stop_relay_inner().await;

        let result: Result<ConnectionResult> = async {
        let static_dir = self.config.mobile_web_dir.as_deref();

        let relay_url = match &method {
            ConnectionMethod::Lan { ip } => {
                self.embedded_relay_host
                    .start(self.config.lan_port, self.config.mobile_web_dir.clone())
                    .await?;
                let url_result = match ip {
                    Some(ip) => lan::build_lan_relay_url_with_ip(self.config.lan_port, ip),
                    None => lan::build_lan_relay_url(self.config.lan_port),
                };
                match url_result {
                    Ok(url) => url,
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            ConnectionMethod::Ngrok => {
                self.embedded_relay_host
                    .start(self.config.lan_port, self.config.mobile_web_dir.clone())
                    .await?;

                let tunnel = match ngrok::start_ngrok_tunnel(self.config.lan_port).await {
                    Ok(tunnel) => tunnel,
                    Err(e) => {
                        return Err(e);
                    }
                };
                let url = tunnel.public_url.clone();
                *self.ngrok_tunnel.write().await = Some(tunnel);
                url
            }
            ConnectionMethod::BitfunServer => validate_relay_base_url(&self.config.bitfun_server_url)?
                .as_str()
                .trim_end_matches('/')
                .to_string(),
            ConnectionMethod::CustomServer { url } => validate_relay_base_url(url)?
                .as_str()
                .trim_end_matches('/')
                .to_string(),
            _ => unreachable!(),
        };

        let mut pairing = self.pairing.write().await;
        pairing.reset().await;
        let qr_payload = pairing.initiate(&relay_url).await?;

        let ws_url = match &method {
            ConnectionMethod::Lan { .. } | ConnectionMethod::Ngrok => {
                format!("ws://127.0.0.1:{}/ws", self.config.lan_port)
            }
            _ => build_relay_websocket_url(&relay_url)?,
        };

        let (client, mut event_rx) = RelayClient::new();
        client.connect(&ws_url).await?;
        client
            .create_room(
                &self.device_identity.device_id,
                &qr_payload.public_key,
                Some(&qr_payload.room_id),
            )
            .await?;

        // Wait for RoomCreated before HTTP upload / QR generation so the relay
        // has registered the room (avoids upload 404 races on BitFun/Custom).
        // Mirror start_device_connection's AuthOk wait pattern.
        {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
            let expected_room = qr_payload.room_id.clone();
            let mut got_room = false;
            while !got_room {
                match tokio::time::timeout_at(deadline, event_rx.recv()).await {
                    Ok(Some(relay_client::RelayEvent::RoomCreated { room_id })) => {
                        if room_id == expected_room {
                            info!("Room created on relay: {room_id}");
                            got_room = true;
                        } else {
                            log::warn!(
                                "Unexpected RoomCreated room_id={room_id}, expected={expected_room}"
                            );
                        }
                    }
                    Ok(Some(other)) => {
                        log::debug!(
                            "Skipping relay event while waiting for RoomCreated: {other:?}"
                        );
                    }
                    Ok(None) => {
                        anyhow::bail!("relay connection closed before RoomCreated");
                    }
                    Err(_) => {
                        anyhow::bail!("timeout waiting for RoomCreated");
                    }
                }
            }
        }

        let web_app_url: String = match &method {
            ConnectionMethod::Lan { .. } | ConnectionMethod::Ngrok => relay_url.clone(),
            ConnectionMethod::BitfunServer => {
                if let Some(web_dir) = static_dir {
                    match upload_mobile_web_to_relay(&relay_url, &qr_payload.room_id, web_dir).await
                    {
                        Ok(()) => {
                            let url = format!(
                                "{}/r/{}",
                                relay_url.trim_end_matches('/'),
                                qr_payload.room_id
                            );
                            info!("Uploaded mobile-web to relay: {url}");
                            url
                        }
                        Err(e) => {
                            error!("Failed to upload mobile-web to relay: {e}; falling back to server-hosted version");
                            self.config.web_app_url.clone()
                        }
                    }
                } else {
                    info!("No mobile_web_dir configured; using server-hosted mobile web");
                    self.config.web_app_url.clone()
                }
            }
            ConnectionMethod::CustomServer { .. } => {
                if let Some(web_dir) = static_dir {
                    match upload_mobile_web_to_relay(&relay_url, &qr_payload.room_id, web_dir).await
                    {
                        Ok(()) => {
                            let url = format!(
                                "{}/r/{}",
                                relay_url.trim_end_matches('/'),
                                qr_payload.room_id
                            );
                            info!("Uploaded mobile-web to relay: {url}");
                            url
                        }
                        Err(e) => {
                            error!("Failed to upload mobile-web to custom relay: {e}; using custom server URL directly");
                            relay_url.clone()
                        }
                    }
                } else {
                    info!("No mobile_web_dir configured; using custom server URL directly");
                    relay_url.clone()
                }
            }
            _ => self.config.web_app_url.clone(),
        };

        let client_language = crate::service::config::get_app_language_code().await;
        let account_username = self.account_pairing_username.read().await.clone();
        let qr_url = QrGenerator::build_url(
            &qr_payload,
            &web_app_url,
            &client_language,
            account_username.as_deref(),
        );
        let qr_svg = QrGenerator::generate_svg_from_url(&qr_url)?;
        let qr_data = QrGenerator::generate_png_base64_from_url(&qr_url)?;

        *self.active_method.write().await = Some(method.clone());
        *self.relay_client.write().await = Some(client);
        let room_owner = RoomConnectionOwner {
            generation: self
                .room_connection_generation
                .fetch_add(1, Ordering::AcqRel)
                + 1,
            room_id: qr_payload.room_id.clone(),
        };
        *self.active_room_owner.write().await = Some(room_owner.clone());

        let pairing_arc = self.pairing.clone();
        let relay_arc = self.relay_client.clone();
        let server_arc = self.remote_server.clone();
        let active_method_arc = self.active_method.clone();
        let room_lifecycle = self.relay_lifecycle.clone();
        let active_room_owner = self.active_room_owner.clone();
        let trusted_mobile_identity_arc = self.trusted_mobile_identity.clone();
        let delegated_identity_fn_arc = self.delegated_identity_fn.clone();
        let account_pairing_verifier_arc = self.account_pairing_verifier.clone();
        let local_device_id = self.device_identity.device_id.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Lease only this event's effects. `start`/`stop_relay` can
                // acquire the lifecycle mutex between events, and a retiring
                // loop observes the owner mismatch before touching shared
                // pairing/client/server state.
                let _room_effect = room_lifecycle.lock().await;
                if !room_owner_is_current(&*active_room_owner.read().await, &room_owner) {
                    break;
                }
                match event {
                    relay_client::RelayEvent::PairRequest {
                        correlation_id,
                        public_key,
                        device_id,
                        device_name: _,
                    } => {
                        info!("PairRequest from {device_id}");
                        let mut p = pairing_arc.write().await;
                        match p.on_peer_joined(&public_key).await {
                            Ok(challenge) => {
                                if let Some(secret) = p.shared_secret() {
                                    let challenge_json =
                                        serde_json::to_string(&challenge).unwrap_or_default();
                                    if let Ok((enc, nonce)) =
                                        encryption::encrypt_to_base64(secret, &challenge_json)
                                    {
                                        if let Some(ref client) = *relay_arc.read().await {
                                            let _ = client
                                                .send_relay_response(&correlation_id, &enc, &nonce)
                                                .await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Pairing error on pair_request: {e}");
                            }
                        }
                    }
                    relay_client::RelayEvent::CommandReceived {
                        correlation_id,
                        encrypted_data,
                        nonce,
                    } => {
                        let mut handled_as_active_command = false;
                        {
                            let server_guard = server_arc.read().await;
                            if let Some(ref server) = *server_guard {
                                match server.decrypt_command(&encrypted_data, &nonce) {
                                    Ok((cmd, request_id)) => {
                                        handled_as_active_command = true;
                                        debug!("Remote command decrypted");
                                        let response_resolution = if matches!(
                                            cmd,
                                            remote_server::RemoteCommand::GetDelegatedIdentity
                                        ) {
                                            RemoteConnectService::resolve_delegated_identity_response(
                                                &delegated_identity_fn_arc,
                                                &trusted_mobile_identity_arc,
                                                &local_device_id,
                                            )
                                            .await
                                        } else {
                                            DelegatedIdentityResolution {
                                                response: server.dispatch(&cmd).await,
                                                _host_lease: None,
                                            }
                                        };
                                        match server
                                            .encrypt_response(
                                                &response_resolution.response,
                                                request_id.as_deref(),
                                            )
                                        {
                                            Ok((enc, resp_nonce)) => {
                                                if let Some(ref client) = *relay_arc.read().await {
                                                    let _ = client
                                                        .send_relay_response(
                                                            &correlation_id,
                                                            &enc,
                                                            &resp_nonce,
                                                        )
                                                        .await;
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to encrypt response: {e}");
                                            }
                                        }
                                        // `response_resolution` owns the account lease for
                                        // delegated credentials. Keep it alive through both
                                        // encryption and the awaited room send above.
                                        drop(response_resolution);
                                    }
                                    Err(e) => {
                                        debug!(
                                            "Active session could not decrypt command, falling back to pairing verification: {e}"
                                        );
                                    }
                                }
                            }
                        }
                        if handled_as_active_command {
                            continue;
                        }

                        let p = pairing_arc.read().await;
                        if let Some(secret) = p.shared_secret() {
                            let shared_secret = *secret;
                            if let Ok(json) = encryption::decrypt_from_base64(
                                &shared_secret,
                                &encrypted_data,
                                &nonce,
                            ) {
                                if let Ok(response) =
                                    serde_json::from_str::<pairing::PairingResponse>(&json)
                                {
                                    if let Err(error) = response.validate_untrusted() {
                                        drop(p);
                                        RemoteConnectService::send_pairing_error_response(
                                            &relay_arc,
                                            &correlation_id,
                                            &shared_secret,
                                            format!("Invalid pairing response: {error}"),
                                        )
                                        .await;
                                        continue;
                                    }
                                    let account_verification = match RemoteConnectService::resolve_pairing_user_id(
                                        &account_pairing_verifier_arc,
                                        &response,
                                    )
                                    .await
                                    {
                                        Ok(user_id) => user_id,
                                        Err(message) => {
                                            drop(p);
                                            pairing_arc
                                                .write()
                                                .await
                                                .retry_after_identity_rejection()
                                                .await;
                                            RemoteConnectService::send_pairing_error_response(
                                                &relay_arc,
                                                &correlation_id,
                                                &shared_secret,
                                                message,
                                            )
                                            .await;
                                            continue;
                                        }
                                    };
                                    let canonical_user_id =
                                        account_verification.user_id().to_string();
                                    let mobile_install_id = response
                                        .mobile_install_id
                                        .clone()
                                        .unwrap_or_default();
                                    let submitted_identity =
                                        match RemoteConnectService::validate_mobile_identity(
                                            &trusted_mobile_identity_arc,
                                            &mobile_install_id,
                                            &canonical_user_id,
                                        )
                                        .await
                                        {
                                            Ok(identity) => identity,
                                            Err(message) => {
                                                drop(p);
                                                pairing_arc
                                                    .write()
                                                    .await
                                                    .retry_after_identity_rejection()
                                                    .await;
                                                RemoteConnectService::send_pairing_error_response(
                                                    &relay_arc,
                                                    &correlation_id,
                                                    &shared_secret,
                                                    message,
                                                )
                                                .await;
                                                continue;
                                            }
                                        };
                                    drop(p);
                                    let mut pw = pairing_arc.write().await;
                                    match pw.verify_response(&response).await {
                                        Ok(true) => {
                                            info!("Pairing verified successfully");
                                            RemoteConnectService::persist_mobile_identity(
                                                &trusted_mobile_identity_arc,
                                                submitted_identity.clone(),
                                            )
                                            .await;
                                            if let Some(s) = pw.shared_secret() {
                                                let server = RemoteServer::new(*s);

                                                let initial_sync = server
                                                    .generate_initial_sync(Some(
                                                        submitted_identity.user_id.clone(),
                                                    ))
                                                    .await;
                                                if let Ok((enc, resp_nonce)) =
                                                    server.encrypt_response(&initial_sync, None)
                                                {
                                                    if let Some(ref client) =
                                                        *relay_arc.read().await
                                                    {
                                                        info!(
                                                            "Sending initial sync to mobile after pairing"
                                                        );
                                                        let _ = client
                                                            .send_relay_response(
                                                                &correlation_id,
                                                                &enc,
                                                                &resp_nonce,
                                                            )
                                                            .await;
                                                    }
                                                }

                                                *server_arc.write().await = Some(server);

                                                // The delegated account identity is NOT
                                                // pushed here: the relay pending request
                                                // for this correlation_id is consumed by
                                                // the initial_sync response, so a second
                                                // frame would be dropped. Paired clients
                                                // pull it via `get_delegated_identity`.
                                            }
                                            // Keep the host's account lease through every
                                            // successful pairing commit and response-side effect.
                                            drop(account_verification);
                                        }
                                        Ok(false) => {
                                            error!("Pairing verification failed");
                                            RemoteConnectService::send_pairing_error_response(
                                                &relay_arc,
                                                &correlation_id,
                                                &shared_secret,
                                                "Pairing verification failed".to_string(),
                                            )
                                            .await;
                                        }
                                        Err(e) => {
                                            error!("Pairing verification error: {e}");
                                            RemoteConnectService::send_pairing_error_response(
                                                &relay_arc,
                                                &correlation_id,
                                                &shared_secret,
                                                format!("Pairing verification error: {e}"),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    relay_client::RelayEvent::Reconnected => {
                        info!("Relay reconnected — pairing + server preserved for mobile polling");
                    }
                    relay_client::RelayEvent::Disconnected => {
                        info!("Relay disconnected");
                        pairing_arc.write().await.disconnect().await;
                        *server_arc.write().await = None;
                    }
                    relay_client::RelayEvent::Error { message } => {
                        error!("Relay error: {message}");
                        if message.contains("Room not found") {
                            info!("Room expired, disconnecting");
                            pairing_arc.write().await.disconnect().await;
                            *server_arc.write().await = None;
                        }
                    }
                    _ => {}
                }
            }

            // Stream exit is itself generation-fenced. In particular, an old
            // loop closing after reconnect must not disconnect the new room.
            let _room_effect = room_lifecycle.lock().await;
            let mut owner = active_room_owner.write().await;
            if clear_room_owner_if_current(&mut *owner, &room_owner) {
                drop(owner);
                *relay_arc.write().await = None;
                pairing_arc.write().await.disconnect().await;
                *server_arc.write().await = None;
                *active_method_arc.write().await = None;
                *trusted_mobile_identity_arc.write().await = None;
            }
        });

        let state = pairing.state().await;
        Ok(ConnectionResult {
            method,
            qr_data: Some(qr_data),
            qr_svg: Some(qr_svg),
            qr_url: Some(qr_url),
            bot_pairing_code: None,
            bot_link: None,
            pairing_state: state,
        })
        }
        .await;

        if result.is_err() {
            self.stop_relay_inner().await;
        }
        result
    }

    async fn start_bot_connection(&self, method: &ConnectionMethod) -> Result<ConnectionResult> {
        let _lifecycle = self.bot_lifecycle.lock().await;
        let pairing_code = PairingProtocol::generate_bot_pairing_code();

        let bot_link = match method {
            ConnectionMethod::BotTelegram => {
                match &self.config.bot_telegram {
                    Some(bot::BotConfig::Telegram { bot_token }) if !bot_token.is_empty() => {
                        // Stop any existing Telegram bot
                        let generation = self.bot_telegram_slot.advance();
                        if let Some(handle) = self.bot_telegram_handle.write().await.take() {
                            handle.stop();
                        }

                        let tg_bot = Arc::new(bot::telegram::TelegramBot::new_fenced(
                            bot::telegram::TelegramConfig {
                                bot_token: bot_token.clone(),
                            },
                            bot::BotRuntimeFence::new(
                                self.bot_account_identity_epoch.clone(),
                                self.bot_telegram_slot.clone(),
                                generation,
                            ),
                        ));
                        tg_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = tg_bot.clone();
                        let bot_for_loop = tg_bot.clone();
                        let tg_bot_ref = self.telegram_bot.clone();
                        let bot_lifecycle = self.bot_lifecycle.clone();
                        let bot_slot = self.bot_telegram_slot.clone();

                        *tg_bot_ref.write().await = Some(tg_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(chat_id) => {
                                    let lifecycle = bot_lifecycle.lock().await;
                                    // Guard against the race where stop_bots() cleared
                                    // bot_connected_info between pairing completing and
                                    // this task running.
                                    if !*stop_rx.borrow() && bot_slot.is_current(generation) {
                                        *bot_connected_info.write().await =
                                            Some(format!("Telegram({chat_id})"));
                                        drop(lifecycle);
                                        info!("Telegram bot paired, starting message loop");
                                        bot_for_loop.run_message_loop(stop_rx).await;
                                    } else {
                                        info!("Telegram pairing completed but bot was stopped; discarding");
                                    }
                                }
                                Err(e) => {
                                    info!("Telegram pairing ended: {e}");
                                }
                            }
                        });

                        *self.bot_telegram_handle.write().await = Some(BotHandle { stop_tx });

                        "https://t.me/BotFather".to_string()
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Telegram bot token not configured. Please set bot token first."
                        ));
                    }
                }
            }
            ConnectionMethod::BotFeishu => {
                match &self.config.bot_feishu {
                    Some(bot::BotConfig::Feishu { app_id, app_secret })
                        if !app_id.is_empty() && !app_secret.is_empty() =>
                    {
                        let generation = self.bot_feishu_slot.advance();
                        if let Some(handle) = self.bot_feishu_handle.write().await.take() {
                            handle.stop();
                        }

                        let fs_bot = Arc::new(bot::feishu::FeishuBot::new_fenced(
                            bot::feishu::FeishuConfig {
                                app_id: app_id.clone(),
                                app_secret: app_secret.clone(),
                            },
                            bot::BotRuntimeFence::new(
                                self.bot_account_identity_epoch.clone(),
                                self.bot_feishu_slot.clone(),
                                generation,
                            ),
                        ));
                        fs_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = fs_bot.clone();
                        let bot_for_loop = fs_bot.clone();
                        let fs_bot_ref = self.feishu_bot.clone();
                        let bot_lifecycle = self.bot_lifecycle.clone();
                        let bot_slot = self.bot_feishu_slot.clone();

                        *fs_bot_ref.write().await = Some(fs_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(chat_id) => {
                                    let lifecycle = bot_lifecycle.lock().await;
                                    // Guard against the race where stop_bots() cleared
                                    // bot_connected_info between pairing completing and
                                    // this task running.
                                    if !*stop_rx.borrow() && bot_slot.is_current(generation) {
                                        *bot_connected_info.write().await =
                                            Some(format!("Feishu({chat_id})"));
                                        drop(lifecycle);
                                        info!("Feishu bot paired, starting message loop");
                                        bot_for_loop.run_message_loop(stop_rx).await;
                                    } else {
                                        info!("Feishu pairing completed but bot was stopped; discarding");
                                    }
                                }
                                Err(e) => {
                                    info!("Feishu pairing ended: {e}");
                                }
                            }
                        });

                        *self.bot_feishu_handle.write().await = Some(BotHandle { stop_tx });

                        "https://open.feishu.cn/app".to_string()
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Feishu bot credentials not configured. \
                             Please set App ID and App Secret first."
                        ));
                    }
                }
            }
            ConnectionMethod::BotWeixin => {
                match &self.config.bot_weixin {
                    Some(bot::BotConfig::Weixin {
                        ilink_token,
                        base_url,
                        bot_account_id,
                    }) if !ilink_token.is_empty() && !bot_account_id.is_empty() => {
                        let generation = self.bot_weixin_slot.advance();
                        if let Some(handle) = self.bot_weixin_handle.write().await.take() {
                            handle.stop();
                        }

                        let wx_cfg = bot::weixin::WeixinConfig {
                            ilink_token: ilink_token.clone(),
                            base_url: if base_url.trim().is_empty() {
                                "https://ilinkai.weixin.qq.com".to_string()
                            } else {
                                base_url.clone()
                            },
                            bot_account_id: bot_account_id.clone(),
                        };

                        let wx_bot = Arc::new(bot::weixin::WeixinBot::new_fenced(
                            wx_cfg,
                            bot::BotRuntimeFence::new(
                                self.bot_account_identity_epoch.clone(),
                                self.bot_weixin_slot.clone(),
                                generation,
                            ),
                        ));
                        wx_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = wx_bot.clone();
                        let bot_for_loop = wx_bot.clone();
                        let wx_bot_ref = self.weixin_bot.clone();
                        let bot_lifecycle = self.bot_lifecycle.clone();
                        let bot_slot = self.bot_weixin_slot.clone();

                        *wx_bot_ref.write().await = Some(wx_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(peer_id) => {
                                    let lifecycle = bot_lifecycle.lock().await;
                                    if !*stop_rx.borrow() && bot_slot.is_current(generation) {
                                        *bot_connected_info.write().await =
                                            Some(format!("Weixin({peer_id})"));
                                        drop(lifecycle);
                                        info!("Weixin bot paired, starting message loop");
                                        bot_for_loop.run_message_loop(stop_rx).await;
                                    } else {
                                        info!("Weixin pairing completed but bot was stopped; discarding");
                                    }
                                }
                                Err(e) => {
                                    info!("Weixin pairing ended: {e}");
                                }
                            }
                        });

                        *self.bot_weixin_handle.write().await = Some(BotHandle { stop_tx });

                        "https://www.wechat.com".to_string()
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Weixin not linked. Complete WeChat QR login in Remote Connect first."
                        ));
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "start_bot_connection: unsupported method {method:?}"
                ));
            }
        };

        Ok(ConnectionResult {
            method: method.clone(),
            qr_data: None,
            qr_svg: None,
            qr_url: None,
            bot_pairing_code: Some(pairing_code),
            bot_link: Some(bot_link),
            pairing_state: PairingState::WaitingForScan,
        })
    }

    /// Restore a previously paired bot from persistence.
    /// Skips the pairing step and directly starts the message loop.
    pub async fn restore_bot(&self, saved: &bot::SavedBotConnection) -> Result<()> {
        let _lifecycle = self.bot_lifecycle.lock().await;
        match saved.config {
            bot::BotConfig::Telegram { ref bot_token } => {
                let generation = self.bot_telegram_slot.advance();
                if let Some(handle) = self.bot_telegram_handle.write().await.take() {
                    handle.stop();
                }

                let tg_bot = Arc::new(bot::telegram::TelegramBot::new_fenced(
                    bot::telegram::TelegramConfig {
                        bot_token: bot_token.clone(),
                    },
                    bot::BotRuntimeFence::new(
                        self.bot_account_identity_epoch.clone(),
                        self.bot_telegram_slot.clone(),
                        generation,
                    ),
                ));

                let chat_id: i64 = saved.chat_id.parse().map_err(|_| {
                    anyhow::anyhow!("invalid saved telegram chat_id: {}", saved.chat_id)
                })?;
                tg_bot
                    .restore_chat_state(chat_id, saved.chat_state.clone())
                    .await;

                let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                *self.telegram_bot.write().await = Some(tg_bot.clone());
                *self.bot_connected_info.write().await = Some(format!("Telegram({chat_id})"));

                let bot_for_loop = tg_bot.clone();
                tokio::spawn(async move {
                    info!("Telegram bot restored from persistence, starting message loop");
                    bot_for_loop.run_message_loop(stop_rx).await;
                });

                *self.bot_telegram_handle.write().await = Some(BotHandle { stop_tx });
                info!("Telegram bot restored for chat_id={chat_id}");
            }
            bot::BotConfig::Feishu {
                ref app_id,
                ref app_secret,
            } => {
                let generation = self.bot_feishu_slot.advance();
                if let Some(handle) = self.bot_feishu_handle.write().await.take() {
                    handle.stop();
                }

                let fs_bot = Arc::new(bot::feishu::FeishuBot::new_fenced(
                    bot::feishu::FeishuConfig {
                        app_id: app_id.clone(),
                        app_secret: app_secret.clone(),
                    },
                    bot::BotRuntimeFence::new(
                        self.bot_account_identity_epoch.clone(),
                        self.bot_feishu_slot.clone(),
                        generation,
                    ),
                ));

                fs_bot
                    .restore_chat_state(&saved.chat_id, saved.chat_state.clone())
                    .await;

                let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                *self.feishu_bot.write().await = Some(fs_bot.clone());

                let cid = saved.chat_id.clone();
                *self.bot_connected_info.write().await = Some(format!("Feishu({cid})"));

                let bot_for_loop = fs_bot.clone();
                tokio::spawn(async move {
                    info!("Feishu bot restored from persistence, starting message loop");
                    bot_for_loop.run_message_loop(stop_rx).await;
                });

                *self.bot_feishu_handle.write().await = Some(BotHandle { stop_tx });
                info!("Feishu bot restored for chat_id={}", saved.chat_id);
            }
            bot::BotConfig::Weixin {
                ref ilink_token,
                ref base_url,
                ref bot_account_id,
            } => {
                let generation = self.bot_weixin_slot.advance();
                if let Some(handle) = self.bot_weixin_handle.write().await.take() {
                    handle.stop();
                }

                let wx_cfg = bot::weixin::WeixinConfig {
                    ilink_token: ilink_token.clone(),
                    base_url: if base_url.trim().is_empty() {
                        "https://ilinkai.weixin.qq.com".to_string()
                    } else {
                        base_url.clone()
                    },
                    bot_account_id: bot_account_id.clone(),
                };

                let wx_bot = Arc::new(bot::weixin::WeixinBot::new_fenced(
                    wx_cfg,
                    bot::BotRuntimeFence::new(
                        self.bot_account_identity_epoch.clone(),
                        self.bot_weixin_slot.clone(),
                        generation,
                    ),
                ));
                wx_bot
                    .restore_chat_state(&saved.chat_id, saved.chat_state.clone())
                    .await;

                let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                *self.weixin_bot.write().await = Some(wx_bot.clone());

                let cid = saved.chat_id.clone();
                *self.bot_connected_info.write().await = Some(format!("Weixin({cid})"));

                let bot_for_loop = wx_bot.clone();
                tokio::spawn(async move {
                    info!("Weixin bot restored from persistence, starting message loop");
                    bot_for_loop.run_message_loop(stop_rx).await;
                });

                *self.bot_weixin_handle.write().await = Some(BotHandle { stop_tx });
                info!("Weixin bot restored for chat_id={}", saved.chat_id);
            }
        }
        Ok(())
    }

    pub async fn pairing_state(&self) -> PairingState {
        self.pairing.read().await.state().await
    }

    /// Stop relay connections (LAN / ngrok / BitFun Server / Custom Server).
    /// Bot connections are left running.
    pub async fn stop_relay(&self) {
        let _lifecycle = self.relay_lifecycle.lock().await;
        self.stop_relay_inner().await;
    }

    async fn stop_relay_inner(&self) {
        // Fence the retiring event loop before disconnecting its client. A
        // late event or stream-exit cleanup must not mutate the next room.
        *self.active_room_owner.write().await = None;
        if let Some(ref client) = *self.relay_client.read().await {
            client.disconnect().await;
        }
        *self.relay_client.write().await = None;
        *self.remote_server.write().await = None;
        *self.active_method.write().await = None;

        if let Some(ref mut tunnel) = *self.ngrok_tunnel.write().await {
            tunnel.stop().await;
        }
        *self.ngrok_tunnel.write().await = None;

        self.embedded_relay_host.stop().await;

        self.pairing.write().await.reset().await;
        *self.trusted_mobile_identity.write().await = None;
        info!("Relay connections stopped (bots unaffected)");
    }

    /// Stop all bot connections.
    pub async fn stop_bots(&self) {
        let _lifecycle = self.bot_lifecycle.lock().await;
        self.bot_telegram_slot.advance();
        if let Some(handle) = self.bot_telegram_handle.write().await.take() {
            handle.stop();
        }
        *self.telegram_bot.write().await = None;

        self.bot_feishu_slot.advance();
        if let Some(handle) = self.bot_feishu_handle.write().await.take() {
            handle.stop();
        }
        *self.feishu_bot.write().await = None;

        self.bot_weixin_slot.advance();
        if let Some(handle) = self.bot_weixin_handle.write().await.take() {
            handle.stop();
        }
        *self.weixin_bot.write().await = None;
        *self.bot_connected_info.write().await = None;

        info!("Bot connections stopped");
    }

    /// Legacy `stop()` — only stops relay for backward compatibility.
    /// Bot connections persist independently.
    pub async fn stop(&self) {
        self.stop_relay().await;
    }

    /// Stop everything (relay + bots).
    pub async fn stop_all(&self) {
        self.stop_relay().await;
        self.stop_bots().await;
    }

    pub async fn is_connected(&self) -> bool {
        self.pairing.read().await.state().await == PairingState::Connected
    }

    pub async fn active_method(&self) -> Option<ConnectionMethod> {
        self.active_method.read().await.clone()
    }

    pub async fn peer_device_name(&self) -> Option<String> {
        self.pairing
            .read()
            .await
            .peer_device_name()
            .map(String::from)
    }

    /// Check whether a specific bot type is currently running.
    pub async fn is_bot_running(&self, bot_type: &str) -> bool {
        match bot_type {
            "telegram" => self.bot_telegram_handle.read().await.is_some(),
            "feishu" => self.bot_feishu_handle.read().await.is_some(),
            "weixin" => self.bot_weixin_handle.read().await.is_some(),
            _ => false,
        }
    }

    pub async fn bot_connected_info(&self) -> Option<String> {
        self.bot_connected_info.read().await.clone()
    }

    pub async fn trusted_mobile_user_id(&self) -> Option<String> {
        self.trusted_mobile_identity
            .read()
            .await
            .as_ref()
            .map(|identity| identity.user_id.clone())
    }

    // ── P2: Account-authenticated device routing ───────────────────────────

    /// Connect to the relay's WS endpoint and authenticate with an account
    /// token. This establishes a parallel device-routing pathway that does not
    /// interfere with the room-pairing flow.  Incoming device messages are
    /// forwarded via the returned event receiver.
    ///
    /// The caller (desktop Tauri layer) owns the AccountSession containing the
    /// master_key and is responsible for decrypting device-message payloads.
    /// Returns true if a device-routing WebSocket connection is currently
    /// active (i.e. `start_device_connection` has been called and not yet
    /// disconnected).
    pub async fn is_device_connected(&self) -> bool {
        let guard = self.device_relay_client.read().await;
        let Some(client) = guard.as_ref() else {
            return false;
        };
        matches!(
            client.connection_state().await,
            relay_client::ConnectionState::Connected | relay_client::ConnectionState::Reconnecting
        )
    }

    /// Start account device routing. Returns
    /// `(event_rx, authenticated_device_id, connection_id)`.
    ///
    /// `AuthOk` is consumed here (not forwarded) so callers must use the returned
    /// `authenticated_device_id` — and this method adopts it into the persisted
    /// local `DeviceIdentity` before returning.
    pub async fn start_device_connection(
        &self,
        relay_url: &str,
        token: &str,
        device_name: &str,
    ) -> Result<(
        tokio::sync::mpsc::UnboundedReceiver<relay_client::RelayEvent>,
        String,
        u64,
    )> {
        let _lifecycle = self.device_relay_lifecycle.lock().await;
        // Disconnect previous device connection if any.
        self.stop_device_connection_inner().await;

        let ws_url = build_relay_websocket_url(relay_url)?;

        let (client, mut event_rx) = RelayClient::new();
        client.connect(&ws_url).await?;
        client.connect_authenticated(token, device_name).await?;

        // Wait for AuthOk (or AuthError) before proceeding so that the
        // device is registered as online on the relay before the caller
        // gets back control.  This prevents an immediate device-list
        // query from seeing the local device as offline.
        //
        // The relay client may emit other events first (e.g. `Connected`),
        // so we loop until we see AuthOk/AuthError or time out.
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut authenticated_device_id: Option<String> = None;
        let mut auth_error: Option<String> = None;
        while authenticated_device_id.is_none() && auth_error.is_none() {
            match tokio::time::timeout_at(deadline, event_rx.recv()).await {
                Ok(Some(relay_client::RelayEvent::AuthOk { user_id, device_id })) => {
                    log::info!("Device connection auth ok: user={user_id} device={device_id}");
                    // AuthOk is consumed here and never forwarded. Adopt immediately
                    // so getDeviceInfo / 本机 marking match the token-bound device.
                    if let Err(e) = DeviceIdentity::adopt_account_device_id(&device_id) {
                        log::warn!("Failed to adopt AuthOk device_id: {e}");
                    }
                    authenticated_device_id = Some(device_id);
                }
                Ok(Some(relay_client::RelayEvent::AuthError { message })) => {
                    auth_error = Some(message);
                }
                Ok(Some(other)) => {
                    // Non-auth event (e.g. Connected) — skip and keep waiting.
                    log::debug!(
                        "Skipping non-auth relay event while waiting for AuthOk: {other:?}"
                    );
                }
                Ok(None) => {
                    anyhow::bail!("relay connection closed before auth response");
                }
                Err(_) => {
                    anyhow::bail!("timeout waiting for relay auth response");
                }
            }
        }
        if let Some(msg) = auth_error {
            anyhow::bail!("relay auth error: {msg}");
        }
        let authenticated_device_id = authenticated_device_id
            .ok_or_else(|| anyhow::anyhow!("relay auth completed without device_id"))?;

        let online_arc = self.online_devices.clone();
        let device_client_arc = self.device_relay_client.clone();
        let device_lifecycle = self.device_relay_lifecycle.clone();
        let active_connection_id = self.active_device_connection_id.clone();
        let connection_id = self
            .device_connection_generation
            .fetch_add(1, Ordering::AcqRel)
            + 1;
        *device_client_arc.write().await = Some(client);
        *active_connection_id.write().await = Some(connection_id);
        // Spawn event forwarder that updates presence state; the raw event stream
        // is also forwarded to a new channel for the caller to consume.
        let (forward_tx, forward_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let _effect = device_lifecycle.lock().await;
                if *active_connection_id.read().await != Some(connection_id) {
                    break;
                }
                match &event {
                    relay_client::RelayEvent::DevicePresence { devices } => {
                        *online_arc.write().await = devices.clone();
                    }
                    relay_client::RelayEvent::Disconnected => {
                        *online_arc.write().await = Vec::new();
                    }
                    _ => {}
                }
                let _ = forward_tx.send(event);
            }
            let _effect = device_lifecycle.lock().await;
            let mut active = active_connection_id.write().await;
            if *active == Some(connection_id) {
                *active = None;
                drop(active);
                *device_client_arc.write().await = None;
                online_arc.write().await.clear();
            }
        });

        Ok((forward_rx, authenticated_device_id, connection_id))
    }

    /// Disconnect the account-authenticated device-routing connection.
    pub async fn stop_device_connection(&self) {
        let _lifecycle = self.device_relay_lifecycle.lock().await;
        self.stop_device_connection_inner().await;
    }

    async fn stop_device_connection_inner(&self) {
        *self.active_device_connection_id.write().await = None;
        if let Some(client) = self.device_relay_client.write().await.take() {
            client.disconnect().await;
        }
        self.online_devices.write().await.clear();
    }

    /// Send an encrypted device-to-device message via the account relay.
    pub async fn send_device_message(
        &self,
        target_device_id: &str,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        let guard = self.device_relay_client.read().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("device routing not connected"))?;
        client
            .send_device_message(target_device_id, correlation_id, encrypted_data, nonce)
            .await
    }

    /// Send through the exact device-routing client captured by the caller.
    /// The lifecycle lease prevents connection replacement between the owner
    /// check and the transport write.
    pub async fn send_device_message_if_connection(
        &self,
        expected_connection_id: u64,
        target_device_id: &str,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<bool> {
        let _lifecycle = self.device_relay_lifecycle.lock().await;
        if *self.active_device_connection_id.read().await != Some(expected_connection_id) {
            return Ok(false);
        }
        let guard = self.device_relay_client.read().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("device routing not connected"))?;
        client
            .send_device_message(target_device_id, correlation_id, encrypted_data, nonce)
            .await?;
        Ok(true)
    }

    /// Current online devices in the account (presence list).
    pub async fn online_devices(&self) -> Vec<relay_client::DevicePresenceEntry> {
        self.online_devices.read().await.clone()
    }

    /// Send an encrypted response to the paired mobile/IM client via the room
    /// channel. Used to delegate account identity after pairing.
    pub async fn send_room_response(
        &self,
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<()> {
        let guard = self.relay_client.read().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("relay client not connected"))?;
        client
            .send_relay_response(correlation_id, encrypted_data, nonce)
            .await
    }

    /// Send only if the relay connection still belongs to the pairing secret
    /// captured by the caller. Holding the relay lifecycle and pairing read
    /// leases across the send prevents a concurrently replaced room from
    /// receiving a response prepared for the previous room.
    pub async fn send_room_response_if_pairing_secret(
        &self,
        expected_secret: &[u8; 32],
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
    ) -> Result<bool> {
        let _lifecycle = self.relay_lifecycle.lock().await;
        let pairing = self.pairing.read().await;
        if pairing.shared_secret() != Some(expected_secret) {
            return Ok(false);
        }
        let guard = self.relay_client.read().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("relay client not connected"))?;
        client
            .send_relay_response(correlation_id, encrypted_data, nonce)
            .await?;
        Ok(true)
    }

    /// Room-first variant for host-authorized secret-bearing responses. The
    /// returned authorization lease stays alive through the transport write;
    /// hosts can use it to keep account replacement from completing without
    /// introducing an account-lock -> room-lock inversion.
    pub async fn send_room_response_if_pairing_secret_authorized<F, Fut, L>(
        &self,
        expected_secret: &[u8; 32],
        correlation_id: &str,
        encrypted_data: &str,
        nonce: &str,
        authorize: F,
    ) -> Result<bool>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<L, String>>,
        L: Send,
    {
        let _lifecycle = self.relay_lifecycle.lock().await;
        let pairing = self.pairing.read().await;
        if pairing.shared_secret() != Some(expected_secret) {
            return Ok(false);
        }
        let _authorization = authorize().await.map_err(anyhow::Error::msg)?;
        if pairing.shared_secret() != Some(expected_secret) {
            return Ok(false);
        }
        let guard = self.relay_client.read().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("relay client not connected"))?;
        client
            .send_relay_response(correlation_id, encrypted_data, nonce)
            .await?;
        Ok(true)
    }

    /// Get the pairing shared secret (for encrypting delegate identity).
    pub async fn pairing_shared_secret(&self) -> Option<[u8; 32]> {
        self.pairing.read().await.shared_secret().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn pairing_response(user_id: Option<&str>, password: Option<&str>) -> pairing::PairingResponse {
        pairing::PairingResponse {
            challenge_echo: "echo".to_string(),
            device_id: "mobile-1".to_string(),
            device_name: "Phone".to_string(),
            mobile_install_id: Some("install-1".to_string()),
            user_id: user_id.map(str::to_string),
            password: password.map(str::to_string),
        }
    }

    fn no_verifier() -> Arc<RwLock<Option<AccountPairingVerifierFn>>> {
        Arc::new(RwLock::new(None))
    }

    #[test]
    fn retiring_room_owner_cannot_clear_replacement() {
        let owner_a = RoomConnectionOwner {
            generation: 1,
            room_id: "room-a".to_string(),
        };
        let owner_b = RoomConnectionOwner {
            generation: 2,
            room_id: "room-b".to_string(),
        };
        let mut active = Some(owner_b.clone());

        assert!(!clear_room_owner_if_current(&mut active, &owner_a));
        assert_eq!(active, Some(owner_b.clone()));
        assert!(clear_room_owner_if_current(&mut active, &owner_b));
        assert_eq!(active, None);
    }

    fn verifier_returning(
        canonical_user_id: &'static str,
    ) -> Arc<RwLock<Option<AccountPairingVerifierFn>>> {
        Arc::new(RwLock::new(Some(
            Arc::new(move |_username: String, _password: String| {
                Box::pin(async move {
                    Ok(AccountPairingVerification::new(
                        canonical_user_id.to_string(),
                    ))
                })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<AccountPairingVerification, String>,
                                > + Send
                                + Sync,
                        >,
                    >
            }) as AccountPairingVerifierFn,
        )))
    }

    #[tokio::test]
    async fn account_mode_requires_password() {
        let result = RemoteConnectService::resolve_pairing_user_id(
            &verifier_returning("user-123"),
            &pairing_response(Some("alice"), None),
        )
        .await;
        assert_eq!(result.unwrap_err(), "Missing password");
    }

    #[tokio::test]
    async fn account_mode_requires_username() {
        let result = RemoteConnectService::resolve_pairing_user_id(
            &verifier_returning("user-123"),
            &pairing_response(None, Some("secret")),
        )
        .await;
        assert_eq!(result.unwrap_err(), "Missing username");
    }

    #[tokio::test]
    async fn account_credentials_are_never_downgraded_when_verifier_is_gone() {
        // QR advertised auth=account, but the desktop signed out before the
        // scan finished: reject instead of falling back to password-less pairing.
        let result = RemoteConnectService::resolve_pairing_user_id(
            &no_verifier(),
            &pairing_response(Some("alice"), Some("secret")),
        )
        .await;
        assert!(result.unwrap_err().contains("signed out"));
    }

    #[tokio::test]
    async fn legacy_mode_without_verifier_uses_plain_user_id() {
        let result = RemoteConnectService::resolve_pairing_user_id(
            &no_verifier(),
            &pairing_response(Some("local-user"), None),
        )
        .await;
        assert_eq!(result.unwrap().user_id(), "local-user");
    }

    #[tokio::test]
    async fn verifier_result_binds_canonical_user_id() {
        let canonical = RemoteConnectService::resolve_pairing_user_id(
            &verifier_returning("canonical-user-123"),
            &pairing_response(Some("alice"), Some("secret")),
        )
        .await
        .expect("verification should succeed");
        assert_eq!(canonical.user_id(), "canonical-user-123");
        let canonical_user_id = canonical.user_id().to_string();

        let trusted = Arc::new(RwLock::new(None));
        let identity = RemoteConnectService::validate_mobile_identity(
            &trusted,
            "install-1",
            &canonical_user_id,
        )
        .await
        .expect("first pairing binds the identity");
        assert_eq!(identity.user_id, "canonical-user-123");
        RemoteConnectService::persist_mobile_identity(&trusted, identity).await;

        // Reconnect with the same canonical id still matches.
        assert!(RemoteConnectService::validate_mobile_identity(
            &trusted,
            "install-1",
            &canonical_user_id,
        )
        .await
        .is_ok());
        // A different account user id is rejected against the bound identity.
        assert!(RemoteConnectService::validate_mobile_identity(
            &trusted,
            "install-1",
            "other-user"
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn account_pairing_preserves_password_whitespace_for_verification() {
        let verifier = Arc::new(RwLock::new(Some(
            Arc::new(|_username: String, password: String| {
                Box::pin(async move {
                    assert_eq!(password, "  secret with spaces  ");
                    Ok(AccountPairingVerification::new(
                        "canonical-user-123".to_string(),
                    ))
                })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<AccountPairingVerification, String>,
                                > + Send
                                + Sync,
                        >,
                    >
            }) as AccountPairingVerifierFn,
        )));
        let verified = RemoteConnectService::resolve_pairing_user_id(
            &verifier,
            &pairing_response(Some("alice"), Some("  secret with spaces  ")),
        )
        .await
        .expect("pairing should pass the exact password to the verifier");
        assert_eq!(verified.user_id(), "canonical-user-123");
    }

    #[tokio::test]
    async fn delegated_identity_requires_a_trusted_pairing_before_minting_credentials() {
        let provider_called = Arc::new(AtomicBool::new(false));
        let called = provider_called.clone();
        let provider: DelegatedIdentityFn = Arc::new(move || {
            let called = called.clone();
            Box::pin(async move {
                called.store(true, Ordering::SeqCst);
                Some(DelegatedIdentityAuthorization::new(
                    "account-token".to_string(),
                    "account-user".to_string(),
                    [7_u8; 32],
                ))
            })
        });
        let provider = Arc::new(RwLock::new(Some(provider)));
        let trusted = Arc::new(RwLock::new(None));

        let response = RemoteConnectService::resolve_delegated_identity_response(
            &provider,
            &trusted,
            "desktop-1",
        )
        .await;

        assert!(matches!(
            response.response,
            remote_server::RemoteResponse::Error { .. }
        ));
        assert!(!provider_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn delegated_identity_uses_the_account_bound_during_pairing() {
        let provider: DelegatedIdentityFn = Arc::new(|| {
            Box::pin(async {
                Some(DelegatedIdentityAuthorization::new(
                    "account-token".to_string(),
                    "paired-user".to_string(),
                    [7_u8; 32],
                ))
            })
        });
        let provider = Arc::new(RwLock::new(Some(provider)));
        let trusted = Arc::new(RwLock::new(Some(TrustedMobileIdentity {
            mobile_install_id: "install-1".to_string(),
            user_id: "paired-user".to_string(),
        })));

        let response = RemoteConnectService::resolve_delegated_identity_response(
            &provider,
            &trusted,
            "desktop-1",
        )
        .await;

        assert!(matches!(
            response.response,
            remote_server::RemoteResponse::DelegateIdentity {
                user_id,
                device_id,
                ..
            } if user_id == "paired-user" && device_id == "desktop-1"
        ));
    }

    #[tokio::test]
    async fn delegated_identity_rejects_a_provider_for_another_account() {
        let provider: DelegatedIdentityFn = Arc::new(|| {
            Box::pin(async {
                Some(DelegatedIdentityAuthorization::new(
                    "account-token".to_string(),
                    "replacement-user".to_string(),
                    [7_u8; 32],
                ))
            })
        });
        let provider = Arc::new(RwLock::new(Some(provider)));
        let trusted = Arc::new(RwLock::new(Some(TrustedMobileIdentity {
            mobile_install_id: "install-1".to_string(),
            user_id: "paired-user".to_string(),
        })));

        let response = RemoteConnectService::resolve_delegated_identity_response(
            &provider,
            &trusted,
            "desktop-1",
        )
        .await;
        assert!(matches!(
            response.response,
            remote_server::RemoteResponse::Error { message }
                if message.contains("no longer matches")
        ));
    }

    #[tokio::test]
    async fn delegated_identity_keeps_account_lease_until_response_is_released() {
        let account_lifecycle = Arc::new(Mutex::new(()));
        let provider_lifecycle = account_lifecycle.clone();
        let provider: DelegatedIdentityFn = Arc::new(move || {
            let provider_lifecycle = provider_lifecycle.clone();
            Box::pin(async move {
                let lease = provider_lifecycle.lock_owned().await;
                Some(DelegatedIdentityAuthorization::with_host_lease(
                    "account-token".to_string(),
                    "paired-user".to_string(),
                    [7_u8; 32],
                    lease,
                ))
            })
        });
        let provider = Arc::new(RwLock::new(Some(provider)));
        let trusted = Arc::new(RwLock::new(Some(TrustedMobileIdentity {
            mobile_install_id: "install-1".to_string(),
            user_id: "paired-user".to_string(),
        })));

        let response = RemoteConnectService::resolve_delegated_identity_response(
            &provider,
            &trusted,
            "desktop-1",
        )
        .await;
        assert!(matches!(
            &response.response,
            remote_server::RemoteResponse::DelegateIdentity { .. }
        ));
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                account_lifecycle.clone().lock_owned(),
            )
            .await
            .is_err(),
            "account replacement must remain blocked while the response is in flight"
        );

        drop(response);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            account_lifecycle.lock_owned(),
        )
        .await
        .expect("account replacement should proceed after the response is released");
    }
}
