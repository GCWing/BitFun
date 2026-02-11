use super::types::*;
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
        // AI instruction for response language (not user-facing)
        let lang_instruction = match language {
            Language::Chinese => "Please respond in Chinese.",
            Language::English => "Please respond in English.",
        };

        // Build Git state section
        let git_state_section = if let Some(git) = git_state {
            let mut section = format!(
                "## Git Status\n\n- Current branch: {}\n- Unstaged files: {}\n- Staged files: {}\n- Unpushed commits: {}\n",
                git.current_branch, git.unstaged_files, git.staged_files, git.unpushed_commits
            );

            if !git.modified_files.is_empty() {
                section.push_str("\nModified files:\n");
                for file in git.modified_files.iter().take(10) {
                    section.push_str(&format!("  - {} ({:?})\n", file.path, file.change_type));
                }
            }
            section
        } else {
            String::new()
        };

        // Build Git diff section
        let git_diff_section = if !git_diff.is_empty() {
            let max_diff_length = 8000;
            if git_diff.len() > max_diff_length {
                let truncated_diff = git_diff
                    .char_indices()
                    .take_while(|(idx, _)| *idx < max_diff_length)
                    .map(|(_, c)| c)
                    .collect::<String>();
                format!(
                    "## Code Changes (Git Diff)\n\n{}\n\n... (diff content too long, truncated, total length: {} characters)\n",
                    truncated_diff, git_diff.len()
                )
            } else {
                format!("## Code Changes (Git Diff)\n\n{}", git_diff)
            }
        } else {
            String::new()
        };

        // Use template replacement
        WORK_STATE_ANALYSIS_PROMPT
            .replace("{lang_instruction}", lang_instruction)
            .replace("{git_state_section}", &git_state_section)
            .replace("{git_diff_section}", &git_diff_section)
    }

    fn parse_complete_analysis(&self, response: &str) -> AgentResult<AIGeneratedAnalysis> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        debug!("Parsing JSON response: length={}", json_str.len());

        let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            error!(
                "Failed to parse complete analysis response: {}, response: {}",
                e, response
            );
            AgentError::internal_error(format!("Failed to parse complete analysis response: {}", e))
        })?;

        let summary = parsed["summary"]
            .as_str()
            .unwrap_or("You were working on development, with multiple files modified.")
            .to_string();

        let ongoing_work = Vec::new();

        let mut predicted_actions =
            if let Some(actions_array) = parsed["predicted_actions"].as_array() {
                self.parse_predicted_actions_from_value(actions_array)?
            } else {
                Vec::new()
            };

        if predicted_actions.len() < 3 {
            warn!(
                "AI generated insufficient predicted actions ({}), adding defaults",
                predicted_actions.len()
            );
            while predicted_actions.len() < 3 {
                predicted_actions.push(PredictedAction {
                    description: "Continue current development".to_string(),
                    priority: ActionPriority::Medium,
                    icon: String::new(),
                    is_reminder: false,
                });
            }
        } else if predicted_actions.len() > 3 {
            warn!(
                "AI generated too many predicted actions ({}), truncating to 3",
                predicted_actions.len()
            );
            predicted_actions.truncate(3);
        }

        let mut quick_actions = if let Some(actions_array) = parsed["quick_actions"].as_array() {
            self.parse_quick_actions_from_value(actions_array)?
        } else {
            Vec::new()
        };

        if quick_actions.len() < 6 {
            // Don't fill defaults here, frontend has its own defaultActions with i18n support
            warn!(
                "AI generated insufficient quick actions ({}), frontend will use defaults",
                quick_actions.len()
            );
        } else if quick_actions.len() > 6 {
            warn!(
                "AI generated too many quick actions ({}), truncating to 6",
                quick_actions.len()
            );
            quick_actions.truncate(6);
        }

        debug!(
            "Parsing completed: predicted_actions={}, quick_actions={}",
            predicted_actions.len(),
            quick_actions.len()
        );

        Ok(AIGeneratedAnalysis {
            summary,
            ongoing_work,
            predicted_actions,
            quick_actions,
        })
    }

    fn parse_predicted_actions_from_value(
        &self,
        actions_array: &[serde_json::Value],
    ) -> AgentResult<Vec<PredictedAction>> {
        let mut actions = Vec::new();

        for action_value in actions_array {
            let description = action_value["description"]
                .as_str()
                .unwrap_or("Continue current work")
                .to_string();

            let priority_str = action_value["priority"].as_str().unwrap_or("Medium");

            let priority = match priority_str {
                "High" => ActionPriority::High,
                "Low" => ActionPriority::Low,
                _ => ActionPriority::Medium,
            };

            let icon = action_value["icon"].as_str().unwrap_or("").to_string();

            let is_reminder = action_value["is_reminder"].as_bool().unwrap_or(false);

            actions.push(PredictedAction {
                description,
                priority,
                icon,
                is_reminder,
            });
        }

        Ok(actions)
    }

    fn parse_quick_actions_from_value(
        &self,
        actions_array: &[serde_json::Value],
    ) -> AgentResult<Vec<QuickAction>> {
        let mut quick_actions = Vec::new();

        for action_value in actions_array {
            let title = action_value["title"]
                .as_str()
                .unwrap_or("Quick Action")
                .to_string();

            let command = action_value["command"].as_str().unwrap_or("").to_string();

            let icon = action_value["icon"].as_str().unwrap_or("").to_string();

            let action_type_str = action_value["action_type"].as_str().unwrap_or("Custom");

            let action_type = match action_type_str {
                "Continue" => QuickActionType::Continue,
                "ViewStatus" => QuickActionType::ViewStatus,
                "Commit" => QuickActionType::Commit,
                "Visualize" => QuickActionType::Visualize,
                _ => QuickActionType::Custom,
            };

            quick_actions.push(QuickAction {
                title,
                command,
                icon,
                action_type,
            });
        }

        Ok(quick_actions)
    }
}
