//! Weixin (微信) iLink bot integration for Remote Connect.
//!
//! Uses Tencent iLink HTTP APIs (`getupdates` long-poll, `sendmessage`) documented in
//! `@tencent-weixin/openclaw-weixin`. Login is QR-based; after login the same 6-digit
//! pairing flow as Telegram/Feishu binds the Weixin user to this desktop.

use anyhow::{anyhow, Result};
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use log::{debug, error, info, warn};
use rand::Rng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::command_router::{
    complete_im_bot_pairing, current_bot_language, execute_forwarded_turn, handle_command,
    parse_command, welcome_message, BotAction, BotChatState, BotInteractionHandler,
    BotInteractiveRequest, BotLanguage, BotMessageSender, HandleResult,
};
use super::{load_bot_persistence, save_bot_persistence, BotConfig, SavedBotConnection};
use crate::service::remote_connect::remote_server::ImageAttachment;

const DEFAULT_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
const DEFAULT_ILINK_BOT_TYPE: &str = "3";
const CHANNEL_VERSION: &str = "1.0.2";
const LONG_POLL_TIMEOUT_SECS: u64 = 36;
const API_TIMEOUT_SECS: u64 = 20;
const QR_POLL_TIMEOUT_SECS: u64 = 36;
const SESSION_EXPIRED_ERRCODE: i64 = -14;
const SESSION_PAUSE_SECS: u64 = 3600;
const MAX_TEXT_CHUNK: usize = 3500;
const MAX_QR_REFRESH: u32 = 3;
/// Weixin CDN host for encrypted upload (same as `@tencent-weixin/openclaw-weixin`).
const DEFAULT_CDN_BASE_URL: &str = "https://novac2c.cdn.weixin.qq.com/c2c";
/// Same cap as Feishu bot file send.
const MAX_WEIXIN_FILE_BYTES: u64 = 30 * 1024 * 1024;
const CDN_UPLOAD_MAX_RETRIES: u32 = 3;
/// Same cap as Feishu inbound images.
const MAX_INBOUND_IMAGES: usize = 5;

// ── AES-128-ECB (PKCS#7) + CDN upload helpers (iLink file/image/video send) ─

fn aes_ecb_ciphertext_len(plaintext_len: usize) -> usize {
    let pad = 16 - (plaintext_len % 16);
    let pad = if pad == 0 { 16 } else { pad };
    plaintext_len + pad
}

fn encrypt_aes_128_ecb_pkcs7(plaintext: &[u8], key: &[u8; 16]) -> Vec<u8> {
    let cipher = Aes128::new_from_slice(key).expect("AES-128 key len");
    let pad_len = 16 - (plaintext.len() % 16);
    let pad_len = if pad_len == 0 { 16 } else { pad_len };
    let mut buf = plaintext.to_vec();
    buf.extend(std::iter::repeat(pad_len as u8).take(pad_len));
    let mut out = Vec::with_capacity(buf.len());
    for chunk in buf.chunks_exact(16) {
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    out
}

fn md5_hex_lower(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

fn build_cdn_upload_url(cdn_base: &str, upload_param: &str, filekey: &str) -> String {
    let base = cdn_base.trim_end_matches('/');
    format!(
        "{}/upload?encrypted_query_param={}&filekey={}",
        base,
        urlencoding::encode(upload_param),
        urlencoding::encode(filekey)
    )
}

/// CDN download URL (same as `@tencent-weixin/openclaw-weixin` `buildCdnDownloadUrl`).
fn build_cdn_download_url(cdn_base: &str, encrypted_query_param: &str) -> String {
    let base = cdn_base.trim_end_matches('/');
    format!(
        "{}/download?encrypted_query_param={}",
        base,
        urlencoding::encode(encrypted_query_param)
    )
}

fn decrypt_aes_128_ecb_pkcs7(ciphertext: &[u8], key: &[u8; 16]) -> Result<Vec<u8>> {
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(anyhow!(
            "invalid ciphertext length {}",
            ciphertext.len()
        ));
    }
    let cipher = Aes128::new_from_slice(key).expect("AES-128 key len");
    let mut out = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext.chunks_exact(16) {
        let mut block = aes::cipher::generic_array::GenericArray::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    let Some(&pad_byte) = out.last() else {
        return Err(anyhow!("empty after decrypt"));
    };
    let pad = pad_byte as usize;
    if pad == 0 || pad > 16 || pad > out.len() {
        return Err(anyhow!("invalid PKCS#7 padding (pad={pad})"));
    }
    if !out[out.len() - pad..].iter().all(|&b| b == pad_byte) {
        return Err(anyhow!("invalid PKCS#7 padding bytes"));
    }
    out.truncate(out.len() - pad);
    Ok(out)
}

/// `CDNMedia.aes_key`: base64(raw 16 bytes) or base64(32-char hex) — OpenClaw `parseAesKey`.
fn parse_weixin_cdn_aes_key(aes_key_base64: &str) -> Result<[u8; 16]> {
    let decoded = B64
        .decode(aes_key_base64.trim())
        .map_err(|e| anyhow!("aes_key base64: {e}"))?;
    if decoded.len() == 16 {
        let mut k = [0u8; 16];
        k.copy_from_slice(&decoded);
        return Ok(k);
    }
    if decoded.len() == 32 {
        let s = std::str::from_utf8(&decoded).map_err(|_| anyhow!("aes_key: expected utf8 hex"))?;
        if s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            let bytes = hex::decode(s).map_err(|e| anyhow!("aes_key inner hex: {e}"))?;
            if bytes.len() == 16 {
                let mut k = [0u8; 16];
                k.copy_from_slice(&bytes);
                return Ok(k);
            }
        }
    }
    Err(anyhow!(
        "aes_key: unsupported encoding (decoded {} bytes)",
        decoded.len()
    ))
}

fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return "image/jpeg";
    }
    if bytes.len() >= 8
        && bytes[..8] == [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
    {
        return "image/png";
    }
    if bytes.len() >= 6
        && (&bytes[..6] == b"GIF87a".as_slice() || &bytes[..6] == b"GIF89a".as_slice())
    {
        return "image/gif";
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    "image/jpeg"
}

#[derive(Debug)]
struct UploadedMediaInfo {
    download_encrypted_query_param: String,
    aeskey_hex: String,
    file_size_plain: u64,
    file_size_cipher: usize,
}

// ── QR login session store (in-memory, same role as OpenClaw installer) ─────

#[derive(Debug, Clone)]
struct QrLoginSession {
    qrcode: String,
    qr_image_url: String,
    started_at_ms: i64,
    refresh_count: u32,
}

enum QrSessionLookup {
    Missing,
    TimedOut,
    Found(QrLoginSession),
}

fn qr_sessions() -> &'static Mutex<HashMap<String, QrLoginSession>> {
    static CELL: OnceLock<Mutex<HashMap<String, QrLoginSession>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn normalize_weixin_account_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn random_wechat_uin_header() -> String {
    let n: u32 = rand::thread_rng().gen();
    base64::engine::general_purpose::STANDARD.encode(n.to_string().as_bytes())
}

fn ensure_trailing_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{url}/")
    }
}

fn sync_buf_path(bot_account_id: &str) -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    base
        .join(".bitfun")
        .join("weixin")
        .join(format!("{bot_account_id}_get_updates_buf.txt"))
}

fn load_sync_buf(bot_account_id: &str) -> String {
    let p = sync_buf_path(bot_account_id);
    std::fs::read_to_string(&p).unwrap_or_default().trim().to_string()
}

fn save_sync_buf(bot_account_id: &str, buf: &str) {
    let p = sync_buf_path(bot_account_id);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&p, buf) {
        warn!("weixin: failed to save sync buf {}: {e}", p.display());
    }
}

// ── Public QR API (used from Tauri) ───────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WeixinQrStartResponse {
    pub session_key: String,
    pub qr_image_url: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WeixinQrPollStatus {
    Wait,
    Scanned,
    Confirmed,
    Expired,
    Error,
}

#[derive(Debug, Serialize)]
pub struct WeixinQrPollResponse {
    pub status: WeixinQrPollStatus,
    pub message: String,
    /// Present when a new QR was issued after expiry (client should refresh image).
    pub qr_image_url: Option<String>,
    pub ilink_token: Option<String>,
    pub bot_account_id: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QrCodeApiResponse {
    qrcode: Option<String>,
    qrcode_img_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QrStatusApiResponse {
    status: Option<String>,
    bot_token: Option<String>,
    ilink_bot_id: Option<String>,
    baseurl: Option<String>,
}

/// Start Weixin QR login: fetch QR from iLink and register a session.
pub async fn weixin_qr_start(base_url_override: Option<String>) -> Result<WeixinQrStartResponse> {
    let base = ensure_trailing_slash(
        base_url_override
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_BASE_URL),
    );
    let url = format!(
        "{}ilink/bot/get_bot_qrcode?bot_type={}",
        base,
        urlencoding::encode(DEFAULT_ILINK_BOT_TYPE)
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(API_TIMEOUT_SECS))
        .build()?;

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("get_bot_qrcode HTTP {status}: {body}"));
    }
    let parsed: QrCodeApiResponse = resp.json().await?;
    let qrcode = parsed
        .qrcode
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("get_bot_qrcode: missing qrcode"))?;
    let qr_image_url = parsed
        .qrcode_img_content
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("get_bot_qrcode: missing qrcode_img_content"))?;

    let session_key = uuid::Uuid::new_v4().to_string();
    let session = QrLoginSession {
        qrcode,
        qr_image_url: qr_image_url.clone(),
        started_at_ms: now_ms(),
        refresh_count: 0,
    };
    qr_sessions()
        .lock()
        .map_err(|e| anyhow!("qr session lock: {e}"))?
        .insert(session_key.clone(), session);

    Ok(WeixinQrStartResponse {
        session_key,
        qr_image_url,
        message: "Scan the QR code with WeChat.".to_string(),
    })
}

/// Poll QR login status (long-poll once). Call repeatedly from the UI until `confirmed` or `error`.
pub async fn weixin_qr_poll(
    session_key: &str,
    base_url_override: Option<String>,
) -> Result<WeixinQrPollResponse> {
    let base = ensure_trailing_slash(
        base_url_override
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_BASE_URL),
    );

    let lookup = {
        let mut map = qr_sessions()
            .lock()
            .map_err(|e| anyhow!("qr session lock: {e}"))?;
        match map.get(session_key) {
            None => QrSessionLookup::Missing,
            Some(s) => {
                if now_ms() - s.started_at_ms > 5 * 60_000 {
                    map.remove(session_key);
                    QrSessionLookup::TimedOut
                } else {
                    QrSessionLookup::Found(s.clone())
                }
            }
        }
    };

    match lookup {
        QrSessionLookup::Missing => Ok(WeixinQrPollResponse {
            status: WeixinQrPollStatus::Error,
            message: "No active QR session. Start login again.".to_string(),
            qr_image_url: None,
            ilink_token: None,
            bot_account_id: None,
            base_url: None,
        }),
        QrSessionLookup::TimedOut => Ok(WeixinQrPollResponse {
            status: WeixinQrPollStatus::Error,
            message: "QR session expired. Start again.".to_string(),
            qr_image_url: None,
            ilink_token: None,
            bot_account_id: None,
            base_url: None,
        }),
        QrSessionLookup::Found(session) => {
            let qrcode_enc = urlencoding::encode(&session.qrcode);
            let url = format!("{}ilink/bot/get_qrcode_status?qrcode={}", base, qrcode_enc);

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(QR_POLL_TIMEOUT_SECS))
                .build()?;

            let resp = client
                .get(&url)
                .header("iLink-App-ClientVersion", "1")
                .send()
                .await;

            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    if e.is_timeout() {
                        return Ok(WeixinQrPollResponse {
                            status: WeixinQrPollStatus::Wait,
                            message: "waiting".to_string(),
                            qr_image_url: None,
                            ilink_token: None,
                            bot_account_id: None,
                            base_url: None,
                        });
                    }
                    qr_sessions()
                        .lock()
                        .map_err(|e| anyhow!("qr session lock: {e}"))?
                        .remove(session_key);
                    return Err(anyhow!("get_qrcode_status: {e}"));
                }
            };

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                qr_sessions()
                    .lock()
                    .map_err(|e| anyhow!("qr session lock: {e}"))?
                    .remove(session_key);
                return Ok(WeixinQrPollResponse {
                    status: WeixinQrPollStatus::Error,
                    message: format!("HTTP {status}: {body}"),
                    qr_image_url: None,
                    ilink_token: None,
                    bot_account_id: None,
                    base_url: None,
                });
            }

            let status_json: QrStatusApiResponse = resp.json().await?;
            let st = status_json.status.as_deref().unwrap_or("wait");

            match st {
                "wait" => Ok(WeixinQrPollResponse {
                    status: WeixinQrPollStatus::Wait,
                    message: "waiting".to_string(),
                    qr_image_url: None,
                    ilink_token: None,
                    bot_account_id: None,
                    base_url: None,
                }),
                "scaned" => Ok(WeixinQrPollResponse {
                    status: WeixinQrPollStatus::Scanned,
                    message: "Scanned; confirm on your phone.".to_string(),
                    qr_image_url: None,
                    ilink_token: None,
                    bot_account_id: None,
                    base_url: None,
                }),
                "confirmed" => {
                    let token = status_json
                        .bot_token
                        .clone()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("confirmed but bot_token missing"))?;
                    let raw_id = status_json
                        .ilink_bot_id
                        .clone()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("confirmed but ilink_bot_id missing"))?;
                    let normalized = normalize_weixin_account_id(&raw_id);
                    let baseurl = status_json
                        .baseurl
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| base.trim_end_matches('/').to_string());

                    qr_sessions()
                        .lock()
                        .map_err(|e| anyhow!("qr session lock: {e}"))?
                        .remove(session_key);

                    Ok(WeixinQrPollResponse {
                        status: WeixinQrPollStatus::Confirmed,
                        message: "WeChat linked.".to_string(),
                        qr_image_url: None,
                        ilink_token: Some(token),
                        bot_account_id: Some(normalized),
                        base_url: Some(baseurl),
                    })
                }
                "expired" => {
                    let over_limit = {
                        let mut map = qr_sessions()
                            .lock()
                            .map_err(|e| anyhow!("qr session lock: {e}"))?;
                        let Some(s) = map.get_mut(session_key) else {
                            return Ok(WeixinQrPollResponse {
                                status: WeixinQrPollStatus::Error,
                                message: "Session lost. Start again.".to_string(),
                                qr_image_url: None,
                                ilink_token: None,
                                bot_account_id: None,
                                base_url: None,
                            });
                        };
                        s.refresh_count += 1;
                        if s.refresh_count > MAX_QR_REFRESH {
                            map.remove(session_key);
                            true
                        } else {
                            false
                        }
                    };

                    if over_limit {
                        return Ok(WeixinQrPollResponse {
                            status: WeixinQrPollStatus::Error,
                            message: "QR expired too many times; start again.".to_string(),
                            qr_image_url: None,
                            ilink_token: None,
                            bot_account_id: None,
                            base_url: None,
                        });
                    }

                    let refresh_url = format!(
                        "{}ilink/bot/get_bot_qrcode?bot_type={}",
                        base,
                        urlencoding::encode(DEFAULT_ILINK_BOT_TYPE)
                    );
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(API_TIMEOUT_SECS))
                        .build()?;
                    let refresh = client.get(&refresh_url).send().await?;
                    if !refresh.status().is_success() {
                        qr_sessions()
                            .lock()
                            .map_err(|e| anyhow!("qr session lock: {e}"))?
                            .remove(session_key);
                        return Ok(WeixinQrPollResponse {
                            status: WeixinQrPollStatus::Error,
                            message: "Failed to refresh QR.".to_string(),
                            qr_image_url: None,
                            ilink_token: None,
                            bot_account_id: None,
                            base_url: None,
                        });
                    }
                    let parsed: QrCodeApiResponse = refresh.json().await?;
                    let qrcode = parsed
                        .qrcode
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("refresh: missing qrcode"))?;
                    let qr_image_url = parsed
                        .qrcode_img_content
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("refresh: missing qrcode_img_content"))?;

                    {
                        let mut m = qr_sessions()
                            .lock()
                            .map_err(|e| anyhow!("qr session lock: {e}"))?;
                        if let Some(s) = m.get_mut(session_key) {
                            s.qrcode = qrcode;
                            s.qr_image_url = qr_image_url.clone();
                            s.started_at_ms = now_ms();
                        }
                    }

                    Ok(WeixinQrPollResponse {
                        status: WeixinQrPollStatus::Expired,
                        message: "QR refreshed.".to_string(),
                        qr_image_url: Some(qr_image_url),
                        ilink_token: None,
                        bot_account_id: None,
                        base_url: None,
                    })
                }
                _ => Ok(WeixinQrPollResponse {
                    status: WeixinQrPollStatus::Wait,
                    message: st.to_string(),
                    qr_image_url: None,
                    ilink_token: None,
                    bot_account_id: None,
                    base_url: None,
                }),
            }
        }
    }
}

// ── iLink authenticated client ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeixinConfig {
    pub ilink_token: String,
    pub base_url: String,
    /// Normalized ilink bot id (filesystem-safe); used for sync buffer path.
    pub bot_account_id: String,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

pub struct WeixinBot {
    config: WeixinConfig,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    chat_states: Arc<RwLock<HashMap<String, BotChatState>>>,
    context_tokens: Arc<RwLock<HashMap<String, String>>>,
    session_pause_until_ms: Arc<RwLock<HashMap<String, i64>>>,
}

impl WeixinBot {
    pub fn new(config: WeixinConfig) -> Self {
        Self {
            config,
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            chat_states: Arc::new(RwLock::new(HashMap::new())),
            context_tokens: Arc::new(RwLock::new(HashMap::new())),
            session_pause_until_ms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn restore_chat_state(&self, peer_id: &str, state: BotChatState) {
        self.chat_states
            .write()
            .await
            .insert(peer_id.to_string(), state);
    }

    fn base_url(&self) -> String {
        ensure_trailing_slash(&self.config.base_url)
    }

    async fn is_session_paused(&self) -> bool {
        let id = &self.config.bot_account_id;
        let mut m = self.session_pause_until_ms.write().await;
        let now = now_ms();
        if let Some(until) = m.get(id).copied() {
            if now >= until {
                m.remove(id);
                return false;
            }
            return true;
        }
        false
    }

    async fn pause_session(&self) {
        let until = now_ms() + (SESSION_PAUSE_SECS as i64) * 1000;
        self.session_pause_until_ms
            .write()
            .await
            .insert(self.config.bot_account_id.clone(), until);
        warn!(
            "weixin: session expired (err -14), pausing API for {}s",
            SESSION_PAUSE_SECS
        );
    }

    fn build_auth_headers(&self, body: &str) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
        let mut h = HeaderMap::new();
        h.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        h.insert(
            HeaderName::from_static("authorizationtype"),
            HeaderValue::from_static("ilink_bot_token"),
        );
        h.insert(
            HeaderName::from_static("content-length"),
            HeaderValue::from_str(&body.len().to_string()).unwrap_or(HeaderValue::from_static("0")),
        );
        h.insert(
            HeaderName::from_static("x-wechat-uin"),
            HeaderValue::from_str(&random_wechat_uin_header()).unwrap_or(HeaderValue::from_static("MA==")),
        );
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", self.config.ilink_token.trim())) {
            h.insert(HeaderName::from_static("authorization"), v);
        }
        h
    }

    async fn post_ilink(&self, endpoint: &str, body: Value, timeout: Duration) -> Result<String> {
        let url = format!("{}{}", self.base_url(), endpoint.trim_start_matches('/'));
        let body_str = serde_json::to_string(&body)?;
        let client = reqwest::Client::builder().timeout(timeout).build()?;
        let resp = client
            .post(&url)
            .headers(self.build_auth_headers(&body_str))
            .body(body_str)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("ilink {endpoint} HTTP {status}: {text}"));
        }
        Ok(text)
    }

    fn cdn_base_url(&self) -> &'static str {
        DEFAULT_CDN_BASE_URL
    }

    async fn fetch_weixin_cdn_bytes(&self, encrypted_query_param: &str) -> Result<Vec<u8>> {
        let url = build_cdn_download_url(self.cdn_base_url(), encrypted_query_param);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        let resp = client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("weixin CDN GET {status}: {body}"));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    /// Decrypt one inbound `image_item` (CDN download + AES-128-ECB), matching OpenClaw `downloadMediaFromItem`.
    async fn inbound_image_bytes_from_item(&self, item: &Value) -> Result<Vec<u8>> {
        let img = &item["image_item"];
        let param = img["media"]["encrypt_query_param"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("image: missing encrypt_query_param"))?;

        let key: Option<[u8; 16]> =
            if let Some(hex_s) = img["aeskey"].as_str().filter(|s| !s.is_empty()) {
                let bytes = hex::decode(hex_s.trim()).map_err(|e| anyhow!("image aeskey hex: {e}"))?;
                if bytes.len() != 16 {
                    return Err(anyhow!("image aeskey must decode to 16 bytes"));
                }
                let mut k = [0u8; 16];
                k.copy_from_slice(&bytes);
                Some(k)
            } else if let Some(b64) = img["media"]["aes_key"].as_str().filter(|s| !s.is_empty()) {
                Some(parse_weixin_cdn_aes_key(b64)?)
            } else {
                None
            };

        let enc = self.fetch_weixin_cdn_bytes(param).await?;
        match key {
            Some(k) => decrypt_aes_128_ecb_pkcs7(&enc, &k),
            None => Ok(enc),
        }
    }

    /// Collect up to [`MAX_INBOUND_IMAGES`] images from `item_list` as Feishu-style `ImageAttachment` data URLs.
    async fn inbound_image_attachments_from_message(&self, msg: &Value) -> (Vec<ImageAttachment>, usize) {
        const MAX_BYTES: usize = 1024 * 1024;
        let Some(items) = msg["item_list"].as_array() else {
            return (vec![], 0);
        };
        let total_with_param = items
            .iter()
            .filter(|i| {
                i["type"].as_i64() == Some(2)
                    && i["image_item"]["media"]["encrypt_query_param"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
            })
            .count();
        let skipped = total_with_param.saturating_sub(MAX_INBOUND_IMAGES);

        let mut attachments = Vec::new();
        for item in items {
            if attachments.len() >= MAX_INBOUND_IMAGES {
                break;
            }
            if item["type"].as_i64() != Some(2) {
                continue;
            }
            match self.inbound_image_bytes_from_item(item).await {
                Ok(raw) => {
                    let mime = sniff_image_mime(&raw);
                    let data_url = if raw.len() <= MAX_BYTES {
                        let b64 = B64.encode(&raw);
                        format!("data:{mime};base64,{b64}")
                    } else {
                        let raw_fallback = raw.clone();
                        match crate::agentic::image_analysis::optimize_image_with_size_limit(
                            raw,
                            "openai",
                            Some(mime),
                            Some(MAX_BYTES),
                        ) {
                            Ok(processed) => {
                                let b64 = B64.encode(&processed.data);
                                format!("data:{};base64,{}", processed.mime_type, b64)
                            }
                            Err(e) => {
                                warn!("Weixin image compression failed: {e}");
                                let b64 = B64.encode(&raw_fallback);
                                format!("data:{mime};base64,{b64}")
                            }
                        }
                    };
                    attachments.push(ImageAttachment {
                        name: format!("weixin_image_{}.jpg", attachments.len() + 1),
                        data_url,
                    });
                }
                Err(e) => warn!("Weixin inbound image download failed: {e}"),
            }
        }
        (attachments, skipped)
    }

    /// `ilink/bot/getuploadurl` — returns `upload_param` for CDN POST.
    async fn ilink_get_upload_url(
        &self,
        to_user_id: &str,
        filekey: &str,
        media_type: i64,
        rawsize: u64,
        rawfilemd5: &str,
        filesize: usize,
        aeskey_hex: &str,
    ) -> Result<String> {
        let body = json!({
            "filekey": filekey,
            "media_type": media_type,
            "to_user_id": to_user_id,
            "rawsize": rawsize,
            "rawfilemd5": rawfilemd5,
            "filesize": filesize,
            "no_need_thumb": true,
            "aeskey": aeskey_hex,
            "base_info": { "channel_version": CHANNEL_VERSION }
        });
        let raw = self
            .post_ilink(
                "ilink/bot/getuploadurl",
                body,
                Duration::from_secs(API_TIMEOUT_SECS),
            )
            .await?;
        let v: Value = serde_json::from_str(&raw)?;
        v["upload_param"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("getuploadurl: missing upload_param"))
    }

    async fn post_weixin_cdn_upload(
        &self,
        cdn_url: &str,
        ciphertext: &[u8],
    ) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 1..=CDN_UPLOAD_MAX_RETRIES {
            let resp = client
                .post(cdn_url)
                .header("Content-Type", "application/octet-stream")
                .body(ciphertext.to_vec())
                .send()
                .await;
            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    last_err = Some(anyhow!("CDN upload attempt {attempt}: {e}"));
                    if attempt < CDN_UPLOAD_MAX_RETRIES {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    continue;
                }
            };
            let status = resp.status();
            if status.is_client_error() {
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow!("CDN client error {status}: {body}"));
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                last_err = Some(anyhow!("CDN server error {status}: {body}"));
                if attempt < CDN_UPLOAD_MAX_RETRIES {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
            let download_param = resp
                .headers()
                .get("x-encrypted-param")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            return download_param.ok_or_else(|| {
                anyhow!("CDN response missing x-encrypted-param header")
            });
        }
        Err(last_err.unwrap_or_else(|| anyhow!("CDN upload failed")))
    }

    /// Read plaintext → encrypt → getuploadurl → POST to CDN (same pipeline as OpenClaw weixin plugin).
    async fn upload_bytes_to_weixin_cdn(
        &self,
        to_user_id: &str,
        plaintext: &[u8],
        media_type: i64,
    ) -> Result<UploadedMediaInfo> {
        let rawsize = plaintext.len() as u64;
        let rawfilemd5 = md5_hex_lower(plaintext);
        let mut aeskey = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut aeskey);
        let aeskey_hex = hex::encode(aeskey);
        let filesize_cipher = aes_ecb_ciphertext_len(plaintext.len());
        let ciphertext = encrypt_aes_128_ecb_pkcs7(plaintext, &aeskey);

        let mut filekey_raw = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut filekey_raw);
        let filekey = hex::encode(filekey_raw);

        let upload_param = self
            .ilink_get_upload_url(
                to_user_id,
                &filekey,
                media_type,
                rawsize,
                &rawfilemd5,
                filesize_cipher,
                &aeskey_hex,
            )
            .await?;

        let cdn_url = build_cdn_upload_url(self.cdn_base_url(), &upload_param, &filekey);
        debug!(
            "weixin CDN upload: media_type={media_type} rawsize={rawsize} cipher_len={}",
            ciphertext.len()
        );
        let download_encrypted_query_param = self.post_weixin_cdn_upload(&cdn_url, &ciphertext).await?;

        Ok(UploadedMediaInfo {
            download_encrypted_query_param,
            aeskey_hex,
            file_size_plain: rawsize,
            file_size_cipher: ciphertext.len(),
        })
    }

    /// `aes_key` in JSON: base64 of raw 16-byte key (standard); matches typical iLink clients.
    fn media_aes_key_b64(aeskey_hex: &str) -> Result<String> {
        let bytes = hex::decode(aeskey_hex.trim()).map_err(|e| anyhow!("aeskey hex: {e}"))?;
        if bytes.len() != 16 {
            return Err(anyhow!("aeskey must decode to 16 bytes"));
        }
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    async fn send_message_with_items(
        &self,
        to_user_id: &str,
        context_token: &str,
        items: Vec<Value>,
    ) -> Result<()> {
        let client_id = format!("bitfun-wx-{}", uuid::Uuid::new_v4());
        let msg = json!({
            "from_user_id": "",
            "to_user_id": to_user_id,
            "client_id": client_id,
            "message_type": 2,
            "message_state": 2,
            "item_list": items,
            "context_token": context_token,
        });
        let body = json!({
            "msg": msg,
            "base_info": { "channel_version": CHANNEL_VERSION }
        });
        self.post_ilink(
            "ilink/bot/sendmessage",
            body,
            Duration::from_secs(API_TIMEOUT_SECS),
        )
        .await?;
        Ok(())
    }

    /// Upload a workspace file and send as image / video / file attachment (like Feishu `send_file_to_feishu_chat`).
    async fn send_workspace_file_to_peer(
        &self,
        peer_id: &str,
        raw_path: &str,
        workspace_root: Option<&std::path::Path>,
    ) -> Result<()> {
        let content = super::read_workspace_file(raw_path, MAX_WEIXIN_FILE_BYTES, workspace_root).await?;
        let mime = super::detect_mime_type(std::path::Path::new(&content.name));

        let token = {
            let m = self.context_tokens.read().await;
            m.get(peer_id)
                .cloned()
                .ok_or_else(|| anyhow!("missing context_token for peer {peer_id}"))?
        };

        let item: Value = if mime.starts_with("image/") {
            let up = self
                .upload_bytes_to_weixin_cdn(peer_id, &content.bytes, 1)
                .await?;
            let aes_b64 = Self::media_aes_key_b64(&up.aeskey_hex)?;
            json!({
                "type": 2,
                "image_item": {
                    "media": {
                        "encrypt_query_param": up.download_encrypted_query_param,
                        "aes_key": aes_b64,
                        "encrypt_type": 1
                    },
                    "mid_size": up.file_size_cipher
                }
            })
        } else if mime.starts_with("video/") {
            let up = self
                .upload_bytes_to_weixin_cdn(peer_id, &content.bytes, 2)
                .await?;
            let aes_b64 = Self::media_aes_key_b64(&up.aeskey_hex)?;
            json!({
                "type": 5,
                "video_item": {
                    "media": {
                        "encrypt_query_param": up.download_encrypted_query_param,
                        "aes_key": aes_b64,
                        "encrypt_type": 1
                    },
                    "video_size": up.file_size_cipher
                }
            })
        } else {
            let up = self
                .upload_bytes_to_weixin_cdn(peer_id, &content.bytes, 3)
                .await?;
            let aes_b64 = Self::media_aes_key_b64(&up.aeskey_hex)?;
            json!({
                "type": 4,
                "file_item": {
                    "media": {
                        "encrypt_query_param": up.download_encrypted_query_param,
                        "aes_key": aes_b64,
                        "encrypt_type": 1
                    },
                    "file_name": content.name,
                    "len": format!("{}", up.file_size_plain)
                }
            })
        };

        self.send_message_with_items(peer_id, &token, vec![item]).await?;
        info!("Weixin file sent to peer={peer_id} name={}", content.name);
        Ok(())
    }

    fn expired_download_message(language: BotLanguage) -> &'static str {
        if language.is_chinese() {
            "这个下载链接已过期，请重新让助手发送一次。"
        } else {
            "This download link has expired. Please ask the agent again."
        }
    }

    fn sending_file_message(language: BotLanguage, file_name: &str) -> String {
        if language.is_chinese() {
            format!("正在发送“{file_name}”……")
        } else {
            format!("Sending \"{file_name}\"…")
        }
    }

    fn send_file_failed_message(language: BotLanguage, file_name: &str, error: &str) -> String {
        if language.is_chinese() {
            format!("无法发送“{file_name}”：{error}")
        } else {
            format!("Could not send \"{file_name}\": {error}")
        }
    }

    async fn handle_download_request(
        &self,
        peer_id: &str,
        token: &str,
        workspace_root: Option<String>,
    ) {
        let (path, language) = {
            let mut states = self.chat_states.write().await;
            let state = states.get_mut(peer_id);
            let language = current_bot_language().await;
            let path = state.and_then(|s| s.pending_files.remove(token));
            (path, language)
        };

        match path {
            None => {
                let _ = self
                    .send_text(peer_id, Self::expired_download_message(language))
                    .await;
            }
            Some(path) => {
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                let _ = self
                    .send_text(peer_id, &Self::sending_file_message(language, &file_name))
                    .await;
                let root = workspace_root.as_deref().map(std::path::Path::new);
                match self.send_workspace_file_to_peer(peer_id, &path, root).await {
                    Ok(()) => info!("Weixin file delivered to {peer_id}: {path}"),
                    Err(e) => {
                        warn!("Weixin file send failed: {e}");
                        let _ = self
                            .send_text(
                                peer_id,
                                &Self::send_file_failed_message(language, &file_name, &e.to_string()),
                            )
                            .await;
                    }
                }
            }
        }
    }

    async fn get_updates_once(&self, buf: &str, timeout: Duration) -> Result<Value> {
        if self.is_session_paused().await {
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Ok(json!({
                "ret": 0,
                "msgs": [],
                "get_updates_buf": buf
            }));
        }

        let body = json!({
            "get_updates_buf": buf,
            "base_info": { "channel_version": CHANNEL_VERSION }
        });
        let raw = self
            .post_ilink("ilink/bot/getupdates", body, timeout)
            .await?;
        let v: Value = serde_json::from_str(&raw)?;
        let ret = v["ret"].as_i64().unwrap_or(0);
        let errcode = v["errcode"].as_i64().unwrap_or(0);
        if errcode == SESSION_EXPIRED_ERRCODE || ret == SESSION_EXPIRED_ERRCODE {
            self.pause_session().await;
        }
        Ok(v)
    }

    async fn send_message_raw(&self, to_user_id: &str, context_token: &str, text: &str) -> Result<()> {
        let client_id = format!("bitfun-wx-{}", uuid::Uuid::new_v4());
        let item_list = if text.is_empty() {
            None
        } else {
            Some(vec![json!({
                "type": 1,
                "text_item": { "text": text }
            })])
        };
        let msg = json!({
            "from_user_id": "",
            "to_user_id": to_user_id,
            "client_id": client_id,
            "message_type": 2,
            "message_state": 2,
            "item_list": item_list,
            "context_token": context_token,
        });
        let body = json!({
            "msg": msg,
            "base_info": { "channel_version": CHANNEL_VERSION }
        });
        self.post_ilink(
            "ilink/bot/sendmessage",
            body,
            Duration::from_secs(API_TIMEOUT_SECS),
        )
        .await?;
        Ok(())
    }

    /// Send text to peer; uses last known `context_token` for that peer.
    pub async fn send_text(&self, peer_id: &str, text: &str) -> Result<()> {
        let token = {
            let m = self.context_tokens.read().await;
            m.get(peer_id)
                .cloned()
                .ok_or_else(|| anyhow!("missing context_token for peer {peer_id}"))?
        };
        for chunk in chunk_text_for_weixin(text) {
            self.send_message_raw(peer_id, &token, &chunk).await?;
        }
        Ok(())
    }

    fn is_weixin_media_item_type(type_id: i64) -> bool {
        matches!(type_id, 2 | 3 | 4 | 5)
    }

    fn body_from_item_list(items: &[Value]) -> String {
        for item in items {
            let t = item["type"].as_i64().unwrap_or(0);
            if t == 1 {
                if let Some(tx) = item["text_item"]["text"].as_str() {
                    let text = tx.to_string();
                    let ref_msg = &item["ref_msg"];
                    if !ref_msg.is_object() {
                        return text;
                    }
                    let ref_title = ref_msg["title"].as_str();
                    let ref_item = &ref_msg["message_item"];
                    if ref_item.is_object() {
                        let mt = ref_item["type"].as_i64().unwrap_or(0);
                        if Self::is_weixin_media_item_type(mt) {
                            return text;
                        }
                        let ref_body = Self::body_from_item_list(std::slice::from_ref(ref_item));
                        if ref_title.is_none() && ref_body.is_empty() {
                            return text;
                        }
                        let mut parts: Vec<String> = Vec::new();
                        if let Some(tt) = ref_title {
                            parts.push(tt.to_string());
                        }
                        if !ref_body.is_empty() {
                            parts.push(ref_body);
                        }
                        if parts.is_empty() {
                            return text;
                        }
                        let joined = parts.join(" | ");
                        return format!("[引用: {joined}]\n{text}");
                    }
                    if let Some(tt) = ref_title {
                        return format!("[引用: {tt}]\n{text}");
                    }
                    return text;
                }
            }
            if t == 3 {
                if let Some(tx) = item["voice_item"]["text"].as_str() {
                    return tx.to_string();
                }
            }
        }
        String::new()
    }

    fn body_from_message(msg: &Value) -> String {
        let Some(items) = msg["item_list"].as_array() else {
            return String::new();
        };
        Self::body_from_item_list(items)
    }

    /// True if the message carries at least one `image_item` (pairing wait UX / guards).
    fn has_inbound_image_items(msg: &Value) -> bool {
        let Some(items) = msg["item_list"].as_array() else {
            return false;
        };
        items.iter().any(|i| {
            i["type"].as_i64() == Some(2)
                && i["image_item"]["media"]["encrypt_query_param"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
        })
    }

    fn is_user_message(msg: &Value) -> bool {
        msg["message_type"].as_i64() == Some(1)
    }

    fn peer_id(msg: &Value) -> Option<String> {
        msg["from_user_id"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }

    fn context_token(msg: &Value) -> Option<String> {
        msg["context_token"]
            .as_str()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }

    pub async fn register_pairing(&self, pairing_code: &str) -> Result<()> {
        self.pending_pairings.write().await.insert(
            pairing_code.to_string(),
            PendingPairing {
                created_at: chrono::Utc::now().timestamp(),
            },
        );
        Ok(())
    }

    pub async fn verify_pairing_code(&self, code: &str) -> bool {
        let mut pairings = self.pending_pairings.write().await;
        if let Some(p) = pairings.remove(code) {
            let age = chrono::Utc::now().timestamp() - p.created_at;
            return age < 300;
        }
        false
    }

    fn format_actions_footer(language: BotLanguage, actions: &[BotAction]) -> String {
        if actions.is_empty() {
            return String::new();
        }
        let header = if language.is_chinese() {
            "\n\n——\n快捷操作（可发送对应命令或回复数字）：\n"
        } else {
            "\n\n——\nQuick actions (send the command or reply with the number):\n"
        };
        let mut s = header.to_string();
        for (i, a) in actions.iter().enumerate() {
            let n = i + 1;
            if language.is_chinese() {
                s.push_str(&format!("{n}. {} → {}\n", a.label, a.command));
            } else {
                s.push_str(&format!("{n}. {} → {}\n", a.label, a.command));
            }
        }
        s
    }

    fn clean_reply_text(language: BotLanguage, text: &str, has_actions: bool) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut replaced_cancel = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("/cancel_task ") {
                if has_actions && !replaced_cancel {
                    let hint = if language.is_chinese() {
                        "如需停止本次请求，请发送命令 /cancel_task 或下方列出的取消命令。"
                    } else {
                        "To stop this request, send /cancel_task or the cancel command listed below."
                    };
                    lines.push(hint.to_string());
                    replaced_cancel = true;
                }
                continue;
            }
            lines.push(line.to_string());
        }
        lines.join("\n").trim().to_string()
    }

    async fn send_handle_result(&self, peer_id: &str, result: &HandleResult) {
        let language = current_bot_language().await;
        let footer = Self::format_actions_footer(language, &result.actions);
        let body = Self::clean_reply_text(language, &result.reply, !result.actions.is_empty());
        let combined = format!("{body}{footer}");
        if let Err(e) = self.send_text(peer_id, &combined).await {
            warn!("weixin send_handle_result: {e}");
        }
    }

    async fn notify_files_ready(&self, peer_id: &str, text: &str) {
        let result = {
            let mut states = self.chat_states.write().await;
            let state = states.entry(peer_id.to_string()).or_insert_with(|| {
                let mut s = BotChatState::new(peer_id.to_string());
                s.paired = true;
                s
            });
            let workspace_root = state.current_workspace.clone();
            super::prepare_file_download_actions(
                text,
                state,
                workspace_root.as_deref().map(std::path::Path::new),
            )
        };
        if let Some(result) = result {
            self.send_handle_result(peer_id, &result).await;
        }
    }

    async fn persist_chat_state(&self, peer_id: &str, state: &BotChatState) {
        let mut data = load_bot_persistence();
        data.upsert(SavedBotConnection {
            bot_type: "weixin".to_string(),
            chat_id: peer_id.to_string(),
            config: BotConfig::Weixin {
                ilink_token: self.config.ilink_token.clone(),
                base_url: self.config.base_url.clone(),
                bot_account_id: self.config.bot_account_id.clone(),
            },
            chat_state: state.clone(),
            connected_at: chrono::Utc::now().timestamp(),
        });
        save_bot_persistence(&data);
    }

    /// Pairing + message loop: long-poll getupdates.
    pub async fn wait_for_pairing(
        &self,
        stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<String> {
        info!("Weixin bot waiting for pairing code (getupdates)...");
        let mut buf = load_sync_buf(&self.config.bot_account_id);

        loop {
            if *stop_rx.borrow() {
                return Err(anyhow!("bot stop requested"));
            }

            let poll = tokio::select! {
                _ = stop_rx.changed() => {
                    return Err(anyhow!("bot stop requested"));
                }
                r = self.get_updates_once(
                    &buf,
                    Duration::from_secs(LONG_POLL_TIMEOUT_SECS),
                ) => r,
            };

            let resp = match poll {
                Ok(v) => v,
                Err(e) => {
                    error!("weixin getupdates: {e}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            let ret = resp["ret"].as_i64().unwrap_or(0);
            let errcode = resp["errcode"].as_i64().unwrap_or(0);
            if (ret != 0 && ret != SESSION_EXPIRED_ERRCODE) || (errcode != 0 && errcode != SESSION_EXPIRED_ERRCODE) {
                if errcode == SESSION_EXPIRED_ERRCODE || ret == SESSION_EXPIRED_ERRCODE {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                warn!("weixin getupdates ret={ret} errcode={errcode}");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            if let Some(new_buf) = resp["get_updates_buf"].as_str() {
                buf = new_buf.to_string();
                save_sync_buf(&self.config.bot_account_id, &buf);
            }

            if let Some(msgs) = resp["msgs"].as_array() {
                for msg in msgs {
                    if !Self::is_user_message(msg) {
                        continue;
                    }
                    let Some(peer) = Self::peer_id(msg) else { continue };
                    if let Some(ct) = Self::context_token(msg) {
                        self.context_tokens
                            .write()
                            .await
                            .insert(peer.clone(), ct);
                    }
                    let text = Self::body_from_message(msg).trim().to_string();
                    let language = current_bot_language().await;

                    if text == "/start" {
                        let _ = self.send_text(&peer, welcome_message(language)).await;
                        continue;
                    }

                    if text.len() == 6 && text.chars().all(|c| c.is_ascii_digit()) {
                        if self.verify_pairing_code(&text).await {
                            info!("Weixin pairing successful peer={peer}");
                            let mut state = BotChatState::new(peer.clone());
                            let result = complete_im_bot_pairing(&mut state).await;
                            self.chat_states
                                .write()
                                .await
                                .insert(peer.clone(), state.clone());
                            self.persist_chat_state(&peer, &state).await;

                            let footer =
                                Self::format_actions_footer(language, &result.actions);
                            let _ = self
                                .send_text(&peer, &format!("{}{}", result.reply, footer))
                                .await;
                            return Ok(peer);
                        } else {
                            let err = if language.is_chinese() {
                                "配对码无效或已过期，请重试。"
                            } else {
                                "Invalid or expired pairing code."
                            };
                            let _ = self.send_text(&peer, err).await;
                        }
                    } else if !text.is_empty() {
                        let err = if language.is_chinese() {
                            "请输入 BitFun 桌面端远程连接中显示的 6 位配对码。"
                        } else {
                            "Please send the 6-digit pairing code from BitFun Desktop Remote Connect."
                        };
                        let _ = self.send_text(&peer, err).await;
                    } else if Self::has_inbound_image_items(msg) {
                        let err = if language.is_chinese() {
                            "配对请直接发送 6 位数字配对码；完成配对后再发送图片与助手对话。"
                        } else {
                            "To pair, send the 6-digit code only. After pairing you can send images to chat."
                        };
                        let _ = self.send_text(&peer, err).await;
                    }
                }
            }
        }
    }

    pub async fn run_message_loop(self: Arc<Self>, stop_rx: tokio::sync::watch::Receiver<bool>) {
        info!("Weixin message loop started");
        let mut stop = stop_rx;
        let mut buf = load_sync_buf(&self.config.bot_account_id);

        loop {
            if *stop.borrow() {
                break;
            }

            let poll = tokio::select! {
                _ = stop.changed() => break,
                r = self.get_updates_once(
                    &buf,
                    Duration::from_secs(LONG_POLL_TIMEOUT_SECS),
                ) => r,
            };

            let resp = match poll {
                Ok(v) => v,
                Err(e) => {
                    error!("weixin getupdates (loop): {e}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            let ret = resp["ret"].as_i64().unwrap_or(0);
            let errcode = resp["errcode"].as_i64().unwrap_or(0);
            if (ret != 0 && ret != SESSION_EXPIRED_ERRCODE) || (errcode != 0 && errcode != SESSION_EXPIRED_ERRCODE) {
                if errcode == SESSION_EXPIRED_ERRCODE || ret == SESSION_EXPIRED_ERRCODE {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            if let Some(new_buf) = resp["get_updates_buf"].as_str() {
                buf = new_buf.to_string();
                save_sync_buf(&self.config.bot_account_id, &buf);
            }

            let Some(msgs) = resp["msgs"].as_array() else { continue };

            for msg in msgs {
                if !Self::is_user_message(msg) {
                    continue;
                }
                let Some(peer) = Self::peer_id(msg) else { continue };
                if let Some(ct) = Self::context_token(msg) {
                    self.context_tokens
                        .write()
                        .await
                        .insert(peer.clone(), ct);
                }
                let msg_value = msg.clone();
                let bot = self.clone();
                tokio::spawn(async move {
                    let (images, skipped_images) =
                        bot.inbound_image_attachments_from_message(&msg_value).await;
                    let language = current_bot_language().await;
                    // Match Feishu: truncation is a separate user-visible message, not mixed into command text.
                    if skipped_images > 0 {
                        let note = if language.is_chinese() {
                            format!(
                                "仅会处理前 {} 张图片，其余 {} 张已丢弃。",
                                MAX_INBOUND_IMAGES, skipped_images
                            )
                        } else {
                            format!(
                                "Only the first {} images will be processed; the remaining {} were discarded.",
                                MAX_INBOUND_IMAGES, skipped_images
                            )
                        };
                        let _ = bot.send_text(&peer, &note).await;
                    }
                    let body = WeixinBot::body_from_message(&msg_value);
                    let text = if body.trim().is_empty() && !images.is_empty() {
                        if language.is_chinese() {
                            "[用户发送了一张图片]".to_string()
                        } else {
                            "[User sent an image]".to_string()
                        }
                    } else {
                        body
                    };
                    bot.handle_incoming_message(peer, &text, images).await;
                });
            }
        }
        info!("Weixin message loop stopped");
    }

    async fn handle_incoming_message(
        self: &Arc<Self>,
        peer_id: String,
        text: &str,
        images: Vec<ImageAttachment>,
    ) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(peer_id.clone()).or_insert_with(|| {
            let mut s = BotChatState::new(peer_id.clone());
            s.paired = true;
            s
        });
        let language = current_bot_language().await;

        if !state.paired {
            let trimmed = text.trim();
            if trimmed == "/start" {
                drop(states);
                let _ = self.send_text(&peer_id, welcome_message(language)).await;
                return;
            }
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                if self.verify_pairing_code(trimmed).await {
                    let result = complete_im_bot_pairing(state).await;
                    self.persist_chat_state(&peer_id, state).await;
                    drop(states);
                    let footer =
                        Self::format_actions_footer(language, &result.actions);
                    let _ = self
                        .send_text(&peer_id, &format!("{}{}", result.reply, footer))
                        .await;
                    return;
                } else {
                    let err = if language.is_chinese() {
                        "配对码无效或已过期。"
                    } else {
                        "Invalid or expired pairing code."
                    };
                    drop(states);
                    let _ = self.send_text(&peer_id, err).await;
                    return;
                }
            }
            drop(states);
            let err = if language.is_chinese() {
                "请输入 6 位配对码。"
            } else {
                "Please send the 6-digit pairing code."
            };
            let _ = self.send_text(&peer_id, err).await;
            return;
        }

        let trimmed = text.trim();
        if trimmed.starts_with("download_file:") {
            let token = trimmed["download_file:".len()..].trim().to_string();
            let workspace_root = state.current_workspace.clone();
            drop(states);
            self.handle_download_request(&peer_id, &token, workspace_root)
                .await;
            return;
        }

        let cmd = parse_command(text);
        let result = handle_command(state, cmd, images).await;
        self.persist_chat_state(&peer_id, state).await;
        drop(states);

        self.send_handle_result(&peer_id, &result).await;

        if let Some(forward) = result.forward_to_session {
            let bot = self.clone();
            let peer = peer_id.clone();
            tokio::spawn(async move {
                let interaction_bot = bot.clone();
                let peer_c = peer.clone();
                let handler: BotInteractionHandler = Arc::new(move |interaction: BotInteractiveRequest| {
                    let interaction_bot = interaction_bot.clone();
                    let peer_i = peer_c.clone();
                    Box::pin(async move {
                        interaction_bot
                            .deliver_interaction(peer_i, interaction)
                            .await;
                    })
                });
                let msg_bot = bot.clone();
                let peer_m = peer.clone();
                let sender: BotMessageSender = Arc::new(move |t: String| {
                    let msg_bot = msg_bot.clone();
                    let peer_s = peer_m.clone();
                    Box::pin(async move {
                        let _ = msg_bot.send_text(&peer_s, &t).await;
                    })
                });
                let verbose_mode = load_bot_persistence().verbose_mode;
                let turn_result =
                    execute_forwarded_turn(forward, Some(handler), Some(sender), verbose_mode).await;
                let _ = bot.send_text(&peer, &turn_result.display_text).await;
                bot.notify_files_ready(&peer, &turn_result.full_text).await;
            });
        }
    }

    async fn deliver_interaction(&self, peer_id: String, interaction: BotInteractiveRequest) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(peer_id.clone()).or_insert_with(|| {
            let mut s = BotChatState::new(peer_id.clone());
            s.paired = true;
            s
        });
        state.pending_action = Some(interaction.pending_action.clone());
        self.persist_chat_state(&peer_id, state).await;
        drop(states);

        let result = HandleResult {
            reply: interaction.reply,
            actions: interaction.actions,
            forward_to_session: None,
        };
        self.send_handle_result(&peer_id, &result).await;
    }
}

fn chunk_text_for_weixin(text: &str) -> Vec<String> {
    if text.len() <= MAX_TEXT_CHUNK {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.len() <= MAX_TEXT_CHUNK {
            out.push(rest.to_string());
            break;
        }
        let mut cut = MAX_TEXT_CHUNK;
        while cut > 0 && !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        if cut == 0 {
            cut = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
        out.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
    out
}

#[cfg(test)]
mod weixin_inbound_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aes_ecb_roundtrip() {
        let key = [9u8; 16];
        let plain = b"hello weixin cdn";
        let ct = encrypt_aes_128_ecb_pkcs7(plain, &key);
        let back = decrypt_aes_128_ecb_pkcs7(&ct, &key).unwrap();
        assert_eq!(back.as_slice(), plain.as_slice());
    }

    #[test]
    fn parse_aes_key_raw16_base64() {
        let raw = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let b64 = B64.encode(raw);
        let k = parse_weixin_cdn_aes_key(&b64).unwrap();
        assert_eq!(k, raw);
    }

    #[test]
    fn parse_aes_key_hex_wrapped_base64() {
        let raw = [0xabu8; 16];
        let hex_str = hex::encode(raw);
        let b64 = B64.encode(hex_str.as_bytes());
        let k = parse_weixin_cdn_aes_key(&b64).unwrap();
        assert_eq!(k, raw);
    }

    #[test]
    fn body_from_message_plain_text() {
        let msg = json!({
            "item_list": [{ "type": 1, "text_item": { "text": "hi" } }]
        });
        assert_eq!(WeixinBot::body_from_message(&msg), "hi");
    }

    #[test]
    fn body_from_message_quoted_text() {
        let msg = json!({
            "item_list": [{
                "type": 1,
                "text_item": { "text": "reply" },
                "ref_msg": { "title": " earlier ", "message_item": { "type": 1, "text_item": { "text": "orig" } } }
            }]
        });
        let b = WeixinBot::body_from_message(&msg);
        assert!(b.contains("[引用:"));
        assert!(b.contains("reply"));
    }
}
