//! Tauri commands for Remote Connect.

use crate::api::session_storage_path::desktop_effective_session_storage_path;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::service::remote_connect::{
    bot::{self, weixin, BotConfig},
    lan, AccountClient, AccountSession, ConnectionMethod, ConnectionResult, DeviceIdentity,
    PairingState, RemoteConnectConfig, RemoteConnectService,
};
use bitfun_core::service::session::{DialogTurnData, SessionMetadata};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tauri::State;
use tokio::sync::RwLock;

static REMOTE_CONNECT_SERVICE: OnceLock<Arc<RwLock<Option<RemoteConnectService>>>> =
    OnceLock::new();

/// In-memory account session (token + master key). The master key is never
/// persisted to disk; it is lost on restart and re-derived on next login.
static ACCOUNT_SESSION: OnceLock<Arc<RwLock<Option<AccountSession>>>> = OnceLock::new();

/// The relay URL associated with the current account session (needed for sync
/// and device-routing calls).
static ACCOUNT_RELAY_URL: OnceLock<Arc<RwLock<Option<String>>>> = OnceLock::new();

/// Global handle to the DialogScheduler, set during app startup. Used by the
/// device-routing background task to execute commands received from peer
/// devices (ExecuteOnDevice).
static DIALOG_SCHEDULER: OnceLock<Arc<bitfun_core::agentic::coordination::DialogScheduler>> =
    OnceLock::new();

/// Set the global scheduler handle. Called once during app startup.
pub fn set_dialog_scheduler(scheduler: Arc<bitfun_core::agentic::coordination::DialogScheduler>) {
    let _ = DIALOG_SCHEDULER.set(scheduler);
}

fn get_account_session() -> &'static Arc<RwLock<Option<AccountSession>>> {
    ACCOUNT_SESSION.get_or_init(|| Arc::new(RwLock::new(None)))
}

fn get_account_relay_url() -> &'static Arc<RwLock<Option<String>>> {
    ACCOUNT_RELAY_URL.get_or_init(|| Arc::new(RwLock::new(None)))
}

/// Read both the session and relay URL, returning owned clones to avoid
/// holding locks across awaits.
async fn read_account_context() -> Result<(AccountSession, String), String> {
    let session = get_account_session().read().await.clone();
    let relay_url = get_account_relay_url().read().await.clone();
    match (session, relay_url) {
        (Some(s), Some(u)) => Ok((s, u)),
        _ => Err("not logged in".to_string()),
    }
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
pub fn init_on_startup() {
    tokio::spawn(async {
        if let Err(e) = ensure_service().await {
            log::warn!("Remote connect startup init failed: {e}");
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
        RemoteConnectService::new(config).map_err(|e| format!("init remote connect: {e}"))?;
    *holder.write().await = Some(service);

    // Auto-restore previously paired bots
    restore_saved_bots().await;

    Ok(())
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
                    if parts.len() >= 4 && parts[0] == "0.0.0.0" && parts[1] == "0.0.0.0" {
                        if is_ipv4(parts[2]) && is_ipv4(parts[3]) {
                            // Key by interface IP so it can be matched later
                            map.insert(parts[3].to_string(), parts[2].to_string());
                        }
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
    let holder = get_service_holder();
    let guard = holder.read().await;
    let service = guard.as_ref().ok_or("service not initialized")?;
    let id = service.device_identity();
    Ok(DeviceInfo {
        device_id: id.device_id.clone(),
        device_name: id.device_name.clone(),
        mac_address: id.mac_address.clone(),
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
    let mut data = bot::load_bot_persistence();
    data.connections.clear();
    bot::save_bot_persistence(&data);
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
    let mut data = bot::load_bot_persistence();
    data.form_state = request;
    bot::save_bot_persistence(&data);
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
        let service = RemoteConnectService::new(config).map_err(|e| format!("init: {e}"))?;
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
        let service = RemoteConnectService::new(config).map_err(|e| format!("init: {e}"))?;
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
    let mut data = bot::load_bot_persistence();
    data.verbose_mode = verbose;
    bot::save_bot_persistence(&data);
    log::info!("Saved bot verbose_mode={} to persistence", verbose);
    Ok(())
}

// ── Account commands ────────────────────────────────────────────────────

/// Result returned to the frontend after a successful register/login.
/// The master key is deliberately NOT included — it stays in Rust memory.
#[derive(Serialize, Deserialize, Clone)]
pub struct AccountLoginResult {
    pub token: String,
    pub user_id: String,
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

fn current_device_identity() -> Result<DeviceIdentity, String> {
    DeviceIdentity::from_current_machine().map_err(|e| format!("detect device: {e}"))
}

#[tauri::command]
pub async fn account_login(request: AccountAuthRequest) -> Result<AccountLoginResult, String> {
    let device = current_device_identity()?;
    let client = AccountClient::new();
    let session = client
        .login(
            &request.relay_url,
            &request.username,
            &request.password,
            &device,
        )
        .await
        .map_err(|e| format!("{e}"))?;
    let result = AccountLoginResult {
        token: session.token.clone(),
        user_id: session.user_id.clone(),
    };
    *get_account_session().write().await = Some(session);
    *get_account_relay_url().write().await = Some(request.relay_url.clone());
    log::info!("Account logged in: {}", result.user_id);
    Ok(result)
}

#[tauri::command]
pub async fn account_status() -> Result<AccountStatus, String> {
    let guard = get_account_session().read().await;
    Ok(AccountStatus {
        logged_in: guard.is_some(),
        user_id: guard.as_ref().map(|s| s.user_id.clone()),
    })
}

#[tauri::command]
pub async fn account_logout() -> Result<(), String> {
    // Disconnect device routing before clearing the session.
    if let Some(service) = get_service_holder().read().await.as_ref() {
        service.stop_device_connection().await;
    }
    *get_account_session().write().await = None;
    *get_account_relay_url().write().await = None;
    log::info!("Account logged out");
    Ok(())
}

// ── P2: Device routing commands ──────────────────────────────────────────

#[derive(Serialize)]
pub struct OnlineDeviceInfo {
    pub device_id: String,
    pub device_name: String,
}

/// Connect to the account relay for device-to-device routing. Must be called
/// after `account_login`. The event receiver is consumed in a background task
/// that logs presence updates; device messages are forwarded to the RemoteConnectService.
#[tauri::command]
pub async fn account_connect_devices() -> Result<Vec<OnlineDeviceInfo>, String> {
    let (session, relay_url) = read_account_context().await?;
    let device_name = current_device_identity()?.device_name;
    let holder = get_service_holder().read().await;
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;
    let mut event_rx = service
        .start_device_connection(&relay_url, &session.token, &device_name)
        .await
        .map_err(|e| format!("{e}"))?;

    // Background task: consume events (presence / device messages / auth errors)
    let session_arc = get_account_session().clone();
    tokio::spawn(async move {
        use bitfun_core::service::remote_connect::relay_client::RelayEvent;
        while let Some(event) = event_rx.recv().await {
            match event {
                RelayEvent::AuthOk { user_id, device_id } => {
                    log::info!("Device routing auth ok: user={user_id} device={device_id}");
                }
                RelayEvent::AuthError { message } => {
                    log::warn!("Device routing auth error: {message}");
                }
                RelayEvent::DevicePresence { devices } => {
                    log::info!("Device presence updated: {} online", devices.len());
                }
                RelayEvent::DeviceMessageReceived {
                    source_device_id,
                    correlation_id: _,
                    encrypted_data,
                    nonce,
                } => {
                    let session_guard = session_arc.read().await.clone();
                    let Some(ref session) = session_guard else {
                        continue;
                    };
                    use bitfun_core::service::remote_connect::encryption::decrypt_from_base64;
                    match decrypt_from_base64(&session.master_key, &encrypted_data, &nonce) {
                        Ok(plaintext) => {
                            use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
                            match serde_json::from_str::<RemoteCommand>(&plaintext) {
                                Ok(RemoteCommand::ExecuteOnDevice {
                                    session_id,
                                    content,
                                    agent_type,
                                    workspace_path,
                                }) => {
                                    log::info!(
                                        "ExecuteOnDevice from {source_device_id}: \
                                         session={:?} content_len={}",
                                        session_id,
                                        content.len()
                                    );
                                    // Submit the dialog turn via the global scheduler.
                                    if let Some(scheduler) = DIALOG_SCHEDULER.get() {
                                        use bitfun_core::agentic::coordination::{
                                            DialogSubmissionPolicy, DialogTriggerSource,
                                        };
                                        let session_id = session_id
                                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                        let policy = DialogSubmissionPolicy::for_source(
                                            DialogTriggerSource::RemoteRelay,
                                        );
                                        let wp = workspace_path.unwrap_or_default();
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
                                    // Import the session into local storage.
                                    // The session_data is a SessionBundle JSON.
                                    // We write it to the default workspace's session dir.
                                    // A full import requires knowing the workspace path;
                                    // for now we log receipt — the frontend can trigger
                                    // account_import_remote_sessions after receiving
                                    // a presence update.
                                    log::info!("Session bundle received, run account_import_remote_sessions to import");
                                }
                                Ok(cmd) => {
                                    log::info!("Received device command: {cmd:?}");
                                }
                                Err(e) => {
                                    log::warn!("Could not parse device command: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to decrypt device message: {e}");
                        }
                    }
                }
                RelayEvent::Disconnected => {
                    log::info!("Device routing disconnected");
                }
                _ => {}
            }
        }
    });

    // Give the relay a moment to send initial presence
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let devices = service.online_devices().await;
    Ok(devices
        .into_iter()
        .map(|d| OnlineDeviceInfo {
            device_id: d.device_id,
            device_name: d.device_name,
        })
        .collect())
}

/// Get the current online device list.
#[tauri::command]
pub async fn account_online_devices() -> Result<Vec<OnlineDeviceInfo>, String> {
    let holder = get_service_holder().read().await;
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;
    let devices = service.online_devices().await;
    Ok(devices
        .into_iter()
        .map(|d| OnlineDeviceInfo {
            device_id: d.device_id,
            device_name: d.device_name,
        })
        .collect())
}

/// Send an encrypted session to a peer device. The `session_json` is encrypted
/// with the master key before being sent over the relay.
#[tauri::command]
pub async fn account_send_session_to_device(
    target_device_id: String,
    session_id: String,
    session_json: String,
) -> Result<(), String> {
    let (session, _) = read_account_context().await?;
    let holder = get_service_holder().read().await;
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;

    // Wrap the raw session JSON in a SendSessionToDevice command envelope so the
    // receiving device knows what to do with the payload.
    use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
    let envelope = serde_json::to_string(&RemoteCommand::SendSessionToDevice {
        session_data: session_json,
        session_id: session_id.clone(),
        session_name: None,
    })
    .map_err(|e| format!("serialize envelope: {e}"))?;

    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    let (encrypted_data, nonce) =
        encrypt_to_base64(&session.master_key, &envelope).map_err(|e| format!("{e}"))?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    service
        .send_device_message(&target_device_id, &correlation_id, &encrypted_data, &nonce)
        .await
        .map_err(|e| format!("{e}"))
}

// ── P4: Session / settings sync commands ─────────────────────────────────

/// Upload a single session blob (encrypted client-side with the master key).
#[tauri::command]
pub async fn account_sync_session(session_id: String, session_json: String) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .upload_session(&relay_url, &session, &session_id, &session_json)
        .await
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
        .fetch_sessions(&relay_url, &session)
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(sessions
        .into_iter()
        .map(|(id, json)| SyncedSession {
            session_id: id,
            session_json: json,
        })
        .collect())
}

/// Delete a synced session blob from the relay.
#[tauri::command]
pub async fn account_delete_synced_session(session_id: String) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .delete_session(&relay_url, &session, &session_id)
        .await
        .map_err(|e| format!("{e}"))
}

/// Upload settings blob (encrypted client-side with the master key).
#[tauri::command]
pub async fn account_sync_settings(settings_json: String) -> Result<(), String> {
    let (session, relay_url) = read_account_context().await?;
    AccountClient::new()
        .upload_settings(&relay_url, &session, &settings_json)
        .await
        .map_err(|e| format!("{e}"))
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

/// Export a single local session as an encrypted blob and upload it to the relay.
/// Uses the workspace + session_id to load metadata and turns from disk.
#[tauri::command]
pub async fn account_export_local_session(
    session_id: String,
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<(), String> {
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

    AccountClient::new()
        .upload_session(&relay_url, &acct_session, &session_id, &bundle_json)
        .await
        .map_err(|e| format!("{e}"))
}

/// Export all local sessions for a workspace and upload them to the relay.
/// Returns the number of sessions synced.
#[tauri::command]
pub async fn account_export_all_sessions(
    workspace_path: String,
    app_state: State<'_, crate::api::app_state::AppState>,
    path_manager: State<'_, Arc<bitfun_core::infrastructure::PathManager>>,
) -> Result<usize, String> {
    let (acct_session, relay_url) = read_account_context().await?;

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;

    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    let sessions = manager
        .list_session_metadata(&storage_path)
        .await
        .map_err(|e| format!("list sessions: {e}"))?;

    let client = AccountClient::new();
    let mut count = 0usize;
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

        if client
            .upload_session(&relay_url, &acct_session, &meta.session_id, &bundle_json)
            .await
            .is_ok()
        {
            count += 1;
        }
    }
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
    let (acct_session, relay_url) = read_account_context().await?;

    let storage_path =
        desktop_effective_session_storage_path(&app_state, &workspace_path, None, None).await;

    let manager = PersistenceManager::new(path_manager.inner().clone())
        .map_err(|e| format!("create persistence manager: {e}"))?;

    let remote_sessions = AccountClient::new()
        .fetch_sessions(&relay_url, &acct_session)
        .await
        .map_err(|e| format!("{e}"))?;

    let mut imported = Vec::new();
    for (session_id, bundle_json) in remote_sessions {
        // Skip if session already exists locally
        if manager
            .load_session_metadata(&storage_path, &session_id)
            .await
            .ok()
            .flatten()
            .is_some()
        {
            continue;
        }

        // Deserialize the bundle
        let bundle: SessionBundle =
            serde_json::from_str(&bundle_json).map_err(|e| format!("deserialize bundle: {e}"))?;

        // Write metadata
        let metadata: SessionMetadata = serde_json::from_value(bundle.metadata)
            .map_err(|e| format!("deserialize metadata: {e}"))?;
        if manager
            .save_session_metadata(&storage_path, &metadata)
            .await
            .is_err()
        {
            continue;
        }

        // Write turns
        for turn_val in &bundle.turns {
            let turn: DialogTurnData = serde_json::from_value(turn_val.clone())
                .map_err(|e| format!("deserialize turn: {e}"))?;
            let _ = manager.save_dialog_turn(&storage_path, &turn).await;
        }

        imported.push(session_id);
    }

    log::info!("Imported {} remote sessions", imported.len());
    Ok(imported)
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
    let (session, _) = read_account_context().await?;
    let holder = get_service_holder().read().await;
    let service = holder
        .as_ref()
        .ok_or_else(|| "remote connect service not initialized".to_string())?;

    use bitfun_core::service::remote_connect::remote_server::RemoteCommand;
    let envelope = serde_json::to_string(&RemoteCommand::ExecuteOnDevice {
        session_id,
        content,
        agent_type,
        workspace_path,
    })
    .map_err(|e| format!("serialize envelope: {e}"))?;

    use bitfun_core::service::remote_connect::encryption::encrypt_to_base64;
    let (encrypted_data, nonce) =
        encrypt_to_base64(&session.master_key, &envelope).map_err(|e| format!("{e}"))?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    service
        .send_device_message(&target_device_id, &correlation_id, &encrypted_data, &nonce)
        .await
        .map_err(|e| format!("{e}"))
}
