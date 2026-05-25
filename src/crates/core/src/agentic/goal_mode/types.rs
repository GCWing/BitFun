use serde::{Deserialize, Serialize};

pub const GOAL_MODE_METADATA_KEY: &str = "goal_mode";
pub const GOAL_MODE_FUNC_AGENT: &str = "session-title-func-agent";
pub const MAX_GOAL_CONTINUATIONS: u32 = 100;
pub const MAX_CONTEXT_SUMMARY_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalModeState {
    pub active: bool,
    pub goal_text: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_hint: Option<String>,
    #[serde(default)]
    pub activated_at_ms: u64,
    #[serde(default)]
    pub continuation_count: u32,
}

impl GoalModeState {
    pub fn is_active(&self) -> bool {
        self.active && !self.goal_text.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalGenerationResult {
    pub goal_text: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoalVerificationResult {
    pub achieved: bool,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(default)]
    pub guidance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalActivationResult {
    pub goal_text: String,
    pub success_criteria: Vec<String>,
    pub kickoff_message: String,
    pub display_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalContinuationPlan {
    pub wrapped_message: String,
    pub display_message: String,
    pub user_message_metadata: serde_json::Value,
}
