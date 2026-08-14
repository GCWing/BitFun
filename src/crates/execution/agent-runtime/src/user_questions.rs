//! Portable contracts for user-question tool handlers.

use dashmap::{mapref::entry::Entry, DashMap};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, LazyLock,
};
use tokio::sync::oneshot;

pub use bitfun_runtime_ports::PendingUserInput;

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
    #[error("Waiting channel identity does not match: {tool_id}")]
    IdentityMismatch { tool_id: String },
    #[error("Channel closed, cannot send answer: {tool_id}")]
    ChannelClosed { tool_id: String },
}

pub struct UserInputManager {
    channels: Arc<DashMap<PendingUserInputKey, PendingUserInputEntry>>,
    next_registration_sequence: AtomicU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PendingUserInputKey {
    session_id: String,
    turn_id: String,
    tool_id: String,
    registration_sequence: u64,
}

impl PendingUserInputKey {
    fn from_pending(pending: &PendingUserInput) -> Self {
        Self {
            session_id: pending.session_id.clone(),
            turn_id: pending.turn_id.clone(),
            tool_id: pending.tool_id.clone(),
            registration_sequence: pending.registration_sequence,
        }
    }

    fn from_request(request: &bitfun_runtime_ports::AgentUserAnswersRequest) -> Self {
        Self {
            session_id: request.session_id.clone(),
            turn_id: request.turn_id.clone(),
            tool_id: request.tool_id.clone(),
            registration_sequence: request.registration_sequence,
        }
    }
}

struct PendingUserInputEntry {
    pending: PendingUserInput,
    sender: oneshot::Sender<UserInputResponse>,
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
            next_registration_sequence: AtomicU64::new(0),
        }
    }

    pub fn register(
        &self,
        mut pending: PendingUserInput,
        sender: oneshot::Sender<UserInputResponse>,
    ) -> u64 {
        pending.registration_sequence = self
            .next_registration_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let registration_sequence = pending.registration_sequence;
        debug!("Registered waiting channel: tool_id={}", pending.tool_id);
        let key = PendingUserInputKey::from_pending(&pending);
        self.channels
            .insert(key, PendingUserInputEntry { pending, sender });
        registration_sequence
    }

    /// Compatibility entry point for hosts that only need the answer channel.
    /// Runtime-owned interactive tools should use [`Self::register`] so restore
    /// snapshots retain the authoritative question payload and Session route.
    pub fn register_channel(
        &self,
        tool_id: String,
        sender: oneshot::Sender<UserInputResponse>,
    ) -> u64 {
        self.register(
            PendingUserInput {
                tool_id,
                session_id: "unknown".to_string(),
                turn_id: "unknown".to_string(),
                source_session_id: "unknown".to_string(),
                source_turn_id: "unknown".to_string(),
                registration_sequence: 0,
                input: serde_json::json!({ "questions": [] }),
            },
            sender,
        )
    }

    pub fn send_answer(
        &self,
        request: bitfun_runtime_ports::AgentUserAnswersRequest,
    ) -> Result<(), UserInputSendError> {
        info!("Sending user answer: tool_id={}", request.tool_id);

        let key = PendingUserInputKey::from_request(&request);
        let entry = match self.channels.entry(key) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(vacant) => {
                drop(vacant);
                let error = if self
                    .channels
                    .iter()
                    .any(|entry| entry.key().tool_id == request.tool_id)
                {
                    UserInputSendError::IdentityMismatch {
                        tool_id: request.tool_id,
                    }
                } else {
                    UserInputSendError::MissingChannel {
                        tool_id: request.tool_id,
                    }
                };
                warn!("{}", error);
                return Err(error);
            }
        };

        let entry = entry.remove();
        entry
            .sender
            .send(UserInputResponse {
                answers: request.answers,
            })
            .map_err(|_| UserInputSendError::ChannelClosed {
                tool_id: request.tool_id.clone(),
            })?;
        debug!("Answer sent: tool_id={}", request.tool_id);
        Ok(())
    }

    pub fn cancel_registration(&self, pending: &PendingUserInput) -> bool {
        let key = PendingUserInputKey::from_pending(pending);
        if self.channels.remove(&key).is_some() {
            debug!(
                "Cancelled waiting registration: session_id={}, turn_id={}, tool_id={}, registration_sequence={}",
                pending.session_id,
                pending.turn_id,
                pending.tool_id,
                pending.registration_sequence
            );
            true
        } else {
            false
        }
    }

    pub fn has_pending(&self, tool_id: &str) -> bool {
        self.remove_closed_channels();
        self.channels
            .iter()
            .any(|entry| entry.key().tool_id == tool_id)
    }

    pub fn pending_tool_ids(&self) -> Vec<String> {
        self.remove_closed_channels();
        let mut tool_ids = self
            .channels
            .iter()
            .map(|entry| entry.key().tool_id.clone())
            .collect::<Vec<_>>();
        tool_ids.sort();
        tool_ids.dedup();
        tool_ids
    }

    pub fn pending_inputs(&self) -> Vec<PendingUserInput> {
        self.remove_closed_channels();
        let mut pending = self
            .channels
            .iter()
            .map(|entry| entry.pending.clone())
            .collect::<Vec<_>>();
        pending.sort_by_key(|input| input.registration_sequence);
        pending
    }

    fn remove_closed_channels(&self) {
        self.channels.retain(|_, entry| !entry.sender.is_closed());
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
    use super::{
        AskUserQuestionInput, PendingUserInput, Question, UserInputManager, UserInputResponse,
        UserInputSendError,
    };
    use bitfun_runtime_ports::AgentUserAnswersRequest;
    use serde_json::json;

    #[tokio::test]
    async fn user_input_manager_delivers_answer_and_clears_channel() {
        let manager = UserInputManager::new();
        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();

        let registration_sequence = manager.register(pending_input("tool-1"), sender);
        assert!(manager.has_pending("tool-1"));
        manager
            .send_answer(answer_request(
                "session-1",
                "turn-1",
                "tool-1",
                registration_sequence,
                json!({ "0": "yes" }),
            ))
            .expect("answer should be sent");

        let response = receiver.await.expect("receiver should get answer");
        assert_eq!(response.answers, json!({ "0": "yes" }));
        assert!(!manager.has_pending("tool-1"));
    }

    #[tokio::test]
    async fn identity_mismatch_does_not_consume_the_current_answer_channel() {
        let manager = UserInputManager::new();
        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let registration_sequence = manager.register(pending_input("shared-tool"), sender);

        let mismatch = manager
            .send_answer(AgentUserAnswersRequest {
                session_id: "stale-session".to_string(),
                turn_id: "turn-1".to_string(),
                tool_id: "shared-tool".to_string(),
                registration_sequence,
                answers: json!({ "0": "stale" }),
            })
            .expect_err("a stale Session identity must be rejected");
        assert!(matches!(
            mismatch,
            UserInputSendError::IdentityMismatch { ref tool_id }
                if tool_id == "shared-tool"
        ));
        assert!(manager.has_pending("shared-tool"));

        manager
            .send_answer(AgentUserAnswersRequest {
                session_id: "session-1".to_string(),
                turn_id: "turn-1".to_string(),
                tool_id: "shared-tool".to_string(),
                registration_sequence,
                answers: json!({ "0": "current" }),
            })
            .expect("the exact registered identity should still be answerable");

        let response = receiver
            .await
            .expect("receiver should get the current answer");
        assert_eq!(response.answers, json!({ "0": "current" }));
        assert!(!manager.has_pending("shared-tool"));
    }

    #[tokio::test]
    async fn concurrent_sessions_can_wait_on_the_same_provider_tool_id() {
        let manager = UserInputManager::new();
        let (sender_a, receiver_a) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let (sender_b, receiver_b) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let mut pending_a = pending_input("shared-tool");
        pending_a.session_id = "session-a".to_string();
        pending_a.turn_id = "turn-a".to_string();
        let mut pending_b = pending_input("shared-tool");
        pending_b.session_id = "session-b".to_string();
        pending_b.turn_id = "turn-b".to_string();

        let sequence_a = manager.register(pending_a, sender_a);
        let sequence_b = manager.register(pending_b, sender_b);

        let pending = manager.pending_inputs();
        assert_eq!(pending.len(), 2);
        assert_eq!(
            pending
                .iter()
                .map(|input| (input.session_id.as_str(), input.turn_id.as_str()))
                .collect::<Vec<_>>(),
            vec![("session-a", "turn-a"), ("session-b", "turn-b")]
        );

        manager
            .send_answer(answer_request(
                "session-a",
                "turn-a",
                "shared-tool",
                sequence_a,
                json!({ "0": "answer-a" }),
            ))
            .expect("session A should retain its own answer channel");
        manager
            .send_answer(answer_request(
                "session-b",
                "turn-b",
                "shared-tool",
                sequence_b,
                json!({ "0": "answer-b" }),
            ))
            .expect("session B should retain its own answer channel");

        assert_eq!(
            receiver_a.await.expect("session A receiver").answers,
            json!({ "0": "answer-a" })
        );
        assert_eq!(
            receiver_b.await.expect("session B receiver").answers,
            json!({ "0": "answer-b" })
        );
    }

    #[tokio::test]
    async fn user_input_manager_cancel_closes_only_the_exact_registration() {
        let manager = UserInputManager::new();
        let (sender_a, receiver_a) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let (sender_b, receiver_b) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let mut pending_a = pending_input("shared-tool");
        pending_a.session_id = "session-a".to_string();
        pending_a.turn_id = "turn-a".to_string();
        let mut pending_b = pending_input("shared-tool");
        pending_b.session_id = "session-b".to_string();
        pending_b.turn_id = "turn-b".to_string();

        pending_a.registration_sequence = manager.register(pending_a.clone(), sender_a);
        pending_b.registration_sequence = manager.register(pending_b.clone(), sender_b);

        assert!(manager.cancel_registration(&pending_a));
        assert!(receiver_a.await.is_err());
        assert!(!manager.cancel_registration(&pending_a));
        assert!(manager.has_pending("shared-tool"));

        manager
            .send_answer(answer_request(
                "session-b",
                "turn-b",
                "shared-tool",
                pending_b.registration_sequence,
                json!({ "0": "answer-b" }),
            ))
            .expect("cancelling session A must not consume session B");
        assert_eq!(
            receiver_b.await.expect("session B receiver").answers,
            json!({ "0": "answer-b" })
        );
    }

    #[tokio::test]
    async fn user_input_manager_distinguishes_missing_and_closed_channels() {
        let manager = UserInputManager::new();
        let missing = manager
            .send_answer(answer_request(
                "session-1",
                "turn-1",
                "missing-tool",
                1,
                json!({ "0": "yes" }),
            ))
            .expect_err("missing channel");
        assert_eq!(
            missing,
            UserInputSendError::MissingChannel {
                tool_id: "missing-tool".to_string(),
            }
        );

        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let registration_sequence = manager.register(pending_input("closed-tool"), sender);
        drop(receiver);
        let closed = manager
            .send_answer(answer_request(
                "session-1",
                "turn-1",
                "closed-tool",
                registration_sequence,
                json!({ "0": "yes" }),
            ))
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

        manager.register(pending_input("tool-1"), sender);

        assert_eq!(manager.pending_tool_ids(), vec!["tool-1".to_string()]);
    }

    #[test]
    fn user_input_manager_reports_authoritative_pending_inputs_in_registration_order() {
        let manager = UserInputManager::new();
        let (sender_2, _receiver_2) = tokio::sync::oneshot::channel::<UserInputResponse>();
        let (sender_1, _receiver_1) = tokio::sync::oneshot::channel::<UserInputResponse>();

        manager.register(pending_input("tool-2"), sender_2);
        manager.register(pending_input("tool-1"), sender_1);

        let pending = manager.pending_inputs();
        assert_eq!(
            pending
                .iter()
                .map(|input| input.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["tool-2", "tool-1"]
        );
        assert!(pending[0].registration_sequence < pending[1].registration_sequence);
    }

    #[test]
    fn closed_user_input_channels_are_not_restored_as_pending() {
        let manager = UserInputManager::new();
        let (sender, receiver) = tokio::sync::oneshot::channel::<UserInputResponse>();
        manager.register(pending_input("cancelled-tool"), sender);

        drop(receiver);

        assert!(manager.pending_inputs().is_empty());
        assert!(!manager.has_pending("cancelled-tool"));
        assert!(manager.pending_tool_ids().is_empty());
    }

    fn pending_input(tool_id: &str) -> PendingUserInput {
        PendingUserInput {
            tool_id: tool_id.to_string(),
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            source_session_id: "session-1".to_string(),
            source_turn_id: "turn-1".to_string(),
            registration_sequence: 0,
            input: serde_json::to_value(AskUserQuestionInput {
                questions: vec![Question {
                    question: "Continue?".to_string(),
                    header: "Confirm".to_string(),
                    options: Vec::new(),
                    multi_select: false,
                }],
            })
            .expect("serialize pending input"),
        }
    }

    fn answer_request(
        session_id: &str,
        turn_id: &str,
        tool_id: &str,
        registration_sequence: u64,
        answers: serde_json::Value,
    ) -> AgentUserAnswersRequest {
        AgentUserAnswersRequest {
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            tool_id: tool_id.to_string(),
            registration_sequence,
            answers,
        }
    }
}
