//! Telegram bot integration for Remote Connect.
//!
//! Users create their own bot via @BotFather, obtain a token, and enter it
//! in BitFun settings.  The desktop polls for updates via the Telegram Bot
//! API (long polling) and routes messages through the shared command router.

use anyhow::{anyhow, Result};
use bitfun_services_integrations::remote_connect::bot::telegram::TelegramBotApi;
pub use bitfun_services_integrations::remote_connect::bot::telegram::{
    TelegramConfig, MAX_TELEGRAM_FILE_BYTES,
};
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::command_router::{
    complete_im_bot_pairing, current_bot_language, execute_forwarded_turn, handle_command,
    parse_command, welcome_message, BotAction, BotChatState, BotInteractionHandler,
    BotInteractiveRequest, BotLanguage, BotMessageSender, HandleResult,
};
use super::{load_bot_persistence, save_bot_persistence, BotConfig, SavedBotConnection};
use crate::service::remote_connect::remote_server::ImageAttachment;

pub struct TelegramBot {
    api: TelegramBotApi,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    last_update_id: Arc<RwLock<i64>>,
    chat_states: Arc<RwLock<HashMap<i64, BotChatState>>>,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

impl TelegramBot {
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

    pub fn new(config: TelegramConfig) -> Self {
        Self {
            api: TelegramBotApi::new(config),
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            last_update_id: Arc::new(RwLock::new(0)),
            chat_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Restore a previously paired chat so the bot skips the pairing step.
    pub async fn restore_chat_state(&self, chat_id: i64, state: BotChatState) {
        self.chat_states.write().await.insert(chat_id, state);
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        self.api.send_message(chat_id, text).await
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
        self.api
            .send_message_with_keyboard(chat_id, text, actions)
            .await
    }

    /// Send a local file to a Telegram chat as a document attachment.
    /// Caller is expected to pre-check the size against `MAX_TELEGRAM_FILE_BYTES`.
    async fn send_file_as_document(&self, chat_id: i64, file_path: &str) -> Result<()> {
        self.api.send_file_as_document(chat_id, file_path).await
    }

    /// Scan `text` for downloadable file references and push every matching
    /// file directly to the Telegram chat as an attachment.  Files exceeding
    /// `MAX_TELEGRAM_FILE_BYTES` are skipped with a brief notice; per-file
    /// upload failures are reported as plain-text replies.
    async fn notify_files_ready(&self, chat_id: i64, text: &str) {
        let language = current_bot_language().await;
        let workspace_root = {
            let states = self.chat_states.read().await;
            states.get(&chat_id).and_then(|s| s.active_workspace_path())
        };
        let files = super::collect_auto_push_files(
            text,
            workspace_root.as_deref().map(std::path::Path::new),
        );
        if files.is_empty() {
            return;
        }

        // Skip the "正在为你发送 N 个文件……" intro: the document message
        // itself is visible in the chat; only error / size-skip notices
        // below need to surface to the user.
        for file in files {
            if file.size > MAX_TELEGRAM_FILE_BYTES {
                let notice = super::auto_push_skip_too_large_message(
                    language,
                    &file.name,
                    file.size,
                    MAX_TELEGRAM_FILE_BYTES,
                );
                let _ = self.send_message(chat_id, &notice).await;
                continue;
            }
            match self.send_file_as_document(chat_id, &file.abs_path).await {
                Ok(()) => info!(
                    "Telegram auto-pushed file to chat {chat_id}: {}",
                    file.abs_path
                ),
                Err(e) => {
                    warn!(
                        "Telegram auto-push failed for {} in chat {chat_id}: {e}",
                        file.name
                    );
                    let notice =
                        super::auto_push_failed_message(language, &file.name, &e.to_string());
                    let _ = self.send_message(chat_id, &notice).await;
                }
            }
        }
    }

    /// Send a `HandleResult`, using an inline keyboard when actions are present.
    ///
    /// For the "Processing your message…" reply the cancel command line in the
    /// text is replaced with a friendlier prompt, and a Cancel Task button is
    /// added via the inline keyboard.
    async fn send_handle_result(&self, chat_id: i64, result: &HandleResult) {
        let text = if result.menu.items.is_empty() && result.menu.title.is_empty() {
            result.reply.clone()
        } else {
            result.menu.render_text_block()
        };
        // Empty replies (e.g. the silent "forward only" result returned by
        // `handle_chat`) must not be sent — Telegram rejects empty bodies
        // and a lone whitespace message is just noise to the user.
        if text.trim().is_empty() {
            return;
        }
        if result.actions.is_empty() {
            if let Err(e) = self.send_message(chat_id, &text).await {
                warn!("Failed to send Telegram message to {chat_id}: {e}");
            }
        } else if let Err(e) = self
            .send_message_with_keyboard(chat_id, &text, &result.actions)
            .await
        {
            warn!("Failed to send Telegram keyboard message: {e}; falling back to plain text");
            if let Err(e2) = self.send_message(chat_id, &result.reply).await {
                warn!("Telegram fallback plain send to {chat_id} also failed: {e2}");
            }
        }
    }

    /// Register the bot command menu visible in Telegram's "/" menu.
    pub async fn set_bot_commands(&self) -> Result<()> {
        self.api.set_bot_commands().await
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

    /// Returns `(chat_id, text, images)` tuples for each incoming message.
    pub async fn poll_updates(&self) -> Result<Vec<(i64, String, Vec<ImageAttachment>)>> {
        let last_update_id = *self.last_update_id.read().await;
        let (new_last_update_id, messages) = self.api.poll_updates(last_update_id).await?;
        if new_last_update_id > last_update_id {
            *self.last_update_id.write().await = new_last_update_id;
        }
        Ok(messages
            .into_iter()
            .map(|message| (message.chat_id, message.text, message.images))
            .collect())
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
                let result =
                    execute_forwarded_turn(forward, Some(handler), Some(sender), verbose_mode)
                        .await;
                if !result.display_text.is_empty() {
                    bot.send_message(chat_id, &result.display_text).await.ok();
                }
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
        super::command_router::apply_interactive_request(state, &interaction);
        self.persist_chat_state(chat_id, state).await;
        drop(states);

        let result = HandleResult {
            reply: interaction.reply,
            actions: interaction.actions,
            forward_to_session: None,
            menu: interaction.menu,
        };
        self.send_handle_result(chat_id, &result).await;
    }

    async fn persist_chat_state(&self, chat_id: i64, state: &BotChatState) {
        let mut data = load_bot_persistence();
        data.upsert(SavedBotConnection {
            bot_type: "telegram".to_string(),
            chat_id: chat_id.to_string(),
            config: BotConfig::Telegram {
                bot_token: self.api.config().bot_token.clone(),
            },
            chat_state: state.clone(),
            connected_at: chrono::Utc::now().timestamp(),
        });
        save_bot_persistence(&data);
    }
}
