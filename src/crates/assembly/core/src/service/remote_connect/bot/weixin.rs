//! Weixin iLink bot orchestration for Remote Connect.
//!
//! Provider HTTP/CDN/QR/message parsing lives in `bitfun-services-integrations`.
//! This module keeps product pairing, command routing, persistence, and agent
//! turn orchestration.

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bitfun_services_integrations::remote_connect::bot::weixin as weixin_provider;
use bitfun_services_integrations::remote_connect::bot::weixin::WeixinProviderClient;
pub use bitfun_services_integrations::remote_connect::bot::weixin::{
    WeixinConfig, WeixinQrPollResponse, WeixinQrPollStatus, WeixinQrStartResponse,
    MAX_INBOUND_IMAGES, MAX_WEIXIN_FILE_BYTES, WEIXIN_SESSION_EXPIRED_ERRCODE,
};
use log::{error, info, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::command_router::{
    complete_im_bot_pairing, current_bot_language, execute_forwarded_turn, handle_command,
    parse_command, welcome_message, BotChatState, BotInteractionHandler, BotInteractiveRequest,
    BotMessageSender, HandleResult,
};
use super::{load_bot_persistence, save_bot_persistence, BotConfig, SavedBotConnection};
use crate::service::remote_connect::remote_server::ImageAttachment;

const LONG_POLL_TIMEOUT_SECS: u64 = 36;

#[derive(Debug, Clone)]
struct PendingPairing {
    created_at: i64,
}

pub struct WeixinBot {
    api: Arc<WeixinProviderClient>,
    pending_pairings: Arc<RwLock<HashMap<String, PendingPairing>>>,
    chat_states: Arc<RwLock<HashMap<String, BotChatState>>>,
    context_tokens: Arc<RwLock<HashMap<String, String>>>,
}

pub async fn weixin_qr_start(base_url_override: Option<String>) -> Result<WeixinQrStartResponse> {
    weixin_provider::weixin_qr_start(base_url_override).await
}

pub async fn weixin_qr_poll(
    session_key: &str,
    base_url_override: Option<String>,
) -> Result<WeixinQrPollResponse> {
    weixin_provider::weixin_qr_poll(session_key, base_url_override).await
}

impl WeixinBot {
    pub fn new(config: WeixinConfig) -> Self {
        Self {
            api: Arc::new(WeixinProviderClient::new(config)),
            pending_pairings: Arc::new(RwLock::new(HashMap::new())),
            chat_states: Arc::new(RwLock::new(HashMap::new())),
            context_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn restore_chat_state(&self, peer_id: &str, state: BotChatState) {
        self.chat_states
            .write()
            .await
            .insert(peer_id.to_string(), state);
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
        if let Some(pairing) = pairings.remove(code) {
            let age = chrono::Utc::now().timestamp() - pairing.created_at;
            return age < 300;
        }
        false
    }

    pub async fn send_text(&self, peer_id: &str, text: &str) -> Result<()> {
        let token = self.context_token_for_peer(peer_id).await?;
        if let Err(err) = self.api.send_text_chunks(peer_id, &token, text).await {
            if WeixinProviderClient::is_context_token_error(&err) {
                let mut tokens = self.context_tokens.write().await;
                if tokens
                    .get(peer_id)
                    .map(|cached| cached == &token)
                    .unwrap_or(false)
                {
                    tokens.remove(peer_id);
                    warn!(
                        "weixin: dropped stale context_token for peer {peer_id} after send error: {err}"
                    );
                }
            }
            return Err(err);
        }
        Ok(())
    }

    async fn context_token_for_peer(&self, peer_id: &str) -> Result<String> {
        self.context_tokens
            .read()
            .await
            .get(peer_id)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "context_token unavailable for peer {peer_id} (waiting for next inbound message)"
                )
            })
    }

    async fn try_send_text(&self, peer_id: &str, text: &str, ctx: &str) {
        if let Err(err) = self.send_text(peer_id, text).await {
            warn!("weixin: {ctx} send to peer {peer_id} failed: {err}");
        }
    }

    async fn send_handle_result(&self, peer_id: &str, result: &HandleResult) {
        let language = current_bot_language().await;
        let text = if result.menu.items.is_empty() && result.menu.title.is_empty() {
            result.reply.clone()
        } else {
            result.menu.render_plain_text(language)
        };
        if text.trim().is_empty() {
            return;
        }
        if let Err(err) = self.send_text(peer_id, &text).await {
            warn!("weixin send_handle_result: {err}");
        }
    }

    async fn inbound_image_attachments_from_message(
        &self,
        msg: &Value,
    ) -> (Vec<ImageAttachment>, usize) {
        const MAX_BYTES: usize = 1024 * 1024;

        let (raw_images, skipped) = self.api.download_inbound_images(msg).await;
        let mut attachments = Vec::with_capacity(raw_images.len());
        for raw in raw_images {
            let data_url = if raw.bytes.len() <= MAX_BYTES {
                let b64 = B64.encode(&raw.bytes);
                format!("data:{};base64,{b64}", raw.mime_type)
            } else {
                match crate::agentic::image_analysis::optimize_image_with_size_limit(
                    raw.bytes.clone(),
                    "openai",
                    Some(raw.mime_type),
                    Some(MAX_BYTES),
                ) {
                    Ok(processed) => {
                        let b64 = B64.encode(&processed.data);
                        format!("data:{};base64,{}", processed.mime_type, b64)
                    }
                    Err(err) => {
                        warn!("Weixin image compression failed: {err}");
                        let b64 = B64.encode(&raw.bytes);
                        format!("data:{};base64,{b64}", raw.mime_type)
                    }
                }
            };
            attachments.push(ImageAttachment {
                name: raw.name,
                data_url,
            });
        }
        (attachments, skipped)
    }

    async fn notify_files_ready(&self, peer_id: &str, text: &str) {
        let language = current_bot_language().await;
        let workspace_root = {
            let states = self.chat_states.read().await;
            states
                .get(peer_id)
                .and_then(|state| state.active_workspace_path())
        };
        let files = super::collect_auto_push_files(
            text,
            workspace_root.as_deref().map(std::path::Path::new),
        );
        if files.is_empty() {
            return;
        }

        let root_path = workspace_root.as_deref().map(std::path::Path::new);
        for file in files {
            if file.size > MAX_WEIXIN_FILE_BYTES {
                let notice = super::auto_push_skip_too_large_message(
                    language,
                    &file.name,
                    file.size,
                    MAX_WEIXIN_FILE_BYTES,
                );
                if let Err(err) = self.send_text(peer_id, &notice).await {
                    warn!("Weixin auto-push skip notice failed for peer {peer_id}: {err}");
                }
                continue;
            }

            let send_result = match self.context_token_for_peer(peer_id).await {
                Ok(token) => {
                    self.api
                        .send_workspace_file_to_peer(peer_id, &token, &file.abs_path, root_path)
                        .await
                }
                Err(err) => Err(err),
            };

            match send_result {
                Ok(()) => info!(
                    "Weixin auto-pushed file to peer {peer_id}: {}",
                    file.abs_path
                ),
                Err(err) => {
                    warn!(
                        "Weixin auto-push failed for {} to peer {peer_id}: {err}",
                        file.name
                    );
                    let notice =
                        super::auto_push_failed_message(language, &file.name, &err.to_string());
                    if let Err(send_err) = self.send_text(peer_id, &notice).await {
                        warn!(
                            "Weixin auto-push failure notice failed for peer {peer_id}: {send_err}"
                        );
                    }
                }
            }
        }
    }

    async fn persist_chat_state(&self, peer_id: &str, state: &BotChatState) {
        let config = self.api.config();
        let mut data = load_bot_persistence();
        data.upsert(SavedBotConnection {
            bot_type: "weixin".to_string(),
            chat_id: peer_id.to_string(),
            config: BotConfig::Weixin {
                ilink_token: config.ilink_token.clone(),
                base_url: config.base_url.clone(),
                bot_account_id: config.bot_account_id.clone(),
            },
            chat_state: state.clone(),
            connected_at: chrono::Utc::now().timestamp(),
        });
        save_bot_persistence(&data);
    }

    pub async fn wait_for_pairing(
        &self,
        stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<String> {
        info!("Weixin bot waiting for pairing code (getupdates)...");
        let mut buf = weixin_provider::load_sync_buf(&self.api.config().bot_account_id);

        loop {
            if *stop_rx.borrow() {
                return Err(anyhow!("bot stop requested"));
            }

            let poll = tokio::select! {
                _ = stop_rx.changed() => {
                    return Err(anyhow!("bot stop requested"));
                }
                result = self.api.get_updates_once(
                    &buf,
                    Duration::from_secs(LONG_POLL_TIMEOUT_SECS),
                ) => result,
            };

            let resp = match poll {
                Ok(value) => value,
                Err(err) => {
                    error!("weixin getupdates: {err}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            let ret = resp["ret"].as_i64().unwrap_or(0);
            let errcode = resp["errcode"].as_i64().unwrap_or(0);
            if (ret != 0 && ret != WEIXIN_SESSION_EXPIRED_ERRCODE)
                || (errcode != 0 && errcode != WEIXIN_SESSION_EXPIRED_ERRCODE)
            {
                if errcode == WEIXIN_SESSION_EXPIRED_ERRCODE
                    || ret == WEIXIN_SESSION_EXPIRED_ERRCODE
                {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                warn!("weixin getupdates ret={ret} errcode={errcode}");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            if let Some(new_buf) = resp["get_updates_buf"].as_str() {
                buf = new_buf.to_string();
                weixin_provider::save_sync_buf(&self.api.config().bot_account_id, &buf);
            }

            if let Some(msgs) = resp["msgs"].as_array() {
                for msg in msgs {
                    if !weixin_provider::is_user_message(msg) {
                        continue;
                    }
                    let Some(peer) = weixin_provider::peer_id(msg) else {
                        continue;
                    };
                    if let Some(token) = weixin_provider::context_token(msg) {
                        self.context_tokens
                            .write()
                            .await
                            .insert(peer.clone(), token);
                    }
                    let text = weixin_provider::body_from_message(msg).trim().to_string();
                    let language = current_bot_language().await;

                    if text == "/start" {
                        self.try_send_text(&peer, welcome_message(language), "welcome")
                            .await;
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

                            self.send_handle_result(&peer, &result).await;
                            return Ok(peer);
                        }
                        let err = if language.is_chinese() {
                            "\u{914d}\u{5bf9}\u{7801}\u{65e0}\u{6548}\u{6216}\u{5df2}\u{8fc7}\u{671f}\u{ff0c}\u{8bf7}\u{91cd}\u{8bd5}\u{3002}"
                        } else {
                            "Invalid or expired pairing code."
                        };
                        self.try_send_text(&peer, err, "pairing-invalid").await;
                    } else if !text.is_empty() {
                        let err = if language.is_chinese() {
                            "\u{8bf7}\u{8f93}\u{5165} BitFun \u{684c}\u{9762}\u{7aef}\u{8fdc}\u{7a0b}\u{8fde}\u{63a5}\u{4e2d}\u{663e}\u{793a}\u{7684} 6 \u{4f4d}\u{914d}\u{5bf9}\u{7801}\u{3002}"
                        } else {
                            "Please send the 6-digit pairing code from BitFun Desktop Remote Connect."
                        };
                        self.try_send_text(&peer, err, "pairing-prompt").await;
                    } else if weixin_provider::has_inbound_image_items(msg) {
                        let err = if language.is_chinese() {
                            "\u{914d}\u{5bf9}\u{8bf7}\u{76f4}\u{63a5}\u{53d1}\u{9001} 6 \u{4f4d}\u{6570}\u{5b57}\u{914d}\u{5bf9}\u{7801}\u{ff1b}\u{5b8c}\u{6210}\u{914d}\u{5bf9}\u{540e}\u{518d}\u{53d1}\u{9001}\u{56fe}\u{7247}\u{4e0e}\u{52a9}\u{624b}\u{5bf9}\u{8bdd}\u{3002}"
                        } else {
                            "To pair, send the 6-digit code only. After pairing you can send images to chat."
                        };
                        self.try_send_text(&peer, err, "pairing-image-hint").await;
                    }
                }
            }
        }
    }

    pub async fn run_message_loop(self: Arc<Self>, stop_rx: tokio::sync::watch::Receiver<bool>) {
        info!("Weixin message loop started");
        let mut stop = stop_rx;
        let mut buf = weixin_provider::load_sync_buf(&self.api.config().bot_account_id);

        loop {
            if *stop.borrow() {
                break;
            }

            let poll = tokio::select! {
                _ = stop.changed() => break,
                result = self.api.get_updates_once(
                    &buf,
                    Duration::from_secs(LONG_POLL_TIMEOUT_SECS),
                ) => result,
            };

            let resp = match poll {
                Ok(value) => value,
                Err(err) => {
                    error!("weixin getupdates (loop): {err}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            let ret = resp["ret"].as_i64().unwrap_or(0);
            let errcode = resp["errcode"].as_i64().unwrap_or(0);
            if (ret != 0 && ret != WEIXIN_SESSION_EXPIRED_ERRCODE)
                || (errcode != 0 && errcode != WEIXIN_SESSION_EXPIRED_ERRCODE)
            {
                if errcode == WEIXIN_SESSION_EXPIRED_ERRCODE
                    || ret == WEIXIN_SESSION_EXPIRED_ERRCODE
                {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            if let Some(new_buf) = resp["get_updates_buf"].as_str() {
                buf = new_buf.to_string();
                weixin_provider::save_sync_buf(&self.api.config().bot_account_id, &buf);
            }

            let Some(msgs) = resp["msgs"].as_array() else {
                continue;
            };

            for msg in msgs {
                if !weixin_provider::is_user_message(msg) {
                    continue;
                }
                let Some(peer) = weixin_provider::peer_id(msg) else {
                    continue;
                };
                if let Some(token) = weixin_provider::context_token(msg) {
                    self.context_tokens
                        .write()
                        .await
                        .insert(peer.clone(), token);
                }
                let msg_value = msg.clone();
                let bot = self.clone();
                tokio::spawn(async move {
                    let (images, skipped_images) =
                        bot.inbound_image_attachments_from_message(&msg_value).await;
                    let language = current_bot_language().await;
                    if skipped_images > 0 {
                        let note = if language.is_chinese() {
                            format!(
                                "\u{4ec5}\u{4f1a}\u{5904}\u{7406}\u{524d} {} \u{5f20}\u{56fe}\u{7247}\u{ff0c}\u{5176}\u{4f59} {} \u{5f20}\u{5df2}\u{4e22}\u{5f03}\u{3002}",
                                MAX_INBOUND_IMAGES, skipped_images
                            )
                        } else {
                            format!(
                                "Only the first {} images will be processed; the remaining {} were discarded.",
                                MAX_INBOUND_IMAGES, skipped_images
                            )
                        };
                        bot.try_send_text(&peer, &note, "image-truncation-notice")
                            .await;
                    }
                    let body = weixin_provider::body_from_message(&msg_value);
                    let text = if body.trim().is_empty() && !images.is_empty() {
                        if language.is_chinese() {
                            "[\u{7528}\u{6237}\u{53d1}\u{9001}\u{4e86}\u{4e00}\u{5f20}\u{56fe}\u{7247}]".to_string()
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
            let mut state = BotChatState::new(peer_id.clone());
            state.paired = true;
            state
        });
        let language = current_bot_language().await;

        if !state.paired {
            let trimmed = text.trim();
            if trimmed == "/start" {
                drop(states);
                self.try_send_text(&peer_id, welcome_message(language), "welcome")
                    .await;
                return;
            }
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                if self.verify_pairing_code(trimmed).await {
                    let result = complete_im_bot_pairing(state).await;
                    self.persist_chat_state(&peer_id, state).await;
                    drop(states);
                    self.send_handle_result(&peer_id, &result).await;
                    return;
                }
                let err = if language.is_chinese() {
                    "\u{914d}\u{5bf9}\u{7801}\u{65e0}\u{6548}\u{6216}\u{5df2}\u{8fc7}\u{671f}\u{3002}"
                } else {
                    "Invalid or expired pairing code."
                };
                drop(states);
                self.try_send_text(&peer_id, err, "pairing-invalid").await;
                return;
            }
            drop(states);
            let err = if language.is_chinese() {
                "\u{8bf7}\u{8f93}\u{5165} 6 \u{4f4d}\u{914d}\u{5bf9}\u{7801}\u{3002}"
            } else {
                "Please send the 6-digit pairing code."
            };
            self.try_send_text(&peer_id, err, "pairing-prompt").await;
            return;
        }

        let command = parse_command(text);
        let result = handle_command(state, command, images).await;
        self.persist_chat_state(&peer_id, state).await;
        drop(states);

        self.send_handle_result(&peer_id, &result).await;

        if let Some(forward) = result.forward_to_session {
            let bot = self.clone();
            let peer = peer_id.clone();
            let typing_token = self.context_tokens.read().await.get(&peer_id).cloned();
            let typing_for_turn = self.api.start_typing(peer_id.clone(), typing_token);
            tokio::spawn(async move {
                let interaction_bot = bot.clone();
                let peer_c = peer.clone();
                let handler: BotInteractionHandler =
                    Arc::new(move |interaction: BotInteractiveRequest| {
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
                let sender: BotMessageSender = Arc::new(move |text: String| {
                    let msg_bot = msg_bot.clone();
                    let peer_s = peer_m.clone();
                    Box::pin(async move {
                        if let Err(err) = msg_bot.send_text(&peer_s, &text).await {
                            warn!(
                                "weixin: send intermediate message to peer {peer_s} failed: {err}"
                            );
                        }
                    })
                });
                let verbose_mode = load_bot_persistence().verbose_mode;
                let turn_result =
                    execute_forwarded_turn(forward, Some(handler), Some(sender), verbose_mode)
                        .await;
                if !turn_result.display_text.is_empty() {
                    if let Err(err) = bot.send_text(&peer, &turn_result.display_text).await {
                        warn!("weixin: send final reply to peer {peer} failed: {err}");
                    }
                }
                bot.notify_files_ready(&peer, &turn_result.full_text).await;
                typing_for_turn.stop().await;
            });
        }
    }

    async fn deliver_interaction(&self, peer_id: String, interaction: BotInteractiveRequest) {
        let mut states = self.chat_states.write().await;
        let state = states.entry(peer_id.clone()).or_insert_with(|| {
            let mut state = BotChatState::new(peer_id.clone());
            state.paired = true;
            state
        });
        super::command_router::apply_interactive_request(state, &interaction);
        self.persist_chat_state(&peer_id, state).await;
        drop(states);

        let result = HandleResult {
            reply: interaction.reply,
            actions: interaction.actions,
            forward_to_session: None,
            menu: interaction.menu,
        };
        self.send_handle_result(&peer_id, &result).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn context_token_error_heuristic_uses_provider_contract() {
        let app_err = anyhow!(
            "ilink ilink/bot/sendmessage application error ret=0 errcode=12345 errmsg=context_token expired"
        );
        assert!(WeixinProviderClient::is_context_token_error(&app_err));

        let net_err = anyhow!("error sending request: connection refused");
        assert!(!WeixinProviderClient::is_context_token_error(&net_err));
    }

    #[test]
    fn body_from_message_plain_text_uses_provider_parser() {
        let msg = json!({
            "item_list": [{ "type": 1, "text_item": { "text": "hi" } }]
        });
        assert_eq!(weixin_provider::body_from_message(&msg), "hi");
    }

    #[test]
    fn body_from_message_quoted_text_uses_provider_parser() {
        let msg = json!({
            "item_list": [{
                "type": 1,
                "text_item": { "text": "reply" },
                "ref_msg": { "title": " earlier ", "message_item": { "type": 1, "text_item": { "text": "orig" } } }
            }]
        });
        let body = weixin_provider::body_from_message(&msg);
        assert!(body.contains("[\u{5f15}\u{7528}:"));
        assert!(body.contains("reply"));
    }
}
