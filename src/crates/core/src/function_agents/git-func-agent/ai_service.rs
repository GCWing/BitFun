use super::types::{
    AICommitAnalysis, AgentError, AgentResult, CommitFormat, CommitMessageOptions, CommitType,
    Language, ProjectContext,
};
use crate::infrastructure::ai::AIClient;
use crate::util::types::Message;
/**
 * AI service layer
 *
 * Handles AI client interaction and provides intelligent analysis for commit message generation
 */
use log::{debug, error, warn};
use serde_json::Value;
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
        let language_desc = match options.language {
            Language::Chinese => "Chinese",
            Language::English => "English",
        };

        let format_desc = match options.format {
            CommitFormat::Conventional => "Conventional Commits",
            CommitFormat::Angular => "Angular Style",
            CommitFormat::Simple => "Simple Format",
            CommitFormat::Custom => "Custom Format",
        };

        COMMIT_MESSAGE_PROMPT
            .replace("{project_type}", &project_context.project_type)
            .replace("{tech_stack}", &project_context.tech_stack.join(", "))
            .replace("{format_desc}", format_desc)
            .replace("{language_desc}", language_desc)
            .replace("{diff_content}", diff_content)
            .replace("{max_title_length}", &options.max_title_length.to_string())
    }

    fn parse_commit_response(&self, response: &str) -> AgentResult<AICommitAnalysis> {
        let json_str = self.extract_json_from_response(response)?;

        let value: Value = serde_json::from_str(&json_str).map_err(|e| {
            AgentError::analysis_error(format!("Failed to parse AI response: {}", e))
        })?;

        Ok(AICommitAnalysis {
            commit_type: self.parse_commit_type(value["type"].as_str().unwrap_or("chore"))?,
            scope: value["scope"].as_str().map(|s| s.to_string()),
            title: value["title"]
                .as_str()
                .ok_or_else(|| AgentError::analysis_error("Missing title field"))?
                .to_string(),
            body: value["body"].as_str().map(|s| s.to_string()),
            breaking_changes: value["breaking_changes"].as_str().map(|s| s.to_string()),
            reasoning: value["reasoning"]
                .as_str()
                .unwrap_or("AI analysis")
                .to_string(),
            confidence: value["confidence"].as_f64().unwrap_or(0.8) as f32,
        })
    }

    fn extract_json_from_response(&self, response: &str) -> AgentResult<String> {
        let trimmed = response.trim();

        if trimmed.starts_with('{') {
            return Ok(trimmed.to_string());
        }

        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end_offset) = trimmed[json_start..].find("```") {
                let json_end = json_start + end_offset;
                let json_str = trimmed[json_start..json_end].trim();
                return Ok(json_str.to_string());
            }
        }

        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                let json_str = &trimmed[start..=end];
                return Ok(json_str.to_string());
            }
        }

        Err(AgentError::analysis_error(
            "Cannot extract JSON from response",
        ))
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

    fn parse_commit_type(&self, s: &str) -> AgentResult<CommitType> {
        match s.to_lowercase().as_str() {
            "feat" | "feature" => Ok(CommitType::Feat),
            "fix" => Ok(CommitType::Fix),
            "docs" | "doc" => Ok(CommitType::Docs),
            "style" => Ok(CommitType::Style),
            "refactor" => Ok(CommitType::Refactor),
            "perf" | "performance" => Ok(CommitType::Perf),
            "test" => Ok(CommitType::Test),
            "chore" => Ok(CommitType::Chore),
            "ci" => Ok(CommitType::CI),
            "revert" => Ok(CommitType::Revert),
            _ => Ok(CommitType::Chore),
        }
    }
}
