use super::types::*;
use crate::function_agents::common::{AgentError, AgentResult, Language};
use crate::infrastructure::ai::AIClient;
use crate::util::types::Message;
use bitfun_product_domains::function_agents::startchat_func_agent::{
    build_work_state_analysis_prompt, parse_work_state_analysis_response,
};
/**
 * AI analysis service
 *
 * Provides AI-driven work state analysis for the Startchat function agent
 */
use log::{debug, error, warn};
use std::sync::Arc;

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
        build_work_state_analysis_prompt(git_state, git_diff, language)
    }

    fn parse_complete_analysis(&self, response: &str) -> AgentResult<AIGeneratedAnalysis> {
        let parsed_analysis = parse_work_state_analysis_response(response).map_err(|error| {
            error!("{}, response: {}", error.message, response);
            error
        })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_agents::common::AgentErrorType;
    use crate::util::types::AIConfig;
    use bitfun_ai_adapters::types::ReasoningMode;

    fn test_service() -> AIWorkStateService {
        AIWorkStateService {
            ai_client: Arc::new(AIClient::new(AIConfig {
                name: "test".to_string(),
                base_url: "http://127.0.0.1".to_string(),
                request_url: "http://127.0.0.1".to_string(),
                api_key: "test".to_string(),
                model: "test-model".to_string(),
                format: "openai".to_string(),
                context_window: 8192,
                max_tokens: None,
                temperature: None,
                top_p: None,
                reasoning_mode: ReasoningMode::Default,
                inline_think_in_text: false,
                custom_headers: None,
                custom_headers_mode: None,
                skip_ssl_verify: false,
                reasoning_effort: None,
                thinking_budget_tokens: None,
                custom_request_body: None,
                custom_request_body_mode: None,
            })),
        }
    }

    #[test]
    fn parse_complete_analysis_preserves_product_domain_response_policy() {
        let service = test_service();
        let analysis = service
            .parse_complete_analysis(
                r#"The answer is:
```json
{
  "summary": "Working on product-domain owner closure.",
  "predicted_actions": [
    {"description": "Run checks", "priority": "High", "icon": "check", "is_reminder": false}
  ],
  "quick_actions": [
    {"title": "Status", "command": "git status", "icon": "git", "action_type": "ViewStatus"}
  ]
}
```
"#,
            )
            .unwrap();

        assert_eq!(analysis.summary, "Working on product-domain owner closure.");
        assert_eq!(analysis.predicted_actions.len(), 3);
        assert_eq!(analysis.quick_actions.len(), 1);

        let missing_json = service.parse_complete_analysis("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::InternalError);
        assert_eq!(
            missing_json.message,
            "Failed to extract JSON from analysis response"
        );

        let invalid_json = service
            .parse_complete_analysis(
                r#"```json
not json
```"#,
            )
            .unwrap_err();
        assert_eq!(invalid_json.error_type, AgentErrorType::InternalError);
        assert_eq!(
            invalid_json.message,
            "Failed to extract JSON from analysis response"
        );
    }
}
