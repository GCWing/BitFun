use crate::{
    validate_collapsed_tool_usage, validate_tool_allowed_by_list, CollapsedToolUsageError,
    ToolExecutionAccessError, ToolRestrictionError, ToolRuntimeRestrictions,
};
use serde_json::Value;
use std::collections::VecDeque;
use std::fmt;

/// Number of consecutive identical tool calls tolerated before blocking.
pub const TOOL_CALL_LOOP_THRESHOLD: usize = 3;

/// Bounded per-session history window for loop detection.
pub const TOOL_CALL_HISTORY_WINDOW: usize = 10;

#[derive(Debug, Clone)]
struct RecentToolCall {
    tool_name: String,
    arguments: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallLoopHistory {
    entries: VecDeque<RecentToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallLoopBlock {
    pub tool_name: String,
    pub threshold: usize,
    pub attempt: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallLoopDecision {
    Allowed,
    Blocked(ToolCallLoopBlock),
}

impl ToolCallLoopDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn into_blocked(self) -> Option<ToolCallLoopBlock> {
        match self {
            Self::Allowed => None,
            Self::Blocked(block) => Some(block),
        }
    }
}

impl ToolCallLoopHistory {
    pub fn check_and_record(&mut self, tool_name: &str, arguments: &Value) -> ToolCallLoopDecision {
        let identical_priors = self
            .entries
            .iter()
            .rev()
            .take(TOOL_CALL_LOOP_THRESHOLD)
            .take_while(|past| past.tool_name == tool_name && &past.arguments == arguments)
            .count();
        let is_loop = identical_priors >= TOOL_CALL_LOOP_THRESHOLD;

        self.entries.push_back(RecentToolCall {
            tool_name: tool_name.to_string(),
            arguments: arguments.clone(),
        });
        while self.entries.len() > TOOL_CALL_HISTORY_WINDOW {
            self.entries.pop_front();
        }

        if is_loop {
            ToolCallLoopDecision::Blocked(ToolCallLoopBlock {
                tool_name: tool_name.to_string(),
                threshold: TOOL_CALL_LOOP_THRESHOLD,
                attempt: TOOL_CALL_LOOP_THRESHOLD + 1,
                message: build_tool_call_loop_block_message(tool_name),
            })
        } else {
            ToolCallLoopDecision::Allowed
        }
    }
}

pub fn build_tool_call_loop_block_message(tool_name: &str) -> String {
    format!(
        "Tool-call loop blocked: '{}' was already called {} times in a row in this session with identical arguments. Refusing to execute this {}th identical call. Issue a different tool call, or stop tool-calling and respond to the user. If you wrote a file recently and want to continue or modify it, do not call Write again for the same path; use the latest Read result for that file, or call Read once if no current Read result is available, then use Edit with `old_string` taken from the current file content.",
        tool_name,
        TOOL_CALL_LOOP_THRESHOLD,
        TOOL_CALL_LOOP_THRESHOLD + 1
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ToolExecutionAdmissionRequest<'a> {
    pub tool_name: &'a str,
    pub allowed_tools: &'a [String],
    pub runtime_tool_restrictions: &'a ToolRuntimeRestrictions,
    pub collapsed_tools: &'a [String],
    pub loaded_collapsed_tools: &'a [String],
    pub get_tool_spec_tool_name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionAdmissionRejection {
    AllowedList(ToolExecutionAccessError),
    RuntimeRestriction(ToolRestrictionError),
    Collapsed(CollapsedToolUsageError),
}

impl fmt::Display for ToolExecutionAdmissionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowedList(error) => write!(formatter, "{error}"),
            Self::RuntimeRestriction(error) => write!(formatter, "{error}"),
            Self::Collapsed(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ToolExecutionAdmissionRejection {}

pub fn validate_tool_execution_admission(
    request: ToolExecutionAdmissionRequest<'_>,
) -> Result<(), ToolExecutionAdmissionRejection> {
    validate_tool_allowed_by_list(request.tool_name, request.allowed_tools)
        .map_err(ToolExecutionAdmissionRejection::AllowedList)?;
    request
        .runtime_tool_restrictions
        .ensure_tool_allowed(request.tool_name)
        .map_err(ToolExecutionAdmissionRejection::RuntimeRestriction)?;
    validate_collapsed_tool_usage(
        request.tool_name,
        request.collapsed_tools,
        request.loaded_collapsed_tools,
        request.get_tool_spec_tool_name,
    )
    .map_err(ToolExecutionAdmissionRejection::Collapsed)
}
