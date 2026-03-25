use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionStartedInfo {
    pub tool_use_id: String,
    pub tool_name: String,
    pub user_facing_name: Option<String>,
    pub input: serde_json::Value,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: u64,
    pub ai_intent: Option<String>, // AI intent content
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionProgressInfo {
    pub tool_use_id: String,
    pub tool_name: String,
    pub progress_message: String,
    pub percentage: Option<f32>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTerminalReadyInfo {
    pub tool_use_id: String,
    pub terminal_session_id: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionCompletedInfo {
    pub tool_use_id: String,
    pub tool_name: String,
    pub result: serde_json::Value,
    pub duration_ms: u64,
    pub cost_usd: Option<f64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionErrorInfo {
    pub tool_use_id: String,
    pub tool_name: String,
    pub error_message: String,
    pub error_type: String,
    pub duration_ms: Option<u64>,
    pub timestamp: u64,
}
