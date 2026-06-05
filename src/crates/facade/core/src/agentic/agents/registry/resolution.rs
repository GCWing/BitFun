use super::builtin::default_model_id_for_builtin_agent;
use super::AgentRegistry;
use crate::service::config::global::GlobalConfigManager;
use crate::service::config::GlobalConfig;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, error, warn};
use std::path::Path;

impl AgentRegistry {
    /// get model ID used by agent from agent_models[agent_type] in configuration
    /// - custom subagent: read model configuration from custom_config cache
    /// - built-in subagent/mode: read model configuration from global configuration ai.agent_models
    pub async fn get_model_id_for_agent(
        &self,
        agent_type: &str,
        workspace_root: Option<&Path>,
    ) -> BitFunResult<String> {
        if self.find_agent_entry(agent_type, workspace_root).is_none() {
            error!("[AgentRegistry] Agent not found: {}", agent_type);
            return Err(BitFunError::agent(format!(
                "[AgentRegistry] Agent not found: {}",
                agent_type
            )));
        }

        if let Some(entry) = self.find_agent_entry(agent_type, workspace_root) {
            if let Some(config) = entry.custom_config {
                let model = config.model;
                if !model.is_empty() {
                    debug!(
                        "[AgentRegistry] Custom subagent '{}' using model from cache: {}",
                        agent_type, model
                    );
                    return Ok(model);
                }

                debug!(
                    "[AgentRegistry] Custom subagent '{}' using default model: fast",
                    agent_type
                );
                return Ok("fast".to_string());
            }
        }

        if let Ok(config_service) = GlobalConfigManager::get_service().await {
            let global_config: GlobalConfig = config_service.get_config(None).await?;
            if let Some(model_id) = global_config.ai.agent_models.get(agent_type) {
                if !model_id.is_empty() {
                    return Ok(model_id.clone());
                }
            }
        } else {
            error!(
                "[AgentRegistry] Config service not available, cannot get model config for Agent '{}'",
                agent_type
            )
        };

        let default_model_id = default_model_id_for_builtin_agent(agent_type);
        warn!(
            "[AgentRegistry] Agent '{}' has no model configured, using default model '{}'",
            agent_type, default_model_id
        );
        Ok(default_model_id.to_string())
    }

    pub fn default_agent_type(&self) -> &str {
        "agentic"
    }
}
