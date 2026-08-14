//! Native Hook inspection projections shared by product surfaces.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeHookOverview {
    pub enabled: bool,
    pub project_hooks_enabled: bool,
    pub files: Vec<NativeHookFileSummary>,
    pub rules: Vec<NativeHookRuleSummary>,
    pub total_handlers: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeHookFileSummary {
    pub scope: String,
    /// Secret-safe host-relative label, never an absolute filesystem path.
    pub location: String,
    pub exists: bool,
    pub loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeHookRuleSummary {
    pub event: String,
    pub matcher: String,
    pub matcher_is_valid: bool,
    pub scope: String,
    pub handlers: Vec<NativeHookHandlerSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeHookHandlerSummary {
    /// A bounded display summary. The executable command remains with the owner.
    pub command_summary: String,
    pub command_truncated: bool,
    pub timeout_seconds: u64,
    pub status_message: Option<String>,
}
