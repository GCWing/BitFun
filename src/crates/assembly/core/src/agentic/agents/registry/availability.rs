use super::types::{subagent_key_for, AgentEntry};
use crate::agentic::agents::{resolve_mode_config_profile_id, SubAgentSource};
use crate::service::config::types::{
    AgentSubagentOverrideConfig, AgentSubagentOverrideState, ParentSubagentOverrideConfig,
};
use bitfun_agent_runtime::agents::{
    resolve_subagent_availability, resolve_subagent_default_enabled, subagent_source_kind,
    ResolvedSubagentAvailability, SubagentOverrideLayers as ResolvedOverrideLayers,
    SubagentOverrideState,
};
fn to_runtime_override_state(state: AgentSubagentOverrideState) -> SubagentOverrideState {
    match state {
        AgentSubagentOverrideState::Enabled => SubagentOverrideState::Enabled,
        AgentSubagentOverrideState::Disabled => SubagentOverrideState::Disabled,
    }
}

fn normalize_parent_agent_id(parent_agent_type: Option<&str>) -> Option<String> {
    parent_agent_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| resolve_mode_config_profile_id(value).into_owned())
}

fn override_for_parent<'a>(
    overrides: &'a AgentSubagentOverrideConfig,
    parent_agent_type: Option<&str>,
) -> Option<&'a ParentSubagentOverrideConfig> {
    let parent_agent_type = normalize_parent_agent_id(parent_agent_type)?;
    overrides.get(&parent_agent_type)
}

fn subagent_override_for_parent(
    overrides: &AgentSubagentOverrideConfig,
    parent_agent_type: Option<&str>,
    subagent_key: &str,
) -> Option<AgentSubagentOverrideState> {
    override_for_parent(overrides, parent_agent_type)
        .and_then(|parent| parent.get(subagent_key).copied())
}

pub(super) fn resolve_default_enabled(entry: &AgentEntry, parent_agent_type: Option<&str>) -> bool {
    resolve_subagent_default_enabled(
        subagent_source_kind(entry.subagent_source),
        &entry.visibility_policy,
        parent_agent_type,
    )
}

fn resolve_override_layers(
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
            project_override: project_overrides
                .and_then(|overrides| {
                    subagent_override_for_parent(overrides, parent_agent_type, &subagent_key)
                })
                .map(to_runtime_override_state),
            user_override: None,
        },
        Some(SubAgentSource::Builtin) | Some(SubAgentSource::User) => ResolvedOverrideLayers {
            project_override: None,
            user_override: subagent_override_for_parent(
                user_overrides,
                parent_agent_type,
                &subagent_key,
            )
            .map(to_runtime_override_state),
        },
        None => ResolvedOverrideLayers::default(),
    }
}

pub(super) fn resolve_availability(
    entry: &AgentEntry,
    parent_agent_type: Option<&str>,
    project_overrides: Option<&AgentSubagentOverrideConfig>,
    user_overrides: &AgentSubagentOverrideConfig,
) -> ResolvedSubagentAvailability {
    let default_enabled = resolve_default_enabled(entry, parent_agent_type);
    let layers =
        resolve_override_layers(entry, parent_agent_type, project_overrides, user_overrides);
    resolve_subagent_availability(
        subagent_source_kind(entry.subagent_source),
        default_enabled,
        layers,
    )
}

pub(super) fn prune_override_config(
    overrides: &mut AgentSubagentOverrideConfig,
    parent_agent_type: &str,
    subagent_key: &str,
) {
    let profile_id = resolve_mode_config_profile_id(parent_agent_type).into_owned();
    if let Some(parent_entry) = overrides.get_mut(&profile_id) {
        parent_entry.remove(subagent_key);
        if parent_entry.is_empty() {
            overrides.remove(&profile_id);
        }
    }
}

pub(super) fn set_override_state(
    overrides: &mut AgentSubagentOverrideConfig,
    parent_agent_type: &str,
    subagent_key: &str,
    state: AgentSubagentOverrideState,
) {
    let profile_id = resolve_mode_config_profile_id(parent_agent_type).into_owned();
    overrides
        .entry(profile_id)
        .or_default()
        .insert(subagent_key.to_string(), state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::agents::definitions::custom::{CustomSubagent, CustomSubagentKind};
    use crate::agentic::agents::registry::types::{AgentCategory, AgentSource};
    use crate::agentic::agents::registry::visibility::SubagentVisibilityPolicy;
    use crate::service::config::types::AgentSubagentOverrideState;
    use std::collections::HashMap;
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
            source: match source {
                SubAgentSource::Builtin => AgentSource::Builtin,
                SubAgentSource::Project => AgentSource::Project,
                SubAgentSource::User => AgentSource::User,
            },
            subagent_source: Some(source),
            agent,
            visibility_policy: SubagentVisibilityPolicy::public(),
            custom_config: None,
        }
    }

    fn overrides(
        parent: &str,
        subagent_key: &str,
        state: AgentSubagentOverrideState,
    ) -> AgentSubagentOverrideConfig {
        let mut all = HashMap::new();
        set_override_state(&mut all, parent, subagent_key, state);
        all
    }

    #[test]
    fn builtin_and_user_subagents_only_use_global_overrides() {
        let builtin_entry = make_entry(SubAgentSource::Builtin, "Explore");
        let builtin_key =
            subagent_key_for(builtin_entry.subagent_source, builtin_entry.agent.as_ref())
                .expect("builtin key");
        let builtin_layers = resolve_override_layers(
            &builtin_entry,
            Some("agentic"),
            Some(&overrides(
                "agentic",
                &builtin_key,
                AgentSubagentOverrideState::Disabled,
            )),
            &overrides("agentic", &builtin_key, AgentSubagentOverrideState::Enabled),
        );
        assert_eq!(builtin_layers.project_override, None);
        assert_eq!(
            builtin_layers.user_override,
            Some(SubagentOverrideState::Enabled)
        );

        let user_entry = make_entry(SubAgentSource::User, "UserScout");
        let user_key = subagent_key_for(user_entry.subagent_source, user_entry.agent.as_ref())
            .expect("user key");
        let user_layers = resolve_override_layers(
            &user_entry,
            Some("agentic"),
            Some(&overrides(
                "agentic",
                &user_key,
                AgentSubagentOverrideState::Disabled,
            )),
            &overrides("agentic", &user_key, AgentSubagentOverrideState::Enabled),
        );
        assert_eq!(user_layers.project_override, None);
        assert_eq!(
            user_layers.user_override,
            Some(SubagentOverrideState::Enabled)
        );
    }

    #[test]
    fn project_subagents_only_use_project_overrides() {
        let entry = make_entry(SubAgentSource::Project, "ProjectScout");
        let key =
            subagent_key_for(entry.subagent_source, entry.agent.as_ref()).expect("project key");
        let layers = resolve_override_layers(
            &entry,
            Some("agentic"),
            Some(&overrides(
                "agentic",
                &key,
                AgentSubagentOverrideState::Disabled,
            )),
            &overrides("agentic", &key, AgentSubagentOverrideState::Enabled),
        );

        assert_eq!(
            layers.project_override,
            Some(SubagentOverrideState::Disabled)
        );
        assert_eq!(layers.user_override, None);
    }
}
