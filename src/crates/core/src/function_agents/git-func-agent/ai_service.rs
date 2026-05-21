use super::types::{AICommitAnalysis, CommitMessageOptions, ProjectContext};
use crate::function_agents::common::{AgentError, AgentResult};
use crate::infrastructure::ai::AIClient;
use crate::util::types::Message;
use bitfun_product_domains::function_agents::git_func_agent::{
    parse_commit_ai_response, prepare_commit_ai_prompt,
};
/**
 * AI service layer
 *
 * Handles AI client interaction and provides intelligent analysis for commit message generation
 */
use log::{debug, error, warn};
use std::sync::Arc;

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

        let prepared_prompt = prepare_commit_ai_prompt(diff_content, project_context, options);
        if prepared_prompt.truncated {
            warn!(
                "Diff too large ({} chars), truncating to {} chars",
                diff_content.len(),
                50_000
            );
        }

        let ai_response = self.call_ai(&prepared_prompt.prompt).await?;

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

    fn parse_commit_response(&self, response: &str) -> AgentResult<AICommitAnalysis> {
        parse_commit_ai_response(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_agents::common::AgentErrorType;
    use crate::util::types::AIConfig;
    use bitfun_ai_adapters::types::ReasoningMode;

    fn test_service() -> AIAnalysisService {
        AIAnalysisService {
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
    fn parse_commit_response_preserves_product_domain_response_policy() {
        let service = test_service();
        let parsed = service
            .parse_commit_response(
                r#"The answer is:
```json
{
  "type": "refactor",
  "title": "refactor(product-domains): add runtime baseline",
  "body": "Keep behavior stable.",
  "confidence": 0.91
}
```
"#,
            )
            .unwrap();

        assert_eq!(
            parsed.title,
            "refactor(product-domains): add runtime baseline"
        );
        assert_eq!(parsed.body.as_deref(), Some("Keep behavior stable."));
        assert_eq!(parsed.confidence, 0.91);

        let missing_json = service.parse_commit_response("no json here").unwrap_err();
        assert_eq!(missing_json.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_json.message, "Cannot extract JSON from response");

        let missing_title = service
            .parse_commit_response(r#"{"type":"refactor","body":"missing title"}"#)
            .unwrap_err();
        assert_eq!(missing_title.error_type, AgentErrorType::AnalysisError);
        assert_eq!(missing_title.message, "Missing title field");
    }
}
