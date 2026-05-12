pub mod types;
mod builtin;
pub mod catalog;
mod custom;
mod custom_loader;
mod query;
mod resolution;
mod support;
pub mod visibility;
#[cfg(test)]
mod tests;

use super::{
    Agent,
};
use self::types::AgentEntry;
use self::types::{
    AgentCategory, SubAgentSource,
};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::{Arc, OnceLock};

/// Full sub-agent definition for editing (user/project custom agents only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomSubagentDetail {
    pub subagent_id: String,
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub readonly: bool,
    pub review: bool,
    pub enabled: bool,
    pub model: String,
    pub path: String,
    /// `"user"` or `"project"`
    pub level: String,
}

/// Registry for managing all available agents
pub struct AgentRegistry {
    /// id -> agent_entry
    agents: RwLock<HashMap<String, AgentEntry>>,
    /// workspace root -> (project subagent id -> agent_entry)
    project_subagents: RwLock<HashMap<PathBuf, HashMap<String, AgentEntry>>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    fn read_project_subagents(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<PathBuf, HashMap<String, AgentEntry>>> {
        match self.project_subagents.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Agent project registry read lock poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    fn write_project_subagents(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<PathBuf, HashMap<String, AgentEntry>>> {
        match self.project_subagents.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Agent project registry write lock poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    fn find_agent_entry(
        &self,
        agent_type: &str,
        workspace_root: Option<&Path>,
    ) -> Option<AgentEntry> {
        if let Some(entry) = self.read_agents().get(agent_type).cloned() {
            return Some(entry);
        }

        let workspace_root = workspace_root?;
        self.read_project_subagents()
            .get(workspace_root)
            .and_then(|entries| entries.get(agent_type).cloned())
    }

    /// Get a agent by ID (searches all categories including hidden)
    pub fn get_agent(
        &self,
        agent_type: &str,
        workspace_root: Option<&Path>,
    ) -> Option<Arc<dyn Agent>> {
        self.find_agent_entry(agent_type, workspace_root)
            .map(|entry| entry.agent)
    }

    /// Check if an agent exists
    pub fn check_agent_exists(&self, agent_type: &str) -> bool {
        self.read_agents().contains_key(agent_type)
            || self
                .read_project_subagents()
                .values()
                .any(|entries| entries.contains_key(agent_type))
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
        if self.read_agents().get(agent_id).is_some_and(|e| {
            e.category == AgentCategory::SubAgent && e.subagent_source == Some(source)
        }) {
            return true;
        }

        self.read_project_subagents().values().any(|entries| {
            entries.get(agent_id).is_some_and(|entry| {
                entry.category == AgentCategory::SubAgent && entry.subagent_source == Some(source)
            })
        })
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
