use crate::{
    validate_collapsed_tool_usage, validate_tool_allowed_by_list, CollapsedToolUsageError,
    ToolExecutionAccessError, ToolRestrictionError, ToolRuntimeRestrictions,
};
use std::fmt;

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
