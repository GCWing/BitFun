use crate::agentic::agents::{
    external_subagent_runtime_key, get_agent_registry, shared_coding_mode_tools, ExploreAgent,
    ExternalProvidedAgent, ExternalSubagentModelBinding, ExternalSubagentRegistration,
    ExternalSubagentRoute,
};
use bitfun_opencode_adapter::{OpenCodePluginConfigProjection, OpenCodePluginToolRef};
use bitfun_product_domains::external_sources::EcosystemId;
use bitfun_product_domains::external_subagents::ExternalSubagentMode;
use bitfun_runtime_ports::{HookFunctionRegistrationBatch, HookFunctionToolRegistration};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

const OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER: &str = "opencode-plugin-config";

pub(crate) fn is_plugin_agent_runtime_key(runtime_agent_key: &str) -> bool {
    runtime_agent_key.starts_with("external_subagent_runtime:opencode-plugin:")
}

#[derive(Debug, Clone)]
pub(crate) struct PluginSkillRootContribution {
    pub(crate) path: PathBuf,
    pub(crate) precedence: usize,
}

#[derive(Debug, Clone)]
struct PublishedSkillGeneration {
    generation_key: String,
    workspace_roots: Vec<PluginSkillRootContribution>,
}

fn skill_generations() -> &'static RwLock<HashMap<PathBuf, PublishedSkillGeneration>> {
    static GENERATIONS: OnceLock<RwLock<HashMap<PathBuf, PublishedSkillGeneration>>> =
        OnceLock::new();
    GENERATIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) struct PluginConfigPublicationPlan {
    workspace_root: PathBuf,
    generation_key: String,
    registrations: Vec<ExternalSubagentRegistration>,
    routes: BTreeMap<String, ExternalSubagentRoute>,
    runtime_agent_keys: BTreeSet<String>,
    workspace_skill_roots: Vec<PluginSkillRootContribution>,
    tool_runtime_agent_keys: BTreeMap<OpenCodePluginToolRef, BTreeSet<String>>,
}

impl PluginConfigPublicationPlan {
    pub(crate) fn empty(workspace_root: &Path, generation_key: &str) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            generation_key: generation_key.to_string(),
            registrations: Vec::new(),
            routes: BTreeMap::new(),
            runtime_agent_keys: BTreeSet::new(),
            workspace_skill_roots: Vec::new(),
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
        let tool_ref = OpenCodePluginToolRef::try_from(tool)
            .map_err(|error| crate::BitFunError::Validation(error.to_string()))?;
        Ok(self
            .tool_runtime_agent_keys
            .get(&tool_ref)
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
                workspace_roots: self.workspace_skill_roots,
            },
        );
    }
}

pub(crate) fn release_workspace(workspace_root: &Path) {
    let workspace_root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    get_agent_registry().release_external_subagent_route_overlay(
        &workspace_root,
        OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER,
    );
    skill_generations()
        .write()
        .expect("plugin skill generation lock poisoned")
        .remove(&workspace_root);
}

pub(crate) fn release_workspace_generation(
    workspace_root: &Path,
    expected_generation_key: &str,
) -> bool {
    let workspace_root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    let mut generations = skill_generations()
        .write()
        .expect("plugin skill generation lock poisoned");
    if generations
        .get(&workspace_root)
        .is_none_or(|generation| generation.generation_key != expected_generation_key)
    {
        return false;
    }
    get_agent_registry().release_external_subagent_route_overlay(
        &workspace_root,
        OPENCODE_PLUGIN_CONFIG_ROUTE_OWNER,
    );
    generations.remove(&workspace_root);
    true
}

pub(crate) fn skill_roots_for_agent(
    workspace_root: Option<&Path>,
    _runtime_agent_key: Option<&str>,
) -> Vec<PluginSkillRootContribution> {
    let Some(workspace_root) = workspace_root else {
        return Vec::new();
    };
    let workspace_root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    skill_generations()
        .read()
        .expect("plugin skill generation lock poisoned")
        .get(&workspace_root)
        .map(|generation| generation.workspace_roots.clone())
        .unwrap_or_default()
}

pub(crate) fn prepare(
    workspace_root: &Path,
    generation_key: &str,
    initial_config: &Map<String, Value>,
    registration_batch: &HookFunctionRegistrationBatch,
) -> crate::BitFunResult<PluginConfigPublicationPlan> {
    let projection = bitfun_opencode_adapter::project_plugin_config(
        workspace_root,
        initial_config,
        registration_batch,
    )
    .map_err(|error| crate::BitFunError::Validation(error.to_string()))?;
    prepare_projection(workspace_root, generation_key, projection)
}

fn prepare_projection(
    workspace_root: &Path,
    generation_key: &str,
    projection: OpenCodePluginConfigProjection,
) -> crate::BitFunResult<PluginConfigPublicationPlan> {
    let workspace_root = crate::agentic::workspace::canonical_local_workspace_path(workspace_root);
    if projection.agents.is_empty() && projection.skill_roots.is_empty() {
        return Ok(PluginConfigPublicationPlan::empty(
            &workspace_root,
            generation_key,
        ));
    }

    let mut registrations = Vec::new();
    let mut routes = BTreeMap::new();
    let mut runtime_agent_keys = BTreeSet::new();
    let mut tool_runtime_agent_keys = BTreeMap::<OpenCodePluginToolRef, BTreeSet<String>>::new();
    for projected in projection.agents {
        let mut tools =
            native_tool_baseline(&projected.logical_id, projected.mode, &workspace_root);
        let permitted_plugin_tools = projected
            .plugin_tools
            .iter()
            .map(|tool| tool.id.clone())
            .collect::<Vec<_>>();
        tools.extend(permitted_plugin_tools.iter().cloned());
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
        hasher.update(projected.contributor.stable_key().as_bytes());
        hasher.update([0]);
        hasher.update(projected.logical_id.as_bytes());
        hasher.update([0]);
        hasher.update([u8::from(projected.hidden)]);
        hasher.update([0]);
        if let Some(temperature) = projected.temperature {
            hasher.update(temperature.to_bits().to_le_bytes());
        } else {
            hasher.update([0xff]);
        }
        let digest = hex::encode(hasher.finalize());
        let runtime_key = external_subagent_runtime_key(&format!("opencode-plugin:{digest}"));
        let behavior_version = format!("sha256:{digest}");
        let agent = Arc::new(ExternalProvidedAgent::new(
            runtime_key.clone(),
            projected.logical_id.clone(),
            projected.description,
            projected.prompt,
            tools,
            projected.permission_constraints,
            projected.temperature,
            false,
            behavior_version,
        ));
        registrations.push(ExternalSubagentRegistration {
            runtime_key: runtime_key.clone(),
            logical_id: projected.logical_id.clone(),
            route_key: format!(
                "opencode:{}:{}",
                hex::encode(Sha256::digest(
                    projected.contributor.stable_key().as_bytes()
                )),
                projected.logical_id.to_ascii_lowercase()
            ),
            ecosystem_id: EcosystemId::new("opencode").map_err(|error| {
                crate::BitFunError::Validation(format!("Invalid OpenCode ecosystem id: {error}"))
            })?,
            provider_label: projected.contributor.label().to_string(),
            model_binding: ExternalSubagentModelBinding::InheritParent,
            hidden: projected.hidden,
            mode: projected.mode,
            agent,
        });
        routes.insert(
            projected.logical_id,
            ExternalSubagentRoute::External(runtime_key.clone()),
        );
        for tool in projected.plugin_tools {
            tool_runtime_agent_keys
                .entry(tool)
                .or_default()
                .insert(runtime_key.clone());
        }
        runtime_agent_keys.insert(runtime_key);
    }

    let workspace_skill_roots = projection
        .skill_roots
        .into_iter()
        .map(|root| PluginSkillRootContribution {
            path: root.path,
            precedence: root.precedence,
        })
        .collect();
    Ok(PluginConfigPublicationPlan {
        workspace_root,
        generation_key: generation_key.to_string(),
        registrations,
        routes,
        runtime_agent_keys,
        workspace_skill_roots,
        tool_runtime_agent_keys,
    })
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
    use bitfun_runtime_ports::{
        HookFunctionConfigContribution, HookFunctionConfigContributor,
        HookFunctionContributorOutcome, HookFunctionGeneration, HookFunctionPluginIdentity,
        HookFunctionRegistrationBatch, HookFunctionToolRegistration,
    };
    use serde_json::{json, Map};

    fn plugin() -> HookFunctionPluginIdentity {
        HookFunctionPluginIdentity {
            id: Some("deveco-harness".to_string()),
            spec: "D:/code/deveco_harness".to_string(),
            entry: "D:/code/deveco_harness/dist/index.js".to_string(),
            index: 0,
        }
    }

    fn registration_batch(config: Map<String, serde_json::Value>) -> HookFunctionRegistrationBatch {
        let plugin = plugin();
        HookFunctionRegistrationBatch {
            generation: HookFunctionGeneration {
                instance_id: "projection-test".to_string(),
                generation_key: "projection-generation".to_string(),
                revision: "projection-revision".to_string(),
            },
            config: config.clone(),
            config_contributors: vec![HookFunctionConfigContributor {
                plugin: plugin.clone(),
                outcome: HookFunctionContributorOutcome::Applied,
            }],
            config_contributions: vec![HookFunctionConfigContribution {
                plugin: plugin.clone(),
                outcome: HookFunctionContributorOutcome::Applied,
                config,
            }],
            diagnostics: Vec::new(),
            hooks: Vec::new(),
            tools: vec![HookFunctionToolRegistration {
                registration_id: "registration-build-project".to_string(),
                id: "build_project".to_string(),
                plugin: Some(plugin),
                description: String::new(),
                parameters: json!({"type": "object"}),
            }],
        }
    }

    #[test]
    fn generation_scoped_release_never_withdraws_a_replacement() {
        let workspace = tempfile::tempdir().expect("workspace");
        PluginConfigPublicationPlan::empty(workspace.path(), "generation-a").commit();

        assert!(!release_workspace_generation(
            workspace.path(),
            "generation-b"
        ));
        assert_eq!(
            active_generation_key(workspace.path()).as_deref(),
            Some("generation-a")
        );
        assert!(release_workspace_generation(
            workspace.path(),
            "generation-a"
        ));
        assert_eq!(active_generation_key(workspace.path()), None);
    }

    #[test]
    fn publishes_plugin_skill_roots_to_all_workspace_agents() {
        let workspace = tempfile::tempdir().expect("workspace");
        let skill_root = tempfile::tempdir().expect("plugin skill root");
        let config = json!({"skills": {"paths": [skill_root.path()]}})
            .as_object()
            .expect("config object")
            .clone();
        let batch = registration_batch(config);
        let projection =
            bitfun_opencode_adapter::project_plugin_config(workspace.path(), &Map::new(), &batch)
                .expect("OpenCode projection");
        let plan = prepare_projection(workspace.path(), "skill-only-generation", projection)
            .expect("skill-only plugin publication");
        assert!(plan.registrations.is_empty());
        plan.commit();

        for agent in [Some("build"), Some("external-agent"), None] {
            let roots = skill_roots_for_agent(Some(workspace.path()), agent);
            assert_eq!(roots.len(), 1);
            assert_eq!(
                roots[0].path,
                dunce::canonicalize(skill_root.path()).unwrap()
            );
        }
        release_workspace(workspace.path());
    }

    #[test]
    fn materializes_projected_agent_fields_and_plugin_tool_permissions() {
        let config = json!({
            "agent": {
                "build": {
                    "mode": "primary",
                    "temperature": 0.7,
                    "description": "Build projects",
                    "prompt": "Build prompt",
                    "permission": {"build_project": "allow"}
                }
            }
        })
        .as_object()
        .expect("config object")
        .clone();
        let batch = registration_batch(config);
        let projection = bitfun_opencode_adapter::project_plugin_config(
            Path::new("C:/workspace"),
            &Map::new(),
            &batch,
        )
        .expect("OpenCode projection");
        let plan = prepare_projection(Path::new("C:/workspace"), "generation-1", projection)
            .expect("publication");

        assert_eq!(plan.registrations.len(), 1);
        let build = &plan.registrations[0];
        assert_eq!(
            build.runtime_key,
            "external_subagent_runtime:opencode-plugin:0b17c0646a8c5a8f84a65251bdd750e0b7157ec13115d567608913d87a3763ea"
        );
        assert_eq!(
            build.route_key,
            "opencode:ed485fb494e18771e0611903da426ed83c36f4f19b5c422c7f38712b8aa16d76:build"
        );
        assert_eq!(build.mode, ExternalSubagentMode::Primary);
        assert!(!build.hidden);
        assert_eq!(build.agent.model_temperature_override(), Some(0.7));
        assert_eq!(build.agent.description(), "Build projects");
        assert!(build
            .agent
            .default_tools()
            .contains(&"build_project".to_string()));
        assert_eq!(plan.runtime_agent_keys.len(), 1);
        assert!(plan
            .runtime_agent_keys
            .iter()
            .all(|key| is_plugin_agent_runtime_key(key)));
        assert_eq!(
            plan.allowed_runtime_agent_keys_for_tool(&batch.tools[0])
                .expect("tool access"),
            plan.runtime_agent_keys
        );
    }

    #[test]
    fn displaced_local_baseline_is_case_insensitive() {
        use crate::agentic::agents::{Agent, CoworkMode};

        assert_eq!(
            native_tool_baseline(
                "cowork",
                ExternalSubagentMode::Primary,
                Path::new("C:/workspace")
            ),
            CoworkMode::new().default_tools()
        );
    }
}
