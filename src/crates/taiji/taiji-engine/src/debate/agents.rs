use serde::{Deserialize, Serialize};
use std::sync::Arc;

use taiji_llm::{ChatMessage, ChatResponse, LlmClient, LlmConfig};

use super::DebateConfig;

/// Pre-defined debate agent roles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentRole {
    Bull,
    Bear,
    Neutral,
}

impl AgentRole {
    pub fn id(&self) -> &'static str {
        match self {
            AgentRole::Bull => "bull",
            AgentRole::Bear => "bear",
            AgentRole::Neutral => "neutral",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AgentRole::Bull => "多方 (Bull)",
            AgentRole::Bear => "空方 (Bear)",
            AgentRole::Neutral => "中立方 (Neutral)",
        }
    }

    pub fn default_direction(&self) -> &'static str {
        match self {
            AgentRole::Bull => "long",
            AgentRole::Bear => "short",
            AgentRole::Neutral => "hold",
        }
    }
}

/// A debate agent bound to a specific role and LLM client.
#[derive(Clone)]
pub struct DebateAgent {
    pub role: AgentRole,
    pub llm_client: Arc<dyn LlmClient>,
    pub config: LlmConfig,
    pub system_prompt: String,
}

impl DebateAgent {
    /// Create a new debate agent with the given role and debate config.
    pub fn new(
        role: AgentRole,
        llm_client: Arc<dyn LlmClient>,
        debate_config: &DebateConfig,
    ) -> Self {
        let system_prompt = match role {
            AgentRole::Bull => debate_config.bull_prompt_template.clone(),
            AgentRole::Bear => debate_config.bear_prompt_template.clone(),
            AgentRole::Neutral => debate_config.neutral_prompt_template.clone(),
        };

        let config = LlmConfig {
            model: debate_config.model.clone(),
            temperature: debate_config.temperature,
            max_tokens: 2048,
            api_key: None,
            base_url: None,
        };

        Self {
            role,
            llm_client,
            config,
            system_prompt,
        }
    }

    /// Send a prompt to this agent and return its response.
    pub async fn respond(&self, prompt: &str) -> Result<ChatResponse, anyhow::Error> {
        let messages = vec![
            ChatMessage::system(&self.system_prompt),
            ChatMessage::user(prompt),
        ];
        self.llm_client.chat(&messages, &self.config).await
    }
}
