# BitFun AI Adapter — 完整 API Reference

> **用途**: taiji-llm 替换的迁移目标。覆盖所有公共类型、所有公共方法、所有配置字段。
>
> **生成时间**: 2026-07-22
>
> **扫描范围**: `bitfun_core_types::ai` + `bitfun_ai_adapters` (client + types) + `assembly/core::client_factory`

---

## 目录

1. [基础类型 (bitfun_core_types)](#1-基础类型-bitfun_core_types)
2. [适配器类型 (bitfun_ai_adapters::types)](#2-适配器类型-bitfun_ai_adapterstypes)
3. [AIClient — 公共方法](#3-aiclient--公共方法)
4. [AIClient — 重试常量](#4-aiclient--重试常量)
5. [HTTP 客户端配置](#5-http-客户端配置)
6. [SSE 执行与重试](#6-sse-执行与重试)
7. [Provider 怪癖处理](#7-provider-怪癖处理)
8. [流聚合器](#8-流聚合器)
9. [AIClientFactory API](#9-aiclientfactory-api)
10. [类型重新导出链](#10-类型重新导出链)

---

## 1. 基础类型 (bitfun_core_types)

**源文件**: `src/crates/contracts/core-types/src/ai.rs`

### 1.1 `ReasoningMode` enum

```rust
pub enum ReasoningMode {
    Default,
    Enabled,
    Disabled,
    Adaptive,
}
```

| 变体 | 含义 |
|---|---|
| `Default` | 使用 Provider 默认行为 |
| `Enabled` | 强制开启推理/思考 |
| `Disabled` | 强制关闭推理 |
| `Adaptive` | 根据上下文自适应 |

### 1.2 `ProxyConfig` struct

```rust
pub struct ProxyConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `enabled` | `bool` | 是否启用代理 |
| `url` | `String` | 代理 URL（如 `http://127.0.0.1:7890`） |
| `username` | `String` | 代理认证用户名 |
| `password` | `String` | 代理认证密码 |

### 1.3 `AIConfig` struct

```rust
pub struct AIConfig {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub request_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<ModelReference>,
    pub format: Option<String>,
    pub context_window: Option<u32>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub reasoning_mode: Option<ReasoningMode>,
    pub inline_think_in_text: Option<bool>,
    pub custom_headers: Option<HashMap<String, String>>,
    pub custom_headers_mode: Option<CustomHeadersMode>,
    pub skip_ssl_verify: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub thinking_budget_tokens: Option<u32>,
    pub custom_request_body: Option<serde_json::Value>,
    pub custom_request_body_mode: Option<CustomRequestBodyMode>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | `Option<String>` | 配置名称/别名 |
| `base_url` | `Option<String>` | API 基础 URL |
| `request_url` | `Option<String>` | 显式请求 URL（覆盖 base_url + 路由解析） |
| `api_key` | `Option<String>` | API 密钥 |
| `model` | `Option<ModelReference>` | 模型引用（ID + 上下文窗口覆盖） |
| `format` | `Option<String>` | API 格式：`"openai"`, `"responses"`, `"anthropic"`, `"gemini"`, `"gemini-code-assist"` |
| `context_window` | `Option<u32>` | 上下文窗口大小（token 数） |
| `max_tokens` | `Option<u32>` | 最大输出 token 数 |
| `temperature` | `Option<f32>` | 采样温度 |
| `top_p` | `Option<f32>` | Top-p 采样 |
| `reasoning_mode` | `Option<ReasoningMode>` | 推理模式 |
| `inline_think_in_text` | `Option<bool>` | 是否在文本中内联 think 标签 |
| `custom_headers` | `Option<HashMap<String, String>>` | 自定义 HTTP 头 |
| `custom_headers_mode` | `Option<CustomHeadersMode>` | 自定义头合并模式（Replace / Merge） |
| `skip_ssl_verify` | `Option<bool>` | 跳过 SSL 证书验证 |
| `reasoning_effort` | `Option<String>` | 推理力度（OpenAI o1 系列：low/medium/high） |
| `thinking_budget_tokens` | `Option<u32>` | 思考 token 预算（Anthropic extended thinking） |
| `custom_request_body` | `Option<serde_json::Value>` | 自定义请求体叠加 |
| `custom_request_body_mode` | `Option<CustomRequestBodyMode>` | 自定义请求体合并模式 |

### 1.4 `ToolCall` struct

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub raw_arguments: Option<String>,
}
```

| 方法 | 签名 | 说明 |
|---|---|---|
| `serialized_arguments` | `fn serialized_arguments(&self) -> String` | 返回参数的 JSON 字符串表示 |

### 1.5 `ToolDefinition` struct

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | `String` | 工具名称 |
| `description` | `String` | 工具描述 |
| `parameters` | `serde_json::Value` | JSON Schema 参数定义 |

### 1.6 `Message` struct

```rust
pub struct Message {
    pub role: String,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub thinking_signature: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub is_error: Option<bool>,
    pub tool_image_attachments: Option<Vec<ToolImageAttachment>>,
}
```

| 构造函数 | 签名 |
|---|---|
| `user` | `fn user(content: impl Into<String>) -> Self` |
| `assistant` | `fn assistant(content: impl Into<String>) -> Self` |
| `assistant_with_tools` | `fn assistant_with_tools(tool_calls: Vec<ToolCall>) -> Self` |
| `system` | `fn system(content: impl Into<String>) -> Self` |

| 字段 | 类型 | 说明 |
|---|---|---|
| `role` | `String` | 角色：user / assistant / system / tool |
| `content` | `Option<String>` | 消息文本内容 |
| `reasoning_content` | `Option<String>` | 推理/思考内容（如 o1 内部链） |
| `thinking_signature` | `Option<String>` | Anthropic thinking 签名 |
| `tool_calls` | `Option<Vec<ToolCall>>` | 工具调用列表 |
| `tool_call_id` | `Option<String>` | 工具调用结果对应的 ID |
| `name` | `Option<String>` | 工具名称（tool 角色） |
| `is_error` | `Option<bool>` | 是否是工具调用错误 |
| `tool_image_attachments` | `Option<Vec<ToolImageAttachment>>` | 工具返回的图片附件 |

### 1.7 `ConnectionTestResult` struct

```rust
pub struct ConnectionTestResult {
    pub success: bool,
    pub response_time_ms: u64,
    pub model_response: Option<String>,
    pub message_code: Option<ConnectionTestMessageCode>,
    pub error_details: Option<String>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `success` | `bool` | 连接是否成功 |
| `response_time_ms` | `u64` | 响应时间（毫秒） |
| `model_response` | `Option<String>` | 模型返回的文本 |
| `message_code` | `Option<ConnectionTestMessageCode>` | 失败分类码 |
| `error_details` | `Option<String>` | 错误详情 |

### 1.8 `ConnectionTestMessageCode` enum

```rust
pub enum ConnectionTestMessageCode {
    ToolCallsNotDetected,
    ImageInputCheckFailed,
    TlsOrCertificateIssue,
    ProxyIssue,
    NetworkIssue,
}
```

| 变体 | 含义 |
|---|---|
| `ToolCallsNotDetected` | 工具调用未被检测到 |
| `ImageInputCheckFailed` | 图片输入检查失败 |
| `TlsOrCertificateIssue` | TLS 或证书问题 |
| `ProxyIssue` | 代理问题 |
| `NetworkIssue` | 网络问题 |

### 1.9 `RemoteModelInfo` struct

```rust
pub struct RemoteModelInfo {
    pub id: String,
    pub display_name: Option<String>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | `String` | 模型 ID |
| `display_name` | `Option<String>` | 显示名称 |

---

## 2. 适配器类型 (bitfun_ai_adapters::types)

**源文件**: `src/crates/adapters/ai-adapters/src/types/ai.rs`

### 2.1 `GeminiResponse` struct

```rust
pub struct GeminiResponse {
    pub text: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub usage: Option<GeminiUsage>,
    pub finish_reason: Option<String>,
    pub provider_metadata: Option<serde_json::Value>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `text` | `Option<String>` | 聚合后的文本响应 |
| `reasoning_content` | `Option<String>` | 推理/思考内容 |
| `tool_calls` | `Option<Vec<ToolCall>>` | 工具调用 |
| `usage` | `Option<GeminiUsage>` | Token 使用量 |
| `finish_reason` | `Option<String>` | 结束原因（stop/length/tool_calls 等） |
| `provider_metadata` | `Option<serde_json::Value>` | Provider 特定元数据 |

### 2.2 `GeminiUsage` struct

```rust
pub struct GeminiUsage {
    pub prompt_token_count: Option<u32>,
    pub candidates_token_count: Option<u32>,
    pub total_token_count: Option<u32>,
    pub reasoning_token_count: Option<u32>,
    pub cached_content_token_count: Option<u32>,
    pub cache_creation_token_count: Option<u32>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `prompt_token_count` | `Option<u32>` | Prompt token 数 |
| `candidates_token_count` | `Option<u32>` | 响应 token 数 |
| `total_token_count` | `Option<u32>` | 总 token 数 |
| `reasoning_token_count` | `Option<u32>` | 推理 token 数 |
| `cached_content_token_count` | `Option<u32>` | 缓存命中的 token 数 |
| `cache_creation_token_count` | `Option<u32>` | 缓存创建的 token 数 |

### 2.3 `StreamResponse` struct

**源文件**: `src/crates/adapters/ai-adapters/src/client.rs`

```rust
pub struct StreamResponse {
    pub stream: Pin<Box<dyn Stream<Item = Result<UnifiedResponse>> + Send>>,
    pub raw_sse_rx: Option<UnboundedReceiver<String>>,
    pub trace_handle: Option<ModelExchangeRequestTraceHandle>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `stream` | `Pin<Box<dyn Stream<Item = Result<UnifiedResponse>> + Send>>` | 统一响应流 |
| `raw_sse_rx` | `Option<UnboundedReceiver<String>>` | 原始 SSE 事件接收器（用于调试） |
| `trace_handle` | `Option<ModelExchangeRequestTraceHandle>` | 请求追踪 handle |

### 2.4 `StreamOptions` struct

```rust
pub struct StreamOptions {
    pub idle_timeout: Option<Duration>,
    pub ttft_timeout: Option<Duration>,
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `idle_timeout` | `Option<Duration>` | 流空闲超时（无新 token 时） |
| `ttft_timeout` | `Option<Duration>` | 首 token 超时（Time To First Token） |

### 2.5 `ApiFormat` enum (pub(crate))

```rust
pub(crate) enum ApiFormat {
    OpenAIChat,
    OpenAIResponses,
    Anthropic,
    Gemini,
    GeminiCodeAssist,
}
```

**解析函数**: `parse(value: &str) -> Result<Self>`

| 输入字符串 | 解析结果 |
|---|---|
| `"openai"` | `OpenAIChat` |
| `"response"`, `"responses"` | `OpenAIResponses` |
| `"anthropic"` | `Anthropic` |
| `"gemini"`, `"google"` | `Gemini` |
| `"gemini-code-assist"`, `"gemini_code_assist"`, `"code-assist"` | `GeminiCodeAssist` |

---

## 3. AIClient — 公共方法

**源文件**: `src/crates/adapters/ai-adapters/src/client.rs`

### 3.1 构造函数

#### `new`

```rust
pub fn new(config: AIConfig) -> Self
```

使用默认 `StreamOptions`（无超时）创建 AIClient，无代理。

#### `new_with_proxy`

```rust
pub fn new_with_proxy(config: AIConfig, proxy_config: Option<ProxyConfig>) -> Self
```

带可选代理配置创建 AIClient。`proxy_config` 为 `None` 时不使用代理。

#### `new_with_runtime_options`

```rust
pub fn new_with_runtime_options(
    config: AIConfig,
    proxy_config: Option<ProxyConfig>,
    stream_options: StreamOptions,
) -> Self
```

完整构造函数：同时指定代理配置和流超时选项。

### 3.2 配置查询

#### `stream_idle_timeout`

```rust
pub fn stream_idle_timeout(&self) -> Option<Duration>
```

返回当前配置的流空闲超时。

#### `stream_ttft_timeout`

```rust
pub fn stream_ttft_timeout(&self) -> Option<Duration>
```

返回当前配置的首 token 超时。

### 3.3 派生构造器（克隆 + 覆盖）

#### `with_reasoning_mode`

```rust
pub fn with_reasoning_mode(self, mode: ReasoningMode) -> Self
```

返回一个新 AIClient，克隆当前配置但推理模式不同。用于在不修改原始配置的情况下切换推理行为。

#### `with_max_tokens`

```rust
pub fn with_max_tokens(self, max_tokens: Option<u32>) -> Self
```

返回一个新 AIClient，克隆当前配置但 max_tokens 不同。

### 3.4 流式 API

#### `send_message_stream`

```rust
pub fn send_message_stream(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
    trace: Option<ModelExchangeRequestTraceHandle>,
) -> Result<StreamResponse, String>
```

发送消息并以流式方式接收响应。自动选择正确的 Provider 格式（OpenAI Chat / Anthropic / Gemini）。

**参数**:
- `messages`: 对话历史
- `tools`: 可选的工具定义列表
- `trace`: 可选的追踪 handle（用于请求生命周期追踪）

**返回**: `StreamResponse` 包含 `Stream<Item = Result<UnifiedResponse>>`。

**内部逻辑**: 根据 `ApiFormat` 派发到：
- `OpenAIResponses` / `OpenAIChat` → `crate::providers::openai::send_message_stream`
- `Anthropic` → `crate::providers::anthropic::send_message_stream`
- `Gemini` / `GeminiCodeAssist` → `crate::providers::gemini::send_message_stream`

#### `send_message_stream_with_extra_body`

```rust
pub fn send_message_stream_with_extra_body(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
    extra_body: Option<serde_json::Value>,
    trace: Option<ModelExchangeRequestTraceHandle>,
) -> Result<StreamResponse, String>
```

同 `send_message_stream`，但支持注入额外的请求体字段。

**参数**:
- `extra_body`: 要合并到请求体中的额外 JSON 字段（如 `{"reasoning_effort": "high"}`）

### 3.5 聚合（非流式）API

#### `send_message`

```rust
pub fn send_message(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
) -> Result<GeminiResponse, String>
```

发送消息并返回聚合后的完整响应（内部通过 `send_message_with_extra_body_trace_and_max_attempts` 实现，最多重试 10 次）。

#### `send_message_with_extra_body`

```rust
pub fn send_message_with_extra_body(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
    extra_body: Option<serde_json::Value>,
) -> Result<GeminiResponse, String>
```

带额外请求体字段的聚合发送。

#### `send_message_with_trace`

```rust
pub fn send_message_with_trace(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
    trace: Option<ModelExchangeRequestTraceHandle>,
) -> Result<GeminiResponse, String>
```

带追踪 handle 的聚合发送。

#### `send_message_with_extra_body_and_trace`

```rust
pub fn send_message_with_extra_body_and_trace(
    &self,
    messages: Vec<Message>,
    tools: Option<&[ToolDefinition]>,
    extra_body: Option<serde_json::Value>,
    trace: Option<ModelExchangeRequestTraceHandle>,
) -> Result<GeminiResponse, String>
```

最完整的聚合发送：额外请求体 + 追踪。

### 3.6 连接测试

#### `test_connection`

```rust
pub fn test_connection(&self) -> Result<ConnectionTestResult, String>
```

使用天气工具测试连接。最多重试 5 次（`TEST_CONNECTION_STREAM_ATTEMPTS`）。

#### `test_image_input_connection`

```rust
pub fn test_image_input_connection(&self) -> Result<ConnectionTestResult, String>
```

使用 4 色象限测试图片（base64 PNG）测试图片输入能力。预期响应包含代码 `"BYGR"`。

### 3.7 模型列表

#### `list_models`

```rust
pub fn list_models(&self) -> Result<Vec<RemoteModelInfo>, String>
```

列出远程可用模型。根据 `ApiFormat` 调用不同的 Provider API 端点（OpenAI `/models`、Anthropic `/v1/models` 等）。结果经过去重。

---

## 4. AIClient — 重试常量

**源文件**: `src/crates/adapters/ai-adapters/src/client.rs`

| 常量 | 值 | 说明 |
|---|---|---|
| `SEND_MESSAGE_STREAM_ATTEMPTS` | `10` | 流式发送最大尝试次数 |
| `TEST_CONNECTION_STREAM_ATTEMPTS` | `5` | 连接测试最大尝试次数 |
| `SEND_MESSAGE_RETRY_BASE_DELAY_MS` | `500` | 聚合发送重试基础延迟（ms） |
| `SEND_MESSAGE_RATE_LIMIT_RETRY_BASE_DELAY_MS` | `2000` | 速率限制重试基础延迟（ms） |
| `SEND_MESSAGE_MAX_EXPONENTIAL_DELAY_MS` | `30_000` | 指数退避上限（ms） |
| `SEND_MESSAGE_MAX_RATE_LIMIT_DELAY_MS` | `60_000` | 速率限制延迟上限（ms） |
| `SEND_MESSAGE_MAX_RETRY_EXPONENT_SHIFT` | `6` | 最大指数偏移（即 2^6 × base） |

### 重试延迟计算（聚合层）

```rust
fn send_message_retry_delay_ms(attempt: u32) -> u64
```

标准指数退避：`min(base_delay * 2^min(attempt, 6), 30000)` + 随机抖动。

### 瞬时错误检测（聚合层）

```rust
fn is_transient_stream_error(error_message: &str) -> bool
```

**判定为可重试**（返回 `true`）:
- `"connection closed"`, `"connection reset"`, `"connection refused"`
- `"broken pipe"`, `"unexpected EOF"`
- `"timeout"`, `"timed out"`, `"deadline exceeded"`
- `"rate limit"`, `"too many requests"`
- `"server error"`, `"internal server error"`
- `"service unavailable"`, `"bad gateway"`, `"gateway timeout"`
- `"overloaded"`, `"capacity"`, `"throttle"`
- `"empty response"`, `"empty stream"`, `"no chunk"`, `"zero length"`, `"empty chunk"`

**判定为不可重试**（返回 `false`）:
- `"invalid"`, `"unauthorized"`, `"forbidden"`, `"not found"`
- `"payment required"`, `"quota exceeded"`, `"insufficient quota"`
- `"content filter"`, `"safety"`, `"moderation"`
- `"context length"`, `"max tokens"`, `"token limit"`

---

## 5. HTTP 客户端配置

**源文件**: `src/crates/adapters/ai-adapters/src/client/http.rs`

### 5.1 `create_http_client`

```rust
pub fn create_http_client(
    proxy_config: Option<ProxyConfig>,
    skip_ssl_verify: bool,
) -> Client
```

构建 reqwest `Client`，配置如下：

| 配置项 | 值 |
|---|---|
| TLS 后端 | `rustls` |
| `connect_timeout` | 10 秒 |
| `pool_idle_timeout` | 30 秒 |
| `tcp_keepalive` | 60 秒 |
| `pool_max_idle_per_host` | 4 |
| `user_agent` | `"BitFun/1.0"` |
| `danger_accept_invalid_certs` | 当 `skip_ssl_verify=true` 时 |

**回退策略**:
1. 代理构建失败时 → 回退到无代理并记录错误日志
2. 整个客户端构建失败时 → 回退到 `Client::new()`

### 5.2 `build_proxy`

```rust
fn build_proxy(config: &ProxyConfig) -> Result<Proxy>
```

创建 reqwest `Proxy`。当 `username` 非空时使用 `Proxy::all(url)?.basic_auth(&username, &password)`。

---

## 6. SSE 执行与重试

**源文件**: `src/crates/adapters/ai-adapters/src/client/sse.rs`

### 6.1 `execute_sse_request`

```rust
pub async fn execute_sse_request<BuildRequest, BuildHandler, Fut, Handler>(
    label: &'static str,
    url: String,
    request_body: Value,
    max_tries: u32,
    ttft_timeout: Option<Duration>,
    trace: Option<ModelExchangeRequestTraceHandle>,
    build_request: BuildRequest,
    build_handler: BuildHandler,
) -> Result<StreamResponse>
```

核心 SSE 执行循环，带自动重试。

**泛型参数**:
- `BuildRequest`: `Fn(Url, Value) -> Result<Request>` — 构建 HTTP 请求
- `BuildHandler`: `Fn(UnboundedSender<String>, TraceHandle, CancellationToken) -> Fut` — 构建 SSE 事件处理器
- `Fut`: `Future<Output = Result<()>>` — 处理器 Future
- `Handler`: SSE 事件处理逻辑

**重试条件**:
- HTTP 状态码为 `5xx`, `408`, `409`, `425`, `429`
- 发生连接/传输错误
- 首 token 超时（TTFT timeout）

### 6.2 `ManagedResponseStream`

```rust
pub struct ManagedResponseStream {
    // 包装 UnboundedReceiverStream<String>
    // 持有 CancellationToken，Drop 时取消后台处理任务
}
```

实现 `Stream<Item = String>` trait。

**生命周期管理**: Drop 时自动取消关联的 SSE 处理任务，确保后台任务不会泄漏。

### 6.3 `StreamSendOutcome` enum

```rust
enum StreamSendOutcome {
    Response,
    Transport,
    TtftTimeout,
}
```

| 变体 | 含义 |
|---|---|
| `Response` | 收到 HTTP 响应（可能是错误状态码） |
| `Transport` | 传输层错误（连接失败等） |
| `TtftTimeout` | 首 token 超时 |

### 6.4 重试延迟计算

#### `exponential_retry_delay_ms`

```rust
fn exponential_retry_delay_ms(attempt: u32) -> u64
```

标准指数退避：`min(500 * 2^min(attempt, 6), 30000)` + 0-1000ms 随机抖动。

#### `rate_limit_retry_delay_ms`

```rust
fn rate_limit_retry_delay_ms(attempt: u32) -> u64
```

速率限制指数退避：`min(2000 * 2^min(attempt, 6), 60000)` + 0-1000ms 随机抖动。

#### `retry_after_delay_ms`

```rust
fn retry_after_delay_ms(headers: &HeaderMap) -> Option<u64>
```

解析 `Retry-After` 响应头：
- 纯数字 → 秒数
- RFC2822 日期 → 计算与当前时间的差值
- 上限 60 秒（`MAX_RETRY_AFTER_DELAY_MS`）

#### `retry_delay_ms`

```rust
fn retry_delay_ms(attempt: u32, headers: &HeaderMap) -> u64
```

综合延迟计算：优先使用 `Retry-After` 头，回退到指数退避。对于 429 响应，确保不低于 `rate_limit_retry_delay_ms`。

### 6.5 SSE 重试常量

| 常量 | 值 | 说明 |
|---|---|---|
| `BASE_RETRY_DELAY_MS` | `500` | 基础重试延迟 |
| `RATE_LIMIT_BASE_RETRY_DELAY_MS` | `2000` | 速率限制基础延迟 |
| `MAX_EXPONENTIAL_DELAY_MS` | `30_000` | 指数退避上限 |
| `MAX_RETRY_EXPONENT_SHIFT` | `6` | 最大指数偏移 |
| `MAX_RETRY_AFTER_DELAY_MS` | `60_000` | Retry-After 头上限 |

### 6.6 `is_retryable_http_status`

```rust
fn is_retryable_http_status(status: u16) -> bool
```

返回 `true` 的状态码：`5xx`, `408`, `409`, `425`, `429`。

---

## 7. Provider 怪癖处理

**源文件**: `src/crates/adapters/ai-adapters/src/client/quirks.rs`

### 7.1 URL 检测函数

| 函数 | 签名 | 说明 |
|---|---|---|
| `is_dashscope_url` | `fn(url: &str) -> bool` | 检测是否阿里 DashScope URL |
| `is_siliconflow_url` | `fn(url: &str) -> bool` | 检测是否 SiliconFlow URL |
| `is_deepseek_url` | `fn(url: &str) -> bool` | 检测是否 DeepSeek URL |
| `is_deepseek_reasoning_effort_model` | `fn(model: &str) -> bool` | 检测是否 DeepSeek reasoning 模型（r1/reasoner） |

### 7.2 `normalize_deepseek_reasoning_effort`

```rust
pub fn normalize_deepseek_reasoning_effort(effort: Option<&str>) -> Option<&'static str>
```

映射推理力度：`"low"`/`"medium"`/`"high"` → 对应值，无效值返回 `None`。

### 7.3 `parse_glm_major_minor`

```rust
pub fn parse_glm_major_minor(model_name: &str) -> Option<(u32, u32)>
```

解析 GLM 模型版本号。例如 `"glm-4.5"` → `Some((4, 5))`。

### 7.4 `should_append_tool_stream`

```rust
pub fn should_append_tool_stream(url: &str, model_name: &str) -> bool
```

判定是否需要追加 `tool_stream` 参数：
- bigmodel.cn 上的所有模型
- dashscope 上的 GLM ≥ 4.5

### 7.5 `apply_openai_compatible_reasoning_fields`

```rust
pub fn apply_openai_compatible_reasoning_fields(
    request_body: &mut Value,
    mode: Option<ReasoningMode>,
    reasoning_effort: Option<String>,
    url: &str,
    model_name: &str,
)
```

按 Provider 注入推理字段到请求体：

| Provider 类型 | 注入字段 |
|---|---|
| DashScope / SiliconFlow | `"enable_thinking": true/false` |
| 其他（标准 OpenAI） | `"thinking": {"type": "enabled"/"disabled"}` |
| DeepSeek reasoning 模型 | 额外添加 `"reasoning_effort": "low"/"medium"/"high"` |

---

## 8. 流聚合器

**源文件**: `src/crates/adapters/ai-adapters/src/client/response_aggregator.rs`

### 8.1 `aggregate_stream_response`

```rust
pub async fn aggregate_stream_response(
    stream_response: StreamResponse,
) -> Result<GeminiResponse, String>
```

消耗 `StreamResponse` 流，产生聚合的 `GeminiResponse`：

1. 遍历流中的每个 `Result<UnifiedResponse>` chunk
2. 累积文本增量 → `text`
3. 累积推理内容 → `reasoning_content`
4. 使用 `PendingToolCalls` 累加器处理工具调用（带边界检测）
5. 取最后一个 chunk 的 `usage` 和 `finish_reason`
6. 收集 `provider_metadata`

### 8.2 `unified_usage_to_gemini_usage`

```rust
fn unified_usage_to_gemini_usage(usage: UnifiedTokenUsage) -> GeminiUsage
```

从 `UnifiedTokenUsage` 映射字段到 `GeminiUsage`。

---

## 9. AIClientFactory API

**源文件**: `src/crates/assembly/core/src/infrastructure/ai/client_factory.rs`

### 9.1 `AIClientFactory` struct

```rust
pub struct AIClientFactory {
    config_service: Arc<ConfigService>,
    client_cache: RwLock<HashMap<String, CachedAIClient>>,
}
```

基于指纹失效的缓存工厂。内部持有 `CachedAIClient`：

```rust
struct CachedAIClient {
    configuration_fingerprint: String,
    client: Arc<AIClient>,
    credential_expires_at: Option<i64>,
}
```

### 9.2 公共方法

#### `get_client_by_func_agent`

```rust
pub fn get_client_by_func_agent(
    &self,
    func_agent_name: &str,
) -> Result<Arc<AIClient>>
```

根据功能 Agent 名称获取对应的 AIClient。

#### `get_client_by_id`

```rust
pub fn get_client_by_id(
    &self,
    model_id: &str,
) -> Result<Arc<AIClient>>
```

根据模型 ID 获取 AIClient。不进行模型选择器解析。

#### `get_client_by_approved_binding`

```rust
pub fn get_client_by_approved_binding(
    &self,
    model_id: &str,
    configuration_fingerprint: &str,
) -> Result<Arc<AIClient>>
```

根据模型 ID + 配置指纹获取 AIClient。用于需要显式批准特定配置的场景。

**缓存逻辑**（三个方法共用）:
1. 从 `ConfigService` 加载 `AIConfig`
2. 计算配置指纹
3. 检查缓存：指纹匹配 + 凭据未过期 → 返回缓存
4. 否则：构建新 `AIClient`，应用订阅凭据，更新缓存

#### `get_client_resolved`

```rust
pub fn get_client_resolved(
    &self,
    model_id: &str,
) -> Result<Arc<AIClient>>
```

解析模型选择器后获取 AIClient。支持 `"primary"` / `"fast"` / `"auto"` 选择器：

- `"primary"` → 返回主模型
- `"fast"` → 返回快速模型，如果未配置则回退到 primary
- `"auto"` → 自动选择
- 其他 → 直接按模型 ID 查询

#### `invalidate_cache`

```rust
pub fn invalidate_cache(&self)
```

清空整个客户端缓存。

#### `get_cache_size`

```rust
pub fn get_cache_size(&self) -> usize
```

返回当前缓存的客户端数量。

#### `invalidate_model`

```rust
pub fn invalidate_model(&self, model_id: &str)
```

清除指定模型的缓存条目。

### 9.3 全局单例函数

#### `initialize_global`

```rust
pub fn initialize_global() -> BitFunResult<()>
```

初始化全局 `AIClientFactory` 单例。基于 `OnceLock<Arc<RwLock<Option<Arc<AIClientFactory>>>>>` 实现，确保只初始化一次。

#### `get_global`

```rust
pub fn get_global() -> BitFunResult<Arc<AIClientFactory>>
```

获取全局单例引用。如果未初始化则返回错误。

#### `is_global_initialized`

```rust
pub fn is_global_initialized() -> bool
```

检查全局单例是否已初始化。

#### `update_global`

```rust
pub fn update_global(new_factory: Arc<AIClientFactory>) -> BitFunResult<()>
```

替换全局单例为新工厂实例。

### 9.4 自由函数

#### `get_global_ai_client_factory`

```rust
pub fn get_global_ai_client_factory() -> BitFunResult<Arc<AIClientFactory>>
```

`get_global()` 的便捷别名。

#### `initialize_global_ai_client_factory`

```rust
pub fn initialize_global_ai_client_factory() -> BitFunResult<()>
```

`initialize_global()` 的便捷别名。

#### `apply_subscription_auth`

```rust
pub fn apply_subscription_auth(
    auth: &SubscriptionAuth,
    ai_config: &mut AIConfig,
) -> Result<Option<i64>>
```

将订阅凭据（API key 等）应用到 AIConfig，返回凭据过期时间（UNIX 时间戳）。

**支持的订阅 Provider**:
- **Codex**: 应用 `CODEX_API_KEY`
- **Antigravity**: 应用 `ANTIGRAVITY_API_KEY`
- **Opencode**: 应用 `OPENCODE_API_KEY`

#### `list_subscription_accounts`

```rust
pub fn list_subscription_accounts() -> Vec<SubscriptionAccount>
```

列出所有已配置的订阅账号。

---

## 10. 类型重新导出链

taiji-llm 替换时需要理解的类型流动路径：

```
bitfun_core_types (src/crates/contracts/core-types/src/ai.rs)
  ├── AIConfig
  ├── ProxyConfig
  ├── ReasoningMode
  ├── Message
  ├── ToolCall
  ├── ToolDefinition
  ├── ConnectionTestResult
  ├── ConnectionTestMessageCode
  └── RemoteModelInfo
        │
        │ pub use bitfun_core_types::{...};
        ▼
bitfun_ai_adapters::types (src/crates/adapters/ai-adapters/src/types/)
  ├── 重新导出上述所有 core-types 类型
  ├── GeminiResponse
  ├── GeminiUsage
  └── resolve_request_url()
        │
        │ pub use bitfun_ai_adapters::types::{...};
        ▼
assembly::core::util::types (src/crates/assembly/core/src/util/types.rs)
  └── 重新导出上述所有类型（供上层代码直接使用）
```

**taiji-llm 应依赖的最低层级**: `bitfun_core_types`（基础类型）+ `bitfun_ai_adapters`（客户端和适配器类型）。

---

## 附录: 关键设计决策

1. **两层重试架构**
   - **传输层 (SSE)**: `sse.rs` 中的 `execute_sse_request` — 处理 HTTP 状态码、连接错误、TTFT 超时
   - **聚合层 (Client)**: `client.rs` 中的 `send_message_with_extra_body_trace_and_max_attempts` — 流式获取后判断内容是否为空/错误，决定是否重试

2. **Provider 派发模式**: `ApiFormat` enum + match → 委托给 `crate::providers::{openai, anthropic, gemini}` 模块。添加新 Provider 需要新增 `ApiFormat` 变体 + 对应的 provider 实现。

3. **缓存策略**: `AIClientFactory` 使用配置指纹 + 凭据过期时间的双条件缓存验证。配置变更或凭据过期自动触发重建。

4. **全局单例**: `AIClientFactory` 通过 `OnceLock<Arc<RwLock<Option<Arc<AIClientFactory>>>>>` 管理全局实例，支持运行时替换（`update_global`）。

5. **流生命周期**: `ManagedResponseStream` 的 Drop 实现自动取消后台 SSE 处理任务，防止资源泄漏。
