//! Portable contracts for persisted thread-goal tool handlers.

use crate::thread_goal::goal_tool_response;
use bitfun_runtime_ports::{ThreadGoal, ThreadGoalStatus};
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

pub const GET_GOAL_TOOL_NAME: &str = "get_goal";
pub const CREATE_GOAL_TOOL_NAME: &str = "create_goal";
pub const UPDATE_GOAL_TOOL_NAME: &str = "update_goal";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CreateGoalArgs {
    pub objective: String,
    pub token_budget: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct UpdateGoalArgs {
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalToolResult {
    pub data: Value,
    pub result_for_assistant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGoalToolError(String);

impl ThreadGoalToolError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ThreadGoalToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThreadGoalToolError {}

pub fn parse_create_goal_args(input: Value) -> Result<CreateGoalArgs, ThreadGoalToolError> {
    serde_json::from_value(input).map_err(|error| {
        ThreadGoalToolError::validation(format!("invalid create_goal args: {error}"))
    })
}

pub fn parse_update_goal_args(input: Value) -> Result<UpdateGoalArgs, ThreadGoalToolError> {
    serde_json::from_value(input).map_err(|error| {
        ThreadGoalToolError::validation(format!("invalid update_goal args: {error}"))
    })
}

pub fn parse_update_goal_status(raw: &str) -> Result<ThreadGoalStatus, ThreadGoalToolError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "complete" => Ok(ThreadGoalStatus::Complete),
        "blocked" => Ok(ThreadGoalStatus::Blocked),
        other => Err(ThreadGoalToolError::validation(format!(
            "update_goal status must be complete or blocked, got {other}"
        ))),
    }
}

pub fn build_goal_tool_result(
    goal: Option<ThreadGoal>,
    include_completion_report: bool,
) -> Result<GoalToolResult, ThreadGoalToolError> {
    let response = goal_tool_response(goal, include_completion_report);
    let data = serde_json::to_value(response).map_err(|error| {
        ThreadGoalToolError::validation(format!("failed to serialize goal tool result: {error}"))
    })?;
    let result_for_assistant = data
        .get("goal")
        .and_then(|goal| goal.get("status"))
        .and_then(|status| status.as_str())
        .map(|status| format!("Thread goal status: {status}"))
        .unwrap_or_else(|| "No thread goal is set.".to_string());
    Ok(GoalToolResult {
        data,
        result_for_assistant,
    })
}
