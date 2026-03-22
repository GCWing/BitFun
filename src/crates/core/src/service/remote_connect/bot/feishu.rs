//! Feishu (Lark) bot integration for Remote Connect.
//!
//! Users create their own Feishu bot on the Feishu Open Platform and provide
//! App ID + App Secret.  The desktop receives messages via Feishu's WebSocket
//! long connection and routes them through the shared command router.

use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::command_router::{
    complete_im_bot_pairing, current_bot_language, execute_forwarded_turn, handle_command,
    parse_command, welcome_message, BotAction, BotActionStyle,
    BotChatState, BotInteractionHandler, BotInteractiveRequest, BotLanguage, BotMessageSender,
    HandleResult,
};
use super::{load_bot_persistence, save_bot_persistence, BotConfig, SavedBotConnection};

// ── Minimal protobuf codec for Feishu WebSocket binary protocol ─────────

mod pb {
    //! Protobuf codec matching Feishu SDK's pbbp2.proto.
    //! Field numbers from pbbp2.pb.go (proto2 with required fields):
    //!   1: SeqID (uint64)
    //!   2: LogID (uint64)
    //!   3: Service (int32)
    //!   4: Method (int32)       — 0 = control, 1 = data
    //!   5: Headers (repeated Header)
    //!   6: PayloadEncoding (string)
    //!   7: PayloadType (string)
    //!   8: Payload (bytes)
    //!   9: LogIDNew (string)

    #[derive(Debug, Default, Clone)]
    pub struct Frame {
        pub seq_id: u64,
        pub log_id: u64,
        pub service: i32,
        pub method: i32,
        pub headers: Vec<(String, String)>,
        pub payload_encoding: String,
        pub payload_type: String,
        pub payload: Vec<u8>,
        pub log_id_new: String,
    }

    pub const FRAME_TYPE_CONTROL: i32 = 0;
    pub const FRAME_TYPE_DATA: i32 = 1;

    fn decode_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            if *pos >= data.len() {
                return None;
            }
            let byte = data[*pos];
            *pos += 1;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
    }

    fn encode_varint(mut val: u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(10);
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            buf.push(byte);
            if val == 0 {
                break;
            }
        }
        buf
    }

    fn read_len<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
        let len = decode_varint(data, pos)? as usize;
        if *pos + len > data.len() {
            return None;
        }
        let slice = &data[*pos..*pos + len];
        *pos += len;
        Some(slice)
    }

    fn decode_header(data: &[u8]) -> Option<(String, String)> {
        let mut pos = 0;
        let (mut key, mut val) = (String::new(), String::new());
        while pos < data.len() {
            let tag = decode_varint(data, &mut pos)? as u32;
            match (tag >> 3, tag & 7) {
                (1, 2) => key = String::from_utf8_lossy(read_len(data, &mut pos)?).into(),
                (2, 2) => val = String::from_utf8_lossy(read_len(data, &mut pos)?).into(),
                (_, 0) => {
                    decode_varint(data, &mut pos)?;
                }
                (_, 2) => {
                    read_len(data, &mut pos)?;
                }
                _ => return None,
            }
        }
        Some((key, val))
    }

    pub fn decode_frame(data: &[u8]) -> Option<Frame> {
        let mut pos = 0;
        let mut f = Frame::default();
        while pos < data.len() {
            let tag = decode_varint(data, &mut pos)? as u32;
            match (tag >> 3, tag & 7) {
                (1, 0) => f.seq_id = decode_varint(data, &mut pos)?,
                (2, 0) => f.log_id = decode_varint(data, &mut pos)?,
                (3, 0) => f.service = decode_varint(data, &mut pos)? as i32,
                (4, 0) => f.method = decode_varint(data, &mut pos)? as i32,
                (5, 2) => {
                    if let Some(h) = decode_header(read_len(data, &mut pos)?) {
                        f.headers.push(h);
                    }
                }
                (6, 2) => {
                    f.payload_encoding = String::from_utf8_lossy(read_len(data, &mut pos)?).into()
                }
                (7, 2) => {
                    f.payload_type = String::from_utf8_lossy(read_len(data, &mut pos)?).into()
                }
                (8, 2) => f.payload = read_len(data, &mut pos)?.to_vec(),
                (9, 2) => f.log_id_new = String::from_utf8_lossy(read_len(data, &mut pos)?).into(),
                (_, 0) => {
                    decode_varint(data, &mut pos)?;
                }
                (_, 2) => {
                    read_len(data, &mut pos)?;
                }
                (_, 5) => {
                    pos += 4;
                } // fixed32
                (_, 1) => {
                    pos += 8;
                } // fixed64
                _ => return None,
            }
        }
        Some(f)
    }

    fn write_varint(buf: &mut Vec<u8>, field: u32, val: u64) {
        buf.extend(encode_varint(((field << 3) | 0) as u64));
        buf.extend(encode_varint(val));
    }

    fn write_bytes(buf: &mut Vec<u8>, field: u32, data: &[u8]) {
        buf.extend(encode_varint(((field << 3) | 2) as u64));
        buf.extend(encode_varint(data.len() as u64));
        buf.extend(data);
    }

    fn encode_header(key: &str, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        write_bytes(&mut buf, 1, key.as_bytes());
        write_bytes(&mut buf, 2, value.as_bytes());
        buf
    }

    pub fn encode_frame(frame: &Frame) -> Vec<u8> {
        let mut buf = Vec::new();
        write_varint(&mut buf, 1, frame.seq_id);
        write_varint(&mut buf, 2, frame.log_id);
        write_varint(&mut buf, 3, frame.service as u64);
        write_varint(&mut buf, 4, frame.method as u64);
        for (k, v) in &frame.headers {
            let hdr = encode_header(k, v);
            write_bytes(&mut buf, 5, &hdr);
        }
        if !frame.payload_encoding.is_empty() {
            write_bytes(&mut buf, 6, frame.payload_encoding.as_bytes());
        }
        if !frame.payload_type.is_empty() {
            write_bytes(&mut buf, 7, frame.payload_type.as_bytes());
        }
        if !frame.payload.is_empty() {
            write_bytes(&mut buf, 8, &frame.payload);
        }
        if !frame.log_id_new.is_empty() {
            write_bytes(&mut buf, 9, frame.log_id_new.as_bytes());
        }
        buf
    }

    impl Frame {
        pub fn get_header(&self, key: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        }

        pub fn new_ping(service_id: i32) -> Self {
            Frame {
                method: FRAME_TYPE_CONTROL,
                service: service_id,
                headers: vec![("type".into(), "ping".into())],
                ..Default::default()
            }
        }

        pub fn new_response(original: &Frame, status_code: u16) -> Self {
            let mut headers = original.headers.clone();
            headers.push(("biz_rt".into(), "0".into()));
            Frame {
                seq_id: original.seq_id,
                log_id: original.log_id,
                service: original.service,
                method: original.method,
                headers,
                payload: serde_json::to_vec(&serde_json::json!({"code": status_code}))
                    .unwrap_or_default(),
                log_id_new: original.log_id_new.clone(),
                ..Default::default()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Clone)]
struct FeishuToken {
    access_token: String,
    expires_at: i64,
}

pub struct FeishuBot {
    config: FeishuConfig,
    token: Arc<RwLock<Option<FeishuToken>>>,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    chat_states: Arc<RwLock<HashMap<String, BotChatState>>>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

struct ParsedMessage {
    chat_id: String,
    message_id: String,
    text: String,
    image_keys: Vec<String>,
}

impl FeishuBot {
    fn invalid_pairing_code_message(language: BotLanguage) -> &'static str {
        if language.is_chinese() {
            "配对码无效或已过期，请重试。"
        } else {
            "Invalid or expired pairing code. Please try again."
        }
    }

    fn enter_pairing_code_message(language: BotLanguage) -> &'static str {
        if language.is_chinese() {
            "请输入 BitFun Desktop 中显示的 6 位配对码。"
        } else {
            "Please enter the 6-digit pairing code from BitFun Desktop."
        }
    }

    fn unsupported_message_type_message(language: BotLanguage) -> &'static str {
        if language.is_chinese() {
            "暂不支持这种消息类型，请发送文本或图片。"
        } else {
            "This message type is not supported. Please send text or images."
        }
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

    pub fn new(config: FeishuConfig) -> Self {
        Self {
            config,
            token: Arc::new(RwLock::new(None)),
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            chat_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn restore_chat_state(&self, chat_id: &str, state: BotChatState) {
        self.chat_states
            .write()
            .await
            .insert(chat_id.to_string(), state);
    }

    async fn get_access_token(&self) -> Result<String> {
        {
            let guard = self.token.read().await;
            if let Some(t) = guard.as_ref() {
                if t.expires_at > chrono::Utc::now().timestamp() + 60 {
                    return Ok(t.access_token.clone());
                }
            }
        }

        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.config.app_id,
                "app_secret": self.config.app_secret,
            }))
            .send()
            .await
            .map_err(|e| anyhow!("feishu token request: {e}"))?;

        let token_resp_text = resp.text().await.unwrap_or_default();
        let body: serde_json::Value = serde_json::from_str(&token_resp_text).map_err(|e| {
            anyhow!(
                "feishu token response parse error: {e}, body: {}",
                &token_resp_text[..token_resp_text.len().min(200)]
            )
        })?;
        let access_token = body["tenant_access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("missing tenant_access_token in response"))?
            .to_string();
        let expire = body["expire"].as_i64().unwrap_or(7200);

        *self.token.write().await = Some(FeishuToken {
            access_token: access_token.clone(),
            expires_at: chrono::Utc::now().timestamp() + expire,
        });

        info!("Feishu access token refreshed");
        Ok(access_token)
    }

    pub async fn send_message(&self, chat_id: &str, content: &str) -> Result<()> {
        let token = self.get_access_token().await?;
        let card = Self::build_markdown_card(content);
        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .query(&[("receive_id_type", "chat_id")])
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": "interactive",
                "content": serde_json::to_string(&card)?,
            }))
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("feishu send_message HTTP {status}: {body}"));
        }
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(code) = parsed.get("code").and_then(|c| c.as_i64()) {
                if code != 0 {
                    let msg = parsed
                        .get("msg")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown");
                    warn!("Feishu send_message API error: code={code}, msg={msg}");
                    return Err(anyhow!(
                        "feishu send_message API error: code={code}, msg={msg}"
                    ));
                }
            }
        }
        debug!("Feishu message sent to {chat_id}");
        Ok(())
    }

    fn build_markdown_card(content: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": "2.0",
            "config": {
                "wide_screen_mode": true,
            },
            "body": {
                "elements": [
                    {
                        "tag": "markdown",
                        "content": content,
                        "text_align": "left",
                        "text_size": "normal",
                        "margin": "0px 0px 0px 0px",
                        "element_id": "bitfun_remote_reply_markdown",
                    }
                ],
            },
        })
    }

    /// Download a user-sent image from a Feishu message using the message resources API.
    /// The returned data-URL is compressed to at most 1 MB.
    async fn download_image_as_data_url(&self, message_id: &str, file_key: &str) -> Result<String> {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let token = match self.get_access_token().await {
            Ok(t) => t,
            Err(e) => {
                return Err(e);
            }
        };
        let client = reqwest::Client::new();
        let url = format!(
            "https://open.feishu.cn/open-apis/im/v1/messages/{}/resources/{}?type=image",
            message_id, file_key
        );
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| anyhow!("feishu download image: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "feishu image download failed: HTTP {status} — {body}"
            ));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .to_string();
        let raw_bytes = resp.bytes().await?;

        const MAX_BYTES: usize = 1024 * 1024;
        if raw_bytes.len() <= MAX_BYTES {
            let b64 = B64.encode(&raw_bytes);
            return Ok(format!("data:{};base64,{}", content_type, b64));
        }

        log::info!(
            "Feishu image exceeds {}KB ({}KB), compressing",
            MAX_BYTES / 1024,
            raw_bytes.len() / 1024
        );
        match crate::agentic::image_analysis::optimize_image_with_size_limit(
            raw_bytes.to_vec(),
            "openai",
            Some(&content_type),
            Some(MAX_BYTES),
        ) {
            Ok(processed) => {
                let b64 = B64.encode(&processed.data);
                Ok(format!("data:{};base64,{}", processed.mime_type, b64))
            }
            Err(e) => {
                log::warn!("Feishu image compression failed, using original: {e}");
                let b64 = B64.encode(&raw_bytes);
                Ok(format!("data:{};base64,{}", content_type, b64))
            }
        }
    }

    /// Download multiple images and convert to ImageAttachment list.
    async fn download_images(
        &self,
        message_id: &str,
        image_keys: &[String],
    ) -> Vec<super::super::remote_server::ImageAttachment> {
        let mut attachments = Vec::new();
        for (i, key) in image_keys.iter().enumerate() {
            match self.download_image_as_data_url(message_id, key).await {
                Ok(data_url) => {
                    attachments.push(super::super::remote_server::ImageAttachment {
                        name: format!("image_{}.png", i + 1),
                        data_url,
                    });
                }
                Err(e) => {
                    warn!("Failed to download Feishu image {key}: {e}");
                }
            }
        }
        attachments
    }

    pub async fn send_action_card(
        &self,
        chat_id: &str,
        language: BotLanguage,
        content: &str,
        actions: &[BotAction],
    ) -> Result<()> {
        let token = self.get_access_token().await?;
        let client = reqwest::Client::new();
        let card = Self::build_action_card(chat_id, language, content, actions);
        let resp = client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .query(&[("receive_id_type", "chat_id")])
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": "interactive",
                "content": serde_json::to_string(&card)?,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("feishu send_action_card failed: {body}"));
        }
        debug!("Feishu action card sent to {chat_id}");
        Ok(())
    }

    async fn send_handle_result(&self, chat_id: &str, result: &HandleResult) -> Result<()> {
        let language = current_bot_language().await;
        if result.actions.is_empty() {
            self.send_message(chat_id, &result.reply).await
        } else {
            self.send_action_card(chat_id, language, &result.reply, &result.actions)
                .await
        }
    }

    /// Upload a local file to Feishu and return its `file_key`.
    ///
    /// Files larger than 30 MB are rejected (Feishu IM file-upload limit).
    async fn upload_file_to_feishu(&self, file_path: &str) -> Result<String> {
        let token = self.get_access_token().await?;

        const MAX_SIZE: u64 = 30 * 1024 * 1024; // Unified 30 MB cap (Feishu API hard limit)
        let content = super::read_workspace_file(file_path, MAX_SIZE, None).await?;

        // Feishu uses its own file_type enum rather than MIME types.
        let ext = std::path::Path::new(&content.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let file_type = match ext.as_str() {
            "pdf" => "pdf",
            "doc" | "docx" => "doc",
            "xls" | "xlsx" => "xls",
            "ppt" | "pptx" => "ppt",
            "mp4" => "mp4",
            _ => "stream",
        };

        let part = reqwest::multipart::Part::bytes(content.bytes)
            .file_name(content.name.clone())
            .mime_str("application/octet-stream")?;

        let form = reqwest::multipart::Form::new()
            .text("file_type", file_type.to_string())
            .text("file_name", content.name)
            .part("file", part);

        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/im/v1/files")
            .bearer_auth(&token)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Feishu file upload failed: {body}"));
        }

        let body: serde_json::Value = resp.json().await?;
        body.pointer("/data/file_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Feishu upload response missing file_key"))
    }

    /// Upload a local file and send it to a Feishu chat as a file message.
    async fn send_file_to_feishu_chat(&self, chat_id: &str, file_path: &str) -> Result<()> {
        let file_key = self.upload_file_to_feishu(file_path).await?;
        let token = self.get_access_token().await?;

        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .query(&[("receive_id_type", "chat_id")])
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "receive_id": chat_id,
                "msg_type": "file",
                "content": serde_json::to_string(&serde_json::json!({"file_key": file_key}))?,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Feishu file message failed: {body}"));
        }
        debug!("Feishu file sent to {chat_id}: {file_path}");
        Ok(())
    }

    /// Scan `text` for downloadable file links (`computer://`, `file://`, and
    /// markdown hyperlinks to local files), store them as pending downloads and
    /// send an interactive card with one download button per file.
    async fn notify_files_ready(&self, chat_id: &str, text: &str) {
        let result = {
            let mut states = self.chat_states.write().await;
            let state = states.entry(chat_id.to_string()).or_insert_with(|| {
                let mut s = BotChatState::new(chat_id.to_string());
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
            if let Err(e) = self.send_handle_result(chat_id, &result).await {
                warn!("Failed to send file notification to Feishu: {e}");
            }
        }
    }

    /// Handle a `download_file:<token>` action: look up the pending file and
    /// upload it to Feishu.  Sends a plain-text error if the token has expired
    /// or the transfer fails.
    async fn handle_download_request(&self, chat_id: &str, token: &str) {
        let (path, language) = {
            let mut states = self.chat_states.write().await;
            let state = states.get_mut(chat_id);
            let language = current_bot_language().await;
            let path = state.and_then(|s| s.pending_files.remove(token));
            (path, language)
        };

        match path {
            None => {
                let _ = self
                    .send_message(chat_id, Self::expired_download_message(language))
                    .await;
            }
            Some(path) => {
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                let _ = self
                    .send_message(chat_id, &Self::sending_file_message(language, &file_name))
                    .await;
                match self.send_file_to_feishu_chat(chat_id, &path).await {
                    Ok(()) => info!("Sent file to Feishu chat {chat_id}: {path}"),
                    Err(e) => {
                        warn!("Failed to send file to Feishu: {e}");
                        let _ = self
                            .send_message(
                                chat_id,
                                &Self::send_file_failed_message(
                                    language,
                                    &file_name,
                                    &e.to_string(),
                                ),
                            )
                            .await;
                    }
                }
            }
        }
    }

    fn build_action_card(
        chat_id: &str,
        language: BotLanguage,
        content: &str,
        actions: &[BotAction],
    ) -> serde_json::Value {
        let body = Self::card_body_text(language, content);
        let mut elements = vec![serde_json::json!({
            "tag": "markdown",
            "content": body,
        })];

        for chunk in actions.chunks(2) {
            let buttons: Vec<_> = chunk
                .iter()
                .map(|action| {
                    let button_type = match action.style {
                        BotActionStyle::Primary => "primary",
                        BotActionStyle::Default => "default",
                    };
                    serde_json::json!({
                        "tag": "button",
                        "text": {
                            "tag": "plain_text",
                            "content": action.label,
                        },
                        "type": button_type,
                        "value": {
                            "chat_id": chat_id,
                            "command": action.command,
                        }
                    })
                })
                .collect();
            elements.push(serde_json::json!({
                "tag": "action",
                "actions": buttons,
            }));
        }

        serde_json::json!({
            "config": {
                "wide_screen_mode": true,
            },
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": "BitFun Remote Connect",
                }
            },
            "elements": elements,
        })
    }

    fn card_body_text(language: BotLanguage, content: &str) -> String {
        let mut removed_command_lines = false;
        let mut lines = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('/') && trimmed.contains(" - ") {
                removed_command_lines = true;
                continue;
            }
            if trimmed.contains("/cancel_task ") {
                lines.push(if language.is_chinese() {
                    "如需停止本次请求，请使用下方的“取消任务”按钮。".to_string()
                } else {
                    "If needed, use the Cancel Task button below to stop this request.".to_string()
                });
                continue;
            }
            lines.push(Self::replace_command_tokens(language, line));
        }

        let mut body = lines.join("\n").trim().to_string();
        if removed_command_lines {
            if !body.is_empty() {
                body.push_str("\n\n");
            }
            body.push_str(if language.is_chinese() {
                "请选择下方操作。"
            } else {
                "Choose an action below."
            });
        }

        if body.is_empty() {
            if language.is_chinese() {
                "请选择下方操作。".to_string()
            } else {
                "Choose an action below.".to_string()
            }
        } else {
            body
        }
    }

    fn replace_command_tokens(language: BotLanguage, line: &str) -> String {
        let replacements = [
            (
                "/switch_workspace",
                if language.is_chinese() {
                    "切换工作区"
                } else {
                    "Switch Workspace"
                },
            ),
            (
                "/pro",
                if language.is_chinese() {
                    "专业模式"
                } else {
                    "Expert Mode"
                },
            ),
            (
                "/assistant",
                if language.is_chinese() {
                    "助理模式"
                } else {
                    "Assistant Mode"
                },
            ),
            (
                "/resume_session",
                if language.is_chinese() {
                    "恢复会话"
                } else {
                    "Resume Session"
                },
            ),
            (
                "/new_code_session",
                if language.is_chinese() {
                    "新建编码会话"
                } else {
                    "New Code Session"
                },
            ),
            (
                "/new_cowork_session",
                if language.is_chinese() {
                    "新建协作会话"
                } else {
                    "New Cowork Session"
                },
            ),
            (
                "/new_claw_session",
                if language.is_chinese() {
                    "新建助理会话"
                } else {
                    "New Claw Session"
                },
            ),
            (
                "/cancel_task",
                if language.is_chinese() {
                    "取消任务"
                } else {
                    "Cancel Task"
                },
            ),
            (
                "/help",
                if language.is_chinese() {
                    "帮助"
                } else {
                    "Help"
                },
            ),
        ];

        replacements
            .iter()
            .fold(line.to_string(), |acc, (from, to)| acc.replace(from, to))
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

    /// Obtain a WebSocket URL from Feishu for long-connection event delivery.
    /// Uses direct AppID/AppSecret auth per Feishu SDK protocol (no bearer token).
    async fn get_ws_endpoint(&self) -> Result<(String, serde_json::Value)> {
        let client = reqwest::Client::new();
        let resp = client
            .post("https://open.feishu.cn/callback/ws/endpoint")
            .json(&serde_json::json!({
                "AppID": self.config.app_id,
                "AppSecret": self.config.app_secret,
            }))
            .send()
            .await
            .map_err(|e| anyhow!("feishu ws endpoint request: {e}"))?;

        let ws_resp_text = resp.text().await.unwrap_or_default();
        let body: serde_json::Value = serde_json::from_str(&ws_resp_text).map_err(|e| {
            anyhow!(
                "feishu ws endpoint parse error: {e}, body: {}",
                &ws_resp_text[..ws_resp_text.len().min(300)]
            )
        })?;
        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            let msg = body["msg"].as_str().unwrap_or("unknown error");
            return Err(anyhow!("feishu ws endpoint error {code}: {msg}"));
        }

        let url = body
            .pointer("/data/URL")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("missing WebSocket URL in feishu response"))?
            .to_string();
        let client_config = body
            .pointer("/data/ClientConfig")
            .cloned()
            .unwrap_or_default();

        Ok((url, client_config))
    }

    /// Parse a Feishu message event into text + image keys.
    /// Supports `text`, `post` (rich text with images), and `image` message types.
    fn parse_message_event_full(event: &serde_json::Value) -> Option<ParsedMessage> {
        let event_type = event
            .pointer("/header/event_type")
            .and_then(|v| v.as_str())?;
        if event_type != "im.message.receive_v1" {
            return None;
        }

        let chat_id = event
            .pointer("/event/message/chat_id")
            .and_then(|v| v.as_str())?
            .to_string();
        let message_id = event
            .pointer("/event/message/message_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let msg_type = event
            .pointer("/event/message/message_type")
            .and_then(|v| v.as_str())?;
        let content_str = event
            .pointer("/event/message/content")
            .and_then(|v| v.as_str())?;
        let content: serde_json::Value = serde_json::from_str(content_str).ok()?;

        match msg_type {
            "text" => {
                let text = content["text"].as_str()?.trim().to_string();
                if text.is_empty() {
                    return None;
                }
                Some(ParsedMessage {
                    chat_id,
                    message_id,
                    text,
                    image_keys: vec![],
                })
            }
            "post" => {
                let (text, image_keys) = Self::extract_from_post(&content);
                if text.is_empty() && image_keys.is_empty() {
                    return None;
                }
                Some(ParsedMessage {
                    chat_id,
                    message_id,
                    text,
                    image_keys,
                })
            }
            "image" => {
                let image_key = content["image_key"].as_str()?.to_string();
                Some(ParsedMessage {
                    chat_id,
                    message_id,
                    text: String::new(),
                    image_keys: vec![image_key],
                })
            }
            _ => None,
        }
    }

    /// Backward-compatible wrapper: returns (chat_id, text) only for text/post with text content.
    #[cfg(test)]
    fn parse_message_event(event: &serde_json::Value) -> Option<(String, String)> {
        let parsed = Self::parse_message_event_full(event)?;
        if parsed.text.is_empty() {
            return None;
        }
        Some((parsed.chat_id, parsed.text))
    }

    /// Extract text and image keys from a Feishu `post` (rich-text) message.
    fn extract_from_post(content: &serde_json::Value) -> (String, Vec<String>) {
        let root = if content["content"].is_array() {
            content
        } else {
            content
                .get("zh_cn")
                .or_else(|| content.get("en_us"))
                .or_else(|| content.as_object().and_then(|obj| obj.values().next()))
                .unwrap_or(content)
        };

        let paragraphs = match root["content"].as_array() {
            Some(p) => p,
            None => return (String::new(), vec![]),
        };

        let mut text_parts: Vec<String> = Vec::new();
        let mut image_keys: Vec<String> = Vec::new();

        for para in paragraphs {
            if let Some(elements) = para.as_array() {
                for elem in elements {
                    match elem["tag"].as_str().unwrap_or("") {
                        "text" | "a" => {
                            if let Some(t) = elem["text"].as_str() {
                                let trimmed = t.trim();
                                if !trimmed.is_empty() {
                                    text_parts.push(trimmed.to_string());
                                }
                            }
                        }
                        "img" => {
                            if let Some(key) = elem["image_key"].as_str() {
                                if !key.is_empty() {
                                    image_keys.push(key.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let title = root["title"].as_str().unwrap_or("").trim();
        if !title.is_empty() {
            text_parts.insert(0, title.to_string());
        }

        (text_parts.join(" "), image_keys)
    }

    /// Extract (chat_id, command) from a Feishu card action callback.
    fn parse_card_action_event(event: &serde_json::Value) -> Option<(String, String)> {
        let event_type = event
            .pointer("/header/event_type")
            .and_then(|v| v.as_str())?;
        if event_type != "card.action.trigger" {
            return None;
        }

        let chat_id = event
            .pointer("/event/action/value/chat_id")
            .and_then(|v| v.as_str())
            .or_else(|| {
                event
                    .pointer("/event/context/open_chat_id")
                    .and_then(|v| v.as_str())
            })?
            .to_string();
        let command = event
            .pointer("/event/action/value/command")
            .and_then(|v| v.as_str())?
            .trim()
            .to_string();

        Some((chat_id, command))
    }

    #[cfg(test)]
    fn parse_ws_event(event: &serde_json::Value) -> Option<(String, String)> {
        Self::parse_message_event(event).or_else(|| Self::parse_card_action_event(event))
    }

    /// Extract chat_id from any im.message.receive_v1 event (regardless of msg_type).
    fn extract_message_chat_id(event: &serde_json::Value) -> Option<String> {
        let event_type = event
            .pointer("/header/event_type")
            .and_then(|v| v.as_str())?;
        if event_type != "im.message.receive_v1" {
            return None;
        }
        event
            .pointer("/event/message/chat_id")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Handle a single incoming protobuf data frame.
    /// Returns Some(chat_id) if pairing succeeded, None to continue waiting.
    async fn handle_data_frame_for_pairing(
        &self,
        frame: &pb::Frame,
        write: &Arc<
            RwLock<
                futures::stream::SplitSink<
                    tokio_tungstenite::WebSocketStream<
                        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                    >,
                    WsMessage,
                >,
            >,
        >,
    ) -> Option<String> {
        let msg_type = frame.get_header("type").unwrap_or("");
        if msg_type != "event" {
            return None;
        }

        let event: serde_json::Value = serde_json::from_slice(&frame.payload).ok()?;

        // Send ack response for this frame
        let resp_frame = pb::Frame::new_response(frame, 200);
        let _ = write
            .write()
            .await
            .send(WsMessage::Binary(pb::encode_frame(&resp_frame)))
            .await;

        if let Some(parsed) = Self::parse_message_event_full(&event) {
            let language = current_bot_language().await;
            let chat_id = parsed.chat_id;
            let msg_text = parsed.text;
            let trimmed = msg_text.trim();

            if trimmed == "/start" {
                self.send_message(&chat_id, welcome_message(language))
                    .await
                    .ok();
            } else if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                if self.verify_pairing_code(trimmed).await {
                    info!("Feishu pairing successful, chat_id={chat_id}");
                    let mut state = BotChatState::new(chat_id.clone());
                    let result = complete_im_bot_pairing(&mut state).await;
                    self.send_handle_result(&chat_id, &result).await.ok();
                    self.chat_states
                        .write()
                        .await
                        .insert(chat_id.clone(), state.clone());
                    self.persist_chat_state(&chat_id, &state).await;

                    return Some(chat_id);
                } else {
                    self.send_message(&chat_id, Self::invalid_pairing_code_message(language))
                        .await
                        .ok();
                }
            } else {
                self.send_message(&chat_id, Self::enter_pairing_code_message(language))
                    .await
                    .ok();
            }
        } else if let Some(chat_id) = Self::extract_message_chat_id(&event) {
            let language = current_bot_language().await;
            self.send_message(&chat_id, Self::enter_pairing_code_message(language))
                .await
                .ok();
        }
        None
    }

    /// Start polling for pairing codes.  Returns the chat_id on success.
    pub async fn wait_for_pairing(
        &self,
        stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<String> {
        info!("Feishu bot waiting for pairing code via WebSocket...");

        if *stop_rx.borrow() {
            return Err(anyhow!("bot stop requested"));
        }

        let (ws_url, config) = self.get_ws_endpoint().await?;

        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .map_err(|e| anyhow!("feishu ws connect: {e}"))?;

        let (write, mut read) = ws_stream.split();
        let write = Arc::new(RwLock::new(write));
        info!("Feishu WebSocket connected (binary proto), waiting for pairing...");

        let service_id = Self::extract_service_id_from_url(&ws_url);

        let ping_interval = config
            .get("PingInterval")
            .and_then(|v| v.as_u64())
            .unwrap_or(120);

        let mut ping_timer = tokio::time::interval(std::time::Duration::from_secs(ping_interval));

        loop {
            tokio::select! {
                _ = stop_rx.changed() => {
                    info!("Feishu wait_for_pairing stopped by signal");
                    return Err(anyhow!("bot stop requested"));
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(WsMessage::Binary(data))) => {
                            let frame = match pb::decode_frame(&data) {
                                Some(f) => f,
                                None => continue,
                            };
                            match frame.method {
                                pb::FRAME_TYPE_DATA => {
                                    if let Some(chat_id) = self.handle_data_frame_for_pairing(&frame, &write).await {
                                        return Ok(chat_id);
                                    }
                                }
                                pb::FRAME_TYPE_CONTROL => {
                                    debug!("Feishu WS control frame: type={}", frame.get_header("type").unwrap_or("?"));
                                }
                                _ => {}
                            }
                        }
                        Some(Ok(WsMessage::Ping(data))) => {
                            let _ = write.write().await.send(WsMessage::Pong(data)).await;
                        }
                        Some(Err(e)) => {
                            error!("Feishu WebSocket error during pairing: {e}");
                            return Err(anyhow!("feishu ws error: {e}"));
                        }
                        None => {
                            return Err(anyhow!("feishu ws connection closed during pairing"));
                        }
                        _ => {}
                    }
                }
                _ = ping_timer.tick() => {
                    let ping = pb::Frame::new_ping(service_id);
                    let _ = write.write().await.send(WsMessage::Binary(pb::encode_frame(&ping))).await;
                }
            }
        }
    }

    fn extract_service_id_from_url(url: &str) -> i32 {
        url.split('?')
            .nth(1)
            .and_then(|qs| {
                qs.split('&').find_map(|pair| {
                    let mut kv = pair.splitn(2, '=');
                    match (kv.next(), kv.next()) {
                        (Some("service_id"), Some(v)) => v.parse::<i32>().ok(),
                        _ => None,
                    }
                })
            })
            .unwrap_or(0)
    }

    /// Main message loop that runs after pairing is complete.
    /// Connects to Feishu WebSocket (binary protobuf protocol) and routes
    /// incoming messages through the command router.
    pub async fn run_message_loop(self: Arc<Self>, stop_rx: tokio::sync::watch::Receiver<bool>) {
        info!("Feishu bot message loop started");
        let mut stop = stop_rx;

        loop {
            if *stop.borrow() {
                info!("Feishu bot message loop stopped by signal");
                break;
            }

            let ws_result = self.get_ws_endpoint().await;
            let (ws_url, config) = match ws_result {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get Feishu WS endpoint: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
            };

            let ping_interval = config
                .get("PingInterval")
                .and_then(|v| v.as_u64())
                .unwrap_or(120);

            let service_id = Self::extract_service_id_from_url(&ws_url);

            let ws_conn = tokio_tungstenite::connect_async(&ws_url).await;
            let (ws_stream, _) = match ws_conn {
                Ok(v) => v,
                Err(e) => {
                    error!("Feishu WS connect failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
            };
            info!("Feishu WebSocket connected for message loop (binary proto)");

            let (write, mut read) = ws_stream.split();
            let write = Arc::new(RwLock::new(write));

            let mut ping_timer =
                tokio::time::interval(std::time::Duration::from_secs(ping_interval));

            loop {
                tokio::select! {
                    _ = stop.changed() => {
                        info!("Feishu bot message loop stopped by signal");
                        return;
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(WsMessage::Binary(data))) => {
                                let frame = match pb::decode_frame(&data) {
                                    Some(f) => f,
                                    None => continue,
                                };

                                match frame.method {
                                    pb::FRAME_TYPE_DATA => {
                                        let msg_type = frame.get_header("type").unwrap_or("");
                                        if msg_type == "event" {
                                            if let Ok(event) = serde_json::from_slice::<serde_json::Value>(&frame.payload) {
                                                // Send ack
                                                let resp = pb::Frame::new_response(&frame, 200);
                                                let _ = write.write().await.send(WsMessage::Binary(pb::encode_frame(&resp))).await;

                                                if let Some(parsed) = Self::parse_message_event_full(&event) {
                                                    let bot = self.clone();
                                                    tokio::spawn(async move {
                                                        const MAX_IMAGES: usize = 5;
                                                        let language = current_bot_language().await;
                                                        let truncated = parsed.image_keys.len() > MAX_IMAGES;
                                                        let keys_to_use = if truncated {
                                                            &parsed.image_keys[..MAX_IMAGES]
                                                        } else {
                                                            &parsed.image_keys
                                                        };
                                                        let images = if keys_to_use.is_empty() {
                                                            vec![]
                                                        } else {
                                                            bot.download_images(&parsed.message_id, keys_to_use).await
                                                        };
                                                        if truncated {
                                                            let msg = format!(
                                                                "{} {} {}",
                                                                if language.is_chinese() {
                                                                    "仅会处理前"
                                                                } else {
                                                                    "Only the first"
                                                                },
                                                                MAX_IMAGES,
                                                                if language.is_chinese() {
                                                                    format!(
                                                                        "张图片，其余 {} 张已丢弃。",
                                                                        parsed.image_keys.len() - MAX_IMAGES
                                                                    )
                                                                } else {
                                                                    format!(
                                                                        "images will be processed; the remaining {} were discarded.",
                                                                        parsed.image_keys.len() - MAX_IMAGES
                                                                    )
                                                                },
                                                            );
                                                            bot.send_message(&parsed.chat_id, &msg).await.ok();
                                                        }
                                                        let text = if parsed.text.is_empty() && !images.is_empty() {
                                                            if language.is_chinese() {
                                                                "[用户发送了一张图片]".to_string()
                                                            } else {
                                                                "[User sent an image]".to_string()
                                                            }
                                                        } else {
                                                            parsed.text
                                                        };
                                                        bot.handle_incoming_message(
                                                            &parsed.chat_id,
                                                            &text,
                                                            images,
                                                        )
                                                        .await;
                                                    });
                                                } else if let Some((chat_id, cmd)) = Self::parse_card_action_event(&event) {
                                                    let bot = self.clone();
                                                    tokio::spawn(async move {
                                                        bot.handle_incoming_message(
                                                            &chat_id,
                                                            &cmd,
                                                            vec![],
                                                        )
                                                        .await;
                                                    });
                                                } else if let Some(chat_id) = Self::extract_message_chat_id(&event) {
                                                    let bot = self.clone();
                                                    tokio::spawn(async move {
                                                        let language = current_bot_language().await;
                                                        bot.send_message(
                                                            &chat_id,
                                                            Self::unsupported_message_type_message(language),
                                                        ).await.ok();
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    pb::FRAME_TYPE_CONTROL => {
                                        debug!("Feishu WS control: type={}", frame.get_header("type").unwrap_or("?"));
                                    }
                                    _ => {}
                                }
                            }
                            Some(Ok(WsMessage::Ping(data))) => {
                                let _ = write.write().await.send(WsMessage::Pong(data)).await;
                            }
                            Some(Err(e)) => {
                                error!("Feishu WS error: {e}");
                                break;
                            }
                            None => {
                                warn!("Feishu WS closed, reconnecting...");
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = ping_timer.tick() => {
                        let ping = pb::Frame::new_ping(service_id);
                        let _ = write.write().await.send(WsMessage::Binary(pb::encode_frame(&ping))).await;
                    }
                }
            }

            let reconnect_interval = config
                .get("ReconnectInterval")
                .and_then(|v| v.as_u64())
                .unwrap_or(3);
            tokio::time::sleep(std::time::Duration::from_secs(reconnect_interval)).await;
        }
    }

    async fn handle_incoming_message(
        self: &Arc<Self>,
        chat_id: &str,
        text: &str,
        images: Vec<super::super::remote_server::ImageAttachment>,
    ) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(chat_id.to_string()).or_insert_with(|| {
            let mut s = BotChatState::new(chat_id.to_string());
            s.paired = true;
            s
        });
        let language = current_bot_language().await;

        if !state.paired {
            let trimmed = text.trim();
            if trimmed == "/start" {
                self.send_message(chat_id, welcome_message(language))
                    .await
                    .ok();
                return;
            }
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                if self.verify_pairing_code(trimmed).await {
                    let result = complete_im_bot_pairing(state).await;
                    self.send_handle_result(chat_id, &result).await.ok();
                    self.persist_chat_state(chat_id, state).await;
                    return;
                } else {
                    self.send_message(chat_id, Self::invalid_pairing_code_message(language))
                        .await
                        .ok();
                    return;
                }
            }
            self.send_message(chat_id, Self::enter_pairing_code_message(language))
                .await
                .ok();
            return;
        }

        // Intercept file download callbacks before normal command routing.
        if text.starts_with("download_file:") {
            let token = text["download_file:".len()..].trim().to_string();
            drop(states);
            self.handle_download_request(chat_id, &token).await;
            return;
        }

        let cmd = parse_command(text);
        let result = handle_command(state, cmd, images).await;

        self.persist_chat_state(chat_id, state).await;
        drop(states);

        self.send_handle_result(chat_id, &result).await.ok();

        if let Some(forward) = result.forward_to_session {
            let bot = self.clone();
            let cid = chat_id.to_string();
            tokio::spawn(async move {
                let interaction_bot = bot.clone();
                let interaction_chat_id = cid.clone();
                let handler: BotInteractionHandler =
                    std::sync::Arc::new(move |interaction: BotInteractiveRequest| {
                        let interaction_bot = interaction_bot.clone();
                        let interaction_chat_id = interaction_chat_id.clone();
                        Box::pin(async move {
                            interaction_bot
                                .deliver_interaction(&interaction_chat_id, interaction)
                                .await;
                        })
                    });
                let msg_bot = bot.clone();
                let msg_cid = cid.clone();
                let sender: BotMessageSender = std::sync::Arc::new(move |text: String| {
                    let msg_bot = msg_bot.clone();
                    let msg_cid = msg_cid.clone();
                    Box::pin(async move {
                        if let Err(err) = msg_bot.send_message(&msg_cid, &text).await {
                            warn!("Failed to send Feishu intermediate message to {msg_cid}: {err}");
                        }
                    })
                });
                let verbose_mode = load_bot_persistence().verbose_mode;
                let result = execute_forwarded_turn(forward, Some(handler), Some(sender), verbose_mode).await;
                if let Err(err) = bot.send_message(&cid, &result.display_text).await {
                    warn!("Failed to send Feishu final message to {cid}: {err}");
                }
                bot.notify_files_ready(&cid, &result.full_text).await;
            });
        }
    }

    async fn deliver_interaction(&self, chat_id: &str, interaction: BotInteractiveRequest) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(chat_id.to_string()).or_insert_with(|| {
            let mut s = BotChatState::new(chat_id.to_string());
            s.paired = true;
            s
        });
        state.pending_action = Some(interaction.pending_action.clone());
        self.persist_chat_state(chat_id, state).await;
        drop(states);

        let result = HandleResult {
            reply: interaction.reply,
            actions: interaction.actions,
            forward_to_session: None,
        };
        self.send_handle_result(chat_id, &result).await.ok();
    }

    async fn persist_chat_state(&self, chat_id: &str, state: &BotChatState) {
        let mut data = load_bot_persistence();
        data.upsert(SavedBotConnection {
            bot_type: "feishu".to_string(),
            chat_id: chat_id.to_string(),
            config: BotConfig::Feishu {
                app_id: self.config.app_id.clone(),
                app_secret: self.config.app_secret.clone(),
            },
            chat_state: state.clone(),
            connected_at: chrono::Utc::now().timestamp(),
        });
        save_bot_persistence(&data);
    }
}

#[cfg(test)]
mod tests {
    use super::FeishuBot;
    use crate::service::remote_connect::bot::command_router::BotLanguage;

    #[test]
    fn parse_text_message_event() {
        let event = serde_json::json!({
            "header": { "event_type": "im.message.receive_v1" },
            "event": {
                "message": {
                    "message_type": "text",
                    "chat_id": "oc_test_chat",
                    "content": "{\"text\":\"/help\"}"
                }
            }
        });

        let parsed = FeishuBot::parse_ws_event(&event);
        assert_eq!(
            parsed,
            Some(("oc_test_chat".to_string(), "/help".to_string()))
        );
    }

    #[test]
    fn parse_card_action_event_uses_embedded_chat_id() {
        let event = serde_json::json!({
            "header": { "event_type": "card.action.trigger" },
            "event": {
                "context": {
                    "open_chat_id": "oc_fallback"
                },
                "action": {
                    "value": {
                        "chat_id": "oc_actual",
                        "command": "/switch_workspace"
                    }
                }
            }
        });

        let parsed = FeishuBot::parse_ws_event(&event);
        assert_eq!(
            parsed,
            Some(("oc_actual".to_string(), "/switch_workspace".to_string()))
        );
    }

    #[test]
    fn card_body_removes_slash_command_list() {
        let body = FeishuBot::card_body_text(
            BotLanguage::EnUS,
            "Available commands:\n/switch_workspace - List and switch workspaces\n/help - Show this help message",
        );

        assert_eq!(body, "Available commands:\n\nChoose an action below.");
    }
}
