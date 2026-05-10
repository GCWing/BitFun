use crate::agentic::core::{
    CompressedMessage, CompressedTodoSnapshot, CompressionContract, CompressionPayload,
};

#[derive(Debug, Clone)]
pub struct CompressionFallbackOptions {
    pub max_tokens: usize,
    pub user_chars: usize,
    pub assistant_chars: usize,
    pub tool_arg_chars: usize,
    pub tool_command_chars: usize,
}

#[derive(Debug, Clone)]
pub struct CompressionSummaryArtifact {
    pub summary_text: String,
    pub payload: CompressionPayload,
    pub used_model_summary: bool,
}

#[derive(Debug, Clone)]
pub(super) enum CompressionUnit {
    Contract {
        contract: CompressionContract,
    },
    ModelSummary {
        text: String,
    },
    TurnMessage {
        entry_id: usize,
        turn_id: Option<String>,
        message: CompressedMessage,
    },
    TurnTodo {
        entry_id: usize,
        turn_id: Option<String>,
        todo: CompressedTodoSnapshot,
    },
}
