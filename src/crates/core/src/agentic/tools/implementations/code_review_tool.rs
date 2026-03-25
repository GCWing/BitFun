//! Code review result submission tool
//!
//! Used to get structured code review results.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
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

    pub fn description_str() -> &'static str {
        "Submit code review results. After completing the review analysis, you must call this tool to submit a structured review report. All text fields must use Chinese (Simplified Chinese)."
    }

    pub fn input_schema_value() -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "object",
                    "description": "Review summary",
                    "properties": {
                        "overall_assessment": {
                            "type": "string",
                            "description": "Overall assessment (2-3 sentences, use Chinese)"
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
                            "description": "Context limitation note (optional, use Chinese)"
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
                                "description": "Issue title (Chinese)"
                            },
                            "description": {
                                "type": "string",
                                "description": "Issue description (Chinese)"
                            },
                            "suggestion": {
                                "type": ["string", "null"],
                                "description": "Fix suggestion (Chinese, optional)"
                            }
                        },
                        "required": ["severity", "certainty", "category", "file", "title", "description"]
                    }
                },
                "positive_points": {
                    "type": "array",
                    "description": "Code strengths (1-2 items, Chinese)",
                    "items": {
                        "type": "string"
                    }
                }
            },
            "required": ["summary", "issues", "positive_points"],
            "additionalProperties": false
        })
    }

    /// Validate and fill missing fields with default values
    ///
    /// When AI-returned data is missing certain fields, fill with default values to avoid entire review failure
    fn validate_and_fill_defaults(input: &mut Value) {
        // Fill summary default values
        if input.get("summary").is_none() {
            warn!("CodeReview tool missing summary field, using default values");
            input["summary"] = json!({
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI did not return complete review results"
            });
        } else {
            if let Some(summary) = input.get_mut("summary") {
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
    }

    /// Generate review result using all default values
    ///
    /// Used when retries fail multiple times
    pub fn create_default_result() -> Value {
        json!({
            "summary": {
                "overall_assessment": "None",
                "risk_level": "low",
                "recommended_action": "approve",
                "confidence_note": "AI review failed, using default result"
            },
            "issues": [],
            "positive_points": ["None"]
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
        Ok(Self::description_str().to_string())
    }

    fn input_schema(&self) -> Value {
        Self::input_schema_value()
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
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        // Fill missing default values
        let mut filled_input = input.clone();
        Self::validate_and_fill_defaults(&mut filled_input);

        // Return success with filled data
        Ok(vec![ToolResult::Result {
            data: filled_input,
            result_for_assistant: Some("Code review results submitted successfully".to_string()),
            image_attachments: None,
        }])
    }
}
