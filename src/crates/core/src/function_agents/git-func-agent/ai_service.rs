use super::types::{AICommitAnalysis, CommitMessageOptions, ProjectContext};
use crate::function_agents::common::{AgentError, AgentResult};
use crate::infrastructure::ai::AIClient;
use crate::util::types::Message;
/**
 * AI service layer
 *
 * Handles AI client interaction and provides intelligent analysis for commit message generation
 */
use log::{debug, error, warn};
use std::sync::Arc;

/// Prompt template constants (embedded at compile time)
const COMMIT_MESSAGE_PROMPT: &str = include_str!("prompts/commit_message.md");

pub struct AIAnalysisService {
    ai_client: Arc<AIClient>,
}

impl AIAnalysisService {
    pub async fn new_with_agent_config(
        factory: std::sync::Arc<crate::infrastructure::ai::AIClientFactory>,
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

    pub async fn generate_commit_message_ai(
        &self,
        diff_content: &str,
        project_context: &ProjectContext,
        options: &CommitMessageOptions,
    ) -> AgentResult<AICommitAnalysis> {
        if diff_content.is_empty() {
            return Err(AgentError::invalid_input("Code changes are empty"));
        }

        let processed_diff = self.truncate_diff_if_needed(diff_content, 50000);

        let prompt = self.build_commit_prompt(&processed_diff, project_context, options);

        let ai_response = self.call_ai(&prompt).await?;

        self.parse_commit_response(&ai_response)
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

    fn build_commit_prompt(
        &self,
        diff_content: &str,
        project_context: &ProjectContext,
        options: &CommitMessageOptions,
    ) -> String {
        super::utils::build_commit_prompt(
            COMMIT_MESSAGE_PROMPT,
            diff_content,
            project_context,
            options,
        )
    }

    fn parse_commit_response(&self, response: &str) -> AgentResult<AICommitAnalysis> {
        let json_str = crate::util::extract_json_from_ai_response(response)
            .ok_or_else(|| AgentError::analysis_error("Cannot extract JSON from response"))?;

        let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            AgentError::analysis_error(format!("Failed to parse AI response: {}", e))
        })?;

        super::utils::parse_commit_analysis_value(&value).map_err(AgentError::analysis_error)
    }

    fn truncate_diff_if_needed(&self, diff: &str, max_chars: usize) -> String {
        if diff.len() <= max_chars {
            return diff.to_string();
        }

        warn!(
            "Diff too large ({} chars), truncating to {} chars",
            diff.len(),
            max_chars
        );

        let mut truncated = diff.chars().take(max_chars - 100).collect::<String>();
        truncated.push_str("\n\n... [content truncated] ...");

        truncated
    }
}
