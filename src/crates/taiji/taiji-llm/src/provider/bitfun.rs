use std::sync::Arc;

use async_trait::async_trait;

use crate::client::{ChatMessage, ChatResponse, ChatStream, LlmClient, LlmConfig, Role, Usage};

/// BitFun AIClient 适配器 —— 通过 AIClientFactory 获取 BitFun 统一 AI 客户端，
/// 然后将其包装为 [`LlmClient`] trait 实现。
///
/// 与自建 provider（openai/claude/deepseek）不同，此适配器不自行管理 HTTP 客户端、
/// API key 或 base URL —— 这些全部由 BitFun 的 ConfigService + AIClientFactory 统一管理。
pub struct BitFunAiAdapter {
    client: Arc<bitfun_ai_adapters::AIClient>,
}

impl BitFunAiAdapter {
    /// 通过全局 AIClientFactory 解析 `model_id` 并创建适配器。
    ///
    /// `model_id` 支持：
    /// - 具体模型配置 ID（如 `"model-123"`）
    /// - 选择器（`"primary"` / `"fast"` / `"auto"`）
    ///
    /// # Errors
    ///
    /// - 全局 AIClientFactory 未初始化
    /// - 模型 ID 不存在或未启用
    pub async fn from_factory(model_id: &str) -> Result<Self, anyhow::Error> {
        let factory = bitfun_core::infrastructure::ai::AIClientFactory::get_global()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let client = factory.get_client_resolved(model_id).await?;
        Ok(Self { client })
    }
}

#[async_trait]
impl LlmClient for BitFunAiAdapter {
    async fn chat(
        &self,
        messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatResponse, anyhow::Error> {
        let msgs: Vec<bitfun_ai_adapters::types::Message> = messages
            .iter()
            .map(|m| match m.role {
                Role::System => bitfun_ai_adapters::types::Message::system(m.content.clone()),
                Role::User => bitfun_ai_adapters::types::Message::user(m.content.clone()),
                Role::Assistant => {
                    bitfun_ai_adapters::types::Message::assistant(m.content.clone())
                }
            })
            .collect();

        let response = self
            .client
            .send_message(msgs, None)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(ChatResponse {
            content: response.text,
            usage: Usage {
                prompt_tokens: response
                    .usage
                    .as_ref()
                    .map(|u| u.prompt_token_count as usize)
                    .unwrap_or(0),
                completion_tokens: response
                    .usage
                    .as_ref()
                    .map(|u| u.candidates_token_count as usize)
                    .unwrap_or(0),
                total_tokens: response
                    .usage
                    .as_ref()
                    .map(|u| u.total_token_count as usize)
                    .unwrap_or(0),
            },
            finish_reason: response.finish_reason.unwrap_or_default(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatStream, anyhow::Error> {
        // TODO: 实现流式适配——将 AIClient.send_message_stream 的
        // StreamResponse 转换为 ChatStream（Pin<Box<dyn Stream<Item = Result<ChatChunk>>>）。
        // 当前上层 Agent 使用非流式 chat() 即可完成决策。
        todo!("BitFunAiAdapter streaming not yet implemented")
    }
}
