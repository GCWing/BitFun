//! Portable contracts for user-question tool handlers.

use dashmap::DashMap;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Question {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    #[serde(rename = "multiSelect", default)]
    pub multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskUserQuestionInput {
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserQuestionToolResult {
    pub data: Value,
    pub result_for_assistant: String,
}

#[derive(Debug, Clone)]
pub struct UserInputResponse {
    pub answers: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UserInputSendError {
    #[error("Waiting channel not found: {tool_id}")]
    MissingChannel { tool_id: String },
    #[error("Channel closed, cannot send answer: {tool_id}")]
    ChannelClosed { tool_id: String },
}

pub struct UserInputManager {
    channels: Arc<DashMap<String, oneshot::Sender<UserInputResponse>>>,
}

impl Default for UserInputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UserInputManager {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(DashMap::new()),
        }
    }

    pub fn register_channel(&self, tool_id: String, sender: oneshot::Sender<UserInputResponse>) {
        debug!("Registered waiting channel: tool_id={}", tool_id);
        self.channels.insert(tool_id, sender);
    }

    pub fn send_answer(&self, tool_id: &str, answers: Value) -> Result<(), UserInputSendError> {
        info!("Sending user answer: tool_id={}", tool_id);

        if let Some((_, sender)) = self.channels.remove(tool_id) {
            let response = UserInputResponse { answers };
            sender
                .send(response)
                .map_err(|_| UserInputSendError::ChannelClosed {
                    tool_id: tool_id.to_string(),
                })?;
            debug!("Answer sent: tool_id={}", tool_id);
            Ok(())
        } else {
            let error = UserInputSendError::MissingChannel {
                tool_id: tool_id.to_string(),
            };
            warn!("{}", error);
            Err(error)
        }
    }

    pub fn cancel(&self, tool_id: &str) -> bool {
        if self.channels.remove(tool_id).is_some() {
            debug!("Cancelled waiting: tool_id={}", tool_id);
            true
        } else {
            false
        }
    }

    pub fn has_pending(&self, tool_id: &str) -> bool {
        self.channels.contains_key(tool_id)
    }

    pub fn pending_tool_ids(&self) -> Vec<String> {
        self.channels
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
}

pub static USER_INPUT_MANAGER: LazyLock<UserInputManager> = LazyLock::new(|| {
    debug!("Initializing global user input manager");
    UserInputManager::new()
});

pub fn get_user_input_manager() -> &'static UserInputManager {
    &USER_INPUT_MANAGER
}

pub const USER_INPUT_AVAILABLE_CONTEXT_KEY: &str = "user_input_available";

pub fn ask_user_question_available_for_acp_transport(acp_transport: Option<&Value>) -> bool {
    !acp_transport.is_some_and(|value| value == "true" || value == &json!(true))
}

pub fn ask_user_question_available_in_context(
    acp_transport: Option<&Value>,
    user_input_available: Option<&Value>,
) -> bool {
    ask_user_question_available_for_acp_transport(acp_transport)
        && !user_input_available.is_some_and(|value| value == "false" || value == &json!(false))
}

pub fn validate_ask_user_question_input(input: &AskUserQuestionInput) -> Result<(), String> {
    if input.questions.is_empty() {
        return Err("At least one question is required".to_string());
    }
    if input.questions.len() > 4 {
        return Err("Maximum 4 questions allowed".to_string());
    }

    for (q_idx, question) in input.questions.iter().enumerate() {
        let q_num = q_idx + 1;

        if question.question.trim().is_empty() {
            return Err(format!("Question {} text is required", q_num));
        }

        if question.header.trim().is_empty() {
            return Err(format!("Question {} header is required", q_num));
        }
        if question.header.chars().count() > 20 {
            return Err(format!(
                "Question {} header must be less than 20 characters",
                q_num
            ));
        }

        if question.options.len() < 2 || question.options.len() > 10 {
            return Err(format!("Question {} must have 2-10 options", q_num));
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

pub fn build_answered_user_question_result(
    input: &AskUserQuestionInput,
    answers: Value,
) -> UserQuestionToolResult {
    let result_for_assistant = format_result_for_assistant(&input.questions, &answers);
    let questions_summary: Vec<Value> = input
        .questions
        .iter()
        .map(|question| {
            json!({
                "question": question.question,
                "header": question.header
            })
        })
        .collect();

    UserQuestionToolResult {
        data: json!({
            "questions": questions_summary,
            "answers": answers,
            "status": "answered"
        }),
        result_for_assistant,
    }
}

pub fn build_cancelled_user_question_result(
    input: &AskUserQuestionInput,
) -> UserQuestionToolResult {
    UserQuestionToolResult {
        data: json!({
            "questions_count": input.questions.len(),
            "status": "cancelled"
        }),
        result_for_assistant: "User input request was cancelled.".to_string(),
    }
}

fn format_result_for_assistant(questions: &[Question], answers: &Value) -> String {
    let answers_obj = answers
        .as_object()
        .or_else(|| answers.get("answers").and_then(|v| v.as_object()));

    if let Some(answers_map) = answers_obj {
        let mut result_lines = vec!["User has answered your questions:".to_string()];

        for (idx, question) in questions.iter().enumerate() {
            let idx_str = idx.to_string();
            let answer_text = if let Some(answer_value) = answers_map.get(&idx_str) {
                if let Some(arr) = answer_value.as_array() {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                } else if let Some(s) = answer_value.as_str() {
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

        result_lines.push("\nYou can now continue with the user's answers in mind.".to_string());
        result_lines.join("\n")
    } else {
        "User has answered your questions (no valid answers received).".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{UserInputManager, UserInputResponse, UserInputSendError};
    use serde_json::json;

    #[tokio::test]
    async fn user_input_manager_delivers_answer_and_clears_channel() {
        let manager = UserInputManager::new();
        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();

        manager.register_channel("tool-1".to_string(), sender);
        assert!(manager.has_pending("tool-1"));
        manager
            .send_answer("tool-1", json!({ "0": "yes" }))
            .expect("answer should be sent");

        let response = receiver.await.expect("receiver should get answer");
        assert_eq!(response.answers, json!({ "0": "yes" }));
        assert!(!manager.has_pending("tool-1"));
    }

    #[tokio::test]
    async fn user_input_manager_cancel_closes_receiver() {
        let manager = UserInputManager::new();
        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();

        manager.register_channel("tool-1".to_string(), sender);

        assert!(manager.cancel("tool-1"));
        assert!(receiver.await.is_err());
        assert!(!manager.cancel("tool-1"));
    }

    #[tokio::test]
    async fn user_input_manager_distinguishes_missing_and_closed_channels() {
        let manager = UserInputManager::new();
        let missing = manager
            .send_answer("missing-tool", json!({ "0": "yes" }))
            .expect_err("missing channel");
        assert_eq!(
            missing,
            UserInputSendError::MissingChannel {
                tool_id: "missing-tool".to_string(),
            }
        );

        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();
        manager.register_channel("closed-tool".to_string(), sender);
        drop(receiver);
        let closed = manager
            .send_answer("closed-tool", json!({ "0": "yes" }))
            .expect_err("closed channel");
        assert_eq!(
            closed,
            UserInputSendError::ChannelClosed {
                tool_id: "closed-tool".to_string(),
            }
        );
    }

    #[test]
    fn user_input_manager_reports_pending_tool_ids() {
        let manager = UserInputManager::new();
        let (sender, _receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();

        manager.register_channel("tool-1".to_string(), sender);

        assert_eq!(manager.pending_tool_ids(), vec!["tool-1".to_string()]);
    }
}
