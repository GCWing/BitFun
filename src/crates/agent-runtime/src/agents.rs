//! Agent and subagent registry owner decisions.

use crate::prompt::UserContextPolicy;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;

pub const SHARED_CODING_MODE_PROMPT_TEMPLATE: &str = "agentic_mode";
pub const SHARED_CODING_MODE_CONFIG_PROFILE_ID: &str = "coding_shared";
pub const SHARED_CODING_MODE_CONFIG_PROFILE_LABEL: &str = "Coding Shared";
pub const SHARED_CODING_MODE_IDS: &[&str] = &["agentic", "Plan", "debug", "Multitask"];

pub fn resolve_mode_config_profile_id<'a>(mode_id: &'a str) -> Cow<'a, str> {
    match mode_id.trim() {
        "agentic" | "Plan" | "debug" | "Multitask" => {
            Cow::Borrowed(SHARED_CODING_MODE_CONFIG_PROFILE_ID)
        }
        _ => Cow::Borrowed(mode_id),
    }
}

pub fn mode_config_profile_member_mode_ids(profile_id: &str) -> &'static [&'static str] {
    match profile_id.trim() {
        SHARED_CODING_MODE_CONFIG_PROFILE_ID => SHARED_CODING_MODE_IDS,
        _ => &[],
    }
}

pub fn mode_config_profile_label(profile_id: &str) -> Option<&'static str> {
    match profile_id.trim() {
        SHARED_CODING_MODE_CONFIG_PROFILE_ID => Some(SHARED_CODING_MODE_CONFIG_PROFILE_LABEL),
        _ => None,
    }
}

pub fn mode_presentation_rank(mode_id: &str) -> u8 {
    match mode_id {
        "agentic" => 0,
        "Cowork" => 1,
        "Plan" => 2,
        "debug" => 3,
        "Multitask" => 4,
        "DeepResearch" => 5,
        "Team" => 6,
        _ => 99,
    }
}

pub fn shared_coding_mode_user_context_policy() -> UserContextPolicy {
    UserContextPolicy::empty()
        .with_workspace_context()
        .with_workspace_instructions()
        .with_workspace_memory_files()
        .with_project_layout()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentListScope {
    TaskVisible,
    RegistryManagement,
}

#[derive(Debug, Clone)]
pub struct SubagentQueryContext<'a> {
    pub parent_agent_type: Option<&'a str>,
    pub workspace_root: Option<&'a Path>,
    pub list_scope: SubagentListScope,
    pub include_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinSubagentExposure {
    Public,
    Restricted,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentVisibilitySummary {
    pub exposure: BuiltinSubagentExposure,
    pub allowed_parent_agent_ids: Vec<String>,
    pub denied_parent_agent_ids: Vec<String>,
    pub show_in_global_registry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentVisibilityPolicy {
    pub exposure: BuiltinSubagentExposure,
    pub allowed_parent_agent_ids: HashSet<String>,
    pub denied_parent_agent_ids: HashSet<String>,
    pub show_in_global_registry: bool,
}

impl SubagentVisibilityPolicy {
    pub fn public() -> Self {
        Self {
            exposure: BuiltinSubagentExposure::Public,
            allowed_parent_agent_ids: HashSet::new(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: true,
        }
    }

    pub fn restricted<I, S>(allowed_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            exposure: BuiltinSubagentExposure::Restricted,
            allowed_parent_agent_ids: allowed_parent_agent_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: true,
        }
    }

    pub fn hidden<I, S>(allowed_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            exposure: BuiltinSubagentExposure::Hidden,
            allowed_parent_agent_ids: allowed_parent_agent_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: false,
        }
    }

    pub fn deny_for<I, S>(mut self, denied_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.denied_parent_agent_ids = denied_parent_agent_ids
            .into_iter()
            .map(Into::into)
            .collect();
        self
    }

    pub fn summary(&self) -> SubagentVisibilitySummary {
        let mut allowed_parent_agent_ids: Vec<String> =
            self.allowed_parent_agent_ids.iter().cloned().collect();
        allowed_parent_agent_ids.sort();

        let mut denied_parent_agent_ids: Vec<String> =
            self.denied_parent_agent_ids.iter().cloned().collect();
        denied_parent_agent_ids.sort();

        SubagentVisibilitySummary {
            exposure: self.exposure,
            allowed_parent_agent_ids,
            denied_parent_agent_ids,
            show_in_global_registry: self.show_in_global_registry,
        }
    }

    pub fn can_access_from_parent(&self, parent_agent_type: Option<&str>) -> bool {
        let normalized_parent = parent_agent_type
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if normalized_parent.is_some_and(|parent| self.denied_parent_agent_ids.contains(parent)) {
            return false;
        }

        match self.exposure {
            BuiltinSubagentExposure::Public => true,
            BuiltinSubagentExposure::Restricted | BuiltinSubagentExposure::Hidden => {
                normalized_parent
                    .is_some_and(|parent| self.allowed_parent_agent_ids.contains(parent))
            }
        }
    }
}

impl Default for SubagentVisibilityPolicy {
    fn default() -> Self {
        Self::public()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentSourceKind {
    Builtin,
    Project,
    User,
    Unspecified,
}

/// Subagent source shown to product surfaces and registry-management APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentSource {
    Builtin,
    Project,
    User,
}

pub const fn subagent_source_kind(source: Option<SubAgentSource>) -> SubagentSourceKind {
    match source {
        Some(SubAgentSource::Builtin) => SubagentSourceKind::Builtin,
        Some(SubAgentSource::Project) => SubagentSourceKind::Project,
        Some(SubAgentSource::User) => SubagentSourceKind::User,
        None => SubagentSourceKind::Unspecified,
    }
}

pub const fn subagent_source_presentation_rank(source: Option<SubAgentSource>) -> u8 {
    match source {
        Some(SubAgentSource::Builtin) => 0,
        Some(SubAgentSource::Project) => 1,
        Some(SubAgentSource::User) => 2,
        None => 3,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubagentOverrideState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStateReason {
    BuiltinDefaultVisible,
    BuiltinDefaultHidden,
    CustomDefaultEnabled,
    EnabledByProjectOverride,
    DisabledByProjectOverride,
    EnabledByUserOverride,
    DisabledByUserOverride,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubagentOverrideLayers {
    pub project_override: Option<SubagentOverrideState>,
    pub user_override: Option<SubagentOverrideState>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSubagentAvailability {
    pub default_enabled: bool,
    pub effective_enabled: bool,
    pub override_state: Option<SubagentOverrideState>,
    pub state_reason: Option<SubagentStateReason>,
}

pub fn resolve_subagent_default_enabled(
    source: SubagentSourceKind,
    visibility: &SubagentVisibilityPolicy,
    parent_agent_type: Option<&str>,
) -> bool {
    match source {
        SubagentSourceKind::Builtin => visibility.can_access_from_parent(parent_agent_type),
        SubagentSourceKind::Project
        | SubagentSourceKind::User
        | SubagentSourceKind::Unspecified => true,
    }
}

pub fn resolve_subagent_availability(
    source: SubagentSourceKind,
    default_enabled: bool,
    layers: SubagentOverrideLayers,
) -> ResolvedSubagentAvailability {
    if source == SubagentSourceKind::Project {
        if let Some(project_override) = layers.project_override {
            return ResolvedSubagentAvailability {
                default_enabled,
                effective_enabled: project_override == SubagentOverrideState::Enabled,
                override_state: Some(project_override),
                state_reason: Some(project_reason(project_override)),
            };
        }
    } else if matches!(
        source,
        SubagentSourceKind::Builtin | SubagentSourceKind::User
    ) {
        if let Some(user_override) = layers.user_override {
            return ResolvedSubagentAvailability {
                default_enabled,
                effective_enabled: user_override == SubagentOverrideState::Enabled,
                override_state: Some(user_override),
                state_reason: Some(user_reason(user_override)),
            };
        }
    }

    ResolvedSubagentAvailability {
        default_enabled,
        effective_enabled: default_enabled,
        override_state: None,
        state_reason: default_reason(source, default_enabled),
    }
}

const fn default_reason(
    source: SubagentSourceKind,
    default_enabled: bool,
) -> Option<SubagentStateReason> {
    match source {
        SubagentSourceKind::Builtin => Some(if default_enabled {
            SubagentStateReason::BuiltinDefaultVisible
        } else {
            SubagentStateReason::BuiltinDefaultHidden
        }),
        SubagentSourceKind::Project | SubagentSourceKind::User => {
            Some(SubagentStateReason::CustomDefaultEnabled)
        }
        SubagentSourceKind::Unspecified => None,
    }
}

const fn project_reason(state: SubagentOverrideState) -> SubagentStateReason {
    match state {
        SubagentOverrideState::Enabled => SubagentStateReason::EnabledByProjectOverride,
        SubagentOverrideState::Disabled => SubagentStateReason::DisabledByProjectOverride,
    }
}

const fn user_reason(state: SubagentOverrideState) -> SubagentStateReason {
    match state {
        SubagentOverrideState::Enabled => SubagentStateReason::EnabledByUserOverride,
        SubagentOverrideState::Disabled => SubagentStateReason::DisabledByUserOverride,
    }
}
