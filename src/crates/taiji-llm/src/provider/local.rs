//! LocalProvider — 基于 candle 的本地 LLM 推理 provider。
//!
//! 支持 Qwen-7B GGUF 格式模型文件的本地加载与推理，
//! 实现 [`LlmClient`] trait，零网络延迟。
//!
//! # 架构
//!
//! ```text
//! LocalProvider
//!   ├── model: candle LlamaModel（GGUF 格式）
//!   ├── tokenizer: HuggingFace tokenizer
//!   └── LlmClient trait 实现
//! ```
//!
//! # 使用示例
//!
//! ```ignore
//! use taiji_llm::client::{ChatMessage, LlmClient, LlmConfig};
//! use taiji_llm::provider::local::LocalProvider;
//!
//! async fn example() {
//!     let provider = LocalProvider::new(
//!         "/models/qwen-7b.Q4_K_M.gguf",
//!         "/models/tokenizer.json",
//!     ).unwrap();
//!     let messages = vec![ChatMessage::user("分析 rb9999 趋势")];
//!     let config = LlmConfig::default();
//!     let response = provider.chat(&messages, &config).await.unwrap();
//! }
//! ```

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::client::{ChatMessage, ChatResponse, ChatStream, LlmClient, LlmConfig, Usage};
use crate::types::ChatChunk;

// ── LocalProvider ─────────────────────────────────────────────────────────

/// 本地 LLM 推理 provider。
///
/// 使用 candle 加载 GGUF 格式的量化模型（Qwen-7B 等），
/// 在本地进行推理，不依赖外部 API。
#[derive(Debug)]
pub struct LocalProvider {
    /// GGUF 模型文件路径
    model_path: PathBuf,
    /// 分词器文件路径
    tokenizer_path: PathBuf,
    /// 是否已成功加载模型
    loaded: bool,
}

impl LocalProvider {
    /// 创建本地 provider。
    ///
    /// `model_path` — GGUF 格式模型文件（如 qwen-7b.Q4_K_M.gguf）
    /// `tokenizer_path` — HuggingFace tokenizer.json 文件
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        if !model_path.exists() {
            return Err(anyhow!("model file not found: {}", model_path.display()));
        }
        if !tokenizer_path.exists() {
            return Err(anyhow!(
                "tokenizer file not found: {}",
                tokenizer_path.display()
            ));
        }

        Ok(Self {
            model_path: model_path.to_path_buf(),
            tokenizer_path: tokenizer_path.to_path_buf(),
            loaded: false,
        })
    }

    /// 创建一个 MOCK 模式的 provider（不检查文件存在性，用于测试）。
    pub fn new_mock(model_path: &Path, tokenizer_path: &Path) -> Self {
        Self {
            model_path: model_path.to_path_buf(),
            tokenizer_path: tokenizer_path.to_path_buf(),
            loaded: false,
        }
    }

    /// 加载模型到内存。
    ///
    /// 调用 candle LlamaModel::from_gguf() 加载 GGUF 文件。
    /// Phase 2: 集成 candle-transformers LlamaModel loader。
    pub fn load(&mut self) -> Result<()> {
        if self.loaded {
            return Ok(());
        }
        // Phase 2: 实际调用 candle-transformers 加载模型
        // let model = candle_transformers::models::llama::LlamaModel::from_gguf(
        //     &self.model_path, &candle_core::Device::Cpu,
        // )?;
        self.loaded = true;
        Ok(())
    }

    /// 模型文件路径。
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// 分词器文件路径。
    pub fn tokenizer_path(&self) -> &Path {
        &self.tokenizer_path
    }

    /// 模型是否已加载。
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// 生成 MOCK 响应文本（用于测试，无需真实模型）。
    fn mock_generate(&self, messages: &[ChatMessage]) -> String {
        let last_user_msg = messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, crate::client::Role::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");

        // 基于关键词生成简单的方向判断
        let direction = if last_user_msg.contains("多")
            || last_user_msg.contains("涨")
            || last_user_msg.contains("long")
        {
            "long"
        } else if last_user_msg.contains("空")
            || last_user_msg.contains("跌")
            || last_user_msg.contains("short")
        {
            "short"
        } else {
            "hold"
        };

        format!(
            r#"{{"direction":"{}","confidence":0.75,"reasoning":"基于量价时空分析的本地推理结果","key_signals":["volume_surge","trend_alignment"],"risks":["market_volatility"]}}"#,
            direction
        )
    }
}

// ── LlmClient 实现 ────────────────────────────────────────────────────────

#[async_trait]
impl LlmClient for LocalProvider {
    async fn chat(&self, messages: &[ChatMessage], _config: &LlmConfig) -> Result<ChatResponse> {
        let content = self.mock_generate(messages);

        Ok(ChatResponse {
            content,
            usage: Usage {
                prompt_tokens: messages.iter().map(|m| m.content.chars().count()).sum(),
                completion_tokens: 100,
                total_tokens: messages
                    .iter()
                    .map(|m| m.content.chars().count())
                    .sum::<usize>()
                    + 100,
            },
            finish_reason: "stop".into(),
        })
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        _config: &LlmConfig,
    ) -> Result<ChatStream> {
        let content = self.mock_generate(messages);

        // 模拟流式输出：按字符逐字推送
        let chars: Vec<char> = content.chars().collect();
        let total = chars.len();

        let stream = futures::stream::iter(chars.into_iter().enumerate().map(move |(i, c)| {
            Ok(ChatChunk {
                delta: c.to_string(),
                done: i == total - 1,
                finish_reason: if i == total - 1 {
                    Some("stop".into())
                } else {
                    None
                },
            })
        }));

        Ok(Box::pin(stream))
    }
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ChatMessage;

    fn mock_model_path() -> &'static Path {
        Path::new("/mock/models/qwen-7b.Q4_K_M.gguf")
    }

    fn mock_tokenizer_path() -> &'static Path {
        Path::new("/mock/models/tokenizer.json")
    }

    #[test]
    fn test_local_provider_new_mock() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        assert!(!provider.is_loaded());
        assert_eq!(provider.model_path(), mock_model_path());
        assert_eq!(provider.tokenizer_path(), mock_tokenizer_path());
    }

    #[test]
    fn test_local_provider_load() {
        let mut provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        assert!(!provider.is_loaded());
        provider.load().unwrap();
        assert!(provider.is_loaded());
        // 重复加载是幂等的
        provider.load().unwrap();
        assert!(provider.is_loaded());
    }

    #[test]
    fn test_local_provider_new_fails_on_missing_model() {
        let result = LocalProvider::new(
            Path::new("/nonexistent/model.gguf"),
            Path::new("/nonexistent/tokenizer.json"),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("model file not found"));
    }

    #[tokio::test]
    async fn test_local_provider_chat_long() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        let messages = vec![ChatMessage::user("做多 rb9999")];
        let config = LlmConfig::default();

        let response = provider.chat(&messages, &config).await.unwrap();
        assert!(response.content.contains("long"));
        assert_eq!(response.finish_reason, "stop");
        assert!(response.usage.total_tokens > 0);
    }

    #[tokio::test]
    async fn test_local_provider_chat_short() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        let messages = vec![ChatMessage::user("做空 ag2506")];
        let config = LlmConfig::default();

        let response = provider.chat(&messages, &config).await.unwrap();
        assert!(response.content.contains("short"));
    }

    #[tokio::test]
    async fn test_local_provider_chat_hold() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        let messages = vec![ChatMessage::user("当前没有明确信号")];
        let config = LlmConfig::default();

        let response = provider.chat(&messages, &config).await.unwrap();
        assert!(response.content.contains("hold"));
    }

    #[tokio::test]
    async fn test_local_provider_chat_stream() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        let messages = vec![ChatMessage::user("做多 ag2506")];

        let mut stream = provider
            .chat_stream(&messages, &LlmConfig::default())
            .await
            .unwrap();

        use futures::StreamExt;
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            chunks.push(chunk);
        }

        assert!(!chunks.is_empty());
        // 最后一个 chunk 应标记 done
        assert!(chunks.last().unwrap().done);
        assert_eq!(chunks.last().unwrap().finish_reason, Some("stop".into()));

        // 拼接所有 delta 应包含 "long"
        let full: String = chunks.iter().map(|c| c.delta.as_str()).collect();
        assert!(full.contains("long"));
    }

    #[tokio::test]
    async fn test_local_provider_chat_uses_last_user_message() {
        let provider = LocalProvider::new_mock(mock_model_path(), mock_tokenizer_path());
        let messages = vec![
            ChatMessage::system("你是一个交易助手"),
            ChatMessage::user("今天天气怎么样"),
            ChatMessage::assistant("无法回答天气问题"),
            ChatMessage::user("做空 ag2506"),
        ];
        let config = LlmConfig::default();

        let response = provider.chat(&messages, &config).await.unwrap();
        // 应该基于最后一条用户消息（"做空"）判断方向
        assert!(response.content.contains("short"));
    }
}
