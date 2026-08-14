//! AskUserQuestion tool
//!
//! Allows AI to ask questions to users during execution and wait for answers

use async_trait::async_trait;
use bitfun_agent_runtime::user_questions::{
    ask_user_question_available_in_context, build_answered_user_question_result,
    build_cancelled_user_question_result, validate_ask_user_question_input, AskUserQuestionInput,
    PendingUserInput, USER_INPUT_AVAILABLE_CONTEXT_KEY,
};
use log::{debug, warn};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::user_input_manager::get_user_input_manager;
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::util::errors::BitFunResult;

struct PendingUserInputSettlement {
    pending: PendingUserInput,
    coordinator: Option<std::sync::Arc<crate::agentic::coordination::ConversationCoordinator>>,
    settled: bool,
}

impl PendingUserInputSettlement {
    fn new(
        pending: PendingUserInput,
        coordinator: Option<std::sync::Arc<crate::agentic::coordination::ConversationCoordinator>>,
    ) -> Self {
        Self {
            pending,
            coordinator,
            settled: false,
        }
    }

    async fn settle(mut self) {
        get_user_input_manager().cancel_registration(&self.pending);
        if let Some(coordinator) = self.coordinator.as_ref() {
            coordinator.resolve_pending_user_input(&self.pending).await;
        }
        self.settled = true;
    }
}

impl Drop for PendingUserInputSettlement {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        get_user_input_manager().cancel_registration(&self.pending);
        if let Some(coordinator) = self.coordinator.as_ref() {
            coordinator.resolve_pending_user_input_now(&self.pending);
        }
    }
}

/// AskUserQuestion tool
pub struct AskUserQuestionTool;

impl Default for AskUserQuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AskUserQuestionTool {
    pub fn new() -> Self {
        Self
    }

    fn is_available_for_tool_context(context: Option<&ToolUseContext>) -> bool {
        ask_user_question_available_in_context(
            context.and_then(|ctx| ctx.custom_data.get("acp_transport")),
            context.and_then(|ctx| ctx.custom_data.get(USER_INPUT_AVAILABLE_CONTEXT_KEY)),
        )
    }

    /// Generate tool ID
    fn generate_tool_id(context: &ToolUseContext) -> String {
        // Prefer tool_call_id
        if let Some(tool_call_id) = &context.tool_call_id {
            return tool_call_id.clone();
        }

        // Only generate UUID as last resort (shouldn't reach here)
        warn!("Unable to get tool_call_id, using UUID for AskUserQuestion tool");
        format!("ask_user_{}", Uuid::new_v4())
    }
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "AskUserQuestion"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Use this tool when you need to ask the user questions during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take

WHEN TO USE:
- The request is ambiguous or could be interpreted in multiple ways
- Multiple valid approaches exist with different trade-offs
- The change affects critical files or has significant impact
- You are unsure about the user's intent or preferences
- The decision has security, performance, or architectural implications

WHEN NOT TO USE:
- The request is clear and specific
- You are following an already-approved plan exactly
- The change is trivial and clearly correct

RECOMMENDATION GUIDELINES:
- Always state your recommendation and reasoning
- Make your recommended option the first option in the list
- Add "(Recommended)" at the end of the recommended option's label
- Provide 2-4 clear options with descriptions of trade-offs

Usage notes:
- This tool ends the current dialog turn and waits for the user's reply before the assistant continues
- Put all questions you need into a single AskUserQuestion call instead of calling it repeatedly in one response
- Users will always be able to select "Other" to provide custom text input
- Use multiSelect: true to allow multiple answers to be selected for a question"#.to_string())
    }

    fn short_description(&self) -> String {
        "Ask the user focused follow-up questions during execution.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask the user. Should be clear, specific, and end with a question mark. Example: \"Which library should we use for date formatting?\" If multiSelect is true, phrase it accordingly, e.g. \"Which features do you want to enable?\""
                            },
                            "header": {
                                "type": "string",
                                "maxLength": 20,
                                "description": "Very short label displayed as a chip/tag (max 20 characters). Examples: \"Auth method\", \"Library\", \"Approach\"."
                            },
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "The display text for this option that the user will see and select. Should be concise (1-5 words) and clearly describe the choice."
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means or what will happen if chosen. Useful for providing context about trade-offs or implications."
                                        }
                                    },
                                    "required": [
                                        "label",
                                        "description"
                                    ],
                                    "additionalProperties": false
                                },
                                "minItems": 2,
                                "maxItems": 10,
                                "description": "The available choices for this question. Must have 2-10 options. Each option should be a distinct, mutually exclusive choice (unless multiSelect is enabled). There should be no 'Other' option, that will be provided automatically."
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "default": false,
                                "description": "Optional. Defaults to false. Set to true to allow the user to select multiple options instead of just one. Use when choices are not mutually exclusive."
                            }
                        },
                        "required": [
                            "question",
                            "header",
                            "options"
                        ],
                        "additionalProperties": false
                    },
                    "minItems": 1,
                    "maxItems": 4,
                    "description": "Questions to ask the user (1-4 questions)"
                }
            },
            "required": [
                "questions"
            ],
            "additionalProperties": false,
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        Self::is_available_for_tool_context(context)
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if !Self::is_available_for_tool_context(Some(context)) {
            return Err(crate::util::errors::BitFunError::tool(
                "AskUserQuestion is unavailable because this execution surface cannot accept interactive user input",
            ));
        }

        // 1. Parse input parameters
        let tool_input: AskUserQuestionInput =
            serde_json::from_value(input.clone()).map_err(|e| {
                crate::util::errors::BitFunError::Validation(format!(
                    "Failed to parse input parameters: {}",
                    e
                ))
            })?;

        // 2. Validate question format
        if let Err(error) = validate_ask_user_question_input(&tool_input) {
            return Err(crate::util::errors::BitFunError::Validation(error));
        }

        let question_count = tool_input.questions.len();
        debug!(
            "AskUserQuestion tool called with {} question(s)",
            question_count
        );

        // 3. Generate tool ID
        let tool_id = Self::generate_tool_id(context);

        // 4. Create oneshot channel
        let (tx, rx) = tokio::sync::oneshot::channel();

        // 5. Register to global manager
        let session_id = context
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let turn_id = context
            .dialog_turn_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let pending = PendingUserInput {
            tool_id: tool_id.clone(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            source_session_id: session_id.clone(),
            source_turn_id: turn_id,
            registration_sequence: 0,
            input: serde_json::to_value(&tool_input)
                .expect("validated AskUserQuestion input must serialize"),
        };
        let coordinator = get_global_coordinator();
        let pending = if let Some(coordinator) = coordinator.as_ref() {
            coordinator.register_pending_user_input(pending, tx).await
        } else {
            let mut pending = pending;
            pending.registration_sequence = get_user_input_manager().register(pending.clone(), tx);
            pending
        };
        let settlement = PendingUserInputSettlement::new(pending, coordinator.clone());

        // 6. Send backend event to notify frontend to display question card
        let event_system = get_global_event_system();

        // Send complete questions array to frontend
        let event = BackendEvent::ToolAwaitingUserInput {
            tool_id: tool_id.clone(),
            session_id,
            questions: serde_json::to_value(&tool_input).unwrap_or_else(|_| json!({})),
        };

        let _ = event_system.emit(event).await;
        debug!(
            "AskUserQuestion tool event emitted, waiting for user input, tool_id: {}",
            tool_id
        );

        // 7. Wait for user answer until the user responds, cancels, or the turn is cancelled.
        let response = rx.await;
        settlement.settle().await;
        match response {
            Ok(response) => {
                debug!(
                    "AskUserQuestion tool received user response, tool_id: {}",
                    tool_id
                );
                let result = build_answered_user_question_result(&tool_input, response.answers);

                Ok(vec![ToolResult::Result {
                    data: result.data,
                    result_for_assistant: Some(result.result_for_assistant),
                    image_attachments: None,
                }])
            }
            Err(_) => {
                warn!("AskUserQuestion tool channel closed, tool_id: {}", tool_id);
                let result = build_cancelled_user_question_result(&tool_input);
                Ok(vec![ToolResult::Result {
                    data: result.data,
                    result_for_assistant: Some(result.result_for_assistant),
                    image_attachments: None,
                }])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AskUserQuestionTool;
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use std::collections::HashMap;

    fn context_with_custom_data(custom_data: HashMap<String, serde_json::Value>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data,
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn ask_user_question_is_hidden_for_acp_transport() {
        let tool = AskUserQuestionTool::new();
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "acp_transport".to_string(),
            serde_json::Value::String("true".to_string()),
        );
        let context = context_with_custom_data(custom_data);

        assert!(!tool.is_available_in_context(Some(&context)).await);
    }

    #[tokio::test]
    async fn ask_user_question_remains_available_without_acp_transport() {
        let tool = AskUserQuestionTool::new();
        let context = context_with_custom_data(HashMap::new());

        assert!(tool.is_available_in_context(Some(&context)).await);
    }

    #[tokio::test]
    async fn ask_user_question_is_hidden_when_human_input_is_unavailable() {
        let tool = AskUserQuestionTool::new();
        let context = context_with_custom_data(HashMap::from([(
            "user_input_available".to_string(),
            serde_json::Value::Bool(false),
        )]));

        assert!(!tool.is_available_in_context(Some(&context)).await);
    }

    #[tokio::test]
    async fn ask_user_question_fails_without_waiting_when_human_input_is_unavailable() {
        let tool = AskUserQuestionTool::new();
        let context = context_with_custom_data(HashMap::from([(
            "user_input_available".to_string(),
            serde_json::Value::Bool(false),
        )]));
        let input = serde_json::json!({
            "questions": [{
                "question": "Continue?",
                "header": "Continue",
                "options": [
                    { "label": "Yes", "description": "Continue" },
                    { "label": "No", "description": "Stop" }
                ]
            }]
        });

        let error = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            tool.call(&input, &context),
        )
        .await
        .expect("non-interactive question must not wait")
        .expect_err("non-interactive question must fail");

        assert!(error.to_string().contains("cannot accept interactive"));
    }

    #[tokio::test]
    async fn ask_user_question_registers_authoritative_restore_facts_until_answered() {
        let tool_id = format!("question-{}", uuid::Uuid::new_v4());
        let mut context = context_with_custom_data(HashMap::new());
        context.tool_call_id = Some(tool_id.clone());
        context.session_id = Some("child-session".to_string());
        context.dialog_turn_id = Some("child-turn".to_string());
        let input = serde_json::json!({
            "questions": [{
                "question": "Continue?",
                "header": "Continue",
                "options": [
                    { "label": "Yes", "description": "Continue" },
                    { "label": "No", "description": "Stop" }
                ]
            }]
        });
        let task_tool_id = tool_id.clone();
        let task =
            tokio::spawn(async move { AskUserQuestionTool::new().call(&input, &context).await });
        let manager = crate::agentic::tools::user_input_manager::get_user_input_manager();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !manager.has_pending(&tool_id) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("question registration deadline");

        let pending = manager
            .pending_inputs()
            .into_iter()
            .find(|pending| pending.tool_id == tool_id)
            .expect("pending restore fact");
        assert_eq!(pending.session_id, "child-session");
        assert_eq!(pending.turn_id, "child-turn");
        assert_eq!(pending.source_session_id, "child-session");
        assert_eq!(pending.source_turn_id, "child-turn");
        assert_eq!(pending.input["questions"][0]["header"], "Continue");

        manager
            .send_answer(bitfun_runtime_ports::AgentUserAnswersRequest {
                session_id: pending.session_id,
                turn_id: pending.turn_id,
                tool_id: pending.tool_id,
                registration_sequence: pending.registration_sequence,
                answers: serde_json::json!({ "0": "Yes" }),
            })
            .expect("deliver answer");
        task.await
            .expect("question task")
            .expect("answered question result");
        assert!(!manager.has_pending(&task_tool_id));
    }

    #[tokio::test]
    async fn cancelling_ask_user_question_settles_the_exact_pending_registration() {
        let tool_id = format!("question-cancel-{}", uuid::Uuid::new_v4());
        let cancellation = tokio_util::sync::CancellationToken::new();
        let mut context = context_with_custom_data(HashMap::new());
        context.tool_call_id = Some(tool_id.clone());
        context.session_id = Some("cancel-session".to_string());
        context.dialog_turn_id = Some("cancel-turn".to_string());
        context.runtime_handles =
            bitfun_runtime_ports::ToolRuntimeHandles::new(None, Some(cancellation.clone()));
        let input = serde_json::json!({
            "questions": [{
                "question": "Continue?",
                "header": "Continue",
                "options": [
                    { "label": "Yes", "description": "Continue" },
                    { "label": "No", "description": "Stop" }
                ]
            }]
        });
        let task =
            tokio::spawn(async move { AskUserQuestionTool::new().call(&input, &context).await });
        let manager = crate::agentic::tools::user_input_manager::get_user_input_manager();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !manager.has_pending(&tool_id) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("question registration deadline");

        cancellation.cancel();
        let error = task
            .await
            .expect("question task")
            .expect_err("cancelled question must stop the tool call");

        assert!(matches!(
            error,
            crate::util::errors::BitFunError::Cancelled(_)
        ));
        assert!(!manager
            .pending_inputs()
            .iter()
            .any(|pending| pending.tool_id == tool_id));
    }

    #[test]
    fn ask_user_question_schema_defaults_multi_select_to_false() {
        let schema = AskUserQuestionTool::new().input_schema();
        let question_schema = &schema["properties"]["questions"]["items"];

        assert_eq!(
            question_schema["properties"]["multiSelect"]["default"],
            false
        );
        assert!(!question_schema["required"]
            .as_array()
            .expect("required array")
            .iter()
            .any(|value| value == "multiSelect"));
    }
}
