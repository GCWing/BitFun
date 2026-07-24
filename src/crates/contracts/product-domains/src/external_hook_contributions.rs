//! Ecosystem-neutral values for statically discovered external Hook contributions.
//!
//! These values describe source projection only. They do not imply activation,
//! execution, runtime support, or a stable wire protocol.

use crate::external_sources::{validate_id, ExternalSourceContractError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Stable identity of one Hook contribution across content revisions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalHookContributionId(String);

impl ExternalHookContributionId {
    pub fn new(value: impl Into<String>) -> Result<Self, ExternalSourceContractError> {
        let value = value.into();
        validate_id(&value, "external Hook contribution")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExternalHookContributionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Hook points with a current static OpenCode mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalHookPoint {
    ToolBefore,
    ToolAfter,
}

impl ExternalHookPoint {
    /// Stable low-cardinality label for diagnostics and future metrics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ToolBefore => "tool_before",
            Self::ToolAfter => "tool_after",
        }
    }
}

/// Static data capabilities that a Hook declaration may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalHookRiskCapability {
    ReadToolArguments,
    ModifyToolArguments,
    ReadToolResult,
    ModifyToolResult,
}

impl ExternalHookRiskCapability {
    /// Stable low-cardinality label for diagnostics and future metrics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadToolArguments => "read_tool_arguments",
            Self::ModifyToolArguments => "modify_tool_arguments",
            Self::ReadToolResult => "read_tool_result",
            Self::ModifyToolResult => "modify_tool_result",
        }
    }
}

/// Declared static risk facts. An incomplete declaration must never authorize
/// Hook execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalHookSafetyDeclaration {
    pub declared_risks: BTreeSet<ExternalHookRiskCapability>,
    pub complete: bool,
}

/// One statically normalized Hook contribution used by source projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalHookContributionDeclaration {
    pub contribution_id: ExternalHookContributionId,
    pub hook_point: ExternalHookPoint,
    pub safety: ExternalHookSafetyDeclaration,
}
