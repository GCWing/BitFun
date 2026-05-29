use serde::{Deserialize, Serialize};

pub const GOAL_MODE_METADATA_KEY: &str = "goal_mode";
pub const GOAL_MODE_FUNC_AGENT: &str = "session-title-func-agent";
pub const MAX_GOAL_CONTINUATIONS: u32 = 100;
pub const MAX_CONTEXT_SUMMARY_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalModeInitialGoal {
    pub goal_text: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_hint: Option<String>,
    #[serde(default)]
    pub created_at_ms: u64,
}

impl Default for GoalModeInitialGoal {
    fn default() -> Self {
        Self {
            goal_text: String::new(),
            success_criteria: Vec::new(),
            user_hint: None,
            created_at_ms: 0,
        }
    }
}

impl GoalModeInitialGoal {
    pub fn new(
        goal_text: String,
        success_criteria: Vec<String>,
        user_hint: Option<String>,
        created_at_ms: u64,
    ) -> Self {
        Self {
            goal_text,
            success_criteria,
            user_hint,
            created_at_ms,
        }
    }

    pub fn is_set(&self) -> bool {
        !self.goal_text.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalModeState {
    pub active: bool,
    #[serde(default)]
    pub initial_goal: GoalModeInitialGoal,
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
        self.active && !self.initial_goal_text().trim().is_empty()
    }

    pub fn initial_goal_text(&self) -> &str {
        if self.initial_goal.is_set() {
            self.initial_goal.goal_text.as_str()
        } else {
            self.goal_text.as_str()
        }
    }

    pub fn initial_success_criteria(&self) -> &[String] {
        if self.initial_goal.is_set() {
            self.initial_goal.success_criteria.as_slice()
        } else {
            self.success_criteria.as_slice()
        }
    }

    pub fn initial_user_hint(&self) -> Option<&str> {
        self.initial_goal
            .user_hint
            .as_deref()
            .or(self.user_hint.as_deref())
    }

    pub fn initial_goal_created_at_ms(&self) -> u64 {
        if self.initial_goal.is_set() {
            self.initial_goal.created_at_ms
        } else {
            self.activated_at_ms
        }
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
