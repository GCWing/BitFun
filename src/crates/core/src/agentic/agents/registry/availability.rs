use super::types::{subagent_key_for, AgentEntry, SubagentStateReason};
use crate::agentic::agents::SubAgentSource;
use crate::service::config::types::{
    AgentSubagentOverrideConfig, AgentSubagentOverrideState, ParentSubagentOverrideConfig,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSubagentAvailability {
    pub default_enabled: bool,
    pub effective_enabled: bool,
    pub override_state: Option<AgentSubagentOverrideState>,
    pub state_reason: Option<SubagentStateReason>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedOverrideLayers {
    pub project_override: Option<AgentSubagentOverrideState>,
    pub user_override: Option<AgentSubagentOverrideState>,
}

fn default_reason(entry: &AgentEntry, default_enabled: bool) -> Option<SubagentStateReason> {
    match entry.subagent_source {
        Some(SubAgentSource::Builtin) => Some(if default_enabled {
            SubagentStateReason::BuiltinDefaultVisible
        } else {
            SubagentStateReason::BuiltinDefaultHidden
        }),
        Some(SubAgentSource::Project) | Some(SubAgentSource::User) => {
            Some(SubagentStateReason::CustomDefaultEnabled)
        }
        None => None,
    }
}

fn project_reason(state: AgentSubagentOverrideState) -> SubagentStateReason {
    match state {
        AgentSubagentOverrideState::Enabled => SubagentStateReason::EnabledByProjectOverride,
        AgentSubagentOverrideState::Disabled => SubagentStateReason::DisabledByProjectOverride,
    }
}

fn user_reason(state: AgentSubagentOverrideState) -> SubagentStateReason {
    match state {
        AgentSubagentOverrideState::Enabled => SubagentStateReason::EnabledByUserOverride,
        AgentSubagentOverrideState::Disabled => SubagentStateReason::DisabledByUserOverride,
    }
}

pub fn normalize_parent_agent_id(parent_agent_type: Option<&str>) -> Option<&str> {
    parent_agent_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn override_for_parent<'a>(
    overrides: &'a AgentSubagentOverrideConfig,
    parent_agent_type: Option<&str>,
) -> Option<&'a ParentSubagentOverrideConfig> {
    let parent_agent_type = normalize_parent_agent_id(parent_agent_type)?;
    overrides.get(parent_agent_type)
}

pub fn subagent_override_for_parent(
    overrides: &AgentSubagentOverrideConfig,
    parent_agent_type: Option<&str>,
    subagent_key: &str,
) -> Option<AgentSubagentOverrideState> {
    override_for_parent(overrides, parent_agent_type).and_then(|parent| parent.get(subagent_key).copied())
}

pub fn resolve_default_enabled(entry: &AgentEntry, parent_agent_type: Option<&str>) -> bool {
    match entry.subagent_source {
        Some(SubAgentSource::Builtin) => entry.visibility_policy.can_access_from_parent(parent_agent_type),
        Some(SubAgentSource::Project) | Some(SubAgentSource::User) => true,
        None => true,
    }
}

pub fn resolve_override_layers(
    entry: &AgentEntry,
    parent_agent_type: Option<&str>,
    project_overrides: Option<&AgentSubagentOverrideConfig>,
    user_overrides: &AgentSubagentOverrideConfig,
) -> ResolvedOverrideLayers {
    let Some(subagent_key) = subagent_key_for(entry.subagent_source, entry.agent.as_ref()) else {
        return ResolvedOverrideLayers::default();
    };

    match entry.subagent_source {
        Some(SubAgentSource::Project) => ResolvedOverrideLayers {
            project_override: project_overrides.and_then(|overrides| {
                subagent_override_for_parent(overrides, parent_agent_type, &subagent_key)
            }),
            user_override: None,
        },
        Some(SubAgentSource::Builtin) | Some(SubAgentSource::User) => ResolvedOverrideLayers {
            project_override: None,
            user_override: subagent_override_for_parent(user_overrides, parent_agent_type, &subagent_key),
        },
        None => ResolvedOverrideLayers::default(),
    }
}

pub fn resolve_availability(
    entry: &AgentEntry,
    parent_agent_type: Option<&str>,
    project_overrides: Option<&AgentSubagentOverrideConfig>,
    user_overrides: &AgentSubagentOverrideConfig,
) -> ResolvedSubagentAvailability {
    let default_enabled = resolve_default_enabled(entry, parent_agent_type);
    let layers = resolve_override_layers(entry, parent_agent_type, project_overrides, user_overrides);

    if let Some(project_override) = layers.project_override {
        return ResolvedSubagentAvailability {
            default_enabled,
            effective_enabled: matches!(project_override, AgentSubagentOverrideState::Enabled),
            override_state: Some(project_override),
            state_reason: Some(project_reason(project_override)),
        };
    }

    if let Some(user_override) = layers.user_override {
        return ResolvedSubagentAvailability {
            default_enabled,
            effective_enabled: matches!(user_override, AgentSubagentOverrideState::Enabled),
            override_state: Some(user_override),
            state_reason: Some(user_reason(user_override)),
        };
    }

    ResolvedSubagentAvailability {
        default_enabled,
        effective_enabled: default_enabled,
        override_state: None,
        state_reason: default_reason(entry, default_enabled),
    }
}

pub fn prune_override_config(
    overrides: &mut AgentSubagentOverrideConfig,
    parent_agent_type: &str,
    subagent_key: &str,
) {
    if let Some(parent_entry) = overrides.get_mut(parent_agent_type) {
        parent_entry.remove(subagent_key);
        if parent_entry.is_empty() {
            overrides.remove(parent_agent_type);
        }
    }
}

pub fn set_override_state(
    overrides: &mut AgentSubagentOverrideConfig,
    parent_agent_type: &str,
    subagent_key: &str,
    state: AgentSubagentOverrideState,
) {
    overrides
        .entry(parent_agent_type.to_string())
        .or_insert_with(HashMap::new)
        .insert(subagent_key.to_string(), state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::agents::definitions::custom::{CustomSubagent, CustomSubagentKind};
    use crate::agentic::agents::registry::types::AgentCategory;
    use crate::agentic::agents::registry::visibility::SubagentVisibilityPolicy;
    use crate::service::config::types::AgentSubagentOverrideState;
    use std::sync::Arc;

    fn make_entry(source: SubAgentSource, id: &str) -> AgentEntry {
        let agent: Arc<dyn crate::agentic::agents::Agent> = match source {
            SubAgentSource::Builtin => Arc::new(crate::agentic::agents::ExploreAgent::new()),
            SubAgentSource::Project => Arc::new(CustomSubagent::new(
                id.to_string(),
                "Project subagent".to_string(),
                vec!["Read".to_string()],
                "prompt".to_string(),
                true,
                "project.md".to_string(),
                CustomSubagentKind::Project,
            )),
            SubAgentSource::User => Arc::new(CustomSubagent::new(
                id.to_string(),
                "User subagent".to_string(),
                vec!["Read".to_string()],
                "prompt".to_string(),
                true,
                "user.md".to_string(),
                CustomSubagentKind::User,
            )),
        };

        AgentEntry {
            category: AgentCategory::SubAgent,
            subagent_source: Some(source),
            agent,
            visibility_policy: SubagentVisibilityPolicy::public(),
            custom_config: None,
        }
    }

    fn overrides(parent: &str, subagent_key: &str, state: AgentSubagentOverrideState) -> AgentSubagentOverrideConfig {
        let mut parent_overrides = HashMap::new();
        parent_overrides.insert(subagent_key.to_string(), state);

        let mut all = HashMap::new();
        all.insert(parent.to_string(), parent_overrides);
        all
    }

    #[test]
    fn builtin_and_user_subagents_only_use_global_overrides() {
        let builtin_entry = make_entry(SubAgentSource::Builtin, "Explore");
        let builtin_key = subagent_key_for(builtin_entry.subagent_source, builtin_entry.agent.as_ref())
            .expect("builtin key");
        let builtin_layers = resolve_override_layers(
            &builtin_entry,
            Some("agentic"),
            Some(&overrides("agentic", &builtin_key, AgentSubagentOverrideState::Disabled)),
            &overrides("agentic", &builtin_key, AgentSubagentOverrideState::Enabled),
        );
        assert_eq!(builtin_layers.project_override, None);
        assert_eq!(builtin_layers.user_override, Some(AgentSubagentOverrideState::Enabled));

        let user_entry = make_entry(SubAgentSource::User, "UserScout");
        let user_key = subagent_key_for(user_entry.subagent_source, user_entry.agent.as_ref())
            .expect("user key");
        let user_layers = resolve_override_layers(
            &user_entry,
            Some("agentic"),
            Some(&overrides("agentic", &user_key, AgentSubagentOverrideState::Disabled)),
            &overrides("agentic", &user_key, AgentSubagentOverrideState::Enabled),
        );
        assert_eq!(user_layers.project_override, None);
        assert_eq!(user_layers.user_override, Some(AgentSubagentOverrideState::Enabled));
    }

    #[test]
    fn project_subagents_only_use_project_overrides() {
        let entry = make_entry(SubAgentSource::Project, "ProjectScout");
        let key = subagent_key_for(entry.subagent_source, entry.agent.as_ref())
            .expect("project key");
        let layers = resolve_override_layers(
            &entry,
            Some("agentic"),
            Some(&overrides("agentic", &key, AgentSubagentOverrideState::Disabled)),
            &overrides("agentic", &key, AgentSubagentOverrideState::Enabled),
        );

        assert_eq!(layers.project_override, Some(AgentSubagentOverrideState::Disabled));
        assert_eq!(layers.user_override, None);
    }
}
