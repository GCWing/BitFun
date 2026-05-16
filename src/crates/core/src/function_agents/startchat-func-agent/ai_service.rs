use super::types::*;
use crate::function_agents::common::{AgentError, AgentResult, Language};
use crate::infrastructure::ai::AIClient;
use crate::util::types::Message;
/**
 * AI analysis service
 *
 * Provides AI-driven work state analysis for the Startchat function agent
 */
use log::{debug, error, warn};
use std::sync::Arc;

/// Prompt template constants (embedded at compile time)
const WORK_STATE_ANALYSIS_PROMPT: &str = include_str!("prompts/work_state_analysis.md");

pub struct AIWorkStateService {
    ai_client: Arc<AIClient>,
}

impl AIWorkStateService {
    pub async fn new_with_agent_config(
        factory: Arc<crate::infrastructure::ai::AIClientFactory>,
        agent_name: &str,
    ) -> AgentResult<Self> {
        let ai_client = match factory.get_client_by_func_agent(agent_name).await {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to get AI client: {}", e);
                return Err(AgentError::internal_error(format!(
                    "Failed to get AI client: {}",
                    e
                )));
            }
        };

        Ok(Self { ai_client })
    }

    pub async fn generate_complete_analysis(
        &self,
        git_state: &Option<GitWorkState>,
        git_diff: &str,
        language: &Language,
    ) -> AgentResult<AIGeneratedAnalysis> {
        let prompt = self.build_complete_analysis_prompt(git_state, git_diff, language);

        debug!(
            "Calling AI to generate complete analysis: prompt_length={}",
            prompt.len()
        );

        let response = self.call_ai(&prompt).await?;

        self.parse_complete_analysis(&response)
    }

    async fn call_ai(&self, prompt: &str) -> AgentResult<String> {
        debug!("Sending request to AI: prompt_length={}", prompt.len());

        let messages = vec![Message::user(prompt.to_string())];
        let response = self
            .ai_client
            .send_message(messages, None)
            .await
            .map_err(|e| {
                error!("AI call failed: {}", e);
                AgentError::internal_error(format!("AI call failed: {}", e))
            })?;

        debug!(
            "AI response received: response_length={}",
            response.text.len()
        );

        if response.text.is_empty() {
            error!("AI response is empty");
            Err(AgentError::internal_error(
                "AI response is empty".to_string(),
            ))
        } else {
            Ok(response.text)
        }
    }

    fn build_complete_analysis_prompt(
        &self,
        git_state: &Option<GitWorkState>,
        git_diff: &str,
        language: &Language,
    ) -> String {
        super::utils::build_complete_analysis_prompt(
            WORK_STATE_ANALYSIS_PROMPT,
            git_state,
            git_diff,
            language,
        )
    }

    fn parse_complete_analysis(&self, response: &str) -> AgentResult<AIGeneratedAnalysis> {
        let json_str = crate::util::extract_json_from_ai_response(response).ok_or_else(|| {
            error!(
                "Failed to extract JSON from analysis response: {}",
                response
            );
            AgentError::internal_error("Failed to extract JSON from analysis response")
        })?;

        debug!("Parsing JSON response: length={}", json_str.len());

        let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            error!(
                "Failed to parse complete analysis response: {}, response: {}",
                e, response
            );
            AgentError::internal_error(format!("Failed to parse complete analysis response: {}", e))
        })?;

        let parsed_analysis = super::utils::parse_complete_analysis_value(&parsed);

        if parsed_analysis.predicted_actions_count < 3 {
            warn!(
                "AI generated insufficient predicted actions ({}), adding defaults",
                parsed_analysis.predicted_actions_count
            );
        } else if parsed_analysis.predicted_actions_count > 3 {
            warn!(
                "AI generated too many predicted actions ({}), truncating to 3",
                parsed_analysis.predicted_actions_count
            );
        }

        if parsed_analysis.quick_actions_count < 6 {
            // Don't fill defaults here, frontend has its own defaultActions with i18n support
            warn!(
                "AI generated insufficient quick actions ({}), frontend will use defaults",
                parsed_analysis.quick_actions_count
            );
        } else if parsed_analysis.quick_actions_count > 6 {
            warn!(
                "AI generated too many quick actions ({}), truncating to 6",
                parsed_analysis.quick_actions_count
            );
        }

        debug!(
            "Parsing completed: predicted_actions={}, quick_actions={}",
            parsed_analysis.analysis.predicted_actions.len(),
            parsed_analysis.analysis.quick_actions.len()
        );

        Ok(parsed_analysis.analysis)
    }
}
