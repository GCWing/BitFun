use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};

use crate::types::{ChatChunk, DecisionOutput};

// ── 核心类型 ──────────────────────────────────────────────────────────

/// 消息角色。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

/// 一条对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// LLM 调用配置。
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// 模型名称，如 "gpt-4o", "claude-sonnet-4-20250514", "deepseek-chat"
    pub model: String,
    /// 采样温度 [0.0, 2.0]
    pub temperature: f32,
    /// 最大输出 token 数
    pub max_tokens: usize,
    /// API key（可用环境变量替代）
    pub api_key: Option<String>,
    /// 自定义 base URL（代理 / 兼容 API）
    pub base_url: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            temperature: 0.7,
            max_tokens: 4096,
            api_key: None,
            base_url: None,
        }
    }
}

/// Token 用量统计。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// LLM 完成响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// 模型生成的完整文本
    pub content: String,
    /// Token 用量
    pub usage: Usage,
    /// 完成原因："stop" | "length" | "tool_calls" | ...
    pub finish_reason: String,
}

/// 流式响应的类型别名。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, anyhow::Error>> + Send>>;

// ── LlmClient trait ────────────────────────────────────────────────────

/// LLM 客户端抽象。
///
/// 所有 provider（OpenAI / Claude / DeepSeek）实现此 trait，
/// 上层 Agent 通过此 trait 调用，不依赖具体 provider。
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 发送非流式对话请求，返回完整响应。
    async fn chat(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatResponse, anyhow::Error>;

    /// 发送流式对话请求，返回 SSE 增量流。
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        config: &LlmConfig,
    ) -> Result<ChatStream, anyhow::Error>;
}

// ── 辅助函数 ───────────────────────────────────────────────────────────

/// 从 ChatResponse 解析 DecisionOutput。
///
/// 期望响应内容是合法的 DecisionOutput JSON；
/// 如果内容以 ```json 包裹，会自动剥离代码块标记。
pub fn parse_decision_output(response: &ChatResponse) -> Result<DecisionOutput, anyhow::Error> {
    let content = response.content.trim();

    // 剥离可能的 ```json ... ``` 包裹
    let json_str = if let Some(inner) = content.strip_prefix("```json") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else if let Some(inner) = content.strip_prefix("```") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        content
    };

    let decision: DecisionOutput = serde_json::from_str(json_str)?;
    Ok(decision)
}

// ── Mock client（测试用）────────────────────────────────────────────────

/// 测试用的 Mock LLM 客户端，返回预设 JSON。
pub struct MockClient {
    pub preset_response: String,
}

impl MockClient {
    pub fn new(preset_response: impl Into<String>) -> Self {
        Self {
            preset_response: preset_response.into(),
        }
    }

    /// 创建一个返回预设 DecisionOutput 的 MockClient。
    pub fn with_decision(direction: &str, confidence: f64, reasoning: &str) -> Self {
        let decision = DecisionOutput {
            direction: direction.into(),
            confidence,
            reasoning: reasoning.into(),
            key_signals: vec!["mock_signal".into()],
            risks: vec!["mock_risk".into()],
        };
        Self {
            preset_response: serde_json::to_string(&decision).unwrap(),
        }
    }
}

#[async_trait]
impl LlmClient for MockClient {
    async fn chat(
        &self,
        _messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatResponse, anyhow::Error> {
        Ok(ChatResponse {
            content: self.preset_response.clone(),
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            finish_reason: "stop".into(),
        })
    }

    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatStream, anyhow::Error> {
        let content = self.preset_response.clone();
        let stream = futures::stream::once(async move {
            Ok(ChatChunk {
                delta: content,
                done: true,
                finish_reason: Some("stop".into()),
            })
        });
        Ok(Box::pin(stream))
    }
}

// ── 测试 ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_response_serde_roundtrip() {
        let original = ChatResponse {
            content: "Hello".into(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: "stop".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ChatResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "Hello");
        assert_eq!(parsed.usage.total_tokens, 15);
        assert_eq!(parsed.finish_reason, "stop");
    }

    #[tokio::test]
    async fn test_mock_client_chat() {
        let client = MockClient::with_decision("long", 0.85, "test reasoning");
        let messages = vec![ChatMessage::user("测试")];
        let config = LlmConfig::default();

        let response = client.chat(&messages, &config).await.unwrap();
        assert!(response.content.contains("long"));
        assert_eq!(response.finish_reason, "stop");

        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "long");
        assert_eq!(decision.confidence, 0.85);
        assert_eq!(decision.reasoning, "test reasoning");
    }

    #[tokio::test]
    async fn test_mock_client_chat_stream() {
        let client = MockClient::with_decision("short", 0.72, "stream test");

        let mut stream = client
            .chat_stream(&[ChatMessage::user("测试")], &LlmConfig::default())
            .await
            .unwrap();

        use futures::StreamExt;
        let chunk = stream.next().await.unwrap().unwrap();
        assert!(chunk.done);
        assert!(chunk.delta.contains("short"));
    }

    #[test]
    fn test_parse_decision_output_strips_code_block() {
        let response = ChatResponse {
            content:
                "```json\n{\"direction\":\"hold\",\"confidence\":0.5,\"reasoning\":\"wait\"}\n```"
                    .into(),
            usage: Usage::default(),
            finish_reason: "stop".into(),
        };
        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "hold");
    }

    #[test]
    fn test_parse_decision_output_plain_json() {
        let response = ChatResponse {
            content: r#"{"direction":"long","confidence":0.9,"reasoning":"strong signal"}"#.into(),
            usage: Usage::default(),
            finish_reason: "stop".into(),
        };
        let decision = parse_decision_output(&response).unwrap();
        assert_eq!(decision.direction, "long");
        assert_eq!(decision.confidence, 0.9);
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_tokens, 4096);
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
    }
}
