//! Skill and subagent projections shared by product surfaces.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub key: String,
    pub name: String,
    pub description: String,
    pub level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub selected_for_runtime: bool,
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub is_shadowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub key: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub supports_follow_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentListProjection {
    pub subagents: Vec<SubagentSummary>,
    #[serde(default)]
    pub has_external: bool,
}
