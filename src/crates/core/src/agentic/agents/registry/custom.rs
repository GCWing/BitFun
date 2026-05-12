use super::types::AgentEntry;
use super::custom_loader::CustomSubagentLoader;
use super::{CustomSubagentDetail, AgentRegistry};
use crate::agentic::agents::{
    Agent, AgentCategory, CustomSubagentConfig, SubAgentSource,
};
use crate::agentic::agents::definitions::custom::{CustomSubagent, CustomSubagentKind};
use crate::agentic::agents::registry::visibility::SubagentVisibilityPolicy;
use crate::agentic::tools::{get_all_registered_tool_names, get_readonly_registered_tool_names};
use crate::service::config::global::GlobalConfigManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

impl AgentRegistry {
    /// load custom subagent: clear project/user source subagents, reload from workspace and register
    pub async fn load_custom_subagents(&self, workspace_root: &Path) {
        // get valid tools and models list for verification
        let valid_tools = get_all_registered_tool_names().await;
        let readonly_tools = get_readonly_registered_tool_names().await;
        let valid_models = Self::get_valid_model_ids().await;

        let custom = CustomSubagentLoader::load_custom_subagents(workspace_root);
        let mut map = self.write_agents();
        map.retain(|_, entry| {
            !(entry.category == AgentCategory::SubAgent
                && entry.subagent_source == Some(SubAgentSource::User))
        });
        let mut project_entries = HashMap::new();
        for mut sub in custom {
            let id = sub.id().to_string();
            let source = SubAgentSource::from_custom_kind(sub.kind);
            // validate and correct tools and model
            Self::validate_custom_subagent(&mut sub, &valid_tools, &readonly_tools, &valid_models);
            // create CustomSubagentConfig cache configuration information
            let custom_config = CustomSubagentConfig {
                enabled: sub.enabled,
                model: sub.model.clone(),
            };
            let entry = AgentEntry {
                category: AgentCategory::SubAgent,
                subagent_source: Some(source),
                agent: Arc::new(sub),
                visibility_policy: SubagentVisibilityPolicy::public(),
                custom_config: Some(custom_config),
            };

            match source {
                SubAgentSource::User => {
                    if map.contains_key(&id) {
                        warn!(
                            "Custom subagent {} (source {:?}) conflicts with existing, skip",
                            id, source
                        );
                        continue;
                    }
                    map.insert(id, entry);
                }
                SubAgentSource::Project => {
                    if map.contains_key(&id) {
                        warn!(
                            "Custom subagent {} (source {:?}) conflicts with existing, skip",
                            id, source
                        );
                        continue;
                    }
                    project_entries.insert(id, entry);
                }
                SubAgentSource::Builtin => {}
            }
        }
        drop(map);
        self.write_project_subagents()
            .insert(workspace_root.to_path_buf(), project_entries);
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
    /// - model: if invalid, set to "fast", record warning log
    fn validate_custom_subagent(
        subagent: &mut CustomSubagent,
        valid_tools: &[String],
        readonly_tools: &[String],
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
        if subagent.review {
            subagent.readonly = true;
            let readonly_tools_set: std::collections::HashSet<&str> =
                readonly_tools.iter().map(|s| s.as_str()).collect();
            let (review_tools, writable_tools): (Vec<_>, Vec<_>) = valid
                .into_iter()
                .partition(|t| readonly_tools_set.contains(t.as_str()));
            if !writable_tools.is_empty() {
                warn!(
                    "[Subagent {}] Writable tools filtered out from review subagent: {:?}",
                    agent_id, writable_tools
                );
            }
            subagent.tools = review_tools;
        } else {
            subagent.tools = valid;
        }

        // validate model: if invalid, set to "fast"
        if !valid_models.contains(&subagent.model) {
            warn!(
                "[Subagent {}] Invalid model '{}', reset to 'fast'",
                agent_id, subagent.model
            );
            subagent.model = "fast".to_string();
        }
    }

    fn ensure_review_tools_are_readonly(
        agent_id: &str,
        tools: &[String],
        readonly_tools: &[String],
    ) -> BitFunResult<()> {
        let readonly_tools_set: std::collections::HashSet<&str> =
            readonly_tools.iter().map(|s| s.as_str()).collect();
        let writable_tools: Vec<&str> = tools
            .iter()
            .map(String::as_str)
            .filter(|tool| !readonly_tools_set.contains(tool))
            .collect();

        if writable_tools.is_empty() {
            return Ok(());
        }

        Err(BitFunError::agent(format!(
            "Review Sub-Agent '{}' can only use read-only tools; remove writable tools: {}",
            agent_id,
            writable_tools.join(", ")
        )))
    }

    /// clear all custom subagents (project/user source), only keep built-in subagents. called when closing workspace.
    pub fn clear_custom_subagents(&self) {
        let before = self.read_project_subagents().len();
        self.write_project_subagents().clear();
        debug!("Cleared project subagent caches: workspaces {}", before);
    }

    /// get custom subagent configuration (used for updating configuration)
    /// only custom subagent is valid, return clone of CustomSubagentConfig
    pub fn get_custom_subagent_config(
        &self,
        agent_id: &str,
        workspace_root: Option<&Path>,
    ) -> Option<CustomSubagentConfig> {
        if let Some(entry) = self.read_agents().get(agent_id) {
            if entry.category == AgentCategory::SubAgent {
                return entry.custom_config.clone();
            }
        }

        workspace_root
            .and_then(|root| self.read_project_subagents().get(root).cloned())
            .and_then(|entries| entries.get(agent_id).cloned())
            .and_then(|entry| {
                (entry.category == AgentCategory::SubAgent)
                    .then(|| entry.custom_config)
                    .flatten()
            })
    }

    pub fn has_project_custom_subagent(&self, agent_id: &str) -> bool {
        self.read_project_subagents().values().any(|entries| {
            entries.get(agent_id).is_some_and(|entry| {
                entry.category == AgentCategory::SubAgent
                    && entry.subagent_source == Some(SubAgentSource::Project)
                    && entry.custom_config.is_some()
            })
        })
    }

    /// update custom subagent configuration and save to file
    /// use as_any() downcast to get prompt etc. data from memory, no need to re-read file
    pub fn update_and_save_custom_subagent_config(
        &self,
        agent_id: &str,
        enabled: Option<bool>,
        model: Option<String>,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<()> {
        let mut map = self.write_agents();
        if let Some(entry) = map.get_mut(agent_id) {
            return Self::update_custom_entry_config(agent_id, entry, enabled, model);
        }
        drop(map);

        let workspace_root = workspace_root.ok_or_else(|| {
            BitFunError::agent(format!(
                "workspace_path is required to update project subagent '{}'",
                agent_id
            ))
        })?;
        let mut project_maps = self.write_project_subagents();
        let entries = project_maps.get_mut(workspace_root).ok_or_else(|| {
            BitFunError::agent(format!(
                "Project subagents are not loaded for workspace: {}",
                workspace_root.display()
            ))
        })?;
        let entry = entries
            .get_mut(agent_id)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;

        Self::update_custom_entry_config(agent_id, entry, enabled, model)
    }

    fn update_custom_entry_config(
        agent_id: &str,
        entry: &mut AgentEntry,
        enabled: Option<bool>,
        model: Option<String>,
    ) -> BitFunResult<()> {
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

    /// Load custom subagents if needed, then return full definition for the editor UI
    pub async fn get_custom_subagent_detail(
        &self,
        agent_id: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<CustomSubagentDetail> {
        if let Some(root) = workspace_root {
            self.load_custom_subagents(root).await;
        }
        self.get_custom_subagent_detail_inner(agent_id, workspace_root)
    }

    fn get_custom_subagent_detail_inner(
        &self,
        agent_id: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<CustomSubagentDetail> {
        let entry = self
            .find_agent_entry(agent_id, workspace_root)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;
        if entry.category != AgentCategory::SubAgent {
            return Err(BitFunError::agent(format!(
                "Agent '{}' is not a subagent",
                agent_id
            )));
        }
        if entry.subagent_source == Some(SubAgentSource::Builtin) {
            return Err(BitFunError::agent(
                "Built-in subagents cannot be edited here".to_string(),
            ));
        }
        let custom = entry
            .agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .ok_or_else(|| {
                BitFunError::agent(format!(
                    "Subagent '{}' is not a custom subagent file",
                    agent_id
                ))
            })?;
        let (enabled, model) = match &entry.custom_config {
            Some(c) => (c.enabled, c.model.clone()),
            None => (custom.enabled, custom.model.clone()),
        };
        let level = match custom.kind {
            CustomSubagentKind::User => "user",
            CustomSubagentKind::Project => "project",
        };
        Ok(CustomSubagentDetail {
            subagent_id: agent_id.to_string(),
            name: custom.name.clone(),
            description: custom.description.clone(),
            prompt: custom.prompt.clone(),
            tools: custom.tools.clone(),
            readonly: custom.readonly,
            review: custom.review,
            enabled,
            model,
            path: custom.path.clone(),
            level: level.to_string(),
        })
    }

    /// Update description, prompt, tools, and readonly for a custom sub-agent (id and file path unchanged)
    pub async fn update_custom_subagent_definition(
        &self,
        agent_id: &str,
        workspace_root: Option<&Path>,
        description: String,
        prompt: String,
        tools: Option<Vec<String>>,
        readonly: Option<bool>,
        review: Option<bool>,
    ) -> BitFunResult<()> {
        if let Some(root) = workspace_root {
            self.load_custom_subagents(root).await;
        }
        let entry = self
            .find_agent_entry(agent_id, workspace_root)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;
        if entry.category != AgentCategory::SubAgent {
            return Err(BitFunError::agent(format!(
                "Agent '{}' is not a subagent",
                agent_id
            )));
        }
        if entry.subagent_source == Some(SubAgentSource::Builtin) {
            return Err(BitFunError::agent(
                "Built-in subagents cannot be edited".to_string(),
            ));
        }
        let old = entry
            .agent
            .as_any()
            .downcast_ref::<CustomSubagent>()
            .ok_or_else(|| {
                BitFunError::agent(format!(
                    "Subagent '{}' is not a custom subagent file",
                    agent_id
                ))
            })?;
        let tools = tools.filter(|t| !t.is_empty()).unwrap_or_else(|| {
            vec![
                "LS".to_string(),
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
            ]
        });
        let review = review.unwrap_or(old.review);
        let valid_tools = get_all_registered_tool_names().await;
        let readonly_tools = get_readonly_registered_tool_names().await;
        if review {
            Self::ensure_review_tools_are_readonly(agent_id, &tools, &readonly_tools)?;
        }
        let mut new_subagent = CustomSubagent::new(
            old.name.clone(),
            description,
            tools,
            prompt,
            if review {
                true
            } else {
                readonly.unwrap_or(old.readonly)
            },
            old.path.clone(),
            old.kind,
        );
        new_subagent.review = review;
        new_subagent.enabled = old.enabled;
        new_subagent.model = old.model.clone();

        let valid_models = Self::get_valid_model_ids().await;
        Self::validate_custom_subagent(
            &mut new_subagent,
            &valid_tools,
            &readonly_tools,
            &valid_models,
        );

        new_subagent.save_to_file(None, None)?;

        self.replace_custom_subagent_entry(agent_id, workspace_root, new_subagent)
    }

    fn replace_custom_subagent_entry(
        &self,
        agent_id: &str,
        workspace_root: Option<&Path>,
        new_subagent: CustomSubagent,
    ) -> BitFunResult<()> {
        let mut map = self.write_agents();
        if map.contains_key(agent_id) {
            let old_entry = map
                .get(agent_id)
                .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;
            if old_entry.category != AgentCategory::SubAgent {
                return Err(BitFunError::agent(format!(
                    "Agent '{}' is not a subagent",
                    agent_id
                )));
            }
            if old_entry.subagent_source == Some(SubAgentSource::Builtin) {
                return Err(BitFunError::agent(
                    "Cannot replace built-in subagent".to_string(),
                ));
            }
            let subagent_source = old_entry.subagent_source;
            let cfg = CustomSubagentConfig {
                enabled: new_subagent.enabled,
                model: new_subagent.model.clone(),
            };
            map.insert(
                agent_id.to_string(),
                AgentEntry {
                    category: AgentCategory::SubAgent,
                    subagent_source,
                    agent: Arc::new(new_subagent),
                    visibility_policy: SubagentVisibilityPolicy::public(),
                    custom_config: Some(cfg),
                },
            );
            return Ok(());
        }
        drop(map);

        let root = workspace_root.ok_or_else(|| {
            BitFunError::agent("Workspace path is required to update project subagent".to_string())
        })?;
        let mut pm = self.write_project_subagents();
        let entries = pm.get_mut(root).ok_or_else(|| {
            BitFunError::agent("Project subagent cache not loaded for this workspace".to_string())
        })?;
        let old_entry = entries
            .get(agent_id)
            .ok_or_else(|| BitFunError::agent(format!("Subagent not found: {}", agent_id)))?;
        if old_entry.category != AgentCategory::SubAgent {
            return Err(BitFunError::agent(format!(
                "Agent '{}' is not a subagent",
                agent_id
            )));
        }
        if old_entry.subagent_source == Some(SubAgentSource::Builtin) {
            return Err(BitFunError::agent(
                "Cannot replace built-in subagent".to_string(),
            ));
        }
        let subagent_source = old_entry.subagent_source;
        let cfg = CustomSubagentConfig {
            enabled: new_subagent.enabled,
            model: new_subagent.model.clone(),
        };
        entries.insert(
            agent_id.to_string(),
            AgentEntry {
                category: AgentCategory::SubAgent,
                subagent_source,
                agent: Arc::new(new_subagent),
                visibility_policy: SubagentVisibilityPolicy::public(),
                custom_config: Some(cfg),
            },
        );
        Ok(())
    }

    /// remove single non-built-in subagent, return its file path (used for caller to delete file)
    /// only allow removing entries that are SubAgent and not Builtin
    pub fn remove_subagent(&self, agent_id: &str) -> BitFunResult<Option<String>> {
        let mut map = self.write_agents();
        if let Some(entry) = map.get(agent_id) {
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
            let path = entry
                .agent
                .as_any()
                .downcast_ref::<CustomSubagent>()
                .map(|c| c.path.clone());
            map.remove(agent_id);
            return Ok(path);
        }
        drop(map);

        let mut project_maps = self.write_project_subagents();
        for entries in project_maps.values_mut() {
            if let Some(entry) = entries.get(agent_id) {
                if entry.category != AgentCategory::SubAgent {
                    return Err(BitFunError::agent(format!(
                        "Agent '{}' is not a subagent",
                        agent_id
                    )));
                }
                let path = entry
                    .agent
                    .as_any()
                    .downcast_ref::<CustomSubagent>()
                    .map(|c| c.path.clone());
                entries.remove(agent_id);
                return Ok(path);
            }
        }

        Err(BitFunError::agent(format!(
            "Subagent not found: {}",
            agent_id
        )))
    }
}
