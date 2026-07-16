use crate::{
    validate_deferred_tool_usage, validate_tool_allowed_by_list, DeferredToolUsageError,
    LoadedDeferredToolSpec, ToolExecutionAccessError, ToolRestrictionError,
    ToolRuntimeRestrictions,
};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub struct ToolExecutionAdmissionRequest<'a> {
    pub tool_name: &'a str,
    pub allowed_tools: &'a [String],
    pub runtime_tool_restrictions: &'a ToolRuntimeRestrictions,
    pub invocation_is_deferred: bool,
    pub deferred_tools: &'a [String],
    pub loaded_deferred_tool_specs: &'a [LoadedDeferredToolSpec],
    pub current_catalog_generation: u64,
    pub get_tool_spec_tool_name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolExecutionAdmissionRejection {
    AllowedList(ToolExecutionAccessError),
    RuntimeRestriction(ToolRestrictionError),
    Deferred(DeferredToolUsageError),
}

impl fmt::Display for ToolExecutionAdmissionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowedList(error) => write!(formatter, "{error}"),
            Self::RuntimeRestriction(error) => write!(formatter, "{error}"),
            Self::Deferred(error) => write!(formatter, "{error}"),
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
    validate_deferred_tool_usage(
        request.tool_name,
        request.invocation_is_deferred,
        request.deferred_tools,
        request.loaded_deferred_tool_specs,
        request.current_catalog_generation,
        request.get_tool_spec_tool_name,
    )
    .map_err(ToolExecutionAdmissionRejection::Deferred)
}
