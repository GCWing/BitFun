#![doc = include_str!("../README.md")]

pub mod client;
pub mod diagnostics;
pub mod providers;
pub mod stream;
pub mod tool_call_accumulator;
pub mod types;

pub use client::{
    AIClient, StreamOptions, StreamResponse, DEFAULT_STREAM_IDLE_TIMEOUT_SECS,
    DEFAULT_STREAM_TTFT_TIMEOUT_SECS, REASONING_STREAM_TTFT_TIMEOUT_SECS,
};
pub use stream::{UnifiedResponse, UnifiedTokenUsage, UnifiedToolCall};
pub use types::{
    resolve_request_url, AIConfig, ConnectionTestMessageCode, ConnectionTestResult, GeminiResponse,
    GeminiUsage, Message, ProxyConfig, ReasoningMode, RemoteModelInfo, ToolCall, ToolDefinition,
    ToolImageAttachment,
};
