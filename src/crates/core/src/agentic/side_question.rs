//! Side question (ephemeral) service.
//!
//! This is the core implementation behind the desktop `/btw` feature:
//! - Uses existing session context (no new dialog turn, no persistence writes)
//! - Does not execute tools
//! - Supports streaming output and cancellation by request id

use crate::agentic::coordination::ConversationCoordinator;
use crate::agentic::core::{Message as CoreMessage, MessageContent, MessageRole};
use crate::infrastructure::ai::AIClientFactory;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::message::Message as AIMessage;

use futures::StreamExt;
use log::{debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct SideQuestionRuntime {
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl SideQuestionRuntime {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, request_id: String) -> CancellationToken {
        let token = CancellationToken::new();

        let old = {
            let mut guard = self.tokens.lock().await;
            guard.insert(request_id, token.clone())
        };
        if let Some(old) = old {
            old.cancel();
        }

        token
    }

    pub async fn cancel(&self, request_id: &str) {
        let token = {
            let guard = self.tokens.lock().await;
            guard.get(request_id).cloned()
        };
        if let Some(token) = token {
            token.cancel();
        }
    }

    pub async fn remove(&self, request_id: &str) {
        let mut guard = self.tokens.lock().await;
        guard.remove(request_id);
    }
}

#[derive(Clone)]
pub struct SideQuestionService {
    coordinator: Arc<ConversationCoordinator>,
    ai_client_factory: Arc<AIClientFactory>,
    runtime: Arc<SideQuestionRuntime>,
}

impl SideQuestionService {
    pub fn new(
        coordinator: Arc<ConversationCoordinator>,
        ai_client_factory: Arc<AIClientFactory>,
        runtime: Arc<SideQuestionRuntime>,
    ) -> Self {
        Self {
            coordinator,
            ai_client_factory,
            runtime,
        }
    }

    pub fn runtime(&self) -> &Arc<SideQuestionRuntime> {
        &self.runtime
    }

    fn core_message_to_transcript_line(msg: &CoreMessage) -> Option<String> {
        let role = match msg.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
            MessageRole::System => "System",
        };

        let content = match &msg.content {
            MessageContent::Text(text) => text.trim().to_string(),
            MessageContent::Multimodal { text, images } => {
                let mut out = text.trim().to_string();
                if !images.is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!("[{} image(s) omitted]", images.len()));
                }
                out
            }
            MessageContent::ToolResult {
                tool_name,
                result_for_assistant,
                result,
                is_error,
                ..
            } => {
                let mut out = String::new();
                out.push_str(&format!(
                    "Tool result: name={}, is_error={}",
                    tool_name, is_error
                ));
                if let Some(text) = result_for_assistant
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    out.push('\n');
                    out.push_str(text);
                } else if !result.is_null() {
                    if let Ok(json) = serde_json::to_string_pretty(result) {
                        out.push('\n');
                        out.push_str(&json);
                    }
                }
                out
            }
            MessageContent::Mixed { text, .. } => text.trim().to_string(),
        };

        let content = content.trim();
        if content.is_empty() {
            return None;
        }
        Some(format!("{}:\n{}", role, content))
    }

    fn build_user_prompt(context: &[CoreMessage], question: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        for msg in context {
            if let Some(line) = Self::core_message_to_transcript_line(msg) {
                lines.push(line);
            }
        }

        format!(
            "CONTEXT (recent messages):\n\n{}\n\n---\n\nSIDE QUESTION:\n{}\n",
            lines.join("\n\n"),
            question.trim()
        )
    }

    async fn load_context_messages(
        &self,
        session_id: &str,
        max_context_messages: usize,
    ) -> BitFunResult<Vec<CoreMessage>> {
        let session_manager = self.coordinator.get_session_manager();
        let mut context_messages = session_manager.get_context_messages(session_id).await?;

        if context_messages.len() > max_context_messages {
            context_messages =
                context_messages.split_off(context_messages.len().saturating_sub(max_context_messages));
        }

        Ok(context_messages)
    }

    fn system_prompt() -> &'static str {
        "You are answering a side question about the ongoing chat.\n\
Rules:\n\
- Use only the information present in the provided CONTEXT.\n\
- Do not call tools, do not browse, do not assume access to files or runtime.\n\
- If the context is insufficient, say what is missing.\n\
- Reply concisely, matching the question's language.\n"
    }

    pub async fn ask(
        &self,
        session_id: &str,
        question: &str,
        model_id: Option<&str>,
        max_context_messages: Option<usize>,
    ) -> BitFunResult<String> {
        if session_id.trim().is_empty() {
            return Err(BitFunError::Validation("session_id is required".to_string()));
        }
        if question.trim().is_empty() {
            return Err(BitFunError::Validation("question is required".to_string()));
        }

        let max_context_messages = max_context_messages.unwrap_or(60).clamp(10, 200);
        let model_id = model_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("fast");

        let context_messages = self
            .load_context_messages(session_id, max_context_messages)
            .await?;

        let user_prompt = Self::build_user_prompt(&context_messages, question);

        let client = self
            .ai_client_factory
            .get_client_resolved(model_id)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create AI client: {}", e)))?;

        let messages = vec![
            AIMessage::system(Self::system_prompt().to_string()),
            AIMessage::user(user_prompt),
        ];

        let response = client
            .send_message(messages, None)
            .await
            .map_err(|e| BitFunError::service(format!("AI call failed: {}", e)))?;

        Ok(response.text.trim().to_string())
    }

    pub async fn cancel(&self, request_id: &str) {
        self.runtime.cancel(request_id).await
    }

    pub async fn start_stream(
        &self,
        request: SideQuestionStreamRequest,
    ) -> BitFunResult<mpsc::UnboundedReceiver<SideQuestionStreamEvent>> {
        if request.request_id.trim().is_empty() {
            return Err(BitFunError::Validation("request_id is required".to_string()));
        }
        if request.session_id.trim().is_empty() {
            return Err(BitFunError::Validation("session_id is required".to_string()));
        }
        if request.question.trim().is_empty() {
            return Err(BitFunError::Validation("question is required".to_string()));
        }

        let max_context_messages = request.max_context_messages.unwrap_or(60).clamp(10, 200);
        let model_id = request
            .model_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("fast")
            .to_string();

        let context_messages = self
            .load_context_messages(&request.session_id, max_context_messages)
            .await?;
        let user_prompt = Self::build_user_prompt(&context_messages, &request.question);

        let client = self
            .ai_client_factory
            .get_client_resolved(&model_id)
            .await
            .map_err(|e| BitFunError::service(format!("Failed to create AI client: {}", e)))?;

        let messages = vec![
            AIMessage::system(Self::system_prompt().to_string()),
            AIMessage::user(user_prompt),
        ];

        let cancel_token = self.runtime.register(request.request_id.clone()).await;

        let (tx, rx) = mpsc::unbounded_channel();
        let request_id = request.request_id.clone();
        let session_id = request.session_id.clone();
        let runtime = self.runtime.clone();

        tokio::spawn(async move {
            let mut full_text = String::new();
            let mut last_finish_reason: Option<String> = None;

            let mut stream = match client.send_message_stream(messages, None).await {
                Ok(resp) => resp.stream,
                Err(e) => {
                    let _ = tx.send(SideQuestionStreamEvent::Error {
                        request_id,
                        session_id,
                        error: format!("AI call failed: {}", e),
                    });
                    return;
                }
            };

            while let Some(chunk_result) = stream.next().await {
                if cancel_token.is_cancelled() {
                    debug!("Side question cancelled: request_id={}", request_id);
                    break;
                }

                match chunk_result {
                    Ok(chunk) => {
                        if let Some(reason) = chunk.finish_reason.clone() {
                            last_finish_reason = Some(reason);
                        }
                        if let Some(text) = chunk.text {
                            if !text.is_empty() {
                                full_text.push_str(&text);
                                let _ = tx.send(SideQuestionStreamEvent::TextChunk {
                                    request_id: request_id.clone(),
                                    session_id: session_id.clone(),
                                    text,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(SideQuestionStreamEvent::Error {
                            request_id,
                            session_id,
                            error: format!("Stream error: {}", e),
                        });
                        return;
                    }
                }
            }

            // Cleanup token record.
            runtime.remove(&request_id).await;

            if cancel_token.is_cancelled() {
                // No completion event on cancellation; caller may have already updated UI state.
                return;
            }

            if full_text.trim().is_empty() {
                warn!("Side question stream completed with empty output: request_id={}", request_id);
            }

            let _ = tx.send(SideQuestionStreamEvent::Completed {
                request_id,
                session_id,
                full_text: full_text.trim().to_string(),
                finish_reason: last_finish_reason,
            });
        });

        Ok(rx)
    }
}

#[derive(Debug, Clone)]
pub struct SideQuestionStreamRequest {
    pub request_id: String,
    pub session_id: String,
    pub question: String,
    pub model_id: Option<String>,
    pub max_context_messages: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum SideQuestionStreamEvent {
    TextChunk {
        request_id: String,
        session_id: String,
        text: String,
    },
    Completed {
        request_id: String,
        session_id: String,
        full_text: String,
        finish_reason: Option<String>,
    },
    Error {
        request_id: String,
        session_id: String,
        error: String,
    },
}
