//! Code review result submission tool
//!
//! Used to get structured code review results.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::get_app_language_code;
use crate::service::i18n::code_review_copy_for_language;
use crate::util::errors::BitFunResult;
use async_trait::async_trait;
use log::warn;
use serde_json::{json, Value};

/// Code review tool definition
pub struct CodeReviewTool;

impl CodeReviewTool {
    pub fn new() -> Self {
        Self
    }

    pub fn name_str() -> &'static str {
        "submit_code_review"
    }

    /// Sync schema fallback (e.g. tests); prefers zh-CN wording. For model calls use [`input_schema_for_model`].
    pub fn input_schema_value() -> Value {
        Self::input_schema_value_for_language("zh-CN")
    }

    pub fn description_for_language(lang_code: &str) -> String {
        code_review_copy_for_language(lang_code)
            .description
            .to_string()
    }

    pub fn input_schema_value_for_language(lang_code: &str) -> Value {
        Self::input_schema_value_for_language_with_mode(lang_code, false)
    }

    fn input_schema_value_for_language_with_mode(
        lang_code: &str,
        require_deep_fields: bool,
    ) -> Value {
        let copy = code_review_copy_for_language(lang_code);
        let (
            scope_desc,
            reviewer_summary_desc,
            source_reviewer_desc,
            validation_note_desc,
            plan_desc,
        ) = match lang_code {
            "en-US" => (
                "Human-readable review scope (optional, in English)",
                "Reviewer summary (in English)",
                "Reviewer source / role (optional, in English)",
                "Validation or triage note (optional, in English)",
                "Concrete remediation / follow-up plan items (in English)",
            ),
            "zh-TW" => (
                "Human-readable review scope (optional, in Traditional Chinese)",
                "Reviewer summary (in Traditional Chinese)",
                "Reviewer source / role (optional, in Traditional Chinese)",
                "Validation or triage note (optional, in Traditional Chinese)",
                "Concrete remediation / follow-up plan items (in Traditional Chinese)",
            ),
            _ => (
                "Human-readable review scope (optional, in Simplified Chinese)",
                "Reviewer summary (in Simplified Chinese)",
                "Reviewer source / role (optional, in Simplified Chinese)",
                "Validation or triage note (optional, in Simplified Chinese)",
                "Concrete remediation / follow-up plan items (in Simplified Chinese)",
            ),
        };
        let mut required = vec!["summary", "issues", "positive_points"];
        if require_deep_fields {
            required.extend([
                "review_mode",
                "review_scope",
                "reviewers",
                "remediation_plan",
            ]);
        }

        json!({
            "type": "object",
            "properties": {
                "schema_version": {
                    "type": "integer",
                    "description": "Schema version for forward compatibility",
                    "default": 1
                },
                "summary": {
                    "type": "object",
                    "description": "Review summary",
                    "properties": {
                        "overall_assessment": {
                            "type": "string",
                            "description": copy.overall_assessment
                        },
                        "risk_level": {
                            "type": "string",
                            "enum": ["low", "medium", "high", "critical"],
                            "description": "Risk level"
                        },
                        "recommended_action": {
                            "type": "string",
                            "enum": ["approve", "approve_with_suggestions", "request_changes", "block"],
                            "description": "Recommended action"
                        },
                        "confidence_note": {
                            "type": "string",
                            "description": copy.confidence_note
                        }
                    },
                    "required": ["overall_assessment", "risk_level", "recommended_action"]
                },
                "issues": {
                    "type": "array",
                    "description": "List of issues found",
                    "items": {
                        "type": "object",
                        "properties": {
                            "severity": {
                                "type": "string",
                                "enum": ["critical", "high", "medium", "low", "info"],
                                "description": "Severity level"
                            },
                            "certainty": {
                                "type": "string",
                                "enum": ["confirmed", "likely", "possible"],
                                "description": "Certainty level"
                            },
                            "category": {
                                "type": "string",
                                "description": "Issue category (e.g., security, logic correctness, performance, etc.)"
                            },
                            "file": {
                                "type": "string",
                                "description": "File path"
                            },
                            "line": {
                                "type": ["integer", "null"],
                                "description": "Line number (null if uncertain)"
                            },
                            "title": {
                                "type": "string",
                                "description": copy.issue_title
                            },
                            "description": {
                                "type": "string",
                                "description": copy.issue_description
                            },
                            "suggestion": {
                                "type": ["string", "null"],
                                "description": copy.issue_suggestion
                            },
                            "source_reviewer": {
                                "type": "string",
                                "description": source_reviewer_desc
                            },
                            "validation_note": {
                                "type": "string",
                                "description": validation_note_desc
                            }
                        },
                        "required": ["severity", "certainty", "category", "file", "title", "description"]
                    }
                },
                "positive_points": {
                    "type": "array",
                    "description": copy.positive_points,
                    "items": {
                        "type": "string"
                    }
                },
                "review_mode": {
                    "type": "string",
                    "enum": ["standard", "deep"],
                    "description": "Review mode"
                },
                "review_scope": {
                    "type": "string",
                    "description": scope_desc
                },
                "reviewers": {
                    "type": "array",
                    "description": "Reviewer summaries",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Reviewer display name"
                            },
                            "specialty": {
                                "type": "string",
                                "description": "Reviewer specialty / role"
                            },
                            "status": {
                                "type": "string",
                                "description": "Reviewer result status"
                            },
                            "summary": {
                                "type": "string",
                                "description": reviewer_summary_desc
                            },
                            "issue_count": {
                                "type": "integer",
                                "description": "Validated issue count for this reviewer"
                            }
                        },
                        "required": ["name", "specialty", "status", "summary"],
                        "additionalProperties": false
                    }
                },
                "remediation_plan": {
                    "type": "array",
                    "description": plan_desc,
                    "items": {
                        "type": "string"
                    }
                },
                "report_sections": {
                    "type": "object",
                    "description": "Optional structured sections for richer review report presentation",
                    "properties": {
                        "executive_summary": {
                            "type": "array",
                            "description": "Short user-facing conclusion bullets",
                            "items": {
                                "type": "string"
                            }
                        },
                        "remediation_groups": {
                            "type": "object",
                            "description": "Grouped remediation and follow-up plan items",
                            "properties": {
                                "must_fix": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "should_improve": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "needs_decision": {
                                    "type": "array",
                                    "description": "Items needing user/product judgment. Each item should be an object with a 'question' and 'plan'.",
                                    "items": {
                                        "oneOf": [
                                            {
                                                "type": "object",
                                                "properties": {
                                                    "question": {
                                                        "type": "string",
                                                        "description": "The specific decision the user needs to make"
                                                    },
                                                    "plan": {
                                                        "type": "string",
                                                        "description": "The remediation plan text to execute if the user approves"
                                                    },
                                                    "options": {
                                                        "type": "array",
                                                        "description": "2-4 possible choices or approaches",
                                                        "items": { "type": "string" }
                                                    },
                                                    "tradeoffs": {
                                                        "type": "string",
                                                        "description": "Brief explanation of trade-offs between options"
                                                    },
                                                    "recommendation": {
                                                        "type": "integer",
                                                        "description": "Index of the recommended option (0-based), if any"
                                                    }
                                                },
                                                "required": ["question", "plan"]
                                            },
                                            {
                                                "type": "string"
                                            }
                                        ]
                                    }
                                },
                                "verification": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "additionalProperties": false
                        },
                        "strength_groups": {
                            "type": "object",
                            "description": "Grouped positive observations",
                            "properties": {
                                "architecture": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "maintainability": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "tests": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "security": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "performance": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "user_experience": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "other": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "additionalProperties": false
                        },
                        "coverage_notes": {
                            "type": "array",
                            "description": "Review coverage, confidence, timeout, cancellation, or manual follow-up notes",
                            "items": {
                                "type": "string"
                            }
                        }
                    },
                    "additionalProperties": false
                },
                "schema_version": {
                    "type": "integer",
                    "description": "Schema version for forward compatibility",
                    "minimum": 1
                }
            },
            "required": required,
            "additionalProperties": false
        })
    }

    fn is_deep_review_context(context: Option<&ToolUseContext>) -> bool {
        context
            .and_then(|context| context.agent_type.as_deref())
            .map(str::trim)
            .is_some_and(|agent_type| agent_type == "DeepReview")
    }

    /// Validate and fill missing fields with default values
    ///
    /// When AI-returned data is missing certain fields, fill with default values to avoid entire review failure
    fn validate_and_fill_defaults(input: &mut Value, deep_review: bool) {
        // Fill summary default values
        if input.get("summary").is_none() {
            warn!("CodeReview tool missing summary field, using default values");
            input["summary"] = json!({
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI did not return complete review results"
            });
        } else if let Some(summary) = input.get_mut("summary") {
            if summary.get("overall_assessment").is_none() {
                summary["overall_assessment"] = json!("None");
            }
            if summary.get("risk_level").is_none() {
                summary["risk_level"] = json!("low");
            }
            if summary.get("recommended_action").is_none() {
                summary["recommended_action"] = json!("approve");
            }
        } else {
            warn!(
                "CodeReview tool summary field exists but is not mutable object, using default values"
            );
            input["summary"] = json!({
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI returned invalid summary format"
            });
        }

        // Fill issues default values
        if input.get("issues").is_none() {
            warn!("CodeReview tool missing issues field, using default values");
            input["issues"] = json!([]);
        }

        // Fill positive_points default values
        if input.get("positive_points").is_none() {
            warn!("CodeReview tool missing positive_points field, using default values");
            input["positive_points"] = json!(["None"]);
        }

        if deep_review {
            input["review_mode"] = json!("deep");
            if input.get("review_scope").is_none() {
                input["review_scope"] = json!("Deep review scope was not provided");
            }
        } else if input.get("review_mode").is_none() {
            input["review_mode"] = json!("standard");
        }

        if input.get("reviewers").is_none() {
            input["reviewers"] = json!([]);
        }

        if input.get("remediation_plan").is_none() {
            input["remediation_plan"] = json!([]);
        }

        if input.get("schema_version").is_none() {
            input["schema_version"] = json!(1);
        }
    }

    /// Generate review result using all default values
    ///
    /// Used when retries fail multiple times
    pub fn create_default_result() -> Value {
        json!({
            "schema_version": 1,
            "summary": {
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI review failed, using default result"
            },
            "issues": [],
            "positive_points": ["None"],
            "review_mode": "standard",
            "reviewers": [],
            "remediation_plan": [],
            "schema_version": 1
        })
    }
}

impl Default for CodeReviewTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodeReviewTool {
    fn name(&self) -> &str {
        Self::name_str()
    }

    async fn description(&self) -> BitFunResult<String> {
        let lang = get_app_language_code().await;
        Ok(Self::description_for_language(lang.as_str()))
    }

    fn input_schema(&self) -> Value {
        Self::input_schema_value()
    }

    async fn input_schema_for_model(&self) -> Value {
        let lang = get_app_language_code().await;
        Self::input_schema_value_for_language(lang.as_str())
    }

    async fn input_schema_for_model_with_context(
        &self,
        context: Option<&crate::agentic::tools::framework::ToolUseContext>,
    ) -> Value {
        let lang = get_app_language_code().await;
        Self::input_schema_value_for_language_with_mode(
            lang.as_str(),
            Self::is_deep_review_context(context),
        )
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
        let mut filled_input = input.clone();
        Self::validate_and_fill_defaults(
            &mut filled_input,
            Self::is_deep_review_context(Some(context)),
        );

        Ok(vec![ToolResult::Result {
            data: filled_input,
            result_for_assistant: Some("Code review results submitted successfully".to_string()),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::CodeReviewTool;
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::collections::HashMap;

    fn tool_context(agent_type: Option<&str>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: agent_type.map(str::to_string),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn deep_review_schema_requires_deep_review_fields() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let schema = tool
            .input_schema_for_model_with_context(Some(&context))
            .await;
        let required = schema["required"].as_array().expect("required fields");

        for field in [
            "review_mode",
            "review_scope",
            "reviewers",
            "remediation_plan",
        ] {
            assert!(
                required.iter().any(|value| value.as_str() == Some(field)),
                "DeepReview schema should require {field}"
            );
        }
    }

    #[tokio::test]
    async fn deep_review_submission_defaults_missing_mode_to_deep() {
        let tool = CodeReviewTool::new();
        let context = tool_context(Some("DeepReview"));
        let result = tool
            .call_impl(
                &json!({
                    "summary": {
                        "overall_assessment": "No blocking issues",
                        "risk_level": "low",
                        "recommended_action": "approve"
                    },
                    "issues": [],
                    "positive_points": []
                }),
                &context,
            )
            .await
            .expect("submit review result");

        let ToolResult::Result { data, .. } = &result[0] else {
            panic!("expected tool result");
        };
        assert_eq!(data["review_mode"], "deep");
        assert!(data["reviewers"].as_array().is_some());
        assert!(data["remediation_plan"].as_array().is_some());
    }
}
