use std::sync::Arc;

use taiji_llm::{ChatMessage, DecisionOutput, LlmClient, LlmConfig};

use super::DebateConfig;

/// Configuration for the DecisionAgent.
#[derive(Debug, Clone)]
pub struct DecisionConfig {
    /// LLM model for the decision agent
    pub model: String,
    /// Temperature for decision LLM calls
    pub temperature: f32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            model: "deepseek-chat".into(),
            temperature: 0.3,
        }
    }
}

/// The DecisionAgent synthesizes the debate record and issues a final verdict.
#[derive(Clone)]
pub struct DecisionAgent {
    pub llm_client: Arc<dyn LlmClient>,
    pub config: DecisionConfig,
    pub system_prompt: String,
}

impl DecisionAgent {
    /// Create a new DecisionAgent from debate configuration.
    pub fn new(llm_client: Arc<dyn LlmClient>, debate_config: &DebateConfig) -> Self {
        Self {
            llm_client,
            config: DecisionConfig {
                model: debate_config.model.clone(),
                temperature: debate_config.temperature,
            },
            system_prompt: debate_config.decision_prompt_template.clone(),
        }
    }

    /// Synthesize the full debate record and produce a final DecisionOutput.
    pub async fn decide(&self, transcript: &str) -> Result<DecisionOutput, anyhow::Error> {
        let messages = vec![
            ChatMessage::system(&self.system_prompt),
            ChatMessage::user(transcript),
        ];

        let llm_config = LlmConfig {
            model: self.config.model.clone(),
            temperature: self.config.temperature,
            max_tokens: 1024,
            api_key: None,
            base_url: None,
        };

        let response = self.llm_client.chat(&messages, &llm_config).await?;
        taiji_llm::client::parse_decision_output(&response)
    }
}
