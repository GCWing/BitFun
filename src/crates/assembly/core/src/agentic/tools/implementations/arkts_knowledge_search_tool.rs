//! arkts_knowledge_search tool — search local HarmonyOS documentation.
//!
//! Direct port of deveco-code `arkts_knowledge_search`. Shells out to
//! `devecocli docs search kw1 kw2 ...`. Read-only and concurrency-safe.

use super::devecocli_run::{run_devecocli, DevecocliOptions};
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

const KNOWLEDGE_SEARCH_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ArktsKnowledgeSearchTool;

impl Default for ArktsKnowledgeSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ArktsKnowledgeSearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ArktsKnowledgeSearchTool {
    fn name(&self) -> &str {
        "arkts_knowledge_search"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Search local HarmonyOS documentation for ArkTS/ArkUI answers.

Use this when the user asks about ArkTS syntax, ArkUI components, HarmonyOS APIs, or platform conventions and you need authoritative local docs. No login required.

Parameters:
- question (required, string): concise ArkTS/ArkUI/HarmonyOS question or keywords.

Example:
- {"question": "How to use @State decorator"}
- {"question": "ArkUI List component lazy loading"}"#
            .to_string())
    }

    fn short_description(&self) -> String {
        "Search local HarmonyOS documentation for ArkTS/ArkUI answers.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "Concise ArkTS/ArkUI/HarmonyOS question or keywords for local HarmonyOS documentation search." }
            },
            "required": ["question"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let question = match input.get("question").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                return ValidationResult {
                    result: false,
                    message: Some("question is required".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };
        if question.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("question cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        ValidationResult { result: true, message: None, error_code: None, meta: None }
    }

    fn render_tool_use_message(&self, input: &Value, options: &ToolRenderOptions) -> String {
        let question = input.get("question").and_then(|v| v.as_str()).unwrap_or("");
        let preview: String = question.chars().take(60).collect();
        if options.verbose {
            format!("HarmonyOS docs search: {}", preview)
        } else {
            format!("Docs search: {}", preview)
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let question = input.get("question").and_then(|v| v.as_str()).unwrap_or_default();
        let keywords: Vec<String> = question
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let kw_refs: Vec<&str> = keywords.iter().map(|s| s.as_str()).collect();
        let mut argv: Vec<&str> = vec!["docs", "search"];
        argv.extend(kw_refs);

        let out = run_devecocli(&argv, context, DevecocliOptions { timeout: KNOWLEDGE_SEARCH_TIMEOUT, ..Default::default() }).await?;
        let combined = [out.stdout.as_str(), out.stderr.as_str()]
            .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("\n");
        if out.exit_code != 0 {
            return Err(BitFunError::tool(format!(
                "arkts_knowledge_search failed (exit {}):\n{}", out.exit_code, combined
            )));
        }
        Ok(vec![ToolResult::Result {
            data: json!({
                "tool": "arkts_knowledge_search", "exitCode": out.exit_code,
                "question": question, "keywords": keywords,
            }),
            result_for_assistant: Some(if combined.is_empty() { "No documentation matches found.".to_string() } else { combined }),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::ArktsKnowledgeSearchTool;
    use crate::agentic::tools::framework::{Tool, ToolUseContext, ValidationResult};
    use serde_json::json;
    use std::collections::HashMap;

    fn test_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn rejects_empty_question() {
        let r = ArktsKnowledgeSearchTool::new().validate_input(&json!({"question": "   "}), Some(&test_context())).await;
        assert!(!r.result);
    }

    #[tokio::test]
    async fn rejects_missing_question() {
        let r = ArktsKnowledgeSearchTool::new().validate_input(&json!({}), Some(&test_context())).await;
        assert!(!r.result);
    }

    #[tokio::test]
    async fn accepts_valid_question() {
        let r = ArktsKnowledgeSearchTool::new().validate_input(&json!({"question": "How to use @State"}), Some(&test_context())).await;
        assert!(r.result);
    }

    #[test]
    fn is_readonly() {
        assert!(ArktsKnowledgeSearchTool::new().is_readonly());
    }

    #[test]
    fn tool_name_matches() {
        assert_eq!(ArktsKnowledgeSearchTool::new().name(), "arkts_knowledge_search");
    }
}
