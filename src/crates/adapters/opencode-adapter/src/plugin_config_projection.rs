use openbitfun_product_domains::external_subagents::ExternalSubagentMode;
use openbitfun_product_domains::plugin_capabilities::{
    PluginAgentProjection, PluginCapabilityProjection, PluginContributorIdentity,
    PluginSkillRootContribution, PluginToolRef,
};
use openbitfun_product_domains::tool_permissions::{
    PermissionConstraintLayer, PermissionEffect, PermissionRule,
};
use openbitfun_runtime_ports::{
    HookFunctionContributorOutcome, HookFunctionPluginIdentity, HookFunctionRegistrationBatch,
    HookFunctionToolRegistration,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MAX_AGENT_ID_BYTES: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 4096;
const MAX_PROMPT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_SKILL_ROOTS: usize = 64;
const MIN_AGENT_TEMPERATURE: f64 = 0.0;
const MAX_AGENT_TEMPERATURE: f64 = 2.0;

#[derive(Debug, thiserror::Error)]
pub enum OpenCodePluginConfigProjectionError {
    #[error("{0}")]
    Contribution(String),
    #[error("{0}")]
    Agent(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Skill(String),
}

fn contribution_error(message: impl Into<String>) -> OpenCodePluginConfigProjectionError {
    OpenCodePluginConfigProjectionError::Contribution(message.into())
}

fn agent_error(message: impl Into<String>) -> OpenCodePluginConfigProjectionError {
    OpenCodePluginConfigProjectionError::Agent(message.into())
}

fn permission_error(message: impl Into<String>) -> OpenCodePluginConfigProjectionError {
    OpenCodePluginConfigProjectionError::Permission(message.into())
}

fn skill_error(message: impl Into<String>) -> OpenCodePluginConfigProjectionError {
    OpenCodePluginConfigProjectionError::Skill(message.into())
}

#[derive(Debug)]
struct ConfigContributor {
    plugin: PluginContributorIdentity,
    outcome: ContributorOutcome,
}

#[derive(Debug, Clone)]
struct ConfigContribution {
    plugin: PluginContributorIdentity,
    outcome: ContributorOutcome,
    config: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContributorOutcome {
    Applied,
    Failed,
}

impl From<HookFunctionContributorOutcome> for ContributorOutcome {
    fn from(value: HookFunctionContributorOutcome) -> Self {
        match value {
            HookFunctionContributorOutcome::Applied => Self::Applied,
            HookFunctionContributorOutcome::Failed => Self::Failed,
        }
    }
}

struct ConfigAttribution {
    agent_owners: BTreeMap<String, PluginContributorIdentity>,
    permission_owners: BTreeMap<(String, String), PluginContributorIdentity>,
    skill_owners: BTreeMap<PathBuf, PluginContributorIdentity>,
}

fn identity_component(value: &str) -> String {
    format!("{}:{value}", value.len())
}

fn project_plugin_identity(
    plugin: &HookFunctionPluginIdentity,
) -> Result<PluginContributorIdentity, OpenCodePluginConfigProjectionError> {
    if plugin.spec.trim().is_empty() || plugin.entry.trim().is_empty() {
        return Err(contribution_error(
            "Plugin config contributor identity is incomplete",
        ));
    }
    let id = plugin.id.as_deref().unwrap_or_default();
    let identity_key = format!(
        "{}{}{}{}",
        identity_component(if plugin.id.is_some() { "1" } else { "0" }),
        identity_component(id),
        identity_component(&plugin.spec),
        identity_component(&plugin.entry),
    );
    Ok(PluginContributorIdentity::new(
        format!(
            "{identity_key}{}",
            identity_component(&plugin.index.to_string())
        ),
        format!("{}\n{}\n{}", plugin.spec, plugin.entry, plugin.index),
        plugin.id.clone().unwrap_or_else(|| plugin.spec.clone()),
    ))
}

pub fn project_plugin_tool_ref(
    tool: &HookFunctionToolRegistration,
) -> Result<PluginToolRef, OpenCodePluginConfigProjectionError> {
    let contributor = tool
        .plugin
        .as_ref()
        .ok_or_else(|| contribution_error("Plugin tool identity is missing"))
        .and_then(project_plugin_identity)?;
    validate_tool_id(&tool.id)?;
    Ok(PluginToolRef::new(contributor, tool.id.clone()))
}

pub fn project_plugin_config(
    workspace_root: &Path,
    initial_config: &Map<String, Value>,
    registration_batch: &HookFunctionRegistrationBatch,
) -> Result<PluginCapabilityProjection, OpenCodePluginConfigProjectionError> {
    let contributors = registration_batch
        .config_contributors
        .iter()
        .map(|entry| {
            Ok(ConfigContributor {
                plugin: project_plugin_identity(&entry.plugin)?,
                outcome: entry.outcome.into(),
            })
        })
        .collect::<Result<Vec<_>, OpenCodePluginConfigProjectionError>>()?;
    if contributors.is_empty() {
        return Ok(PluginCapabilityProjection::default());
    }

    let config = &registration_batch.config;
    let contributions = registration_batch
        .config_contributions
        .iter()
        .map(|entry| {
            Ok(ConfigContribution {
                plugin: project_plugin_identity(&entry.plugin)?,
                outcome: entry.outcome.into(),
                config: entry.config.clone(),
            })
        })
        .collect::<Result<Vec<_>, OpenCodePluginConfigProjectionError>>()?;
    let contributions = config_contribution_sequence(&contributions, &contributors, config)?;
    let attribution = attribute_config(initial_config, &contributions, config, workspace_root)?;
    let final_agents = config_object_field(config, "agent")?;
    let plugin_tools = plugin_tool_ids_by_owner(&registration_batch.tools)?;
    let tool_owners = plugin_tools
        .iter()
        .flat_map(|(owner, tools)| tools.iter().cloned().map(|tool| (tool, owner.clone())))
        .collect::<BTreeMap<_, _>>();
    let all_plugin_tools = tool_owners.keys().cloned().collect::<BTreeSet<_>>();

    let mut agents = Vec::new();
    for (logical_id, value) in final_agents {
        let Some(owner) = attribution.agent_owners.get(&logical_id) else {
            continue;
        };
        let definition = value
            .as_object()
            .ok_or_else(|| agent_error(format!("Plugin agent '{logical_id}' must be an object")))?;
        validate_agent_id(&logical_id)?;
        let mode = parse_mode(definition.get("mode"), &logical_id)?;
        let hidden = parse_hidden(definition.get("hidden"), &logical_id)?;
        let temperature = parse_temperature(definition.get("temperature"), &logical_id)?;
        let description = parse_description(definition.get("description"), owner)?;
        let prompt = parse_prompt(definition.get("prompt"), &logical_id)?;
        let mut eligible_tools = plugin_tools.get(owner).cloned().unwrap_or_default();
        if let Some(permission) = definition.get("permission").and_then(Value::as_object) {
            for (tool, effect) in permission {
                if !matches!(effect.as_str(), Some("allow" | "ask")) {
                    continue;
                }
                let Some(tool_owner) = tool_owners.get(tool) else {
                    continue;
                };
                if attribution
                    .permission_owners
                    .get(&(logical_id.clone(), tool.clone()))
                    == Some(tool_owner)
                {
                    eligible_tools.insert(tool.clone());
                }
            }
        }
        let (permission_constraints, denied_plugin_tools) =
            parse_permissions(definition.get("permission"), &all_plugin_tools, &logical_id)?;
        let plugin_tools = eligible_tools
            .into_iter()
            .filter(|tool| !denied_plugin_tools.contains(tool))
            .filter_map(|id| {
                tool_owners
                    .get(&id)
                    .cloned()
                    .map(|contributor| PluginToolRef::new(contributor, id))
            })
            .collect();
        agents.push(PluginAgentProjection {
            contributor: owner.clone(),
            logical_id,
            description,
            prompt,
            mode,
            hidden,
            temperature,
            permission_constraints,
            plugin_tools,
        });
    }

    let mut skill_roots =
        attributed_skill_roots(config, &attribution.skill_owners, workspace_root)?
            .into_values()
            .flatten()
            .collect::<Vec<_>>();
    skill_roots.sort_by_key(|root| root.precedence);

    Ok(PluginCapabilityProjection {
        agents,
        skill_roots,
    })
}

fn config_contribution_sequence(
    contributions: &[ConfigContribution],
    contributors: &[ConfigContributor],
    final_config: &Map<String, Value>,
) -> Result<Vec<ConfigContribution>, OpenCodePluginConfigProjectionError> {
    if contributions.is_empty() {
        if contributors.len() == 1 {
            return Ok(vec![ConfigContribution {
                plugin: contributors[0].plugin.clone(),
                outcome: contributors[0].outcome,
                config: final_config.clone(),
            }]);
        }
        return Err(contribution_error(
            "unsupported_multiple_config_contributors: plugin host did not provide configContributions",
        ));
    }
    if contributions.len() != contributors.len()
        || contributions
            .iter()
            .zip(contributors)
            .any(|(step, contributor)| {
                step.plugin != contributor.plugin || step.outcome != contributor.outcome
            })
    {
        return Err(contribution_error(
            "Plugin config contribution sequence does not match configContributors",
        ));
    }
    if contributions.last().map(|step| &step.config) != Some(final_config) {
        return Err(contribution_error(
            "Plugin config contribution sequence does not end at the final config",
        ));
    }
    Ok(contributions.to_vec())
}

fn attribute_config(
    initial_config: &Map<String, Value>,
    contributions: &[ConfigContribution],
    final_config: &Map<String, Value>,
    workspace_root: &Path,
) -> Result<ConfigAttribution, OpenCodePluginConfigProjectionError> {
    let mut previous = initial_config;
    let mut agent_owners = BTreeMap::new();
    let mut permission_owners = BTreeMap::new();
    let mut skill_owners = BTreeMap::new();
    let mut previous_skills = skill_paths(initial_config)?
        .into_iter()
        .map(|path| normalized_skill_path_identity(&path, workspace_root))
        .collect::<Result<BTreeSet<_>, _>>()?;

    for contribution in contributions {
        let before_agents = config_object_field(previous, "agent")?;
        let after_agents = config_object_field(&contribution.config, "agent")?;
        let agent_ids = before_agents
            .keys()
            .chain(after_agents.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for agent_id in agent_ids {
            let before_agent = before_agents.get(&agent_id);
            let after_agent = after_agents.get(&agent_id);
            if before_agent != after_agent && after_agent.is_some() {
                agent_owners
                    .entry(agent_id.clone())
                    .or_insert_with(|| contribution.plugin.clone());
            } else if after_agent.is_none() {
                agent_owners.remove(&agent_id);
            }

            let before_permissions = agent_permission_object(before_agent, &agent_id)?;
            let after_permissions = agent_permission_object(after_agent, &agent_id)?;
            let permission_keys = before_permissions
                .keys()
                .chain(after_permissions.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            for permission in permission_keys {
                if before_permissions.get(&permission) == after_permissions.get(&permission) {
                    continue;
                }
                let key = (agent_id.clone(), permission.clone());
                if after_permissions.contains_key(&permission) {
                    permission_owners.insert(key, contribution.plugin.clone());
                } else {
                    permission_owners.remove(&key);
                }
            }
        }

        let next_skills = skill_paths(&contribution.config)?
            .into_iter()
            .map(|path| normalized_skill_path_identity(&path, workspace_root))
            .collect::<Result<BTreeSet<_>, _>>()?;
        skill_owners.retain(|path, _| next_skills.contains(path));
        for added in next_skills.difference(&previous_skills) {
            skill_owners.insert(added.clone(), contribution.plugin.clone());
        }
        previous_skills = next_skills;
        previous = &contribution.config;
    }
    if previous != final_config {
        return Err(contribution_error(
            "Plugin config attribution did not reach the final config",
        ));
    }
    Ok(ConfigAttribution {
        agent_owners,
        permission_owners,
        skill_owners,
    })
}

fn agent_permission_object(
    agent: Option<&Value>,
    agent_id: &str,
) -> Result<Map<String, Value>, OpenCodePluginConfigProjectionError> {
    let Some(agent) = agent else {
        return Ok(Map::new());
    };
    let agent = agent
        .as_object()
        .ok_or_else(|| agent_error(format!("Plugin agent '{agent_id}' must be an object")))?;
    match agent.get("permission") {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(permission)) => Ok(permission.clone()),
        Some(_) => Err(permission_error(format!(
            "Plugin agent '{agent_id}' permission must be an object"
        ))),
    }
}

fn validate_tool_id(id: &str) -> Result<(), OpenCodePluginConfigProjectionError> {
    if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
        return Err(contribution_error("Plugin tool id is invalid"));
    }
    Ok(())
}

fn validate_agent_id(id: &str) -> Result<(), OpenCodePluginConfigProjectionError> {
    if id.trim() != id
        || id.is_empty()
        || id.len() > MAX_AGENT_ID_BYTES
        || id.chars().any(char::is_control)
    {
        return Err(agent_error(format!("Invalid plugin agent id '{id}'")));
    }
    Ok(())
}

fn parse_mode(
    value: Option<&Value>,
    id: &str,
) -> Result<ExternalSubagentMode, OpenCodePluginConfigProjectionError> {
    match value.and_then(Value::as_str).unwrap_or("all") {
        "primary" => Ok(ExternalSubagentMode::Primary),
        "subagent" => Ok(ExternalSubagentMode::Subagent),
        "all" => Ok(ExternalSubagentMode::All),
        other => Err(agent_error(format!(
            "Plugin agent '{id}' has unsupported mode '{other}'"
        ))),
    }
}

fn parse_hidden(
    value: Option<&Value>,
    id: &str,
) -> Result<bool, OpenCodePluginConfigProjectionError> {
    match value {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(hidden)) => Ok(*hidden),
        Some(_) => Err(agent_error(format!(
            "Plugin agent '{id}' hidden must be a boolean"
        ))),
    }
}

fn parse_temperature(
    value: Option<&Value>,
    id: &str,
) -> Result<Option<f64>, OpenCodePluginConfigProjectionError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let temperature = value
        .as_f64()
        .ok_or_else(|| agent_error(format!("Plugin agent '{id}' temperature must be a number")))?;
    if !temperature.is_finite()
        || !(MIN_AGENT_TEMPERATURE..=MAX_AGENT_TEMPERATURE).contains(&temperature)
    {
        return Err(agent_error(format!(
            "Plugin agent '{id}' temperature must be between {MIN_AGENT_TEMPERATURE} and {MAX_AGENT_TEMPERATURE}"
        )));
    }
    Ok(Some(temperature))
}

fn parse_description(
    value: Option<&Value>,
    plugin: &PluginContributorIdentity,
) -> Result<String, OpenCodePluginConfigProjectionError> {
    let description = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Agent contributed by {}", plugin.label()));
    if description.len() > MAX_DESCRIPTION_BYTES {
        return Err(agent_error(
            "Plugin agent description exceeds the size limit",
        ));
    }
    Ok(description)
}

fn parse_prompt(
    value: Option<&Value>,
    id: &str,
) -> Result<String, OpenCodePluginConfigProjectionError> {
    let prompt = match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(_) => {
            return Err(agent_error(format!(
                "Plugin agent '{id}' prompt must be a string"
            )))
        }
    };
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(agent_error(format!(
            "Plugin agent '{id}' prompt exceeds the size limit"
        )));
    }
    Ok(prompt)
}

fn plugin_tool_ids_by_owner(
    tools: &[HookFunctionToolRegistration],
) -> Result<
    BTreeMap<PluginContributorIdentity, BTreeSet<String>>,
    OpenCodePluginConfigProjectionError,
> {
    let mut result = BTreeMap::<PluginContributorIdentity, BTreeSet<String>>::new();
    for tool in tools {
        let tool_ref = project_plugin_tool_ref(tool)?;
        result
            .entry(tool_ref.contributor().clone())
            .or_default()
            .insert(tool_ref.id().to_string());
    }
    Ok(result)
}

fn parse_permissions(
    value: Option<&Value>,
    plugin_tools: &BTreeSet<String>,
    agent_id: &str,
) -> Result<(PermissionConstraintLayer, BTreeSet<String>), OpenCodePluginConfigProjectionError> {
    let Some(value) = value else {
        return Ok((PermissionConstraintLayer::default(), BTreeSet::new()));
    };
    let permissions = value.as_object().ok_or_else(|| {
        permission_error(format!(
            "Plugin agent '{agent_id}' permission must be an object"
        ))
    })?;
    let known_native = [
        "bash",
        "read",
        "edit",
        "task",
        "skill",
        "webfetch",
        "websearch",
        "git",
        "external_directory",
    ];
    let mut rules = Vec::new();
    let mut denied = BTreeSet::new();
    for (key, value) in permissions {
        let effect = match value.as_str() {
            Some("allow") => PermissionEffect::Allow,
            Some("ask") => PermissionEffect::Ask,
            Some("deny") => PermissionEffect::Deny,
            _ => {
                return Err(permission_error(format!(
                    "Plugin agent '{agent_id}' permission '{key}' is invalid"
                )))
            }
        };
        if plugin_tools.contains(key) {
            rules.push(PermissionRule::new("custom_tool", key, effect));
            if effect == PermissionEffect::Deny {
                denied.insert(key.clone());
            }
        } else if known_native.contains(&key.as_str()) {
            rules.push(PermissionRule::new(key, "*", effect));
        } else if effect == PermissionEffect::Allow {
            log::warn!(
                "Ignoring unsupported OpenCode plugin permission allow rule: agent_id={}, permission_action={}",
                agent_id,
                key
            );
        } else {
            return Err(permission_error(format!("Plugin agent '{agent_id}' permission '{key}' has no compatible action or plugin tool")));
        }
    }
    Ok((PermissionConstraintLayer::new(rules), denied))
}

fn config_object_field(
    config: &Map<String, Value>,
    field: &str,
) -> Result<Map<String, Value>, OpenCodePluginConfigProjectionError> {
    match config.get(field) {
        None => Ok(Map::new()),
        Some(Value::Object(value)) => Ok(value.clone()),
        Some(_) => Err(agent_error(format!(
            "Plugin config '{field}' must be an object"
        ))),
    }
}

fn skill_paths(
    config: &Map<String, Value>,
) -> Result<Vec<PathBuf>, OpenCodePluginConfigProjectionError> {
    let Some(skills) = config.get("skills") else {
        return Ok(Vec::new());
    };
    let skills = skills
        .as_object()
        .ok_or_else(|| skill_error("Plugin config 'skills' must be an object"))?;
    let Some(paths) = skills.get("paths") else {
        return Ok(Vec::new());
    };
    let paths = paths
        .as_array()
        .ok_or_else(|| skill_error("Plugin config 'skills.paths' must be an array"))?;
    paths
        .iter()
        .map(|path| {
            path.as_str()
                .map(PathBuf::from)
                .ok_or_else(|| skill_error("Plugin config 'skills.paths' entries must be strings"))
        })
        .collect()
}

fn resolve_plugin_skill_path(
    path: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, OpenCodePluginConfigProjectionError> {
    let value = path.to_string_lossy();
    let value = value.trim();
    if value.is_empty() || value.contains('\0') {
        return Err(skill_error("Plugin skill root path is invalid"));
    }
    if let Some(relative) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return dirs::home_dir()
            .map(|home| home.join(relative))
            .ok_or_else(|| {
                skill_error("Plugin skill root uses '~/' but the home directory is unavailable")
            });
    }
    let path = PathBuf::from(value);
    Ok(if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    })
}

fn normalized_skill_path_identity(
    path: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, OpenCodePluginConfigProjectionError> {
    let resolved = resolve_plugin_skill_path(path, workspace_root)?;
    Ok(dunce::canonicalize(&resolved).unwrap_or(resolved))
}

fn attributed_skill_roots(
    final_config: &Map<String, Value>,
    owners: &BTreeMap<PathBuf, PluginContributorIdentity>,
    workspace_root: &Path,
) -> Result<
    BTreeMap<PluginContributorIdentity, Vec<PluginSkillRootContribution>>,
    OpenCodePluginConfigProjectionError,
> {
    let mut seen = BTreeSet::new();
    let mut roots = BTreeMap::<PluginContributorIdentity, Vec<PluginSkillRootContribution>>::new();
    for path in skill_paths(final_config)? {
        let path = resolve_plugin_skill_path(&path, workspace_root)?;
        let identity = normalized_skill_path_identity(&path, workspace_root)?;
        let Some(owner) = owners.get(&identity) else {
            continue;
        };
        if !seen.insert(identity) {
            continue;
        }
        if seen.len() > MAX_PLUGIN_SKILL_ROOTS {
            return Err(skill_error("Plugin skill root count exceeds the limit"));
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            log::warn!("Skipping unavailable OpenCode plugin skill root");
            continue;
        };
        if openbitfun_services_core::bounded_fs::is_symlink_or_reparse(&metadata)
            || !metadata.is_dir()
        {
            log::warn!("Skipping invalid OpenCode plugin skill root");
            continue;
        }
        let Ok(canonical) = dunce::canonicalize(&path) else {
            log::warn!("Skipping OpenCode plugin skill root that cannot be canonicalized");
            continue;
        };
        roots
            .entry(owner.clone())
            .or_default()
            .push(PluginSkillRootContribution {
                path: canonical,
                precedence: seen.len() - 1,
            });
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_runtime_ports::{
        HookFunctionConfigContribution, HookFunctionConfigContributor, HookFunctionGeneration,
        HookFunctionRegistrationBatch,
    };
    use serde_json::json;

    fn plugin(id: &str) -> HookFunctionPluginIdentity {
        HookFunctionPluginIdentity {
            id: Some(id.to_string()),
            spec: format!("D:/plugins/{id}"),
            entry: format!("D:/plugins/{id}/index.js"),
            index: 0,
        }
    }

    fn batch(
        config: Map<String, Value>,
        contributors: Vec<HookFunctionConfigContributor>,
        contributions: Vec<HookFunctionConfigContribution>,
        tools: Vec<HookFunctionToolRegistration>,
    ) -> HookFunctionRegistrationBatch {
        HookFunctionRegistrationBatch {
            generation: HookFunctionGeneration {
                instance_id: "instance".to_string(),
                generation_key: "generation".to_string(),
                revision: "revision".to_string(),
            },
            config,
            config_contributors: contributors,
            config_contributions: contributions,
            diagnostics: Vec::new(),
            hooks: Vec::new(),
            tools,
        }
    }

    fn contributor(plugin: HookFunctionPluginIdentity) -> HookFunctionConfigContributor {
        HookFunctionConfigContributor {
            plugin,
            outcome: HookFunctionContributorOutcome::Applied,
        }
    }

    fn contribution(
        plugin: HookFunctionPluginIdentity,
        config: Map<String, Value>,
    ) -> HookFunctionConfigContribution {
        HookFunctionConfigContribution {
            plugin,
            outcome: HookFunctionContributorOutcome::Applied,
            config,
        }
    }

    fn tool(plugin: HookFunctionPluginIdentity, id: &str) -> HookFunctionToolRegistration {
        HookFunctionToolRegistration {
            registration_id: format!("registration-{id}"),
            id: id.to_string(),
            plugin: Some(plugin),
            description: String::new(),
            parameters: json!({"type": "object"}),
        }
    }

    #[test]
    fn projects_owner_scoped_tool_reference_for_publication() {
        let owner = plugin("first");
        let projected =
            project_plugin_tool_ref(&tool(owner, "build_project")).expect("plugin tool reference");

        assert_eq!(projected.contributor().label(), "first");
        assert_eq!(projected.id(), "build_project");
    }

    #[test]
    fn supports_legacy_single_contributor_without_contribution_snapshots() {
        let owner = plugin("first");
        let config = json!({"agent": {"build": {"prompt": "Build"}}})
            .as_object()
            .unwrap()
            .clone();
        let projection = project_plugin_config(
            Path::new("C:/workspace"),
            &Map::new(),
            &batch(config, vec![contributor(owner)], Vec::new(), Vec::new()),
        )
        .expect("legacy single contributor");

        assert_eq!(projection.agents.len(), 1);
        assert_eq!(projection.agents[0].logical_id, "build");
        assert_eq!(projection.agents[0].contributor.label(), "first");
        assert_eq!(
            projection.agents[0].contributor.behavior_key(),
            "D:/plugins/first\nD:/plugins/first/index.js\n0"
        );
    }

    #[test]
    fn rejects_legacy_multiple_contributors_without_contribution_snapshots() {
        let first = plugin("first");
        let second = plugin("second");
        let config = json!({"agent": {"build": {"prompt": "Build"}}})
            .as_object()
            .unwrap()
            .clone();
        let error = project_plugin_config(
            Path::new("C:/workspace"),
            &Map::new(),
            &batch(
                config,
                vec![contributor(first), contributor(second)],
                Vec::new(),
                Vec::new(),
            ),
        )
        .expect_err("multiple contributors require snapshots");

        assert!(matches!(
            &error,
            OpenCodePluginConfigProjectionError::Contribution(_)
        ));
        assert!(error
            .to_string()
            .contains("unsupported_multiple_config_contributors"));
    }

    #[test]
    fn rejects_inconsistent_contribution_sequences() {
        let first = plugin("first");
        let second = plugin("second");
        let after_first = json!({"agent": {"build": {"prompt": "First"}}})
            .as_object()
            .unwrap()
            .clone();
        let final_config = json!({"agent": {"build": {"prompt": "Second"}}})
            .as_object()
            .unwrap()
            .clone();
        let registration = batch(
            final_config.clone(),
            vec![contributor(first.clone()), contributor(second.clone())],
            vec![
                contribution(second, after_first),
                contribution(first, final_config),
            ],
            Vec::new(),
        );

        let error = project_plugin_config(Path::new("C:/workspace"), &Map::new(), &registration)
            .expect_err("contributor order must match");
        assert!(error
            .to_string()
            .contains("does not match configContributors"));
    }

    #[test]
    fn isolates_agent_tools_across_multiple_contributors() {
        let first = plugin("first");
        let second = plugin("second");
        let after_first = json!({
            "agent": {
                "build": {
                    "prompt": "Build",
                    "permission": {"first_tool": "allow"}
                }
            }
        })
        .as_object()
        .unwrap()
        .clone();
        let final_config = json!({
            "agent": {
                "build": {
                    "prompt": "Build",
                    "description": "Refined by second",
                    "permission": {
                        "first_tool": "allow",
                        "second_tool": "allow"
                    }
                },
                "plan": {
                    "prompt": "Plan",
                    "mode": "subagent",
                    "permission": {"second_tool": "allow"}
                }
            }
        })
        .as_object()
        .unwrap()
        .clone();
        let registration = batch(
            final_config.clone(),
            vec![contributor(first.clone()), contributor(second.clone())],
            vec![
                contribution(first.clone(), after_first),
                contribution(second.clone(), final_config),
            ],
            vec![tool(first, "first_tool"), tool(second, "second_tool")],
        );

        let projection =
            project_plugin_config(Path::new("C:/workspace"), &Map::new(), &registration)
                .expect("multiple contributor projection");
        let build = projection
            .agents
            .iter()
            .find(|agent| agent.logical_id == "build")
            .unwrap();
        let plan = projection
            .agents
            .iter()
            .find(|agent| agent.logical_id == "plan")
            .unwrap();

        assert_eq!(build.contributor.label(), "first");
        assert_eq!(
            build
                .plugin_tools
                .iter()
                .map(PluginToolRef::id)
                .collect::<Vec<_>>(),
            vec!["first_tool", "second_tool"]
        );
        assert_eq!(plan.contributor.label(), "second");
        assert_eq!(
            plan.plugin_tools
                .iter()
                .map(PluginToolRef::id)
                .collect::<Vec<_>>(),
            vec!["second_tool"]
        );
    }

    #[test]
    fn reattributes_deleted_and_recreated_agents_and_permissions() {
        let first = project_plugin_identity(&plugin("first")).unwrap();
        let second = project_plugin_identity(&plugin("second")).unwrap();
        let initial = json!({"agent": {"build": {"prompt": "native"}}})
            .as_object()
            .unwrap()
            .clone();
        let after_first = json!({"agent": {"build": {
            "prompt": "first",
            "permission": {"first_tool": "allow"}
        }}})
        .as_object()
        .unwrap()
        .clone();
        let after_delete = json!({"agent": {}}).as_object().unwrap().clone();
        let final_config = json!({"agent": {"build": {
            "prompt": "second",
            "permission": {"second_tool": "ask"}
        }}})
        .as_object()
        .unwrap()
        .clone();
        let contributions = vec![
            ConfigContribution {
                plugin: first,
                outcome: ContributorOutcome::Applied,
                config: after_first,
            },
            ConfigContribution {
                plugin: second.clone(),
                outcome: ContributorOutcome::Applied,
                config: after_delete,
            },
            ConfigContribution {
                plugin: second.clone(),
                outcome: ContributorOutcome::Applied,
                config: final_config.clone(),
            },
        ];

        let attribution = attribute_config(
            &initial,
            &contributions,
            &final_config,
            Path::new("C:/workspace"),
        )
        .expect("agent attribution");
        assert_eq!(attribution.agent_owners.get("build"), Some(&second));
        assert_eq!(
            attribution
                .permission_owners
                .get(&("build".to_string(), "second_tool".to_string())),
            Some(&second)
        );
        assert!(!attribution
            .permission_owners
            .contains_key(&("build".to_string(), "first_tool".to_string())));
    }

    #[test]
    fn unknown_allow_is_non_expanding_but_unknown_restrictions_fail_closed() {
        let plugin_tools = BTreeSet::new();
        let permissions = json!({"future_action": "allow"});
        let (constraints, denied) =
            parse_permissions(Some(&permissions), &plugin_tools, "build").expect("allow");
        assert!(constraints.rules().is_empty());
        assert!(denied.is_empty());

        for effect in ["ask", "deny"] {
            let permissions = json!({"future_action": effect});
            let error = parse_permissions(Some(&permissions), &plugin_tools, "build")
                .expect_err("unknown restriction cannot be enforced");
            assert!(matches!(
                &error,
                OpenCodePluginConfigProjectionError::Permission(_)
            ));
            assert!(error.to_string().contains("has no compatible action"));
        }
    }

    #[test]
    fn parses_hidden_and_temperature_with_safe_defaults_and_bounds() {
        assert!(!parse_hidden(None, "agent").expect("hidden defaults to false"));
        assert!(parse_hidden(Some(&json!(true)), "agent").expect("boolean hidden"));
        assert!(!parse_hidden(Some(&json!(null)), "agent").expect("null hidden default"));
        assert!(parse_hidden(Some(&json!("true")), "agent").is_err());

        assert_eq!(parse_temperature(None, "agent").unwrap(), None);
        assert_eq!(
            parse_temperature(Some(&json!(0.2)), "agent").unwrap(),
            Some(0.2)
        );
        assert_eq!(
            parse_temperature(Some(&json!(2)), "agent").unwrap(),
            Some(2.0)
        );
        for value in [json!(-0.1), json!(2.1), json!("0.2")] {
            assert!(parse_temperature(Some(&value), "agent").is_err());
        }
    }

    #[test]
    fn rejects_malformed_agent_and_skill_shapes() {
        let owner = plugin("first");
        let config = json!({"agent": []}).as_object().unwrap().clone();
        let error = project_plugin_config(
            Path::new("C:/workspace"),
            &Map::new(),
            &batch(
                config,
                vec![contributor(owner.clone())],
                Vec::new(),
                Vec::new(),
            ),
        )
        .expect_err("agent must be an object");
        assert!(matches!(
            &error,
            OpenCodePluginConfigProjectionError::Agent(_)
        ));
        assert!(error
            .to_string()
            .contains("config 'agent' must be an object"));

        for malformed in [json!({"paths": "not-an-array"}), json!({"paths": [42]})] {
            let config = json!({"skills": malformed}).as_object().unwrap().clone();
            let error = project_plugin_config(
                Path::new("C:/workspace"),
                &Map::new(),
                &batch(
                    config,
                    vec![contributor(owner.clone())],
                    Vec::new(),
                    Vec::new(),
                ),
            )
            .expect_err("malformed skill paths must fail");
            assert!(matches!(
                &error,
                OpenCodePluginConfigProjectionError::Skill(_)
            ));
            assert!(error.to_string().contains("skills.paths"));
        }
    }

    #[test]
    fn attributes_skill_additions_without_republishing_initial_roots() {
        let workspace = tempfile::tempdir().expect("workspace");
        let initial_root = tempfile::tempdir().expect("initial root");
        let plugin_root = tempfile::tempdir().expect("plugin root");
        let owner = plugin("first");
        let initial = json!({"skills": {"paths": [initial_root.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let config = json!({"skills": {"paths": [initial_root.path(), plugin_root.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let registration = batch(
            config.clone(),
            vec![contributor(owner.clone())],
            vec![contribution(owner, config)],
            Vec::new(),
        );

        let projection = project_plugin_config(workspace.path(), &initial, &registration)
            .expect("skill projection");
        assert_eq!(projection.skill_roots.len(), 1);
        assert_eq!(
            projection.skill_roots[0].path,
            dunce::canonicalize(plugin_root.path()).unwrap()
        );
        assert_eq!(projection.skill_roots[0].precedence, 0);
    }

    #[test]
    fn canonical_skill_identity_does_not_republish_an_initial_root() {
        let directory = tempfile::tempdir().expect("skill root");
        let canonical = dunce::canonicalize(directory.path()).expect("canonical path");
        let aliased = canonical.join(".");
        let initial = json!({"skills": {"paths": [aliased]}})
            .as_object()
            .unwrap()
            .clone();
        let final_config = json!({"skills": {"paths": [canonical]}})
            .as_object()
            .unwrap()
            .clone();
        let owner = plugin("first");
        let registration = batch(
            final_config.clone(),
            vec![contributor(owner.clone())],
            vec![contribution(owner, final_config)],
            Vec::new(),
        );

        let projection = project_plugin_config(directory.path(), &initial, &registration)
            .expect("canonical skill attribution");
        assert!(projection.skill_roots.is_empty());
    }

    #[test]
    fn skill_reordering_and_removal_preserve_remaining_owner() {
        let workspace = tempfile::tempdir().expect("workspace");
        let base = tempfile::tempdir().expect("base skill root");
        let first_root = tempfile::tempdir().expect("first plugin skill root");
        let second_root = tempfile::tempdir().expect("second plugin skill root");
        let first = project_plugin_identity(&plugin("first")).unwrap();
        let second = project_plugin_identity(&plugin("second")).unwrap();
        let initial = json!({"skills": {"paths": [base.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let after_first = json!({"skills": {"paths": [base.path(), first_root.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let final_config = json!({"skills": {"paths": [second_root.path(), base.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let contributions = vec![
            ConfigContribution {
                plugin: first,
                outcome: ContributorOutcome::Applied,
                config: after_first,
            },
            ConfigContribution {
                plugin: second.clone(),
                outcome: ContributorOutcome::Applied,
                config: final_config.clone(),
            },
        ];

        let attribution =
            attribute_config(&initial, &contributions, &final_config, workspace.path())
                .expect("skill removal attribution");
        assert!(!attribution.skill_owners.contains_key(
            &normalized_skill_path_identity(first_root.path(), workspace.path()).unwrap()
        ));
        assert_eq!(
            attribution.skill_owners.get(
                &normalized_skill_path_identity(second_root.path(), workspace.path()).unwrap()
            ),
            Some(&second)
        );
    }

    #[test]
    fn resolves_relative_skill_roots_and_skips_unavailable_roots() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_root = workspace.path().join("skills");
        std::fs::create_dir(&skill_root).expect("skill root");
        let owner = plugin("first");
        let config = json!({"skills": {"paths": ["./skills", "./missing"]}})
            .as_object()
            .unwrap()
            .clone();
        let registration = batch(
            config.clone(),
            vec![contributor(owner.clone())],
            vec![contribution(owner, config)],
            Vec::new(),
        );

        let projection = project_plugin_config(workspace.path(), &Map::new(), &registration)
            .expect("unavailable roots are isolated");
        assert_eq!(projection.skill_roots.len(), 1);
        assert_eq!(
            projection.skill_roots[0].path,
            dunce::canonicalize(skill_root).unwrap()
        );
    }

    #[test]
    fn resolves_home_relative_skill_roots() {
        let workspace = tempfile::tempdir().expect("workspace");
        let home = dirs::home_dir().expect("home directory");

        assert_eq!(
            resolve_plugin_skill_path(Path::new("~/skills"), workspace.path()).unwrap(),
            home.join("skills")
        );
    }
}
