//! Telegram Bot API provider client for Remote Connect.

use anyhow::{anyhow, Result};
use log::debug;
use serde::{Deserialize, Serialize};

use super::BotAction;
use crate::remote_connect::ImageAttachment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
}

#[derive(Debug, Clone)]
pub struct TelegramIncomingMessage {
    pub chat_id: i64,
    pub text: String,
    pub images: Vec<ImageAttachment>,
}

pub struct TelegramBotApi {
    config: TelegramConfig,
}

/// Telegram Bot API hard limit for `sendDocument` uploads (50 MB), aligned
/// across all IM platforms by capping at 30 MB to match Feishu / WeChat.
pub const MAX_TELEGRAM_FILE_BYTES: u64 = 30 * 1024 * 1024;

/// Telegram caps `sendMessage.text` at 4096 UTF-16 code units. We chunk on
/// char boundaries and stay slightly under the limit to leave headroom for
/// any client-side counting differences.
const MAX_TELEGRAM_TEXT_CHUNK: usize = 4000;

fn chunk_text_for_telegram(text: &str) -> Vec<String> {
    if text.len() <= MAX_TELEGRAM_TEXT_CHUNK {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.len() <= MAX_TELEGRAM_TEXT_CHUNK {
            out.push(rest.to_string());
            break;
        }
        let mut cut = MAX_TELEGRAM_TEXT_CHUNK;
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

impl TelegramBotApi {
    pub fn new(config: TelegramConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TelegramConfig {
        &self.config
    }

    fn api_url(&self, method: &str) -> String {
        format!(
            "https://api.telegram.org/bot{}/{}",
            self.config.bot_token, method
        )
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        for chunk in chunk_text_for_telegram(text) {
            let resp = client
                .post(self.api_url("sendMessage"))
                .json(&serde_json::json!({
                    "chat_id": chat_id,
                    "text": chunk,
                }))
                .send()
                .await?;

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow!("telegram sendMessage failed: {body}"));
            }
        }
        debug!("Telegram message sent to chat {chat_id}");
        Ok(())
    }

    /// Send a message with Telegram inline keyboard buttons.
    pub async fn send_message_with_keyboard(
        &self,
        chat_id: i64,
        text: &str,
        actions: &[BotAction],
    ) -> Result<()> {
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
            .post(self.api_url("sendMessage"))
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
    pub async fn send_file_as_document(&self, chat_id: i64, file_path: &str) -> Result<()> {
        let content = super::read_workspace_file(file_path, MAX_TELEGRAM_FILE_BYTES, None).await?;

        let part = reqwest::multipart::Part::bytes(content.bytes)
            .file_name(content.name.clone())
            .mime_str("application/octet-stream")?;

        let form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("document", part);

        let client = reqwest::Client::new();
        let resp = client
            .post(self.api_url("sendDocument"))
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

    /// Acknowledge a callback query so Telegram removes the button loading state.
    pub async fn answer_callback_query(&self, callback_query_id: &str) {
        let client = reqwest::Client::new();
        let _ = client
            .post(self.api_url("answerCallbackQuery"))
            .json(&serde_json::json!({ "callback_query_id": callback_query_id }))
            .send()
            .await;
    }

    /// Register the bot command menu visible in Telegram's "/" menu.
    pub async fn set_bot_commands(&self) -> Result<()> {
        let client = reqwest::Client::new();
        let commands = serde_json::json!({
            "commands": [
                { "command": "menu", "description": "Show the main menu" },
                { "command": "new", "description": "Create a new session" },
                { "command": "resume", "description": "Resume an existing session" },
                { "command": "switch", "description": "Switch assistant or workspace" },
                { "command": "model", "description": "Switch the session model" },
                { "command": "cancel", "description": "Cancel the current task" },
                { "command": "expert", "description": "Switch to Expert mode" },
                { "command": "assistant", "description": "Switch to Assistant mode" },
                { "command": "settings", "description": "Open settings" },
                { "command": "help", "description": "Show help" },
            ]
        });
        let resp = client
            .post(self.api_url("setMyCommands"))
            .json(&commands)
            .send()
            .await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            log::warn!("Failed to set Telegram bot commands: {body}");
        }
        Ok(())
    }

    /// Download a Telegram photo by file_id and return it as an ImageAttachment.
    async fn download_photo(&self, file_id: &str) -> Result<ImageAttachment> {
        let client = reqwest::Client::new();

        let resp = client
            .post(self.api_url("getFile"))
            .json(&serde_json::json!({ "file_id": file_id }))
            .send()
            .await?;
        let body: serde_json::Value = resp.json().await?;
        let file_path = body
            .pointer("/result/file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Telegram getFile: missing file_path for file_id={file_id}"))?
            .to_string();

        let download_url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.config.bot_token, file_path
        );
        let bytes = client.get(&download_url).send().await?.bytes().await?;

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

    /// Returns incoming messages from Telegram Bot API long polling.
    pub async fn poll_updates(
        &self,
        last_update_id: i64,
    ) -> Result<(i64, Vec<TelegramIncomingMessage>)> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(35))
            .build()?;

        let resp = client
            .get(self.api_url("getUpdates"))
            .query(&[
                ("offset", (last_update_id + 1).to_string()),
                ("timeout", "30".to_string()),
            ])
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        let results = body["result"].as_array().cloned().unwrap_or_default();

        let mut new_last_update_id = last_update_id;
        let mut messages = Vec::new();
        for update in results {
            if let Some(update_id) = update["update_id"].as_i64() {
                if update_id > new_last_update_id {
                    new_last_update_id = update_id;
                }
            }

            if let Some(cq) = update.get("callback_query") {
                let cq_id = cq["id"].as_str().unwrap_or("").to_string();
                let chat_id = cq.pointer("/message/chat/id").and_then(|v| v.as_i64());
                let data = cq["data"].as_str().map(|s| s.trim().to_string());
                if let (Some(chat_id), Some(data)) = (chat_id, data) {
                    self.answer_callback_query(&cq_id).await;
                    messages.push(TelegramIncomingMessage {
                        chat_id,
                        text: data,
                        images: vec![],
                    });
                }
                continue;
            }

            let Some(chat_id) = update.pointer("/message/chat/id").and_then(|v| v.as_i64()) else {
                continue;
            };

            if let Some(text) = update.pointer("/message/text").and_then(|v| v.as_str()) {
                messages.push(TelegramIncomingMessage {
                    chat_id,
                    text: text.trim().to_string(),
                    images: vec![],
                });
                continue;
            }

            if let Some(photo_array) = update.pointer("/message/photo").and_then(|v| v.as_array()) {
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
                            log::warn!("Failed to download Telegram photo file_id={fid}: {e}");
                            vec![]
                        }
                    }
                } else {
                    vec![]
                };

                messages.push(TelegramIncomingMessage {
                    chat_id,
                    text: caption,
                    images,
                });
            }
        }

        Ok((new_last_update_id, messages))
    }
}

#[cfg(test)]
mod tests {
    use super::chunk_text_for_telegram;

    #[test]
    fn chunks_text_on_char_boundary() {
        let text = "a".repeat(4001);
        let chunks = chunk_text_for_telegram(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1], "a");
    }
}
