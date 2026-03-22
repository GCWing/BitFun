//! Telegram bot integration for Remote Connect.
//!
//! Users create their own bot via @BotFather, obtain a token, and enter it
//! in BitFun settings.  The desktop polls for updates via the Telegram Bot
//! API (long polling) and routes messages through the shared command router.

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::command_router::{
    complete_im_bot_pairing, current_bot_language, execute_forwarded_turn, handle_command,
    parse_command, welcome_message, BotAction, BotChatState,
    BotInteractionHandler, BotInteractiveRequest, BotLanguage, BotMessageSender, HandleResult,
};
use super::{load_bot_persistence, save_bot_persistence, BotConfig, SavedBotConnection};
use crate::service::remote_connect::remote_server::ImageAttachment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
}

pub struct TelegramBot {
    config: TelegramConfig,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    last_update_id: Arc<RwLock<i64>>,
    chat_states: Arc<RwLock<HashMap<i64, BotChatState>>>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

impl TelegramBot {
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

    fn cancel_button_hint(language: BotLanguage) -> &'static str {
        if language.is_chinese() {
            "如需停止本次请求，请点击下方的“取消任务”按钮。"
        } else {
            "If needed, tap the Cancel Task button below to stop this request."
        }
    }

    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            last_update_id: Arc::new(RwLock::new(0)),
            chat_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Restore a previously paired chat so the bot skips the pairing step.
    pub async fn restore_chat_state(&self, chat_id: i64, state: BotChatState) {
        self.chat_states.write().await.insert(chat_id, state);
    }

    fn api_url(&self, method: &str) -> String {
        format!(
            "https://api.telegram.org/bot{}/{}",
            self.config.bot_token, method
        )
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.api_url("sendMessage"))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("telegram sendMessage failed: {body}"));
        }
        debug!("Telegram message sent to chat {chat_id}");
        Ok(())
    }

    /// Send a message with Telegram inline keyboard buttons.
    ///
    /// Each `BotAction` becomes one button row.  The `callback_data` carries
    /// the full command string so the bot receives it as a synthetic message
    /// when the user taps the button.
    ///
    /// Telegram limits `callback_data` to 64 bytes.  All commands used here
    /// (including `/cancel_task turn_<uuid>`) fit within that limit.
    async fn send_message_with_keyboard(
        &self,
        chat_id: i64,
        text: &str,
        actions: &[BotAction],
    ) -> Result<()> {
        // Build inline keyboard: one button per row for clarity.
        let keyboard: Vec<Vec<serde_json::Value>> = actions
            .iter()
            .map(|action| {
                vec![serde_json::json!({
                    "text": action.label,
                    "callback_data": action.command,
                })]
            })
            .collect();

        let client = reqwest::Client::new();
        let resp = client
            .post(&self.api_url("sendMessage"))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": text,
                "reply_markup": {
                    "inline_keyboard": keyboard,
                },
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("telegram sendMessage (keyboard) failed: {body}"));
        }
        debug!("Telegram keyboard message sent to chat {chat_id}");
        Ok(())
    }

    /// Send a local file to a Telegram chat as a document attachment.
    ///
    /// Skips files larger than 50 MB (Telegram Bot API hard limit).
    async fn send_file_as_document(&self, chat_id: i64, file_path: &str) -> Result<()> {
        const MAX_SIZE: u64 = 30 * 1024 * 1024; // Unified 30 MB cap (Feishu API hard limit)
        let content = super::read_workspace_file(file_path, MAX_SIZE, None).await?;

        let part = reqwest::multipart::Part::bytes(content.bytes)
            .file_name(content.name.clone())
            .mime_str("application/octet-stream")?;

        let form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("document", part);

        let client = reqwest::Client::new();
        let resp = client
            .post(&self.api_url("sendDocument"))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("telegram sendDocument failed: {body}"));
        }
        debug!("Telegram document sent to chat {chat_id}: {}", content.name);
        Ok(())
    }

    /// Scan `text` for downloadable file links (`computer://`, `file://`, and
    /// markdown hyperlinks to local files), store them as pending downloads and
    /// send a notification with one inline-keyboard button per file.
    async fn notify_files_ready(&self, chat_id: i64, text: &str) {
        let result = {
            let mut states = self.chat_states.write().await;
            let state = states.entry(chat_id).or_insert_with(|| {
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
            if let Err(e) = self
                .send_message_with_keyboard(chat_id, &result.reply, &result.actions)
                .await
            {
                warn!("Failed to send file notification to Telegram: {e}");
            }
        }
    }

    /// Handle a `download_file:<token>` callback: look up the pending file and
    /// send it.  Sends a plain-text error if the token has expired or the
    /// transfer fails.
    async fn handle_download_request(&self, chat_id: i64, token: &str) {
        let (path, language) = {
            let mut states = self.chat_states.write().await;
            let state = states.get_mut(&chat_id);
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
                match self.send_file_as_document(chat_id, &path).await {
                    Ok(()) => info!("Sent file to Telegram chat {chat_id}: {path}"),
                    Err(e) => {
                        warn!("Failed to send file to Telegram: {e}");
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

    /// Acknowledge a callback query so Telegram removes the button loading state.
    async fn answer_callback_query(&self, callback_query_id: &str) {
        let client = reqwest::Client::new();
        let _ = client
            .post(&self.api_url("answerCallbackQuery"))
            .json(&serde_json::json!({ "callback_query_id": callback_query_id }))
            .send()
            .await;
    }

    /// Send a `HandleResult`, using an inline keyboard when actions are present.
    ///
    /// For the "Processing your message…" reply the cancel command line in the
    /// text is replaced with a friendlier prompt, and a Cancel Task button is
    /// added via the inline keyboard.
    async fn send_handle_result(&self, chat_id: i64, result: &HandleResult) {
        let language = current_bot_language().await;
        let text = Self::clean_reply_text(language, &result.reply, !result.actions.is_empty());
        if result.actions.is_empty() {
            self.send_message(chat_id, &text).await.ok();
        } else {
            if let Err(e) = self
                .send_message_with_keyboard(chat_id, &text, &result.actions)
                .await
            {
                warn!("Failed to send Telegram keyboard message: {e}; falling back to plain text");
                self.send_message(chat_id, &result.reply).await.ok();
            }
        }
    }

    /// Remove raw `/cancel_task <turn_id>` instruction lines and replace them
    /// with a short hint that the button below can be used instead.
    fn clean_reply_text(language: BotLanguage, text: &str, has_actions: bool) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut replaced_cancel = false;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("/cancel_task ") {
                if has_actions && !replaced_cancel {
                    lines.push(Self::cancel_button_hint(language).to_string());
                    replaced_cancel = true;
                }
                continue;
            }
            lines.push(line.to_string());
        }

        lines.join("\n").trim().to_string()
    }

    /// Register the bot command menu visible in Telegram's "/" menu.
    pub async fn set_bot_commands(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let commands = serde_json::json!({
            "commands": [
                { "command": "switch_workspace", "description": "List and switch workspaces" },
                { "command": "pro", "description": "Switch to Expert mode (Code/Cowork)" },
                { "command": "assistant", "description": "Switch to Assistant mode (Claw)" },
                { "command": "resume_session", "description": "Resume an existing session" },
                { "command": "new_code_session", "description": "Create coding session (Expert)" },
                { "command": "new_cowork_session", "description": "Create cowork session (Expert)" },
                { "command": "new_claw_session", "description": "Create claw session (Assistant)" },
                { "command": "cancel_task", "description": "Cancel the current task" },
                { "command": "help", "description": "Show available commands" },
            ]
        });
        let resp = client
            .post(&self.api_url("setMyCommands"))
            .json(&commands)
            .send()
            .await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            warn!("Failed to set Telegram bot commands: {body}");
        }
        Ok(())
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

    /// Download a Telegram photo by file_id and return it as an `ImageAttachment`.
    ///
    /// Telegram photo updates contain multiple `PhotoSize` entries; callers should
    /// pass the `file_id` of the last (largest) entry.
    async fn download_photo(&self, file_id: &str) -> Result<ImageAttachment> {
        let client = reqwest::Client::new();

        // Step 1: resolve file_path via getFile
        let get_file_url = self.api_url("getFile");
        let resp = client
            .post(&get_file_url)
            .json(&serde_json::json!({ "file_id": file_id }))
            .send()
            .await?;
        let body: serde_json::Value = resp.json().await?;
        let file_path = body
            .pointer("/result/file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Telegram getFile: missing file_path for file_id={file_id}"))?
            .to_string();

        // Step 2: download the actual bytes
        let download_url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.config.bot_token, file_path
        );
        let bytes = client.get(&download_url).send().await?.bytes().await?;

        // Step 3: encode as base64 data-URL
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let mime_type = if file_path.ends_with(".jpg") || file_path.ends_with(".jpeg") {
            "image/jpeg"
        } else if file_path.ends_with(".png") {
            "image/png"
        } else if file_path.ends_with(".gif") {
            "image/gif"
        } else if file_path.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        let data_url = format!("data:{mime_type};base64,{b64}");
        let name = file_path
            .rsplit('/')
            .next()
            .unwrap_or("photo.jpg")
            .to_string();

        debug!(
            "Telegram photo downloaded: file_id={file_id}, size={}B",
            bytes.len()
        );
        Ok(ImageAttachment { name, data_url })
    }

    /// Returns `(chat_id, text, images)` tuples for each incoming message.
    ///
    /// Handles both plain-text messages and photo messages with an optional
    /// caption.  For photo messages the highest-resolution variant is downloaded
    /// and returned as an `ImageAttachment`.
    pub async fn poll_updates(&self) -> Result<Vec<(i64, String, Vec<ImageAttachment>)>> {
        let offset = *self.last_update_id.read().await;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(35))
            .build()?;

        let resp = client
            .get(&self.api_url("getUpdates"))
            .query(&[
                ("offset", (offset + 1).to_string()),
                ("timeout", "30".to_string()),
            ])
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        let results = body["result"].as_array().cloned().unwrap_or_default();

        let mut messages = Vec::new();
        for update in results {
            if let Some(update_id) = update["update_id"].as_i64() {
                let mut last = self.last_update_id.write().await;
                if update_id > *last {
                    *last = update_id;
                }
            }

            // Inline keyboard button press – treat callback_data as a message.
            if let Some(cq) = update.get("callback_query") {
                let cq_id = cq["id"].as_str().unwrap_or("").to_string();
                let chat_id = cq.pointer("/message/chat/id").and_then(|v| v.as_i64());
                let data = cq["data"].as_str().map(|s| s.trim().to_string());
                if let (Some(chat_id), Some(data)) = (chat_id, data) {
                    // Answer the callback query to dismiss the button spinner.
                    self.answer_callback_query(&cq_id).await;
                    messages.push((chat_id, data, vec![]));
                }
                continue;
            }

            let Some(chat_id) = update.pointer("/message/chat/id").and_then(|v| v.as_i64()) else {
                continue;
            };

            // Plain-text message
            if let Some(text) = update.pointer("/message/text").and_then(|v| v.as_str()) {
                messages.push((chat_id, text.trim().to_string(), vec![]));
                continue;
            }

            // Photo message (caption is optional)
            if let Some(photo_array) = update.pointer("/message/photo").and_then(|v| v.as_array()) {
                // The last PhotoSize entry has the highest resolution
                let file_id = photo_array
                    .last()
                    .and_then(|p| p["file_id"].as_str())
                    .map(|s| s.to_string());

                let caption = update
                    .pointer("/message/caption")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                let images = if let Some(fid) = file_id {
                    match self.download_photo(&fid).await {
                        Ok(attachment) => vec![attachment],
                        Err(e) => {
                            warn!("Failed to download Telegram photo file_id={fid}: {e}");
                            vec![]
                        }
                    }
                } else {
                    vec![]
                };

                messages.push((chat_id, caption, images));
            }
        }

        Ok(messages)
    }

    /// Start a polling loop that checks for pairing codes.
    /// Returns the chat_id when a valid pairing code is received.
    pub async fn wait_for_pairing(
        &self,
        stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<i64> {
        info!("Telegram bot waiting for pairing code...");
        loop {
            if *stop_rx.borrow() {
                return Err(anyhow!("bot stop requested"));
            }
            let poll_result = tokio::select! {
                result = self.poll_updates() => result,
                _ = stop_rx.changed() => {
                    info!("Telegram wait_for_pairing stopped by signal");
                    return Err(anyhow!("bot stop requested"));
                }
            };
            match poll_result {
                Ok(messages) => {
                    for (chat_id, text, _images) in messages {
                        let trimmed = text.trim();
                        let language = current_bot_language().await;

                        if trimmed == "/start" {
                            self.send_message(chat_id, welcome_message(language))
                                .await
                                .ok();
                            continue;
                        }

                        if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                            if self.verify_pairing_code(trimmed).await {
                                info!("Telegram pairing successful, chat_id={chat_id}");
                                let mut state = BotChatState::new(chat_id.to_string());
                                let result = complete_im_bot_pairing(&mut state).await;
                                self.chat_states
                                    .write()
                                    .await
                                    .insert(chat_id, state.clone());
                                self.persist_chat_state(chat_id, &state).await;
                                self.send_handle_result(chat_id, &result).await;
                                self.set_bot_commands().await.ok();

                                return Ok(chat_id);
                            } else {
                                self.send_message(
                                    chat_id,
                                    Self::invalid_pairing_code_message(language),
                                )
                                .await
                                .ok();
                            }
                        } else {
                            self.send_message(chat_id, Self::enter_pairing_code_message(language))
                                .await
                                .ok();
                        }
                    }
                }
                Err(e) => {
                    error!("Telegram poll error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Main message loop that runs after pairing is complete.
    /// Continuously polls for messages and routes them through the command router.
    pub async fn run_message_loop(self: Arc<Self>, stop_rx: tokio::sync::watch::Receiver<bool>) {
        info!("Telegram bot message loop started");
        let mut stop = stop_rx;

        loop {
            if *stop.borrow() {
                info!("Telegram bot message loop stopped by signal");
                break;
            }

            let poll_result = tokio::select! {
                result = self.poll_updates() => result,
                _ = stop.changed() => {
                    info!("Telegram bot message loop stopped by signal");
                    break;
                }
            };

            match poll_result {
                Ok(messages) => {
                    for (chat_id, text, images) in messages {
                        let bot = self.clone();
                        tokio::spawn(async move {
                            bot.handle_incoming_message(chat_id, &text, images).await;
                        });
                    }
                }
                Err(e) => {
                    error!("Telegram poll error in message loop: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn handle_incoming_message(
        self: &Arc<Self>,
        chat_id: i64,
        text: &str,
        images: Vec<ImageAttachment>,
    ) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(chat_id).or_insert_with(|| {
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
                    self.persist_chat_state(chat_id, state).await;
                    self.send_handle_result(chat_id, &result).await;
                    self.set_bot_commands().await.ok();
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

        self.send_handle_result(chat_id, &result).await;

        if let Some(forward) = result.forward_to_session {
            let bot = self.clone();
            tokio::spawn(async move {
                let interaction_bot = bot.clone();
                let handler: BotInteractionHandler =
                    std::sync::Arc::new(move |interaction: BotInteractiveRequest| {
                        let interaction_bot = interaction_bot.clone();
                        Box::pin(async move {
                            interaction_bot
                                .deliver_interaction(chat_id, interaction)
                                .await;
                        })
                    });
                let msg_bot = bot.clone();
                let sender: BotMessageSender = std::sync::Arc::new(move |text: String| {
                    let msg_bot = msg_bot.clone();
                    Box::pin(async move {
                        msg_bot.send_message(chat_id, &text).await.ok();
                    })
                });
                let verbose_mode = load_bot_persistence().verbose_mode;
                let result = execute_forwarded_turn(forward, Some(handler), Some(sender), verbose_mode).await;
                bot.send_message(chat_id, &result.display_text).await.ok();
                bot.notify_files_ready(chat_id, &result.full_text).await;
            });
        }
    }

    async fn deliver_interaction(&self, chat_id: i64, interaction: BotInteractiveRequest) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(chat_id).or_insert_with(|| {
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
        self.send_handle_result(chat_id, &result).await;
    }

    async fn persist_chat_state(&self, chat_id: i64, state: &BotChatState) {
        let mut data = load_bot_persistence();
        data.upsert(SavedBotConnection {
            bot_type: "telegram".to_string(),
            chat_id: chat_id.to_string(),
            config: BotConfig::Telegram {
                bot_token: self.config.bot_token.clone(),
            },
            chat_state: state.clone(),
            connected_at: chrono::Utc::now().timestamp(),
        });
        save_bot_persistence(&data);
    }
}
