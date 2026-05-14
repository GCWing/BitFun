use super::availability::resolve_availability;
use super::support::{
    get_mode_configs, get_subagent_overrides, load_project_subagent_overrides_local,
    merge_dynamic_mcp_tools,
};
use super::AgentRegistry;
use crate::agentic::agents::registry::types::{is_review_agent_entry, AgentEntry};
use crate::agentic::agents::{
    AgentCategory, AgentInfo, AgentToolPolicy, SubagentListScope, SubagentQueryContext,
};
use crate::agentic::tools::get_all_registered_tool_names;
use crate::service::config::mode_config_canonicalizer::resolve_effective_tools;
use std::collections::HashSet;
use std::path::Path;

impl AgentRegistry {
    fn subagent_source_rank(source: Option<crate::agentic::agents::SubAgentSource>) -> u8 {
        match source {
            Some(crate::agentic::agents::SubAgentSource::Builtin) => 0,
            Some(crate::agentic::agents::SubAgentSource::Project) => 1,
            Some(crate::agentic::agents::SubAgentSource::User) => 2,
            None => 3,
        }
    }

    fn sort_subagents_for_presentation(mut result: Vec<AgentInfo>) -> Vec<AgentInfo> {
        result.sort_by(|a, b| {
            Self::subagent_source_rank(a.subagent_source)
                .cmp(&Self::subagent_source_rank(b.subagent_source))
                .then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.name.cmp(&b.name))
        });
        result
    }

    /// Resolve the current tool policy for an agent.
    ///
    /// This returns both the allowed tool set and any per-agent exposure
    /// overrides that should be applied on top of tool defaults.
    pub async fn get_agent_tool_policy(
        &self,
        agent_type: &str,
        workspace_root: Option<&Path>,
    ) -> AgentToolPolicy {
        let entry = self.find_agent_entry(agent_type, workspace_root);
        let Some(entry) = entry else {
            return AgentToolPolicy {
                allowed_tools: Vec::new(),
                exposure_overrides: Default::default(),
            };
        };
        match entry.category {
            AgentCategory::Mode => {
                let mode_configs = get_mode_configs().await;
                let registered_tool_names = get_all_registered_tool_names().await;
                let valid_tools: HashSet<String> = registered_tool_names.iter().cloned().collect();
                let resolved_tools = resolve_effective_tools(
                    &entry.agent.default_tools(),
                    mode_configs.get(agent_type),
                    &valid_tools,
                );
                let allowed_tools = merge_dynamic_mcp_tools(resolved_tools, &registered_tool_names);
                let allowed_tool_set: HashSet<&str> =
                    allowed_tools.iter().map(String::as_str).collect();
                let mut exposure_overrides = entry.agent.tool_exposure_overrides().clone();
                exposure_overrides.retain(|tool_name, _| allowed_tool_set.contains(tool_name.as_str()));

                AgentToolPolicy {
                    allowed_tools,
                    exposure_overrides,
                }
            }
            AgentCategory::SubAgent | AgentCategory::Hidden => {
                let allowed_tools = entry.agent.default_tools();
                let allowed_tool_set: HashSet<&str> =
                    allowed_tools.iter().map(String::as_str).collect();
                let mut exposure_overrides = entry.agent.tool_exposure_overrides().clone();
                exposure_overrides.retain(|tool_name, _| allowed_tool_set.contains(tool_name.as_str()));

                AgentToolPolicy {
                    allowed_tools,
                    exposure_overrides,
                }
            }
        }
    }

    /// get agent tools from config
    /// if not set, return default tools
    /// mode config canonicalization is handled separately; this only reads resolved configuration
    pub async fn get_agent_tools(
        &self,
        agent_type: &str,
        workspace_root: Option<&Path>,
    ) -> Vec<String> {
        self.get_agent_tool_policy(agent_type, workspace_root)
            .await
            .allowed_tools
    }

    /// get all mode agent information, used for frontend mode selector etc.
    pub async fn get_modes_info(&self) -> Vec<AgentInfo> {
        let map = self.read_agents();
        let mut result: Vec<AgentInfo> = map
            .values()
            .filter(|e| e.category == AgentCategory::Mode)
            .map(AgentInfo::from_agent_entry)
            .collect();
        drop(map);
        result.sort_by(|a, b| {
            let order = |id: &str| -> u8 {
                match id {
                    "agentic" => 0,
                    "Cowork" => 1,
                    "Plan" => 2,
                    "debug" => 3,
                    "DeepResearch" => 4,
                    "Team" => 5,
                    _ => 99,
                }
            };
            order(&a.id).cmp(&order(&b.id))
        });
        result
    }

    /// check if a subagent is readonly (used for TaskTool.is_concurrency_safe etc.)
    pub fn get_subagent_is_readonly(&self, id: &str) -> Option<bool> {
        if let Some(entry) = self.read_agents().get(id) {
            if entry.category == AgentCategory::SubAgent {
                return Some(entry.agent.is_readonly());
            }
        }

        for entries in self.read_project_subagents().values() {
            if let Some(entry) = entries.get(id) {
                if entry.category == AgentCategory::SubAgent {
                    return Some(entry.agent.is_readonly());
                }
            }
        }

        None
    }

    pub fn get_subagent_is_review(&self, id: &str) -> Option<bool> {
        if let Some(entry) = self.read_agents().get(id) {
            if entry.category == AgentCategory::SubAgent {
                return Some(is_review_agent_entry(entry));
            }
        }

        for entries in self.read_project_subagents().values() {
            if let Some(entry) = entries.get(id) {
                if entry.category == AgentCategory::SubAgent {
                    return Some(is_review_agent_entry(entry));
                }
            }
        }

        None
    }

    fn entry_is_visible_for_query(
        entry: &AgentEntry,
        query: &SubagentQueryContext<'_>,
        project_overrides: Option<&crate::service::config::types::AgentSubagentOverrideConfig>,
        user_overrides: &crate::service::config::types::AgentSubagentOverrideConfig,
    ) -> bool {
        if entry.category != AgentCategory::SubAgent {
            return false;
        }

        let availability = resolve_availability(
            entry,
            query.parent_agent_type,
            project_overrides,
            user_overrides,
        );
        if !query.include_disabled && !availability.effective_enabled {
            return false;
        }

        match query.list_scope {
            SubagentListScope::RegistryManagement => entry.visibility_policy.show_in_global_registry,
            SubagentListScope::TaskVisible => {
                entry.visibility_policy.show_in_global_registry
                    || entry
                        .visibility_policy
                        .can_access_from_parent(query.parent_agent_type)
            }
        }
    }

    /// get all subagent information (including source and availability status, used for TaskTool and frontend subagent list etc.)
    pub async fn get_subagents_info(&self, workspace_root: Option<&Path>) -> Vec<AgentInfo> {
        self.get_subagents_for_query(&SubagentQueryContext {
            parent_agent_type: None,
            workspace_root,
            list_scope: SubagentListScope::RegistryManagement,
            include_disabled: true,
        })
        .await
    }

    pub async fn get_subagents_for_query(
        &self,
        query: &SubagentQueryContext<'_>,
    ) -> Vec<AgentInfo> {
        if let Some(workspace_root) = query.workspace_root {
            let is_project_cache_loaded =
                self.read_project_subagents().contains_key(workspace_root);
            if !is_project_cache_loaded {
                self.load_custom_subagents(workspace_root).await;
            }
        }

        let user_overrides = get_subagent_overrides().await;
        let project_overrides = match query.workspace_root {
            Some(workspace_root) => load_project_subagent_overrides_local(workspace_root)
                .await
                .ok(),
            None => None,
        };
        let map = self.read_agents();
        let mut result: Vec<AgentInfo> = map
            .values()
            .filter(|entry| {
                Self::entry_is_visible_for_query(
                    entry,
                    query,
                    project_overrides.as_ref(),
                    &user_overrides,
                )
            })
            .map(|e| {
                let mut agent_info = AgentInfo::from_agent_entry(e);
                let availability = resolve_availability(
                    e,
                    query.parent_agent_type,
                    project_overrides.as_ref(),
                    &user_overrides,
                );
                agent_info.subagent_source = e.subagent_source;
                agent_info.default_enabled = availability.default_enabled;
                agent_info.effective_enabled = availability.effective_enabled;
                agent_info.override_state = availability.override_state;
                agent_info.state_reason = availability.state_reason;
                agent_info
            })
            .collect();
        drop(map);
        if let Some(workspace_root) = query.workspace_root {
            if let Some(project_entries) = self.read_project_subagents().get(workspace_root) {
                result.extend(
                    project_entries
                        .values()
                        .filter(|entry| {
                            Self::entry_is_visible_for_query(
                                entry,
                                query,
                                project_overrides.as_ref(),
                                &user_overrides,
                            )
                        })
                        .map(|entry| {
                            let mut info = AgentInfo::from_agent_entry(entry);
                            let availability = resolve_availability(
                                entry,
                                query.parent_agent_type,
                                project_overrides.as_ref(),
                                &user_overrides,
                            );
                            info.default_enabled = availability.default_enabled;
                            info.effective_enabled = availability.effective_enabled;
                            info.override_state = availability.override_state;
                            info.state_reason = availability.state_reason;
                            info
                        }),
                );
            }
        }
        Self::sort_subagents_for_presentation(result)
    }

    pub async fn can_parent_access_subagent(
        &self,
        subagent_id: &str,
        workspace_root: Option<&Path>,
        parent_agent_type: Option<&str>,
    ) -> bool {
        let query = SubagentQueryContext {
            parent_agent_type,
            workspace_root,
            list_scope: SubagentListScope::TaskVisible,
            include_disabled: false,
        };
        let user_overrides = get_subagent_overrides().await;
        let project_overrides = match query.workspace_root {
            Some(workspace_root) => load_project_subagent_overrides_local(workspace_root)
                .await
                .ok(),
            None => None,
        };

        if let Some(workspace_root) = query.workspace_root {
            let is_project_cache_loaded = self.read_project_subagents().contains_key(workspace_root);
            if !is_project_cache_loaded {
                self.load_custom_subagents(workspace_root).await;
            }
        }

        self.find_agent_entry(subagent_id, workspace_root)
            .is_some_and(|entry| {
                Self::entry_is_visible_for_query(
                    &entry,
                    &query,
                    project_overrides.as_ref(),
                    &user_overrides,
                )
            })
    }
}
