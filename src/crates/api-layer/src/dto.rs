/// Data Transfer Objects (DTO) - Platform-agnostic request and response types
///
/// These types are used by all platforms (CLI, Tauri, Server)
use serde::{Deserialize, Serialize};

/// Execute agent task request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteAgentRequest {
    pub agent_type: String,
    pub model_name: Option<String>,
    pub user_message: String,
    pub context: Option<String>,
    pub images: Option<Vec<ImageData>>,
    pub session_id: Option<String>,
}

/// Execute agent task response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteAgentResponse {
    pub session_id: String,
    pub turn_id: String,
    pub status: String,
    pub message: Option<String>,
}

/// Image data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub data: String, // Base64
    pub mime_type: String,
}

/// Get session history request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSessionHistoryRequest {
    pub session_id: String,
    pub limit: Option<usize>,
}

/// Session history response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHistoryResponse {
    pub session_id: String,
    pub turns: Vec<TurnSummary>,
}

/// Turn summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSummary {
    pub turn_id: String,
    pub user_message: String,
    pub assistant_response: String,
    pub tool_calls: Vec<String>,
    pub timestamp: i64,
}

/// Read file request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileRequest {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// Read file response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileResponse {
    pub content: String,
    pub total_lines: Option<usize>,
}

/// Write file request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileRequest {
    pub path: String,
    pub content: String,
    pub create_dirs: Option<bool>,
}

/// Generic success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_sessions: usize,
}
