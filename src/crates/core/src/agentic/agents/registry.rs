use super::{
    Agent, AgenticMode, CodeReviewAgent, CoworkMode, DebugMode, ExploreAgent, FileFinderAgent,
    GenerateDocAgent, PlanMode,
};
use crate::agentic::agents::custom_subagents::{
    CustomSubagent, CustomSubagentKind, CustomSubagentLoader,
};
use crate::agentic::tools::get_all_registered_tool_names;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::types::{ModeConfig, SubAgentConfig};
use crate::service::config::GlobalConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;
use std::sync::{Arc, OnceLock};

/// subagent source (builtin / project / user), used for frontend display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentSource {
    Builtin,
    Project,
    User,
}

impl SubAgentSource {
    pub fn from_custom_kind(kind: CustomSubagentKind) -> Self {
        match kind {
            CustomSubagentKind::Project => SubAgentSource::Project,
            CustomSubagentKind::User => SubAgentSource::User,
        }
    }
}

/// mutable configuration for custom subagent (enabled, model will change, path/kind can be obtained by downcast)
#[derive(Clone, Debug)]
pub struct CustomSubagentConfig {
    /// whether enabled
    pub enabled: bool,
    /// used model ID
    pub model: String,
}

/// one agent record in registry
#[derive(Clone)]
struct AgentEntry {
    category: AgentCategory,
    /// only when category == SubAgent has value
    subagent_source: Option<SubAgentSource>,
    agent: Arc<dyn Agent>,
    /// custom subagent configuration (enabled, model), only user/project subagent has value
    custom_config: Option<CustomSubagentConfig>,
}

/// Information about a agent for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_readonly: bool,
    pub tool_count: usize,
    pub default_tools: Vec<String>,
    /// whether enabled (agentic always true, other from configuration)
    pub enabled: bool,
    /// subagent source, only subagent has value, used for frontend display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_source: Option<SubAgentSource>,
    pub path: Option<String>,
    /// model configuration, only custom subagent has value (read from file)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl AgentInfo {
    fn from_agent_entry(entry: &AgentEntry) -> Self {
        let agent = entry.agent.as_ref();
        let default_tools = agent.default_tools();

        // get enabled and model from custom_config; path by downcast
        let (enabled, model) = match &entry.custom_config {
            Some(config) => (config.enabled, Some(config.model.clone())),
            None => (true, None),
        };

        // get path by downcast to CustomSubagent (only custom subagent has path)
        let path = agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .map(|c| c.path.clone());

        AgentInfo {
            id: agent.id().to_string(),
            name: agent.name().to_string(),
            description: agent.description().to_string(),
            is_readonly: agent.is_readonly(),
            tool_count: default_tools.len(),
            default_tools,
            enabled,
            subagent_source: entry.subagent_source,
            path,
            model,
        }
    }
}

async fn get_mode_configs() -> HashMap<String, ModeConfig> {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.mode_configs"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

async fn get_subagent_configs() -> HashMap<String, SubAgentConfig> {
    if let Ok(config_service) = GlobalConfigManager::get_service().await {
        config_service
            .get_config(Some("ai.subagent_configs"))
            .await
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

/// Agent category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCategory {
    /// mode agent (displayed in frontend mode selector)
    Mode,
    /// subagent (displayed in frontend subagent list, discovered by TaskTool)
    SubAgent,
    /// hidden agent (not displayed in frontend, not discovered by TaskTool, used internally)
    Hidden,
}

/// Registry for managing all available agents
pub struct AgentRegistry {
    /// id -> agent_entry
    agents: RwLock<HashMap<String, AgentEntry>>,
}

impl AgentRegistry {
    fn read_agents(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, AgentEntry>> {
        match self.agents.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Agent registry read lock poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    fn write_agents(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, AgentEntry>> {
        match self.agents.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Agent registry write lock poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    /// Create a new agent registry with built-in agents
    pub fn new() -> Self {
        let mut agents = HashMap::new();

        // register built-in agents
        let register = |agents: &mut HashMap<String, AgentEntry>,
                        agent: Arc<dyn Agent>,
                        category: AgentCategory,
                        subagent_source: Option<SubAgentSource>| {
            let id = agent.id().to_string();
            if agents.contains_key(&id) {
                error!("Agent {} already registered, skip registration", id);
                return;
            }
            agents.insert(
                id,
                AgentEntry {
                    category,
                    subagent_source,
                    agent,
                    custom_config: None,
                },
            );
        };

        // Register built-in mode agents
        let modes: Vec<Arc<dyn Agent>> = vec![
            Arc::new(AgenticMode::new()),
            Arc::new(DebugMode::new()),
            Arc::new(PlanMode::new()),
            Arc::new(CoworkMode::new()),
        ];
        for mode in modes {
            register(&mut agents, mode, AgentCategory::Mode, None);
        }

        // Register built-in sub-agents
        let builtin_subagents: Vec<Arc<dyn Agent>> = vec![
            Arc::new(ExploreAgent::new()),
            Arc::new(FileFinderAgent::new()),
        ];
        for subagent in builtin_subagents {
            register(
                &mut agents,
                subagent,
                AgentCategory::SubAgent,
                Some(SubAgentSource::Builtin),
            );
        }

        // Register hidden agents
        let hidden_subagents: Vec<Arc<dyn Agent>> = vec![
            Arc::new(CodeReviewAgent::new()),
            Arc::new(GenerateDocAgent::new()),
        ];
        for hidden_agent in hidden_subagents {
            register(&mut agents, hidden_agent, AgentCategory::Hidden, None);
        }

        Self {
            agents: RwLock::new(agents),
        }
    }

    /// Register a new agent. For custom SubAgent, pass Some(custom_config); for builtin/Mode/Hidden pass None.
    pub fn register_agent(
        &self,
        agent: Arc<dyn Agent>,
        category: AgentCategory,
        subagent_source: Option<SubAgentSource>,
        custom_config: Option<CustomSubagentConfig>,
    ) {
        let id = agent.id().to_string();
        let mut map = self.write_agents();
        if map.contains_key(&id) {
            error!("Agent {} already registered, skip registration", id);
            return;
        }
        map.insert(
            id,
            AgentEntry {
                category,
                subagent_source,
                agent,
                custom_config,
            },
        );
    }

    /// Get a agent by ID (searches all categories including hidden)
    pub fn get_agent(&self, agent_type: &str) -> Option<Arc<dyn Agent>> {
        self.read_agents().get(agent_type).map(|e| e.agent.clone())
    }

    /// Check if an agent exists
    pub fn check_agent_exists(&self, agent_type: &str) -> bool {
        self.read_agents().contains_key(agent_type)
    }

    /// Get a mode by ID
    pub fn get_mode_agent(&self, agent_type: &str) -> Option<Arc<dyn Agent>> {
        self.read_agents().get(agent_type).and_then(|e| {
            if e.category == AgentCategory::Mode {
                Some(e.agent.clone())
            } else {
                None
            }
        })
    }

    /// check if a subagent exists with specified source (used for duplicate check before adding)
    pub fn has_subagent(&self, agent_id: &str, source: SubAgentSource) -> bool {
        self.read_agents().get(agent_id).map_or(false, |e| {
            e.category == AgentCategory::SubAgent && e.subagent_source == Some(source)
        })
    }

    /// get agent tools from config
    /// if not set, return default tools
    /// tool configuration synchronization is implemented through tool_config_sync, here only read configuration
    pub async fn get_agent_tools(&self, agent_type: &str) -> Vec<String> {
        let entry = self.read_agents().get(agent_type).cloned();
        let Some(entry) = entry else {
            return Vec::new();
        };
        match entry.category {
            AgentCategory::Mode => {
                let mode_configs = get_mode_configs().await;
                mode_configs
                    .get(agent_type)
                    .map(|config| config.available_tools.clone())
                    .unwrap_or_else(|| entry.agent.default_tools())
            }
            AgentCategory::SubAgent | AgentCategory::Hidden => entry.agent.default_tools(),
        }
    }

    /// get all mode agent information (including enabled status, used for frontend mode selector etc.)
    pub async fn get_modes_info(&self) -> Vec<AgentInfo> {
        let mode_configs = get_mode_configs().await;
        let map = self.read_agents();
        let mut result: Vec<AgentInfo> = map
            .values()
            .filter(|e| e.category == AgentCategory::Mode)
            .map(|e| {
                let mut agent_info = AgentInfo::from_agent_entry(e);
                let agent_type = &agent_info.id;
                agent_info.enabled = if agent_type == "agentic" {
                    true
                } else {
                    mode_configs
                        .get(agent_type)
                        .map(|config| config.enabled)
                        .unwrap_or(true)
                };
                agent_info
            })
            .collect();
        drop(map);
        result.sort_by(|a, b| {
            let order = |id: &str| -> u8 {
                match id {
                    "agentic" => 0,
                    "Plan" => 1,
                    "debug" => 2,
                    "Cowork" => 3,
                    _ => 99,
                }
            };
            order(&a.id).cmp(&order(&b.id))
        });
        result
    }

    /// check if a subagent is readonly (used for TaskTool.is_concurrency_safe etc.)
    pub fn get_subagent_is_readonly(&self, id: &str) -> Option<bool> {
        let map = self.read_agents();
        let entry = map.get(id)?;
        if entry.category != AgentCategory::SubAgent {
            return None;
        }
        Some(entry.agent.is_readonly())
    }

    /// get all subagent information (including source and enabled status, used for TaskTool, frontend subagent list etc.)
    /// - built-in subagent: read enabled status from global configuration ai.subagent_configs
    /// - custom subagent: read enabled and model configuration from custom_config cache
    pub async fn get_subagents_info(&self) -> Vec<AgentInfo> {
        let subagent_configs = get_subagent_configs().await;
        let map = self.read_agents();
        let result: Vec<AgentInfo> = map
            .values()
            .filter(|e| e.category == AgentCategory::SubAgent)
            .map(|e| {
                let mut agent_info = AgentInfo::from_agent_entry(e);
                agent_info.subagent_source = e.subagent_source;

                // custom subagent is already obtained from custom_config in from_agent_entry
                // built-in subagent needs to read enabled from global configuration
                if e.subagent_source == Some(SubAgentSource::Builtin) || e.custom_config.is_none() {
                    agent_info.enabled = subagent_configs
                        .get(&agent_info.id)
                        .map(|config| config.enabled)
                        .unwrap_or(true);
                }
                agent_info
            })
            .collect();
        drop(map);
        result
    }

    /// load custom subagent: clear project/user source subagents, reload from workspace and register
    pub async fn load_custom_subagents(&self, workspace_root: &Path) {
        // get valid tools and models list for verification
        let valid_tools = get_all_registered_tool_names().await;
        let valid_models = Self::get_valid_model_ids().await;

        let custom = CustomSubagentLoader::load_custom_subagents(workspace_root);
        let mut map = self.write_agents();
        // remove all non-built-in subagents
        map.retain(|_, e| {
            !(e.category == AgentCategory::SubAgent
                && e.subagent_source != Some(SubAgentSource::Builtin))
        });
        for mut sub in custom {
            let id = sub.id().to_string();
            let source = SubAgentSource::from_custom_kind(sub.kind);
            if map.contains_key(&id) {
                warn!(
                    "Custom subagent {} (source {:?}) conflicts with existing, skip",
                    id, source
                );
                continue;
            }
            // validate and correct tools and model
            Self::validate_custom_subagent(&mut sub, &valid_tools, &valid_models);
            // create CustomSubagentConfig cache configuration information
            let custom_config = CustomSubagentConfig {
                enabled: sub.enabled,
                model: sub.model.clone(),
            };
            map.insert(
                id,
                AgentEntry {
                    category: AgentCategory::SubAgent,
                    subagent_source: Some(source),
                    agent: Arc::new(sub),
                    custom_config: Some(custom_config),
                },
            );
        }
    }

    /// get valid model ID list: ai.models id + "primary" + "fast"
    async fn get_valid_model_ids() -> Vec<String> {
        let mut valid_models: Vec<String> =
            if let Ok(config_service) = GlobalConfigManager::get_service().await {
                config_service
                    .get_ai_models()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| m.id)
                    .collect()
            } else {
                Vec::new()
            };
        valid_models.push("primary".to_string());
        valid_models.push("fast".to_string());
        valid_models
    }

    /// validate and correct CustomSubagent's tools and model
    /// - tools: filter out invalid tools, record warning log
    /// - model: if invalid, set to "primary", record warning log
    fn validate_custom_subagent(
        subagent: &mut CustomSubagent,
        valid_tools: &[String],
        valid_models: &[String],
    ) {
        let agent_id = subagent.name.clone();

        // validate tools: filter out invalid tools
        let original_tools = subagent.tools.clone();
        let valid_tools_set: std::collections::HashSet<&str> =
            valid_tools.iter().map(|s| s.as_str()).collect();
        let (valid, invalid): (Vec<_>, Vec<_>) = original_tools
            .into_iter()
            .partition(|t| valid_tools_set.contains(t.as_str()));
        if !invalid.is_empty() {
            warn!(
                "[Subagent {}] Invalid tools filtered out: {:?}",
                agent_id, invalid
            );
        }
        subagent.tools = valid;

        // validate model: if invalid, set to "primary"
        if !valid_models.contains(&subagent.model) {
            warn!(
                "[Subagent {}] Invalid model '{}', reset to 'primary'",
                agent_id, subagent.model
            );
            subagent.model = "primary".to_string();
        }
    }

    /// clear all custom subagents (project/user source), only keep built-in subagents. called when closing workspace.
    pub fn clear_custom_subagents(&self) {
        let mut map = self.write_agents();
        let before = map
            .values()
            .filter(|e| e.category == AgentCategory::SubAgent)
            .count();
        map.retain(|_, e| {
            !(e.category == AgentCategory::SubAgent
                && e.subagent_source != Some(SubAgentSource::Builtin))
        });
        let after = map
            .values()
            .filter(|e| e.category == AgentCategory::SubAgent)
            .count();
        debug!(
            "Cleared custom subagents: subagents {} -> {}",
            before, after
        );
    }

    /// get custom subagent configuration (used for updating configuration)
    /// only custom subagent is valid, return clone of CustomSubagentConfig
    pub fn get_custom_subagent_config(&self, agent_id: &str) -> Option<CustomSubagentConfig> {
        let map = self.read_agents();
        let entry = map.get(agent_id)?;
        if entry.category != AgentCategory::SubAgent {
            return None;
        }
        entry.custom_config.clone()
    }

    /// update custom subagent configuration and save to file
    /// use as_any() downcast to get prompt etc. data from memory, no need to re-read file
    pub fn update_and_save_custom_subagent_config(
        &self,
        agent_id: &str,
        enabled: Option<bool>,
        model: Option<String>,
    ) -> BitFunResult<()> {
        let mut map = self.write_agents();
        let entry = map
            .get_mut(agent_id)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;

        if entry.category != AgentCategory::SubAgent {
            return Err(BitFunError::agent(format!(
                "Agent '{}' is not a subagent",
                agent_id
            )));
        }

        let config = entry.custom_config.as_mut().ok_or_else(|| {
            BitFunError::agent(format!("Subagent '{}' is not a custom subagent", agent_id))
        })?;

        // calculate new enabled and model values
        let new_enabled = enabled.unwrap_or(config.enabled);
        let new_model = model.unwrap_or_else(|| config.model.clone());

        // get CustomSubagent reference by as_any() downcast
        let custom_subagent = entry
            .agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .ok_or_else(|| {
                BitFunError::agent(format!(
                    "Failed to downcast agent '{}' to CustomSubagent",
                    agent_id
                ))
            })?;

        // save file with data in memory (no need to re-read)
        custom_subagent.save_to_file(Some(new_enabled), Some(&new_model))?;

        // update memory cache
        config.enabled = new_enabled;
        config.model = new_model;

        Ok(())
    }

    /// remove single non-built-in subagent, return its file path (used for caller to delete file)
    /// only allow removing entries that are SubAgent and not Builtin
    pub fn remove_subagent(&self, agent_id: &str) -> BitFunResult<Option<String>> {
        let mut map = self.write_agents();
        let entry = map
            .get(agent_id)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;
        if entry.category != AgentCategory::SubAgent {
            return Err(BitFunError::agent(format!(
                "Agent '{}' is not a subagent",
                agent_id
            )));
        }
        if entry.subagent_source == Some(SubAgentSource::Builtin) {
            return Err(BitFunError::agent(format!(
                "Cannot remove built-in subagent: {}",
                agent_id
            )));
        }
        // get path by downcast
        let path = entry
            .agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .map(|c| c.path.clone());
        map.remove(agent_id);
        Ok(path)
    }

    /// get model ID used by agent from agent_models[agent_type] in configuration
    /// - custom subagent: read model configuration from custom_config cache
    /// - built-in subagent/mode: read model configuration from global configuration ai.agent_models
    pub async fn get_model_id_for_agent(&self, agent_type: &str) -> BitFunResult<String> {
        // check if agent exists
        if !self.check_agent_exists(agent_type) {
            error!("[AgentRegistry] Agent not found: {}", agent_type);
            return Err(BitFunError::agent(format!(
                "[AgentRegistry] Agent not found: {}",
                agent_type
            )));
        }

        // check if it is a custom subagent, if so, read from cache
        if let Some(config) = self.get_custom_subagent_config(agent_type) {
            let model = config.model;
            if !model.is_empty() {
                debug!(
                    "[AgentRegistry] Custom subagent '{}' using model from cache: {}",
                    agent_type, model
                );
                return Ok(model);
            }
            // empty model, use default value
            debug!(
                "[AgentRegistry] Custom subagent '{}' using default model: primary",
                agent_type
            );
            return Ok("primary".to_string());
        }

        // built-in subagent/mode: read from global configuration
        if let Ok(config_service) = GlobalConfigManager::get_service().await {
            let global_config: GlobalConfig = config_service.get_config(None).await?;

            // check agent_models configuration
            if let Some(model_id) = global_config.ai.agent_models.get(agent_type) {
                if !model_id.is_empty() {
                    return Ok(model_id.clone());
                }
            }
        } else {
            // config service not available
            error!(
                "[AgentRegistry] Config service not available, cannot get model config for Agent '{}'",
                agent_type
            )
        };

        // use default primary model
        warn!(
            "[AgentRegistry] Agent '{}' has no model configured, using default primary model",
            agent_type
        );
        Ok("primary".to_string())
    }

    /// Get the default agent type
    pub fn default_agent_type(&self) -> &str {
        "agentic"
    }
}

// Global agent registry singleton
static GLOBAL_AGENT_REGISTRY: OnceLock<Arc<AgentRegistry>> = OnceLock::new();

/// Get the global agent registry
pub fn get_agent_registry() -> Arc<AgentRegistry> {
    GLOBAL_AGENT_REGISTRY
        .get_or_init(|| {
            debug!("Initializing global agent registry");
            Arc::new(AgentRegistry::new())
        })
        .clone()
}
