//! Core data model module
//!
//! Contains all core data structures and state definitions

pub mod dialog_turn;
pub mod message;
pub mod messages_helper;
pub mod prompt_markup;
pub mod session;
pub mod state;
pub use dialog_turn::{new_turn_id, TurnStats};
pub use message::{
    CompressedMessage, CompressedMessageRole, CompressedTodoItem, CompressedTodoSnapshot,
    CompressedToolCall, CompressionEntry, CompressionPayload, Message, MessageContent, MessageRole,
    MessageSemanticKind, ToolCall, ToolResult,
};
pub use messages_helper::{MessageHelper, RequestReasoningTokenPolicy};
pub use prompt_markup::{
    has_prompt_markup, is_system_reminder_only, render_system_reminder, render_user_query,
    strip_prompt_markup, PromptBlock, PromptBlockKind, PromptEnvelope,
};
pub use session::{CompressionState, Session, SessionConfig, SessionKind, SessionSummary};
pub use state::{ProcessingPhase, SessionState, ToolExecutionState};
