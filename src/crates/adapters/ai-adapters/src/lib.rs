#![doc = include_str!("../README.md")]

#[cfg(feature = "cli-credentials")]
pub mod cli_credentials;
pub mod client;
pub mod diagnostics;
pub mod model_selector;
pub mod providers;
pub mod stream;
pub mod tool_call_accumulator;
pub mod trace;
pub mod types;

pub use client::{
    AIClient, StreamOptions, StreamResponse, DEFAULT_STREAM_IDLE_TIMEOUT_SECS,
    DEFAULT_STREAM_TTFT_TIMEOUT_SECS,
};
pub use model_selector::{
    classify_model_selector, resolve_cache_model_selector, resolve_required_model_selector,
    ModelSelectorError, ModelSelectorKind,
};
pub use stream::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
pub use trace::{
    ModelExchangeRequestAttempt, ModelExchangeRequestTraceHandle, ModelExchangeResponseTrace,
    ModelExchangeTraceConfig, ModelExchangeTraceSink,
};
pub use types::{
    resolve_request_url, AIConfig, ConnectionTestMessageCode, ConnectionTestResult, GeminiResponse,
    GeminiUsage, Message, ProxyConfig, ReasoningMode, RemoteModelInfo, ToolCall, ToolDefinition,
    ToolImageAttachment,
};
