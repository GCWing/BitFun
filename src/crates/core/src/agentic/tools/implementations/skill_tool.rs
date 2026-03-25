//! Skill tool implementation
//!
//! Supports loading and executing skills from user-level and project-level directories
//! Manages skill enabled/disabled status through SkillRegistry

use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use log::debug;
use serde_json::{json, Value};
use std::path::Path;

// Use skills module
use super::skills::{get_skill_registry, SkillLocation};

/// Skill tool
pub struct SkillTool;

impl SkillTool {
    pub fn new() -> Self {
        Self
    }

    fn render_description(&self, skills_list: String) -> String {
        let skills_list = if skills_list.is_empty() {
            "No skills available".to_string()
        } else {
            skills_list
        };

        format!(
            r#"Execute a skill within the main conversation

<skills_instructions>
When users ask you to perform tasks, check if any of the available skills below can help complete the task more effectively. Skills provide specialized capabilities and domain knowledge.

How to use skills:
- Invoke skills using this tool with the skill name only (no arguments)
- The skill's prompt will expand and provide detailed instructions on how to complete the task
- Examples:
  - `command: "pdf"` - invoke the pdf skill
  - `command: "xlsx"` - invoke the xlsx skill
  - `command: "ms-office-suite:pdf"` - invoke using fully qualified name

Important:
- Only use skills listed in <available_skills> below
- Do not invoke a skill that is already running
</skills_instructions>

<available_skills>
{}
</available_skills>"#,
            skills_list
        )
    }

    async fn build_description(&self, workspace_root: Option<&Path>) -> String {
        let registry = get_skill_registry();
        let available_skills = match workspace_root {
            Some(workspace_root) => {
                registry
                    .get_enabled_skills_xml_for_workspace(Some(workspace_root))
                    .await
            }
            None => registry.get_enabled_skills_xml().await,
        };

        self.render_description(available_skills.join("\n"))
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(self.build_description(None).await)
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self
            .build_description(context.and_then(|ctx| ctx.workspace_root()))
            .await)
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The skill name (no arguments). E.g., \"pdf\" or \"xlsx\""
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        if input
            .get("command")
            .and_then(|v| v.as_str())
            .map_or(true, |s| s.is_empty())
        {
            return ValidationResult {
                result: false,
                message: Some("command is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
            format!("The \"{}\" skill is loaded.", command)
        } else {
            "Loading skill...".to_string()
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let skill_name = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("command is required".to_string()))?;

        debug!("Skill tool executing skill: {}", skill_name);

        // Find and load skill through registry
        let registry = get_skill_registry();
        let skill_data = registry
            .find_and_load_skill_for_workspace(skill_name, context.workspace_root())
            .await?;

        let location_str = match skill_data.location {
            SkillLocation::User => "user",
            SkillLocation::Project => "project",
        };

        let result_for_assistant = format!(
            "Skill '{}' loaded successfully. Note: any paths mentioned in this skill are relative to {}, not the workspace.\n\n{}",
            skill_data.name, skill_data.path, skill_data.content
        );

        let result = ToolResult::Result {
            data: json!({
                "skill_name": skill_data.name,
                "description": skill_data.description,
                "location": location_str,
                "content": skill_data.content,
                "success": true
            }),
            result_for_assistant: Some(result_for_assistant),
            image_attachments: None,
        };

        Ok(vec![result])
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::new()
    }
}
