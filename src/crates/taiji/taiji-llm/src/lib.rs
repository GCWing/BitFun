//! taiji-llm — LLM 客户端抽象层。
//!
//! 提供统一的 [`LlmClient`] trait，以及 BitFun AIClientFactory 适配器实现。
//! 上层 Agent 通过 trait 调用，不依赖具体 provider。
//!
//! # 架构
//!
//! ```text
//! Agent (decision_agent / analysis agents)
//!   └── LlmClient trait (client.rs)
//!         ├── BitFunAiAdapter  (provider/bitfun.rs)  ← 通过 AIClientFactory
//!         ├── LocalProvider    (provider/local.rs)   ← candle 本地推理
//!         └── MockClient       (client.rs, 测试用)
//!   └── ChatMessage / ChatResponse / LlmConfig (client.rs)
//!   └── DecisionOutput (types.rs)
//! ```
//!
//! # 使用示例
//!
//! ```ignore
//! use taiji_llm::client::{ChatMessage, LlmClient};
//! use taiji_llm::provider::bitfun::BitFunAiAdapter;
//!
//! async fn example() -> anyhow::Result<()> {
//!     let client = BitFunAiAdapter::from_factory("primary").await?;
//!     let messages = vec![
//!         ChatMessage::system("你是一个交易分析助手"),
//!         ChatMessage::user("分析 rb9999 的趋势方向"),
//!     ];
//!     let config = LlmConfig::default();
//!     let response = client.chat(&messages, &config).await?;
//!     println!("{}", response.content);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod embedding;
pub mod provider;
pub mod types;

// 重新导出常用类型
pub use client::{ChatMessage, ChatResponse, LlmClient, LlmConfig, MockClient, Role, Usage};
pub use embedding::EmbeddingService;
pub use types::{ChatChunk, DecisionOutput};
