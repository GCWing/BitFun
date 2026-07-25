# bitfun-ai-adapters

**描述**: 共享 AI 协议适配器，用于 BitFun 核心和安装器。

**包名**: `bitfun-ai-adapters` | lib: `bitfun_ai_adapters`

## 核心模块

| 模块 | 说明 |
|------|------|
| `client` | AI 客户端抽象（`AIClient` trait + HTTP 实现） |
| `providers` | 各 AI 提供商协议实现 |
| `stream` | 统一流式响应、token 用量、tool call 解析 |
| `types` | 核心类型（`AIConfig`, `Message`, `ToolCall`, `ProxyConfig`） |
| `model_selector` | 模型选择器分类与解析 |
| `trace` | 模型请求追踪（`ModelExchangeTrace`） |
| `diagnostics` | AI 请求/响应诊断 |
| `tool_call_accumulator` | 累积式工具调用收集 |
| `subscription_auth` (feature) | 订阅认证（平台原生 keyring 存储） |

## 关键类型/功能

- `AIClient` trait — 统一 AI 客户端接口
- `StreamOptions` / `StreamResponse` — 流式请求配置和响应
- `UnifiedResponse` / `UnifiedTokenUsage` / `UnifiedToolCall` — 统一响应格式
- `ModelSelectorError` / `ModelSelectorKind` — 模型选择
- `ModelExchangeRequestTraceHandle` / `ModelExchangeResponseTrace` — 追踪
- `ConnectionTestResult` / `ConnectionTestMessageCode` — 连接测试
- `GeminiResponse` / `GeminiUsage` — Gemini 特定类型
- `Message` / `ToolCall` / `ToolDefinition` — 对话消息格式

## 一句话总结

AI 提供商协议的统一适配层，封装请求/响应、流式处理、token 追踪和模型选择。
