//! State definitions
//!
//! Defines session state, tool execution state, etc.

use crate::agentic::tools::framework::ToolResult;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// ============ Session State (aligned with frontend) ============

/// Session state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Processing {
        current_turn_id: String,
        phase: ProcessingPhase,
    },
    Error {
        error: String,
        recoverable: bool,
    },
}

/// Processing phase
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProcessingPhase {
    Starting,       // Starting
    Compacting,     // Context compaction
    Thinking,       // AI thinking
    Streaming,      // Streaming output
    ToolCalling,    // Tool calling
    ToolConfirming, // Waiting for tool confirmation
}

// ============ Tool Execution State ============

/// Tool execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolExecutionState {
    /// Queued waiting for execution
    Queued { position: usize },

    /// Waiting for dependent tools to complete
    Waiting { dependencies: Vec<String> },

    /// Running
    Running {
        started_at: SystemTime,
        progress: Option<f32>, // 0.0-1.0
    },

    /// Streaming output
    Streaming {
        started_at: SystemTime,
        chunks_received: usize,
    },

    /// Waiting for user confirmation
    AwaitingConfirmation {
        params: serde_json::Value,
        timeout_at: SystemTime,
    },

    /// Execution completed
    Completed {
        result: ToolResult,
        duration_ms: u64,
    },

    /// Execution failed
    Failed { error: String, is_retryable: bool },

    /// Cancelled
    Cancelled { reason: String },
}

/// Tool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolStats {
    pub total_tools: usize,
    pub queued: usize,
    pub waiting: usize,
    pub running: usize,
    pub streaming: usize,
    pub awaiting_confirmation: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

// Note: DialogTurnState and ModelRoundState used to live here as a second
// (and divergent) copy of the same names found in `dialog_turn.rs` /
// `model_round.rs`. Both copies were dead code: turn / round lifecycle is
// tracked via `SessionState::Processing` + `TurnStatus` for persistence.
// Removed to avoid future ambiguity.
