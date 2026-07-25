# taiji-llm

**路径**: src/crates/taiji/taiji-llm
**描述**: LLM client abstraction — OpenAI / Claude / DeepSeek providers with structured output parsing

## 依赖
- 内部: bitfun-ai-adapters（../../adapters/ai-adapters）, bitfun-core（../../assembly/core）
- 外部: serde, serde_json, tokio, async-trait, futures, anyhow, candle-core（可选 candle 特性）

## 模块结构
- `client` — LlmClient trait + ChatMessage/ChatResponse/LlmConfig/MockClient
- `provider` — BitFunAiAdapter（通过 AIClientFactory）+ LocalProvider（candle 本地推理）
- `embedding` — EmbeddingService 嵌入服务
- `types` — ChatChunk / DecisionOutput 类型定义

## 核心类型
- `LlmClient` — 统一 LLM 调用 trait
- `ChatMessage` — 聊天消息（system/user/assistant）
- `ChatResponse` — 聊天响应
- `LlmConfig` — 调用配置
- `EmbeddingService` — 文本嵌入服务
- `DecisionOutput` — 结构化决策输出

## 核心函数
- `LlmClient::chat()` — 发送聊天请求
- `BitFunAiAdapter::from_factory()` — 从工厂创建适配器

## 属于领域
- LLM / AI
