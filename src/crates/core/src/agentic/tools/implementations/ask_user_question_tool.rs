//! AskUserQuestion tool
//!
//! Allows AI to ask questions to users during execution and wait for answers

use async_trait::async_trait;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::user_input_manager::get_user_input_manager;
use crate::infrastructure::events::event_system::{get_global_event_system, BackendEvent};
use crate::util::errors::BitFunResult;

/// Question option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

/// Question definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(rename = "multiSelect")]
    pub multi_select: bool,
}

/// Tool input parameters - supports multiple questions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskUserQuestionInput {
    pub questions: Vec<Question>,
}

/// AskUserQuestion tool
pub struct AskUserQuestionTool;

impl AskUserQuestionTool {
    pub fn new() -> Self {
        Self
    }

    /// Validate question format (supports multiple questions)
    fn validate_input(input: &AskUserQuestionInput) -> Result<(), String> {
        // Validate question count
        if input.questions.is_empty() {
            return Err("At least one question is required".to_string());
        }
        if input.questions.len() > 4 {
            return Err("Maximum 4 questions allowed".to_string());
        }

        // Validate each question
        for (q_idx, question) in input.questions.iter().enumerate() {
            let q_num = q_idx + 1;

            // Validate question text
            if question.question.trim().is_empty() {
                return Err(format!("Question {} text is required", q_num));
            }

            // Validate header
            if question.header.trim().is_empty() {
                return Err(format!("Question {} header is required", q_num));
            }
            if question.header.chars().count() > 20 {
                return Err(format!(
                    "Question {} header must be less than 20 characters",
                    q_num
                ));
            }

            // Validate options
            if question.options.len() < 2 || question.options.len() > 4 {
                return Err(format!("Question {} must have 2-4 options", q_num));
            }

            for (opt_idx, opt) in question.options.iter().enumerate() {
                if opt.label.trim().is_empty() {
                    return Err(format!(
                        "Question {} option {} label is required",
                        q_num,
                        opt_idx + 1
                    ));
                }
                if opt.description.trim().is_empty() {
                    return Err(format!(
                        "Question {} option {} description is required",
                        q_num,
                        opt_idx + 1
                    ));
                }
            }
        }

        Ok(())
    }

    /// Format result for AI (supports multiple questions)
    fn format_result_for_assistant(questions: &[Question], answers: &Value) -> String {
        // Try flat structure first (frontend sends {"0": "...", "1": [...]}),
        // then fall back to nested {"answers": {...}} for backward compatibility
        let answers_obj = answers
            .as_object()
            .or_else(|| answers.get("answers").and_then(|v| v.as_object()));

        if let Some(answers_map) = answers_obj {
            let mut result_lines = vec!["User has answered your questions:".to_string()];

            for (idx, question) in questions.iter().enumerate() {
                let idx_str = idx.to_string();
                let answer_text = if let Some(answer_value) = answers_map.get(&idx_str) {
                    if let Some(arr) = answer_value.as_array() {
                        // Multi-select: join answers
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    } else if let Some(s) = answer_value.as_str() {
                        // Single-select
                        s.to_string()
                    } else {
                        "N/A".to_string()
                    }
                } else {
                    "N/A".to_string()
                };

                result_lines.push(format!(
                    "- {} ({}): \"{}\"",
                    question.question, question.header, answer_text
                ));
            }

            result_lines
                .push("\nYou can now continue with the user's answers in mind.".to_string());
            result_lines.join("\n")
        } else {
            "User has answered your questions (no valid answers received).".to_string()
        }
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
        Ok(r#"Use this tool when you need to ask the user question during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take.

Usage notes:
- Users will always be able to select "Other" to provide custom text input
- Use multiSelect: true to allow multiple answers to be selected for a question"#.to_string())
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
                                "description": "Very short label displayed as a chip/tag (max 12 chars). Examples: \"Auth method\", \"Library\", \"Approach\"."
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
                                "maxItems": 4,
                                "description": "The available choices for this question. Must have 2-4 options. Each option should be a distinct, mutually exclusive choice (unless multiSelect is enabled). There should be no 'Other' option, that will be provided automatically."
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "description": "Set to true to allow the user to select multiple options instead of just one. Use when choices are not mutually exclusive."
                            }
                        },
                        "required": [
                            "question",
                            "header",
                            "options",
                            "multiSelect"
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

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // 1. Parse input parameters
        let tool_input: AskUserQuestionInput =
            serde_json::from_value(input.clone()).map_err(|e| {
                crate::util::errors::BitFunError::Validation(format!(
                    "Failed to parse input parameters: {}",
                    e
                ))
            })?;

        // 2. Validate question format
        Self::validate_input(&tool_input)
            .map_err(|e| crate::util::errors::BitFunError::Validation(e))?;

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
        let manager = get_user_input_manager();
        manager.register_channel(tool_id.clone(), tx);

        // 6. Send backend event to notify frontend to display question card
        let event_system = get_global_event_system();
        let session_id = context
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

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

        // 7. Wait for user answer (10 minute timeout)
        let timeout_duration = Duration::from_secs(600); // 10 minutes
        match timeout(timeout_duration, rx).await {
            Ok(Ok(response)) => {
                // Received user answer
                debug!(
                    "AskUserQuestion tool received user response, tool_id: {}",
                    tool_id
                );
                let result_text =
                    Self::format_result_for_assistant(&tool_input.questions, &response.answers);

                // Build question summary for return data
                let questions_summary: Vec<Value> = tool_input
                    .questions
                    .iter()
                    .map(|q| {
                        json!({
                            "question": q.question,
                            "header": q.header
                        })
                    })
                    .collect();

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "questions": questions_summary,
                        "answers": response.answers,
                        "status": "answered"
                    }),
                    result_for_assistant: Some(result_text),
                }])
            }
            Ok(Err(_)) => {
                // Channel was closed
                warn!("AskUserQuestion tool channel closed, tool_id: {}", tool_id);
                Ok(vec![ToolResult::Result {
                    data: json!({
                        "questions_count": tool_input.questions.len(),
                        "status": "cancelled"
                    }),
                    result_for_assistant: Some("User input request was cancelled.".to_string()),
                }])
            }
            Err(_) => {
                // Timeout
                warn!(
                    "AskUserQuestion tool timeout after 600 seconds, tool_id: {}",
                    tool_id
                );
                manager.cancel(&tool_id); // Clean up channel

                Ok(vec![ToolResult::Result {
                    data: json!({
                        "questions_count": tool_input.questions.len(),
                        "status": "timeout"
                    }),
                    result_for_assistant: Some(
                        "User didn't answer your questions within 600 seconds.".to_string(),
                    ),
                }])
            }
        }
    }
}
