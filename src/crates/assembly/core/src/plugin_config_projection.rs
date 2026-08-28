use crate::agentic::agents::{
    external_subagent_runtime_key, get_agent_registry, shared_coding_mode_tools, ExploreAgent,
    ExternalProvidedAgent, ExternalSubagentModelBinding, ExternalSubagentRegistration,
    ExternalSubagentRoute,
};
use bitfun_product_domains::external_sources::EcosystemId;
use bitfun_product_domains::external_subagents::ExternalSubagentMode;
use bitfun_runtime_ports::{
    HookFunctionContributorOutcome, HookFunctionPluginIdentity, HookFunctionRegistrationBatch,
    HookFunctionToolRegistration, PermissionConstraintLayer, PermissionEffect, PermissionRule,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

const MAX_AGENT_ID_BYTES: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 4096;
const MAX_PROMPT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_SKILL_ROOTS: usize = 64;
const MIN_AGENT_TEMPERATURE: f64 = 0.0;
const MAX_AGENT_TEMPERATURE: f64 = 2.0;
const OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER: &str = "opencode-plugin-config";

pub(crate) fn is_plugin_agent_runtime_key(runtime_agent_key: &str) -> bool {
    runtime_agent_key.starts_with("external_subagent_runtime:opencode-plugin:")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PluginIdentity {
    id: Option<String>,
    spec: String,
    entry: String,
    index: usize,
}

impl From<&HookFunctionPluginIdentity> for PluginIdentity {
    fn from(value: &HookFunctionPluginIdentity) -> Self {
        Self {
            id: value.id.clone(),
            spec: value.spec.clone(),
            entry: value.entry.clone(),
            index: value.index,
        }
    }
}

impl PluginIdentity {
    fn stable_key(&self) -> String {
        format!("{}\n{}\n{}", self.spec, self.entry, self.index)
    }

    fn label(&self) -> String {
        self.id.clone().unwrap_or_else(|| self.spec.clone())
    }
}

#[derive(Debug)]
struct ConfigContributor {
    plugin: PluginIdentity,
    outcome: ContributorOutcome,
}

#[derive(Debug, Clone)]
struct ConfigContribution {
    plugin: PluginIdentity,
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

#[derive(Debug, Clone)]
pub(crate) struct PluginSkillRootContribution {
    pub(crate) path: PathBuf,
    pub(crate) precedence: usize,
}

#[derive(Debug, Clone)]
struct PublishedSkillGeneration {
    generation_key: String,
    roots_by_runtime_agent: BTreeMap<String, Vec<PluginSkillRootContribution>>,
}

fn skill_generations() -> &'static RwLock<HashMap<PathBuf, PublishedSkillGeneration>> {
    static GENERATIONS: OnceLock<RwLock<HashMap<PathBuf, PublishedSkillGeneration>>> =
        OnceLock::new();
    GENERATIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) struct PluginConfigProjectionPlan {
    workspace_root: PathBuf,
    generation_key: String,
    registrations: Vec<ExternalSubagentRegistration>,
    routes: BTreeMap<String, ExternalSubagentRoute>,
    runtime_agent_keys: BTreeSet<String>,
    skill_roots_by_runtime_agent: BTreeMap<String, Vec<PluginSkillRootContribution>>,
    tool_runtime_agent_keys: BTreeMap<(PluginIdentity, String), BTreeSet<String>>,
}

impl PluginConfigProjectionPlan {
    pub(crate) fn empty(workspace_root: &Path, generation_key: &str) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            generation_key: generation_key.to_string(),
            registrations: Vec::new(),
            routes: BTreeMap::new(),
            runtime_agent_keys: BTreeSet::new(),
            skill_roots_by_runtime_agent: BTreeMap::new(),
            tool_runtime_agent_keys: BTreeMap::new(),
        }
    }

    pub(crate) fn agent_runtime_keys(&self) -> BTreeSet<String> {
        self.runtime_agent_keys.clone()
    }

    pub(crate) fn allowed_runtime_agent_keys_for_tool(
        &self,
        tool: &HookFunctionToolRegistration,
    ) -> crate::BitFunResult<BTreeSet<String>> {
        let plugin = tool
            .plugin
            .as_ref()
            .map(PluginIdentity::from)
            .ok_or_else(|| {
                crate::BitFunError::Validation("Plugin tool identity is missing".to_string())
            })?;
        Ok(self
            .tool_runtime_agent_keys
            .get(&(plugin, tool.id.clone()))
            .cloned()
            .unwrap_or_default())
    }

    pub(crate) fn commit(self) {
        get_agent_registry().replace_external_subagent_route_overlay(
            &self.workspace_root,
            OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER,
            self.registrations,
            self.routes,
        );
        let mut generations = skill_generations()
            .write()
            .expect("plugin skill generation lock poisoned");
        generations.insert(
            self.workspace_root,
            PublishedSkillGeneration {
                generation_key: self.generation_key,
                roots_by_runtime_agent: self.skill_roots_by_runtime_agent,
            },
        );
    }
}

pub(crate) fn release_workspace(workspace_root: &Path) {
    get_agent_registry().release_external_subagent_route_overlay(
        workspace_root,
        OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER,
    );
    skill_generations()
        .write()
        .expect("plugin skill generation lock poisoned")
        .remove(workspace_root);
}

pub(crate) fn skill_roots_for_agent(
    workspace_root: Option<&Path>,
    runtime_agent_key: Option<&str>,
) -> Vec<PluginSkillRootContribution> {
    let (Some(workspace_root), Some(runtime_agent_key)) = (workspace_root, runtime_agent_key)
    else {
        return Vec::new();
    };
    let workspace_root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    skill_generations()
        .read()
        .expect("plugin skill generation lock poisoned")
        .get(&workspace_root)
        .and_then(|generation| generation.roots_by_runtime_agent.get(runtime_agent_key))
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn prepare(
    workspace_root: &Path,
    generation_key: &str,
    initial_config: &Map<String, Value>,
    registration_batch: &HookFunctionRegistrationBatch,
) -> crate::BitFunResult<PluginConfigProjectionPlan> {
    let contributors = registration_batch
        .config_contributors
        .iter()
        .map(|entry| ConfigContributor {
            plugin: PluginIdentity::from(&entry.plugin),
            outcome: entry.outcome.into(),
        })
        .collect::<Vec<_>>();
    if contributors.is_empty() {
        return Ok(PluginConfigProjectionPlan::empty(
            workspace_root,
            generation_key,
        ));
    }
    let config = &registration_batch.config;
    let contributions = registration_batch
        .config_contributions
        .iter()
        .map(|entry| ConfigContribution {
            plugin: PluginIdentity::from(&entry.plugin),
            outcome: entry.outcome.into(),
            config: entry.config.clone(),
        })
        .collect::<Vec<_>>();
    let contributions = config_contribution_sequence(&contributions, &contributors, config)?;
    let attribution = attribute_config(initial_config, &contributions, config)?;
    let final_agents = config_object_field(config, "agent")?;
    let plugin_tools = plugin_tool_ids_by_owner(&registration_batch.tools)?;
    let tool_owners = plugin_tools
        .iter()
        .flat_map(|(owner, tools)| tools.iter().cloned().map(|tool| (tool, owner.clone())))
        .collect::<BTreeMap<_, _>>();
    let all_plugin_tools = tool_owners.keys().cloned().collect::<BTreeSet<_>>();

    let mut registrations = Vec::new();
    let mut routes = BTreeMap::new();
    let mut runtime_agent_keys = BTreeSet::new();
    let mut runtime_agent_keys_by_plugin = BTreeMap::<PluginIdentity, BTreeSet<String>>::new();
    let mut tool_runtime_agent_keys = BTreeMap::<(PluginIdentity, String), BTreeSet<String>>::new();
    for (logical_id, value) in final_agents {
        let Some(owner) = attribution.agent_owners.get(&logical_id) else {
            continue;
        };
        let definition = value.as_object().ok_or_else(|| {
            crate::BitFunError::Validation(format!("Plugin agent '{logical_id}' must be an object"))
        })?;
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
        let mut tools = native_tool_baseline(&logical_id, mode, workspace_root);
        let permitted_plugin_tools = eligible_tools
            .iter()
            .filter(|tool| !denied_plugin_tools.contains(*tool))
            .cloned()
            .collect::<Vec<_>>();
        tools.extend(permitted_plugin_tools.iter().cloned());
        // A plugin Tool intentionally shadows a same-name native candidate for
        // this plugin Agent. Remove the earlier entry before the final stable
        // de-duplication so the manifest still contains one model-facing name.
        for plugin_tool in &permitted_plugin_tools {
            if let Some(position) = tools.iter().position(|tool| tool == plugin_tool) {
                tools.remove(position);
                tools.push(plugin_tool.clone());
            }
        }
        tools.sort();
        tools.dedup();

        let mut hasher = Sha256::new();
        hasher.update(generation_key.as_bytes());
        hasher.update([0]);
        hasher.update(owner.stable_key().as_bytes());
        hasher.update([0]);
        hasher.update(logical_id.as_bytes());
        hasher.update([0]);
        hasher.update([u8::from(hidden)]);
        hasher.update([0]);
        if let Some(temperature) = temperature {
            hasher.update(temperature.to_bits().to_le_bytes());
        } else {
            hasher.update([0xff]);
        }
        let digest = hex::encode(hasher.finalize());
        let runtime_key = external_subagent_runtime_key(&format!("opencode-plugin:{digest}"));
        let behavior_version = format!("sha256:{digest}");
        let agent = Arc::new(ExternalProvidedAgent::new(
            runtime_key.clone(),
            logical_id.clone(),
            description,
            prompt,
            tools,
            permission_constraints,
            temperature,
            false,
            behavior_version,
        ));
        registrations.push(ExternalSubagentRegistration {
            runtime_key: runtime_key.clone(),
            logical_id: logical_id.clone(),
            route_key: format!(
                "opencode:{}:{}",
                hex::encode(Sha256::digest(owner.stable_key().as_bytes())),
                logical_id.to_ascii_lowercase()
            ),
            ecosystem_id: EcosystemId::new("opencode").map_err(|error| {
                crate::BitFunError::Validation(format!("Invalid OpenCode ecosystem id: {error}"))
            })?,
            provider_label: owner.label(),
            model_binding: ExternalSubagentModelBinding::InheritParent,
            hidden,
            mode,
            agent,
        });
        routes.insert(
            logical_id,
            ExternalSubagentRoute::External(runtime_key.clone()),
        );
        runtime_agent_keys_by_plugin
            .entry(owner.clone())
            .or_default()
            .insert(runtime_key.clone());
        for tool in permitted_plugin_tools {
            let Some(tool_owner) = tool_owners.get(&tool) else {
                continue;
            };
            tool_runtime_agent_keys
                .entry((tool_owner.clone(), tool))
                .or_default()
                .insert(runtime_key.clone());
        }
        runtime_agent_keys.insert(runtime_key);
    }

    let skill_roots_by_plugin = attributed_skill_roots(config, &attribution.skill_owners)?;
    let mut skill_roots_by_runtime_agent = BTreeMap::new();
    for (plugin, runtime_keys) in &runtime_agent_keys_by_plugin {
        let Some(roots) = skill_roots_by_plugin.get(plugin) else {
            continue;
        };
        for runtime_key in runtime_keys {
            skill_roots_by_runtime_agent.insert(runtime_key.clone(), roots.clone());
        }
    }
    Ok(PluginConfigProjectionPlan {
        workspace_root: crate::agentic::workspace::canonical_local_workspace_path(workspace_root),
        generation_key: generation_key.to_string(),
        registrations,
        routes,
        runtime_agent_keys,
        skill_roots_by_runtime_agent,
        tool_runtime_agent_keys,
    })
}

struct ConfigAttribution {
    agent_owners: BTreeMap<String, PluginIdentity>,
    permission_owners: BTreeMap<(String, String), PluginIdentity>,
    skill_owners: BTreeMap<PathBuf, PluginIdentity>,
}

fn config_contribution_sequence(
    contributions: &[ConfigContribution],
    contributors: &[ConfigContributor],
    final_config: &Map<String, Value>,
) -> crate::BitFunResult<Vec<ConfigContribution>> {
    if contributions.is_empty() {
        if contributors.len() == 1 {
            return Ok(vec![ConfigContribution {
                plugin: contributors[0].plugin.clone(),
                outcome: contributors[0].outcome,
                config: final_config.clone(),
            }]);
        }
        return Err(crate::BitFunError::Validation(
            "unsupported_multiple_config_contributors: plugin host did not provide configContributions"
                .to_string(),
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
        return Err(crate::BitFunError::Validation(
            "Plugin config contribution sequence does not match configContributors".to_string(),
        ));
    }
    if contributions.last().map(|step| &step.config) != Some(final_config) {
        return Err(crate::BitFunError::Validation(
            "Plugin config contribution sequence does not end at the final config".to_string(),
        ));
    }
    Ok(contributions.to_vec())
}

fn attribute_config(
    initial_config: &Map<String, Value>,
    contributions: &[ConfigContribution],
    final_config: &Map<String, Value>,
) -> crate::BitFunResult<ConfigAttribution> {
    let mut previous = initial_config;
    let mut agent_owners = BTreeMap::new();
    let mut permission_owners = BTreeMap::new();
    let mut skill_owners = BTreeMap::new();
    let mut previous_skills = skill_paths(initial_config)?
        .into_iter()
        .map(|path| normalized_skill_path_identity(&path))
        .collect::<BTreeSet<_>>();

    for contribution in contributions {
        validate_plugin_identity(&contribution.plugin)?;
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
                // The plugin that first turns a native or absent Agent into a
                // plugin-managed Agent remains its execution owner. Later
                // hooks may refine fields, but do not silently transfer Tool
                // and Skill ownership merely by editing a description or
                // permission entry.
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
            .map(|path| normalized_skill_path_identity(&path))
            .collect::<BTreeSet<_>>();
        skill_owners.retain(|path, _| next_skills.contains(path));
        for added in next_skills.difference(&previous_skills) {
            skill_owners.insert(added.clone(), contribution.plugin.clone());
        }
        previous_skills = next_skills;
        previous = &contribution.config;
    }
    if previous != final_config {
        return Err(crate::BitFunError::Validation(
            "Plugin config attribution did not reach the final config".to_string(),
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
) -> crate::BitFunResult<Map<String, Value>> {
    let Some(agent) = agent else {
        return Ok(Map::new());
    };
    let agent = agent.as_object().ok_or_else(|| {
        crate::BitFunError::Validation(format!("Plugin agent '{agent_id}' must be an object"))
    })?;
    match agent.get("permission") {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(permission)) => Ok(permission.clone()),
        Some(_) => Err(crate::BitFunError::Validation(format!(
            "Plugin agent '{agent_id}' permission must be an object"
        ))),
    }
}

fn native_tool_baseline(
    logical_id: &str,
    mode: ExternalSubagentMode,
    workspace_root: &Path,
) -> Vec<String> {
    if let Some(local_agent) =
        get_agent_registry().get_local_agent(logical_id, Some(workspace_root))
    {
        return local_agent.default_tools();
    }
    if mode == ExternalSubagentMode::Subagent {
        use crate::agentic::agents::Agent;
        ExploreAgent::new().default_tools()
    } else {
        shared_coding_mode_tools()
    }
}

fn validate_plugin_identity(plugin: &PluginIdentity) -> crate::BitFunResult<()> {
    if plugin.spec.trim().is_empty() || plugin.entry.trim().is_empty() {
        return Err(crate::BitFunError::Validation(
            "Plugin config contributor identity is incomplete".to_string(),
        ));
    }
    Ok(())
}

fn validate_agent_id(id: &str) -> crate::BitFunResult<()> {
    if id.trim() != id
        || id.is_empty()
        || id.len() > MAX_AGENT_ID_BYTES
        || id.chars().any(char::is_control)
    {
        return Err(crate::BitFunError::Validation(format!(
            "Invalid plugin agent id '{id}'"
        )));
    }
    Ok(())
}

fn parse_mode(value: Option<&Value>, id: &str) -> crate::BitFunResult<ExternalSubagentMode> {
    match value.and_then(Value::as_str).unwrap_or("all") {
        "primary" => Ok(ExternalSubagentMode::Primary),
        "subagent" => Ok(ExternalSubagentMode::Subagent),
        "all" => Ok(ExternalSubagentMode::All),
        other => Err(crate::BitFunError::Validation(format!(
            "Plugin agent '{id}' has unsupported mode '{other}'"
        ))),
    }
}

fn parse_hidden(value: Option<&Value>, id: &str) -> crate::BitFunResult<bool> {
    match value {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(hidden)) => Ok(*hidden),
        Some(_) => Err(crate::BitFunError::Validation(format!(
            "Plugin agent '{id}' hidden must be a boolean"
        ))),
    }
}

fn parse_temperature(value: Option<&Value>, id: &str) -> crate::BitFunResult<Option<f64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let temperature = value.as_f64().ok_or_else(|| {
        crate::BitFunError::Validation(format!("Plugin agent '{id}' temperature must be a number"))
    })?;
    if !temperature.is_finite()
        || !(MIN_AGENT_TEMPERATURE..=MAX_AGENT_TEMPERATURE).contains(&temperature)
    {
        return Err(crate::BitFunError::Validation(format!(
            "Plugin agent '{id}' temperature must be between {MIN_AGENT_TEMPERATURE} and {MAX_AGENT_TEMPERATURE}"
        )));
    }
    Ok(Some(temperature))
}

fn parse_description(
    value: Option<&Value>,
    plugin: &PluginIdentity,
) -> crate::BitFunResult<String> {
    let description = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Agent contributed by {}", plugin.label()));
    if description.len() > MAX_DESCRIPTION_BYTES {
        return Err(crate::BitFunError::Validation(
            "Plugin agent description exceeds the size limit".to_string(),
        ));
    }
    Ok(description)
}

fn parse_prompt(value: Option<&Value>, id: &str) -> crate::BitFunResult<String> {
    let prompt = match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(_) => {
            return Err(crate::BitFunError::Validation(format!(
                "Plugin agent '{id}' prompt must be a string"
            )))
        }
    };
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(crate::BitFunError::Validation(format!(
            "Plugin agent '{id}' prompt exceeds the size limit"
        )));
    }
    Ok(prompt)
}

fn plugin_tool_ids_by_owner(
    tools: &[HookFunctionToolRegistration],
) -> crate::BitFunResult<BTreeMap<PluginIdentity, BTreeSet<String>>> {
    let mut result = BTreeMap::<PluginIdentity, BTreeSet<String>>::new();
    for tool in tools {
        let identity = tool.plugin.as_ref().map(PluginIdentity::from);
        let identity = identity.ok_or_else(|| {
            crate::BitFunError::Validation("Plugin tool identity is missing".to_string())
        })?;
        validate_plugin_identity(&identity)?;
        let id = tool.id.as_str();
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err(crate::BitFunError::Validation(
                "Plugin tool id is invalid".to_string(),
            ));
        }
        result.entry(identity).or_default().insert(id.to_string());
    }
    Ok(result)
}

fn parse_permissions(
    value: Option<&Value>,
    plugin_tools: &BTreeSet<String>,
    agent_id: &str,
) -> crate::BitFunResult<(PermissionConstraintLayer, BTreeSet<String>)> {
    let Some(value) = value else {
        return Ok((PermissionConstraintLayer::default(), BTreeSet::new()));
    };
    let permissions = value.as_object().ok_or_else(|| {
        crate::BitFunError::Validation(format!(
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
                return Err(crate::BitFunError::Validation(format!(
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
            return Err(crate::BitFunError::Validation(format!("Plugin agent '{agent_id}' permission '{key}' has no compatible action or plugin tool")));
        }
    }
    Ok((PermissionConstraintLayer::new(rules), denied))
}

fn config_object_field(
    config: &Map<String, Value>,
    field: &str,
) -> crate::BitFunResult<Map<String, Value>> {
    match config.get(field) {
        None => Ok(Map::new()),
        Some(Value::Object(value)) => Ok(value.clone()),
        Some(_) => Err(crate::BitFunError::Validation(format!(
            "Plugin config '{field}' must be an object"
        ))),
    }
}

fn skill_paths(config: &Map<String, Value>) -> crate::BitFunResult<Vec<PathBuf>> {
    let Some(skills) = config.get("skills") else {
        return Ok(Vec::new());
    };
    let skills = skills.as_object().ok_or_else(|| {
        crate::BitFunError::Validation("Plugin config 'skills' must be an object".to_string())
    })?;
    let Some(paths) = skills.get("paths") else {
        return Ok(Vec::new());
    };
    let paths = paths.as_array().ok_or_else(|| {
        crate::BitFunError::Validation("Plugin config 'skills.paths' must be an array".to_string())
    })?;
    paths
        .iter()
        .map(|path| {
            path.as_str().map(PathBuf::from).ok_or_else(|| {
                crate::BitFunError::Validation(
                    "Plugin config 'skills.paths' entries must be strings".to_string(),
                )
            })
        })
        .collect()
}

fn normalized_skill_path_identity(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn attributed_skill_roots(
    final_config: &Map<String, Value>,
    owners: &BTreeMap<PathBuf, PluginIdentity>,
) -> crate::BitFunResult<BTreeMap<PluginIdentity, Vec<PluginSkillRootContribution>>> {
    let mut seen = BTreeSet::new();
    let mut roots = BTreeMap::<PluginIdentity, Vec<PluginSkillRootContribution>>::new();
    for path in skill_paths(final_config)? {
        let identity = normalized_skill_path_identity(&path);
        let Some(owner) = owners.get(&identity) else {
            continue;
        };
        if !seen.insert(identity) {
            continue;
        }
        if seen.len() > MAX_PLUGIN_SKILL_ROOTS {
            return Err(crate::BitFunError::Validation(
                "Plugin skill root count exceeds the limit".to_string(),
            ));
        }
        if !path.is_absolute() {
            return Err(crate::BitFunError::Validation(
                "Plugin skill root must be absolute".to_string(),
            ));
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            crate::BitFunError::Validation(format!("Plugin skill root is unavailable: {error}"))
        })?;
        if bitfun_services_core::bounded_fs::is_symlink_or_reparse(&metadata) || !metadata.is_dir()
        {
            return Err(crate::BitFunError::Validation(
                "Plugin skill root must be a regular directory".to_string(),
            ));
        }
        let canonical = dunce::canonicalize(&path).map_err(|error| {
            crate::BitFunError::Validation(format!(
                "Plugin skill root cannot be canonicalized: {error}"
            ))
        })?;
        let owned_roots = roots.entry(owner.clone()).or_default();
        owned_roots.push(PluginSkillRootContribution {
            path: canonical,
            precedence: seen.len() - 1,
        });
    }
    Ok(roots)
}

pub(crate) fn active_generation_key(workspace_root: &Path) -> Option<String> {
    let root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    skill_generations()
        .read()
        .ok()?
        .get(&root)
        .map(|generation| generation.generation_key.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugin() -> Value {
        json!({
            "id": "deveco-harness",
            "spec": "D:/code/deveco_harness",
            "entry": "D:/code/deveco_harness/dist/index.js",
            "index": 0
        })
    }

    fn projection_identity(value: Value) -> PluginIdentity {
        let typed: HookFunctionPluginIdentity =
            serde_json::from_value(value).expect("typed plugin identity");
        PluginIdentity::from(&typed)
    }

    fn open_result() -> Value {
        let plugin = plugin();
        let config = json!({
            "agent": {
                "build": {
                    "mode": "primary",
                    "temperature": 0.7,
                    "description": "Build projects",
                    "prompt": "Build prompt",
                    "permission": {"build_project": "allow", "plan_write": "deny"}
                },
                "explore": {
                    "mode": "subagent",
                    "hidden": true,
                    "description": "Explore projects",
                    "prompt": "Explore prompt",
                    "permission": {"bash": "deny"}
                }
            }
        });
        json!({
            "configContributors": [{"plugin": plugin.clone(), "outcome": "applied"}],
            "config": config.clone(),
            "configContributions": [{"plugin": plugin.clone(), "outcome": "applied", "config": config}],
            "tools": [
                {"id": "build_project", "plugin": plugin.clone()},
                {"id": "plan_write", "plugin": plugin}
            ]
        })
    }

    fn registration_batch(result: &Value) -> HookFunctionRegistrationBatch {
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|tool| HookFunctionToolRegistration {
                registration_id: format!(
                    "registration-{}",
                    tool.get("id").and_then(Value::as_str).unwrap_or_default()
                ),
                id: tool
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                plugin: tool
                    .get("plugin")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .expect("typed tool owner"),
                description: String::new(),
                parameters: json!({"type": "object"}),
            })
            .collect();
        HookFunctionRegistrationBatch {
            generation: bitfun_runtime_ports::HookFunctionGeneration {
                instance_id: "projection-test".to_string(),
                generation_key: "projection-generation".to_string(),
                revision: "projection-revision".to_string(),
            },
            config: result
                .get("config")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
            config_contributors: serde_json::from_value(
                result
                    .get("configContributors")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            )
            .expect("typed config contributors"),
            config_contributions: serde_json::from_value(
                result
                    .get("configContributions")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            )
            .expect("typed config contributions"),
            diagnostics: Vec::new(),
            hooks: Vec::new(),
            tools,
        }
    }

    #[test]
    fn maps_target_agent_fields_and_plugin_tool_permissions() {
        let plan = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&open_result()),
        )
        .expect("projection");

        assert_eq!(plan.registrations.len(), 2);
        let build = plan
            .registrations
            .iter()
            .find(|registration| registration.logical_id == "build")
            .unwrap();
        assert_eq!(build.mode, ExternalSubagentMode::Primary);
        assert!(!build.hidden);
        assert_eq!(build.agent.model_temperature_override(), Some(0.7));
        assert_eq!(build.agent.description(), "Build projects");
        assert!(build
            .agent
            .default_tools()
            .contains(&"build_project".to_string()));
        assert!(!build
            .agent
            .default_tools()
            .contains(&"plan_write".to_string()));
        assert!(build
            .agent
            .permission_constraints()
            .rules()
            .iter()
            .any(|rule| {
                rule.action == "custom_tool"
                    && rule.resource == "plan_write"
                    && rule.effect == PermissionEffect::Deny
            }));

        let explore = plan
            .registrations
            .iter()
            .find(|registration| registration.logical_id == "explore")
            .unwrap();
        assert_eq!(explore.mode, ExternalSubagentMode::Subagent);
        assert!(explore.hidden);
        assert_eq!(explore.agent.model_temperature_override(), None);
        assert!(explore
            .agent
            .permission_constraints()
            .rules()
            .iter()
            .any(|rule| {
                rule.action == "bash"
                    && rule.resource == "*"
                    && rule.effect == PermissionEffect::Deny
            }));
        assert_eq!(plan.runtime_agent_keys.len(), 2);
        assert!(plan
            .runtime_agent_keys
            .iter()
            .all(|key| is_plugin_agent_runtime_key(key)));
    }

    #[test]
    fn displaced_local_baseline_is_case_insensitive() {
        use crate::agentic::agents::{Agent, PlanMode};

        assert_eq!(
            native_tool_baseline(
                "plan",
                ExternalSubagentMode::Primary,
                Path::new("C:/workspace")
            ),
            PlanMode::new().default_tools()
        );
    }

    #[test]
    fn projects_multiple_config_contributors_and_isolates_agent_tools() {
        let mut result = open_result();
        let second = json!({
            "id": "second",
            "spec": "D:/code/second",
            "entry": "D:/code/second/index.js",
            "index": 0
        });
        let mut second_config = result["config"].as_object().unwrap().clone();
        second_config["agent"]["build"]["description"] = json!("Second build");
        second_config["agent"]["build"]["permission"]["second_tool"] = json!("allow");
        second_config["agent"]["plan"] = json!({
            "mode": "subagent",
            "description": "Plan",
            "prompt": "Plan prompt",
            "permission": {"second_tool": "allow"}
        });
        result["configContributors"] = json!([
            {"plugin": plugin(), "outcome": "applied"},
            {"plugin": second.clone(), "outcome":"applied"}
        ]);
        result["config"] = Value::Object(second_config.clone());
        result["configContributions"] = json!([
            {"plugin": plugin(), "outcome": "applied", "config": open_result()["config"].clone()},
            {"plugin": second.clone(), "outcome":"applied", "config": second_config}
        ]);
        result["tools"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "second_tool", "plugin": second}));
        result["tools"].as_array_mut().unwrap().push(
            json!({"id": "second_tool_ungranted", "plugin": json!({
                "id": "second",
                "spec": "D:/code/second",
                "entry": "D:/code/second/index.js",
                "index": 0
            })}),
        );

        let plan = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .expect("multiple config contributors should project");
        assert_eq!(plan.registrations.len(), 3);
        assert!(plan
            .registrations
            .iter()
            .find(|registration| registration.logical_id == "build")
            .unwrap()
            .agent
            .description()
            .contains("Second build"));
        let build_tools = plan
            .registrations
            .iter()
            .find(|registration| registration.logical_id == "build")
            .unwrap()
            .agent
            .default_tools();
        let plan_tools = plan
            .registrations
            .iter()
            .find(|registration| registration.logical_id == "plan")
            .unwrap()
            .agent
            .default_tools();
        assert!(build_tools.contains(&"build_project".to_string()));
        assert!(build_tools.contains(&"second_tool".to_string()));
        assert!(!build_tools.contains(&"second_tool_ungranted".to_string()));
        assert!(plan_tools.contains(&"second_tool".to_string()));
        assert!(plan_tools.contains(&"second_tool_ungranted".to_string()));
    }

    #[test]
    fn supports_legacy_single_contributor_without_contribution_snapshots() {
        let mut result = open_result();
        result
            .as_object_mut()
            .unwrap()
            .remove("configContributions");

        let plan = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .expect("single contributor legacy projection");

        assert_eq!(plan.registrations.len(), 2);
    }

    #[test]
    fn rejects_legacy_multiple_contributors_without_contribution_snapshots() {
        let mut result = open_result();
        result["configContributors"] = json!([
            {"plugin": plugin(), "outcome": "applied"},
            {"plugin": {
                "id": "second",
                "spec": "D:/code/second",
                "entry": "D:/code/second/index.js",
                "index": 0
            }, "outcome": "applied"}
        ]);
        result
            .as_object_mut()
            .unwrap()
            .remove("configContributions");

        let error = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .err()
        .expect("multiple contributors require contribution snapshots");

        assert!(error
            .to_string()
            .contains("unsupported_multiple_config_contributors"));
    }

    #[test]
    fn rejects_inconsistent_config_contribution_sequences() {
        let mut result = open_result();
        result["configContributions"][0]["outcome"] = json!("failed");
        let error = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .err()
        .expect("contributor metadata must align");
        assert!(error
            .to_string()
            .contains("does not match configContributors"));

        let mut result = open_result();
        result["configContributions"][0]["config"] = json!({"agent": {}});
        let error = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .err()
        .expect("last contribution must equal final config");
        assert!(error
            .to_string()
            .contains("does not end at the final config"));
    }

    #[test]
    fn rejects_malformed_agent_and_skill_shapes() {
        let mut result = open_result();
        result["config"]["agent"] = json!([]);
        result
            .as_object_mut()
            .unwrap()
            .remove("configContributions");
        let error = prepare(
            Path::new("C:/workspace"),
            "generation-1",
            &Map::new(),
            &registration_batch(&result),
        )
        .err()
        .expect("agent must be an object");
        assert!(error
            .to_string()
            .contains("config 'agent' must be an object"));

        for malformed in [json!({"paths": "not-an-array"}), json!({"paths": [42]})] {
            let mut result = open_result();
            result["config"]["skills"] = malformed;
            result
                .as_object_mut()
                .unwrap()
                .remove("configContributions");
            let error = prepare(
                Path::new("C:/workspace"),
                "generation-1",
                &Map::new(),
                &registration_batch(&result),
            )
            .err()
            .expect("malformed skill paths must fail");
            assert!(error.to_string().contains("skills.paths"));
        }
    }

    #[test]
    fn canonical_skill_identity_does_not_republish_an_initial_root() {
        let directory = tempfile::tempdir().expect("temp directory");
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

        let contributor = ConfigContribution {
            plugin: projection_identity(plugin()),
            outcome: ContributorOutcome::Applied,
            config: final_config.clone(),
        };
        let attribution =
            attribute_config(&initial, &[contributor], &final_config).expect("skill attribution");
        assert!(attribution.skill_owners.is_empty());
        assert!(
            attributed_skill_roots(&final_config, &attribution.skill_owners)
                .expect("skill roots")
                .is_empty()
        );
    }

    #[test]
    fn attributes_skill_additions_across_reordering_and_removal() {
        let base = tempfile::tempdir().expect("base skill root");
        let first = tempfile::tempdir().expect("first plugin skill root");
        let second = tempfile::tempdir().expect("second plugin skill root");
        let plugin_a = projection_identity(plugin());
        let plugin_b = projection_identity(json!({
            "id": "second",
            "spec": "D:/code/second",
            "entry": "D:/code/second/index.js",
            "index": 0
        }));
        let initial = json!({"skills": {"paths": [base.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let after_a = json!({"skills": {"paths": [base.path(), first.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let final_config = json!({"skills": {"paths": [first.path(), base.path(), second.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let contributions = vec![
            ConfigContribution {
                plugin: plugin_a.clone(),
                outcome: ContributorOutcome::Applied,
                config: after_a,
            },
            ConfigContribution {
                plugin: plugin_b.clone(),
                outcome: ContributorOutcome::Applied,
                config: final_config.clone(),
            },
        ];

        let attribution =
            attribute_config(&initial, &contributions, &final_config).expect("skill attribution");
        assert_eq!(
            attribution
                .skill_owners
                .get(&normalized_skill_path_identity(first.path())),
            Some(&plugin_a)
        );
        assert_eq!(
            attribution
                .skill_owners
                .get(&normalized_skill_path_identity(second.path())),
            Some(&plugin_b)
        );
        assert!(!attribution
            .skill_owners
            .contains_key(&normalized_skill_path_identity(base.path())));

        let removed_config = json!({"skills": {"paths": [base.path(), second.path()]}})
            .as_object()
            .unwrap()
            .clone();
        let mut removal_sequence = contributions;
        removal_sequence.push(ConfigContribution {
            plugin: plugin_b,
            outcome: ContributorOutcome::Applied,
            config: removed_config.clone(),
        });
        let removed = attribute_config(&initial, &removal_sequence, &removed_config)
            .expect("skill removal attribution");
        assert!(!removed
            .skill_owners
            .contains_key(&normalized_skill_path_identity(first.path())));
    }

    #[test]
    fn reattributes_deleted_and_recreated_agents_and_permission_fields() {
        let plugin_a = projection_identity(plugin());
        let plugin_b = projection_identity(json!({
            "id": "second",
            "spec": "D:/code/second",
            "entry": "D:/code/second/index.js",
            "index": 0
        }));
        let initial = json!({"agent": {"build": {"prompt": "native"}}})
            .as_object()
            .unwrap()
            .clone();
        let after_a = json!({"agent": {"build": {
            "prompt": "plugin-a",
            "permission": {"build_project": "allow"}
        }}})
        .as_object()
        .unwrap()
        .clone();
        let after_delete = json!({"agent": {}}).as_object().unwrap().clone();
        let final_config = json!({"agent": {"build": {
            "prompt": "plugin-b",
            "permission": {"second_tool": "ask"}
        }}})
        .as_object()
        .unwrap()
        .clone();
        let contributions = vec![
            ConfigContribution {
                plugin: plugin_a,
                outcome: ContributorOutcome::Applied,
                config: after_a,
            },
            ConfigContribution {
                plugin: plugin_b.clone(),
                outcome: ContributorOutcome::Applied,
                config: after_delete,
            },
            ConfigContribution {
                plugin: plugin_b.clone(),
                outcome: ContributorOutcome::Applied,
                config: final_config.clone(),
            },
        ];

        let attribution =
            attribute_config(&initial, &contributions, &final_config).expect("agent attribution");
        assert_eq!(attribution.agent_owners.get("build"), Some(&plugin_b));
        assert_eq!(
            attribution
                .permission_owners
                .get(&("build".to_string(), "second_tool".to_string())),
            Some(&plugin_b)
        );
        assert!(!attribution
            .permission_owners
            .contains_key(&("build".to_string(), "build_project".to_string())));
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
            assert!(error.to_string().contains("has no compatible action"));
        }
    }

    #[test]
    fn parses_hidden_and_temperature_with_safe_defaults_and_bounds() {
        assert!(!parse_hidden(None, "agent").expect("hidden defaults to false"));
        assert!(parse_hidden(Some(&json!(true)), "agent").expect("boolean hidden"));
        assert!(!parse_hidden(Some(&json!(null)), "agent").expect("null hidden default"));
        assert!(parse_hidden(Some(&json!("true")), "agent")
            .expect_err("non-boolean hidden must fail")
            .to_string()
            .contains("hidden must be a boolean"));

        assert_eq!(parse_temperature(None, "agent").unwrap(), None);
        assert_eq!(
            parse_temperature(Some(&json!(null)), "agent").unwrap(),
            None
        );
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
}
