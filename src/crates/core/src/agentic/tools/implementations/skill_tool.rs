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

    async fn build_description_for_context(&self, context: Option<&ToolUseContext>) -> String {
        let registry = get_skill_registry();
        let available_skills = match context {
            Some(ctx) if ctx.is_remote() => {
                if let Some(fs) = ctx.ws_fs() {
                    let root = ctx
                        .workspace
                        .as_ref()
                        .map(|w| w.root_path_string())
                        .unwrap_or_default();
                    registry
                        .get_resolved_skills_xml_for_remote_workspace(
                            fs,
                            &root,
                            ctx.agent_type.as_deref(),
                        )
                        .await
                } else {
                    registry
                        .get_resolved_skills_xml_for_workspace(None, ctx.agent_type.as_deref())
                        .await
                }
            }
            Some(ctx) => {
                registry
                    .get_resolved_skills_xml_for_workspace(
                        ctx.workspace_root(),
                        ctx.agent_type.as_deref(),
                    )
                    .await
            }
            None => {
                registry
                    .get_resolved_skills_xml_for_workspace(None, None)
                    .await
            }
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
        Ok(self.build_description_for_context(None).await)
    }

    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        let mut s = self.build_description_for_context(context).await;
        if context.map(|c| c.is_remote()).unwrap_or(false)
            && context.and_then(|c| c.ws_fs()).is_none()
        {
            s.push_str(
                "\n\n**Remote workspace:** Project-level skills on the server could not be indexed because workspace I/O is unavailable. Only user-level skills are shown; BitFun will not fall back to scanning the remote path on the local filesystem.",
            );
        }
        Ok(s)
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
            .is_none_or(|s| s.is_empty())
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
        let skill_data = if context.is_remote() {
            if let Some(ws_fs) = context.ws_fs() {
                let root = context
                    .workspace
                    .as_ref()
                    .map(|w| w.root_path_string())
                    .unwrap_or_default();
                registry
                    .find_and_load_skill_for_remote_workspace(
                        skill_name,
                        ws_fs,
                        &root,
                        context.agent_type.as_deref(),
                    )
                    .await?
            } else {
                registry
                    .find_and_load_skill_for_workspace(
                        skill_name,
                        None,
                        context.agent_type.as_deref(),
                    )
                    .await?
            }
        } else {
            registry
                .find_and_load_skill_for_workspace(
                    skill_name,
                    context.workspace_root(),
                    context.agent_type.as_deref(),
                )
                .await?
        };

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

#[cfg(test)]
mod tests {
    use super::SkillTool;
    use crate::agentic::tools::framework::{Tool, ToolResult};
    use crate::agentic::workspace::{
        WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
        WorkspaceServices, WorkspaceShell,
    };
    use crate::agentic::WorkspaceBinding;
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use async_trait::async_trait;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct FakeRemoteFs;

    #[async_trait]
    impl WorkspaceFileSystem for FakeRemoteFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            if path == "/remote/project/.bitfun/skills/remote-only/SKILL.md" {
                return Ok(r#"---
name: remote-only-skill-for-test
description: Remote project skill visible only through workspace services.
---

Use the remote project skill.
"#
                .to_string());
            }
            anyhow::bail!("not found: {}", path)
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.bitfun/skills"
                    | "/remote/project/.bitfun/skills/remote-only"
                    | "/remote/project/.bitfun/skills/remote-only/SKILL.md"
            ))
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(path == "/remote/project/.bitfun/skills/remote-only/SKILL.md")
        }

        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.bitfun/skills" | "/remote/project/.bitfun/skills/remote-only"
            ))
        }

        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            if path == "/remote/project/.bitfun/skills" {
                return Ok(vec![WorkspaceDirEntry {
                    name: "remote-only".to_string(),
                    path: "/remote/project/.bitfun/skills/remote-only".to_string(),
                    is_dir: true,
                    is_symlink: false,
                }]);
            }
            Ok(vec![])
        }
    }

    struct FakeShell;

    #[async_trait]
    impl WorkspaceShell for FakeShell {
        async fn exec_with_options(
            &self,
            _command: &str,
            _options: WorkspaceCommandOptions,
        ) -> anyhow::Result<WorkspaceCommandResult> {
            Ok(WorkspaceCommandResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
                interrupted: false,
                timed_out: false,
            })
        }
    }

    #[tokio::test]
    async fn remote_description_indexes_project_skills_through_workspace_services() {
        let identity =
            workspace_session_identity("/remote/project", Some("conn-1"), Some("remote-host"))
                .expect("remote identity");
        let workspace = WorkspaceBinding::new_remote(
            Some("remote-workspace".to_string()),
            PathBuf::from("/remote/project"),
            "conn-1".to_string(),
            "Remote".to_string(),
            identity,
        );
        let context = crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(workspace),
            custom_data: Default::default(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: Some(WorkspaceServices {
                fs: Arc::new(FakeRemoteFs),
                shell: Arc::new(FakeShell),
            }),
        };

        let description = SkillTool::new()
            .description_with_context(Some(&context))
            .await
            .expect("description");

        assert!(description.contains("remote-only-skill-for-test"));
        assert!(
            description.contains("Remote project skill visible only through workspace services.")
        );
    }

    #[tokio::test]
    async fn remote_call_loads_default_hidden_builtin_team_skill_when_explicitly_invoked() {
        let identity =
            workspace_session_identity("/remote/project", Some("conn-1"), Some("remote-host"))
                .expect("remote identity");
        let workspace = WorkspaceBinding::new_remote(
            Some("remote-workspace".to_string()),
            PathBuf::from("/remote/project"),
            "conn-1".to_string(),
            "Remote".to_string(),
            identity,
        );
        let context = crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            agent_type: Some("agentic".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(workspace),
            custom_data: Default::default(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: Default::default(),
            workspace_services: Some(WorkspaceServices {
                fs: Arc::new(FakeRemoteFs),
                shell: Arc::new(FakeShell),
            }),
        };

        let results = SkillTool::new()
            .call_impl(&json!({ "command": "cso" }), &context)
            .await
            .expect("explicit cso invocation should load the local built-in skill");

        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result payload");
        };
        assert_eq!(data["skill_name"], "cso");
        assert_eq!(data["location"], "user");
        assert!(data["content"]
            .as_str()
            .unwrap_or_default()
            .contains("# /cso"));
    }
}
