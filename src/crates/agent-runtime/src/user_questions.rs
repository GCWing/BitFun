//! Portable contracts for user-question tool handlers.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    #[serde(rename = "multiSelect")]
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

pub fn ask_user_question_available_for_acp_transport(acp_transport: Option<&Value>) -> bool {
    !acp_transport.is_some_and(|value| value == "true" || value == &json!(true))
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
