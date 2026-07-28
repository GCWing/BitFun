# taiji-llm — LLM Client Abstraction Layer

Unified `LlmClient` trait with provider implementations (OpenAI, Claude, DeepSeek) plus a `MockClient` for testing. Also provides embedding support and structured `DecisionOutput` types.

## Usage

```rust
use taiji_llm::client::{LlmClient, LlmConfig, ChatMessage, Role};
use taiji_llm::provider::openai::OpenAiProvider;

let config = LlmConfig::default();
let provider = OpenAiProvider::new(config)?;
let messages = vec![ChatMessage {
    role: Role::User,
    content: "What is the market sentiment today?".into(),
}];
let response = provider.chat(&messages).await?;
```

```bash
cargo add taiji-llm
```

## Modules

| Module | Description |
|--------|-------------|
| `client` | `LlmClient` trait, `ChatMessage`, `ChatResponse`, `MockClient` |
| `embedding` | `EmbeddingService` with Candle and mock backends |
| `provider` | Provider implementations: openai, claude, deepseek, local |
| `types` | `DecisionOutput` — structured decision with direction, confidence, reasoning |
