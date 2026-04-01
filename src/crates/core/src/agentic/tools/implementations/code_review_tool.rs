//! Code review result submission tool
//!
//! Used to get structured code review results.

use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::service::config::get_app_language_code;
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
        match lang_code {
            "en-US" => "Submit code review results. After completing the review analysis, you must call this tool to submit a structured review report. All user-visible text fields must be in English (per app language setting).".to_string(),
            _ => "提交代码审查结果。完成审查分析后必须调用本工具提交结构化审查报告。所有用户可见的文本字段必须使用简体中文。".to_string(),
        }
    }

    pub fn input_schema_value_for_language(lang_code: &str) -> Value {
        let (oa, conf, title, desc, sugg, strengths) = match lang_code {
            "en-US" => (
                "Overall assessment (2-3 sentences, in English)",
                "Context limitation note (optional, in English)",
                "Issue title (in English)",
                "Issue description (in English)",
                "Fix suggestion (in English, optional)",
                "Code strengths (1-2 items, in English)",
            ),
            _ => (
                "总体评价（2-3 句，使用简体中文）",
                "上下文局限说明（可选，使用简体中文）",
                "问题标题（简体中文）",
                "问题描述（简体中文）",
                "修复建议（可选，简体中文）",
                "代码优点（1-2 条，简体中文）",
            ),
        };

        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "object",
                    "description": "Review summary",
                    "properties": {
                        "overall_assessment": {
                            "type": "string",
                            "description": oa
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
                            "description": conf
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
                                "description": title
                            },
                            "description": {
                                "type": "string",
                                "description": desc
                            },
                            "suggestion": {
                                "type": ["string", "null"],
                                "description": sugg
                            }
                        },
                        "required": ["severity", "certainty", "category", "file", "title", "description"]
                    }
                },
                "positive_points": {
                    "type": "array",
                    "description": strengths,
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
        let _ = context;
        self.input_schema_for_model().await
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
