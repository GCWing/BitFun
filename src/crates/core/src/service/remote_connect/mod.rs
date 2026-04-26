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
pub mod device;
pub mod embedded_relay;
pub mod encryption;
pub mod lan;
pub mod ngrok;
pub mod pairing;
pub mod qr_generator;
pub mod relay_client;
pub mod remote_server;

pub use device::DeviceIdentity;
pub use encryption::{decrypt_from_base64, encrypt_to_base64, KeyPair};
pub use pairing::{PairingProtocol, PairingState};
pub use qr_generator::QrGenerator;
pub use relay_client::RelayClient;
pub use remote_server::RemoteServer;

use anyhow::Result;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Supported connection methods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionMethod {
    Lan,
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

/// Unified Remote Connect service that orchestrates all connection methods.
pub struct RemoteConnectService {
    config: RemoteConnectConfig,
    device_identity: DeviceIdentity,
    pairing: Arc<RwLock<PairingProtocol>>,
    relay_client: Arc<RwLock<Option<RelayClient>>>,
    remote_server: Arc<RwLock<Option<RemoteServer>>>,
    active_method: Arc<RwLock<Option<ConnectionMethod>>>,
    ngrok_tunnel: Arc<RwLock<Option<ngrok::NgrokTunnel>>>,
    embedded_relay: Arc<RwLock<Option<embedded_relay::EmbeddedRelayHandle>>>,
    // Bot handles live independently of relay connections
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
}

impl RemoteConnectService {
    pub fn new(config: RemoteConnectConfig) -> Result<Self> {
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
            embedded_relay: Arc::new(RwLock::new(None)),
            bot_telegram_handle: Arc::new(RwLock::new(None)),
            bot_feishu_handle: Arc::new(RwLock::new(None)),
            bot_weixin_handle: Arc::new(RwLock::new(None)),
            telegram_bot: Arc::new(RwLock::new(None)),
            feishu_bot: Arc::new(RwLock::new(None)),
            weixin_bot: Arc::new(RwLock::new(None)),
            bot_connected_info: Arc::new(RwLock::new(None)),
            trusted_mobile_identity: Arc::new(RwLock::new(None)),
        })
    }

    pub fn device_identity(&self) -> &DeviceIdentity {
        &self.device_identity
    }

    async fn validate_mobile_identity(
        trusted_mobile_identity: &Arc<RwLock<Option<TrustedMobileIdentity>>>,
        response: &pairing::PairingResponse,
    ) -> std::result::Result<TrustedMobileIdentity, String> {
        let mobile_install_id = response
            .mobile_install_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Missing mobile installation ID".to_string())?;
        let user_id = response
            .user_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Missing user ID".to_string())?;

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

    async fn persist_mobile_identity(
        trusted_mobile_identity: &Arc<RwLock<Option<TrustedMobileIdentity>>>,
        identity: TrustedMobileIdentity,
    ) {
        *trusted_mobile_identity.write().await = Some(identity);
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
            ConnectionMethod::Lan,
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

        // Relay methods: clean up previous relay (but leave bots alone)
        self.stop_relay().await;

        let static_dir = self.config.mobile_web_dir.as_deref();

        let relay_url = match &method {
            ConnectionMethod::Lan => {
                let handle =
                    embedded_relay::start_embedded_relay(self.config.lan_port, static_dir).await?;
                *self.embedded_relay.write().await = Some(handle);
                match lan::build_lan_relay_url(self.config.lan_port) {
                    Ok(url) => url,
                    Err(e) => {
                        if let Some(ref mut relay) = *self.embedded_relay.write().await {
                            relay.stop();
                        }
                        *self.embedded_relay.write().await = None;
                        return Err(e);
                    }
                }
            }
            ConnectionMethod::Ngrok => {
                let handle =
                    embedded_relay::start_embedded_relay(self.config.lan_port, static_dir).await?;
                *self.embedded_relay.write().await = Some(handle);

                let tunnel = match ngrok::start_ngrok_tunnel(self.config.lan_port).await {
                    Ok(tunnel) => tunnel,
                    Err(e) => {
                        if let Some(ref mut relay) = *self.embedded_relay.write().await {
                            relay.stop();
                        }
                        *self.embedded_relay.write().await = None;
                        return Err(e);
                    }
                };
                let url = tunnel.public_url.clone();
                *self.ngrok_tunnel.write().await = Some(tunnel);
                url
            }
            ConnectionMethod::BitfunServer => self.config.bitfun_server_url.clone(),
            ConnectionMethod::CustomServer { url } => url.clone(),
            _ => unreachable!(),
        };

        let mut pairing = self.pairing.write().await;
        pairing.reset().await;
        let qr_payload = pairing.initiate(&relay_url).await?;

        let ws_url = match &method {
            ConnectionMethod::Lan | ConnectionMethod::Ngrok => {
                format!("ws://127.0.0.1:{}/ws", self.config.lan_port)
            }
            _ => {
                format!(
                    "{}/ws",
                    relay_url
                        .replace("https://", "wss://")
                        .replace("http://", "ws://")
                )
            }
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

        let web_app_url: String = match &method {
            ConnectionMethod::Lan | ConnectionMethod::Ngrok => relay_url.clone(),
            ConnectionMethod::BitfunServer => {
                if let Some(web_dir) = static_dir {
                    match upload_mobile_web(&relay_url, &qr_payload.room_id, web_dir).await {
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
                    match upload_mobile_web(&relay_url, &qr_payload.room_id, web_dir).await {
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
        let qr_url = QrGenerator::build_url(&qr_payload, &web_app_url, &client_language);
        let qr_svg = QrGenerator::generate_svg_from_url(&qr_url)?;
        let qr_data = QrGenerator::generate_png_base64_from_url(&qr_url)?;

        *self.active_method.write().await = Some(method.clone());
        *self.relay_client.write().await = Some(client);

        let pairing_arc = self.pairing.clone();
        let relay_arc = self.relay_client.clone();
        let server_arc = self.remote_server.clone();
        let trusted_mobile_identity_arc = self.trusted_mobile_identity.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
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
                                        debug!("Remote command: {cmd:?}");
                                        let response = server.dispatch(&cmd).await;
                                        match server
                                            .encrypt_response(&response, request_id.as_deref())
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
                                    let submitted_identity =
                                        match RemoteConnectService::validate_mobile_identity(
                                            &trusted_mobile_identity_arc,
                                            &response,
                                        )
                                        .await
                                        {
                                            Ok(identity) => identity,
                                            Err(message) => {
                                                drop(p);
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
                                            }
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

    async fn start_bot_connection(&self, method: &ConnectionMethod) -> Result<ConnectionResult> {
        let pairing_code = PairingProtocol::generate_bot_pairing_code();

        let bot_link = match method {
            ConnectionMethod::BotTelegram => {
                match &self.config.bot_telegram {
                    Some(bot::BotConfig::Telegram { bot_token }) if !bot_token.is_empty() => {
                        // Stop any existing Telegram bot
                        if let Some(handle) = self.bot_telegram_handle.write().await.take() {
                            handle.stop();
                        }

                        let tg_bot = Arc::new(bot::telegram::TelegramBot::new(
                            bot::telegram::TelegramConfig {
                                bot_token: bot_token.clone(),
                            },
                        ));
                        tg_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = tg_bot.clone();
                        let bot_for_loop = tg_bot.clone();
                        let tg_bot_ref = self.telegram_bot.clone();

                        *tg_bot_ref.write().await = Some(tg_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(chat_id) => {
                                    // Guard against the race where stop_bots() cleared
                                    // bot_connected_info between pairing completing and
                                    // this task running.
                                    if !*stop_rx.borrow() {
                                        *bot_connected_info.write().await =
                                            Some(format!("Telegram({chat_id})"));
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
                        if let Some(handle) = self.bot_feishu_handle.write().await.take() {
                            handle.stop();
                        }

                        let fs_bot =
                            Arc::new(bot::feishu::FeishuBot::new(bot::feishu::FeishuConfig {
                                app_id: app_id.clone(),
                                app_secret: app_secret.clone(),
                            }));
                        fs_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = fs_bot.clone();
                        let bot_for_loop = fs_bot.clone();
                        let fs_bot_ref = self.feishu_bot.clone();

                        *fs_bot_ref.write().await = Some(fs_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(chat_id) => {
                                    // Guard against the race where stop_bots() cleared
                                    // bot_connected_info between pairing completing and
                                    // this task running.
                                    if !*stop_rx.borrow() {
                                        *bot_connected_info.write().await =
                                            Some(format!("Feishu({chat_id})"));
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

                        let wx_bot = Arc::new(bot::weixin::WeixinBot::new(wx_cfg));
                        wx_bot.register_pairing(&pairing_code).await?;

                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);

                        let bot_connected_info = self.bot_connected_info.clone();
                        let bot_for_pair = wx_bot.clone();
                        let bot_for_loop = wx_bot.clone();
                        let wx_bot_ref = self.weixin_bot.clone();

                        *wx_bot_ref.write().await = Some(wx_bot.clone());

                        tokio::spawn(async move {
                            let mut stop_rx = stop_rx;
                            match bot_for_pair.wait_for_pairing(&mut stop_rx).await {
                                Ok(peer_id) => {
                                    if !*stop_rx.borrow() {
                                        *bot_connected_info.write().await =
                                            Some(format!("Weixin({peer_id})"));
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
        match saved.config {
            bot::BotConfig::Telegram { ref bot_token } => {
                if let Some(handle) = self.bot_telegram_handle.write().await.take() {
                    handle.stop();
                }

                let tg_bot = Arc::new(bot::telegram::TelegramBot::new(
                    bot::telegram::TelegramConfig {
                        bot_token: bot_token.clone(),
                    },
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
                if let Some(handle) = self.bot_feishu_handle.write().await.take() {
                    handle.stop();
                }

                let fs_bot = Arc::new(bot::feishu::FeishuBot::new(bot::feishu::FeishuConfig {
                    app_id: app_id.clone(),
                    app_secret: app_secret.clone(),
                }));

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

                let wx_bot = Arc::new(bot::weixin::WeixinBot::new(wx_cfg));
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

        if let Some(ref mut relay) = *self.embedded_relay.write().await {
            relay.stop();
        }
        *self.embedded_relay.write().await = None;

        self.pairing.write().await.reset().await;
        *self.trusted_mobile_identity.write().await = None;
        info!("Relay connections stopped (bots unaffected)");
    }

    /// Stop all bot connections.
    pub async fn stop_bots(&self) {
        if let Some(handle) = self.bot_telegram_handle.write().await.take() {
            handle.stop();
        }
        *self.telegram_bot.write().await = None;

        if let Some(handle) = self.bot_feishu_handle.write().await.take() {
            handle.stop();
        }
        *self.feishu_bot.write().await = None;

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
}

// ── Upload mobile-web to relay server ──────────────────────────────

/// File metadata used for the incremental upload check.
#[derive(serde::Serialize)]
struct FileManifestEntry {
    path: String,
    hash: String,
    size: u64,
}

/// Collected file data ready for upload.
struct CollectedFile {
    rel_path: String,
    content: Vec<u8>,
    hash: String,
}

const MAX_UPLOAD_BATCH_BASE64_BYTES: usize = 256 * 1024;

async fn upload_mobile_web(relay_url: &str, room_id: &str, web_dir: &str) -> Result<()> {
    let base = std::path::Path::new(web_dir);
    if !base.join("index.html").exists() {
        return Err(anyhow::anyhow!(
            "mobile-web dir missing index.html: {}",
            web_dir
        ));
    }

    let mut all_files: Vec<CollectedFile> = Vec::new();
    collect_files_with_hash(base, base, &mut all_files)?;

    info!(
        "Collected {} mobile-web files ({} bytes total) for room {room_id}",
        all_files.len(),
        all_files.iter().map(|f| f.content.len()).sum::<usize>()
    );

    let client = reqwest::Client::new();
    let relay_base = relay_url.trim_end_matches('/');

    // Step 1: try incremental check
    let manifest: Vec<FileManifestEntry> = all_files
        .iter()
        .map(|f| FileManifestEntry {
            path: f.rel_path.clone(),
            hash: f.hash.clone(),
            size: f.content.len() as u64,
        })
        .collect();

    let check_url = format!("{relay_base}/api/rooms/{room_id}/check-web-files");
    let check_result = client
        .post(&check_url)
        .json(&serde_json::json!({ "files": manifest }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;

    match check_result {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| anyhow::anyhow!("parse check-web-files response: {e}"))?;
            let needed: Vec<String> = body["needed"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let existing = body["existing_count"].as_u64().unwrap_or(0);
            let total = body["total_count"].as_u64().unwrap_or(0);
            if needed.is_empty() {
                info!("All {total} files already exist on relay server, no upload needed");
                return Ok(());
            }

            info!(
                "Incremental upload: {existing}/{total} files already on server, uploading {} needed",
                needed.len()
            );

            upload_needed_files(&client, relay_base, room_id, &all_files, &needed).await
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            info!("Relay server does not support incremental upload, falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
        Ok(resp) => {
            let status = resp.status();
            info!("check-web-files returned HTTP {status}, falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
        Err(e) => {
            info!("check-web-files request failed ({e}), falling back to full upload");
            upload_all_files(&client, relay_base, room_id, &all_files).await
        }
    }
}

/// Upload only the files that the server said it needs.
async fn upload_needed_files(
    client: &reqwest::Client,
    relay_base: &str,
    room_id: &str,
    all_files: &[CollectedFile],
    needed: &[String],
) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use std::collections::HashMap;

    let needed_set: std::collections::HashSet<&str> = needed.iter().map(|s| s.as_str()).collect();

    let mut files_payload: Vec<(String, serde_json::Value, usize)> = Vec::new();
    for f in all_files {
        if needed_set.contains(f.rel_path.as_str()) {
            let encoded = B64.encode(&f.content);
            let encoded_len = encoded.len();
            files_payload.push((
                f.rel_path.clone(),
                serde_json::json!({
                    "content": encoded,
                    "hash": f.hash,
                }),
                encoded_len,
            ));
        }
    }

    let url = format!("{relay_base}/api/rooms/{room_id}/upload-web-files");
    let total_b64_bytes: usize = files_payload.iter().map(|(_, _, len)| *len).sum();

    info!(
        "Uploading {} needed files ({} bytes base64) to {url}",
        files_payload.len(),
        total_b64_bytes
    );

    let mut current_batch: HashMap<String, serde_json::Value> = HashMap::new();
    let mut current_batch_b64_bytes = 0usize;
    let mut batch_index = 0usize;
    for (path, entry, entry_len) in files_payload {
        let should_flush = !current_batch.is_empty()
            && current_batch_b64_bytes + entry_len > MAX_UPLOAD_BATCH_BASE64_BYTES;
        if should_flush {
            upload_web_files_batch(
                client,
                &url,
                room_id,
                batch_index,
                &current_batch,
                current_batch_b64_bytes,
            )
            .await?;
            batch_index += 1;
            current_batch = HashMap::new();
            current_batch_b64_bytes = 0;
        }
        current_batch.insert(path, entry);
        current_batch_b64_bytes += entry_len;
    }

    if !current_batch.is_empty() {
        upload_web_files_batch(
            client,
            &url,
            room_id,
            batch_index,
            &current_batch,
            current_batch_b64_bytes,
        )
        .await?;
    }

    Ok(())
}

/// Fallback: upload all files using the legacy endpoint.
async fn upload_all_files(
    client: &reqwest::Client,
    relay_base: &str,
    room_id: &str,
    all_files: &[CollectedFile],
) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use std::collections::HashMap;

    let mut files: Vec<(String, String, usize)> = Vec::new();
    for f in all_files {
        let encoded = B64.encode(&f.content);
        let encoded_len = encoded.len();
        files.push((f.rel_path.clone(), encoded, encoded_len));
    }

    let url = format!("{relay_base}/api/rooms/{room_id}/upload-web");

    info!(
        "Full upload: {} files ({} bytes base64) to {url}",
        files.len(),
        files.iter().map(|(_, _, len)| *len).sum::<usize>()
    );

    let mut current_batch: HashMap<String, String> = HashMap::new();
    let mut current_batch_b64_bytes = 0usize;
    let mut batch_index = 0usize;
    for (path, encoded, encoded_len) in files {
        let should_flush = !current_batch.is_empty()
            && current_batch_b64_bytes + encoded_len > MAX_UPLOAD_BATCH_BASE64_BYTES;
        if should_flush {
            upload_web_legacy_batch(
                client,
                &url,
                room_id,
                batch_index,
                &current_batch,
                current_batch_b64_bytes,
            )
            .await?;
            batch_index += 1;
            current_batch = HashMap::new();
            current_batch_b64_bytes = 0;
        }
        current_batch.insert(path, encoded);
        current_batch_b64_bytes += encoded_len;
    }

    if !current_batch.is_empty() {
        upload_web_legacy_batch(
            client,
            &url,
            room_id,
            batch_index,
            &current_batch,
            current_batch_b64_bytes,
        )
        .await?;
    }

    Ok(())
}

async fn upload_web_files_batch(
    client: &reqwest::Client,
    url: &str,
    _room_id: &str,
    batch_index: usize,
    files_payload: &std::collections::HashMap<String, serde_json::Value>,
    _total_b64_bytes: usize,
) -> Result<()> {
    let resp = client
        .post(url)
        .json(&serde_json::json!({ "files": files_payload }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("upload-web-files batch {batch_index}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "upload-web-files batch {batch_index} failed: HTTP {status} — {body}"
        ));
    }
    Ok(())
}

async fn upload_web_legacy_batch(
    client: &reqwest::Client,
    url: &str,
    _room_id: &str,
    batch_index: usize,
    files_payload: &std::collections::HashMap<String, String>,
    _total_b64_bytes: usize,
) -> Result<()> {
    let resp = client
        .post(url)
        .json(&serde_json::json!({ "files": files_payload }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("upload mobile-web batch {batch_index}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "upload mobile-web batch {batch_index} failed: HTTP {status} — {body}"
        ));
    }
    Ok(())
}

/// Recursively collect files with their SHA-256 hash.
fn collect_files_with_hash(
    base: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<CollectedFile>,
) -> Result<()> {
    use sha2::{Digest, Sha256};

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_hash(base, &path, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read(&path)?;
            let mut hasher = Sha256::new();
            hasher.update(&content);
            let hash = format!("{:x}", hasher.finalize());
            out.push(CollectedFile {
                rel_path: rel,
                content,
                hash,
            });
        }
    }
    Ok(())
}
