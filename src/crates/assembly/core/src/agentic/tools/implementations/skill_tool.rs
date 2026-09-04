//! Skill tool implementation
//!
//! Supports loading and executing skills from user-level and project-level directories
//! Manages skill enabled/disabled status through SkillRegistry

use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use async_trait::async_trait;
use log::debug;
use openbitfun_services_core::markdown::expand_prompt_template_arguments_with_names;
use serde_json::{json, Value};

// Use skills module
use super::skills::{get_skill_registry, render_loaded_skill_for_assistant};

/// Skill tool
pub struct SkillTool;

impl SkillTool {
    pub fn new() -> Self {
        Self
    }

    fn render_description(&self) -> String {
        r#"Execute a skill within the main conversation

<skills_instructions>
When users ask you to perform tasks, check whether any skills listed in the current skill listing can help complete the task more effectively. Skills provide specialized capabilities and domain knowledge.

How to use skills:
- Invoke skills using this tool with the listed skill name or stable key and optional arguments
- Pass user-provided invocation text relevant to the skill through `arguments`; never copy an `argument-hint` into arguments
- The skill's prompt will expand and provide detailed instructions on how to complete the task
- Examples:
  - `command: "writing-skills"` - invoke the writing-skills skill
  - `command: "review", arguments: "src/main.rs carefully"` - invoke a skill with arguments
  - `command: "user::openbitfun-system::ppt-design"` - invoke a specific built-in skill by stable key

Important:
- Only use skills listed in the current skill listing's <available_skills> section, unless a trusted host task explicitly supplies an exact stable key or the user's message contains an exact `[$skill-name]` invocation
- Do not invoke a skill that is already running
</skills_instructions>"#
            .to_string()
    }

    pub(crate) async fn resolved_skills_xml_for_context(
        context: Option<&ToolUseContext>,
    ) -> String {
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

        available_skills.join("\n")
    }

    pub(crate) async fn build_available_skills_context_section(
        context: Option<&ToolUseContext>,
    ) -> Option<String> {
        let skills_list = Self::resolved_skills_xml_for_context(context).await;
        let skills_list = skills_list.trim();
        if skills_list.is_empty() {
            return None;
        }

        let mut section = format!("<available_skills>\n{}\n</available_skills>", skills_list);
        if context.map(|c| c.is_remote()).unwrap_or(false)
            && context.and_then(|c| c.ws_fs()).is_none()
        {
            section.push_str(
                "\n\nRemote workspace note: Project-level skills on the server could not be indexed because workspace I/O is unavailable. Only user-level skills are shown; OpenBitFun will not fall back to scanning the remote path on the local filesystem.",
            );
        }
        Some(section)
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    async fn description(&self) -> OpenBitFunResult<String> {
        Ok(self.render_description())
    }

    fn short_description(&self) -> String {
        "Discover and load reusable skills for specialized workflows.".to_string()
    }

    async fn description_with_context(
        &self,
        _context: Option<&ToolUseContext>,
    ) -> OpenBitFunResult<String> {
        Ok(self.render_description())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The skill name or stable key. E.g., \"writing-skills\" or \"user::openbitfun-system::ppt-design\""
                },
                "arguments": {
                    "type": "string",
                    "description": "Optional arguments supplied to the skill prompt"
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

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> OpenBitFunResult<Vec<PermissionIntent>> {
        let skill_name = input
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|skill_name| !skill_name.is_empty())
            .ok_or_else(|| OpenBitFunError::validation("command is required".to_string()))?;
        Ok(vec![PermissionIntent::new(
            "skill",
            vec![skill_name.to_string()],
        )])
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
        if input
            .get("arguments")
            .is_some_and(|value| !value.is_string())
        {
            return ValidationResult {
                result: false,
                message: Some("arguments must be a string".to_string()),
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
    ) -> OpenBitFunResult<Vec<ToolResult>> {
        let skill_name = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OpenBitFunError::tool("command is required".to_string()))?;

        debug!("Skill tool executing skill: {}", skill_name);

        // Find and load skill through registry
        let registry = get_skill_registry();
        let use_stable_key = skill_name.split("::").count() == 3;
        let mut skill_data = if context.is_remote() {
            if let Some(ws_fs) = context.ws_fs() {
                let root = context
                    .workspace
                    .as_ref()
                    .map(|w| w.root_path_string())
                    .unwrap_or_default();
                if use_stable_key {
                    registry
                        .find_and_load_skill_by_key_for_remote_workspace(
                            skill_name,
                            ws_fs,
                            &root,
                            context.agent_type.as_deref(),
                        )
                        .await?
                } else {
                    registry
                        .find_and_load_skill_for_remote_workspace(
                            skill_name,
                            ws_fs,
                            &root,
                            context.agent_type.as_deref(),
                        )
                        .await?
                }
            } else {
                if use_stable_key {
                    registry
                        .find_and_load_skill_by_key_for_workspace(
                            skill_name,
                            None,
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
            }
        } else {
            if use_stable_key {
                registry
                    .find_and_load_skill_by_key_for_workspace(
                        skill_name,
                        context.workspace_root(),
                        context.agent_type.as_deref(),
                    )
                    .await?
            } else {
                registry
                    .find_and_load_skill_for_workspace(
                        skill_name,
                        context.workspace_root(),
                        context.agent_type.as_deref(),
                    )
                    .await?
            }
        };

        if let Some(arguments) = input.get("arguments").and_then(Value::as_str) {
            skill_data.content = expand_prompt_template_arguments_with_names(
                &skill_data.content,
                arguments,
                &skill_data.argument_names,
            );
        }
        let location_str = skill_data.location.as_str();
        let result_for_assistant = render_loaded_skill_for_assistant(&skill_data, use_stable_key);

        let result = ToolResult::Result {
            data: json!({
                "skill_name": skill_data.name,
                "skill_key": skill_data.key,
                "source_slot": skill_data.source_slot,
                "source_id": skill_data.source_id,
                "source_label": skill_data.source_label,
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
    use crate::agentic::tools::implementations::skills::{registry::SkillRegistry, SkillLocation};
    use crate::agentic::workspace::{
        WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
        WorkspaceServices, WorkspaceShell,
    };
    use crate::agentic::WorkspaceBinding;
    use crate::service::remote_ssh::workspace_state::workspace_session_identity;
    use async_trait::async_trait;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct FakeRemoteFs;

    #[async_trait]
    impl WorkspaceFileSystem for FakeRemoteFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            if path == "/remote/project/.openbitfun/skills/remote-only/SKILL.md" {
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
                "/remote/project/.openbitfun/skills"
                    | "/remote/project/.openbitfun/skills/remote-only"
                    | "/remote/project/.openbitfun/skills/remote-only/SKILL.md"
            ))
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(path == "/remote/project/.openbitfun/skills/remote-only/SKILL.md")
        }

        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.openbitfun/skills"
                    | "/remote/project/.openbitfun/skills/remote-only"
            ))
        }

        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            if path == "/remote/project/.openbitfun/skills" {
                return Ok(vec![WorkspaceDirEntry {
                    name: "remote-only".to_string(),
                    path: "/remote/project/.openbitfun/skills/remote-only".to_string(),
                    is_dir: true,
                    is_symlink: false,
                    modified: None,
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

    struct ClaudeRemoteFs;

    #[async_trait]
    impl WorkspaceFileSystem for ClaudeRemoteFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            if path == "/remote/project/.claude/skills/remote-review/SKILL.md" {
                return Ok(
                    "---\ndescription: Review a remote target.\narguments: target focus\n---\n\nReview $target for $focus.\n"
                        .to_string(),
                );
            }
            anyhow::bail!("not found: {}", path)
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            Ok(self.is_dir(path).await? || self.is_file(path).await?)
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(path == "/remote/project/.claude/skills/remote-review/SKILL.md")
        }

        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.claude/skills" | "/remote/project/.claude/skills/remote-review"
            ))
        }

        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            if path == "/remote/project/.claude/skills" {
                return Ok(vec![WorkspaceDirEntry {
                    name: "remote-review".to_string(),
                    path: "/remote/project/.claude/skills/remote-review".to_string(),
                    is_dir: true,
                    is_symlink: false,
                    modified: None,
                }]);
            }
            Ok(vec![])
        }
    }

    fn local_context(root: PathBuf) -> crate::agentic::tools::framework::ToolUseContext {
        crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: Some(WorkspaceBinding::new(None, root)),
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: Default::default(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: openbitfun_runtime_ports::ToolRuntimeHandles::new(None, None),
        }
    }

    #[test]
    fn skill_schema_exposes_optional_arguments() {
        let schema = SkillTool::new().input_schema();

        assert_eq!(schema["properties"]["arguments"]["type"], "string");
        assert_eq!(schema["required"], json!(["command"]));
    }

    #[tokio::test]
    async fn skill_call_expands_arguments_in_loaded_prompt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".openbitfun/skills/argument-skill");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: argument-skill\ndescription: Argument expansion test.\nargument-hint: \"[file] [focus]\"\n---\n\nReview $0 with $ARGUMENTS[1]. Full: $ARGUMENTS\n",
        )
        .expect("skill markdown");
        let context = local_context(temp.path().to_path_buf());

        let results = SkillTool::new()
            .call_impl(
                &json!({
                    "command": "argument-skill",
                    "arguments": "\"src/main.rs\" carefully"
                }),
                &context,
            )
            .await
            .expect("skill arguments should expand");

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result payload");
        };
        let expected = "Review src/main.rs with carefully. Full: \"src/main.rs\" carefully";
        assert_eq!(data["content"], expected);
        assert!(result_for_assistant
            .as_deref()
            .unwrap_or_default()
            .contains(expected));
    }

    #[tokio::test]
    async fn explicit_skill_call_returns_original_name_and_source_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".codex/skills/deep-research");
        fs::create_dir_all(skill_dir.join("agents")).expect("skill agents directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deep-research\ndescription: Academic research workflow.\n---\n\nResearch the topic.\n",
        )
        .expect("skill markdown");
        fs::write(
            skill_dir.join("agents/openai.yaml"),
            "interface:\n  display_name: \"Academic Deep Research\"\npolicy:\n  allow_implicit_invocation: false\n",
        )
        .expect("skill interface metadata");
        let mut context = local_context(temp.path().to_path_buf());
        context.agent_type = Some("DeepResearch".to_string());

        let results = SkillTool::new()
            .call_impl(&json!({ "command": "deep-research" }), &context)
            .await
            .expect("explicit-only skill should remain directly loadable");

        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result payload");
        };
        assert_eq!(data["skill_name"], "deep-research");
        assert!(data.get("skill_display_name").is_none());
        assert_eq!(data["source_slot"], "codex");
        assert_eq!(data["source_id"], "codex");
        assert_eq!(data["source_label"], "Codex");
    }

    #[tokio::test]
    async fn native_deep_research_omits_same_named_skill_only_from_implicit_catalog() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".codex/skills/deep-research");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deep-research\ndescription: Research workflow.\n---\n\nResearch the topic.\n",
        )
        .expect("skill markdown");
        let registry = SkillRegistry::global();
        let resolved = registry
            .get_resolved_skills_for_workspace(Some(temp.path()), Some("DeepResearch"))
            .await;
        let skill = resolved
            .iter()
            .find(|skill| skill.name == "deep-research")
            .expect("same-named skill should remain enabled and discoverable");
        assert!(skill.allow_implicit_invocation);
        let implicit = registry
            .get_implicitly_invocable_skills_for_workspace(Some(temp.path()), Some("DeepResearch"))
            .await;
        assert!(!implicit.iter().any(|skill| skill.name == "deep-research"));

        let loaded = registry
            .find_and_load_skill_by_key_for_workspace(
                &skill.key,
                Some(temp.path()),
                Some("DeepResearch"),
            )
            .await
            .expect("native DeepResearch should still allow explicit stable-key invocation");
        assert_eq!(loaded.name, "deep-research");
        assert_eq!(loaded.source_label, "Codex");
    }

    #[tokio::test]
    async fn local_claude_skill_uses_source_semantics_for_discovery_load_and_arguments() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".claude/skills/deploy-service");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: Deploy a service.\narguments: service environment\n---\n\nDeploy $service to $environment.\n",
        )
        .expect("skill markdown");
        let context = local_context(temp.path().to_path_buf());

        let visible = SkillRegistry::global()
            .get_resolved_skills_for_workspace(Some(temp.path()), None)
            .await;
        assert!(visible
            .iter()
            .any(|skill| { skill.name == "deploy-service" && skill.source_slot == "claude" }));

        let results = SkillTool::new()
            .call_impl(
                &json!({
                    "command": "deploy-service",
                    "arguments": "api staging"
                }),
                &context,
            )
            .await
            .expect("Claude skill should load with the discovery dialect");
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected result payload");
        };
        assert_eq!(data["content"], "Deploy api to staging.");
    }

    #[tokio::test]
    async fn remote_claude_skill_uses_the_same_dialect_for_discovery_and_load() {
        let registry = SkillRegistry::global();
        let visible = registry
            .get_resolved_skills_for_remote_workspace(&ClaudeRemoteFs, "/remote/project", None)
            .await;
        assert!(visible
            .iter()
            .any(|skill| { skill.name == "remote-review" && skill.source_slot == "claude" }));

        let loaded = registry
            .find_and_load_skill_for_remote_workspace(
                "remote-review",
                &ClaudeRemoteFs,
                "/remote/project",
                None,
            )
            .await
            .expect("remote Claude skill should load with the discovery dialect");
        assert_eq!(loaded.name, "remote-review");
        assert_eq!(loaded.source_id, "claude-code");
        assert_eq!(loaded.source_label, "Claude Code");
        assert_eq!(loaded.argument_names, ["target", "focus"]);

        let loaded_by_key = registry
            .find_and_load_skill_by_key_for_remote_workspace(
                &loaded.key,
                &ClaudeRemoteFs,
                "/remote/project",
                None,
            )
            .await
            .expect("remote skill should retain source metadata when loaded by key");
        assert_eq!(loaded_by_key.source_id, loaded.source_id);
        assert_eq!(loaded_by_key.source_label, loaded.source_label);
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
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: Default::default(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: openbitfun_runtime_ports::ToolRuntimeHandles::new(
                Some(WorkspaceServices {
                    fs: Arc::new(FakeRemoteFs),
                    shell: Arc::new(FakeShell),
                }),
                None,
            ),
        };

        let description = SkillTool::build_available_skills_context_section(Some(&context))
            .await
            .expect("available skills section");

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
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: Default::default(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: openbitfun_runtime_ports::ToolRuntimeHandles::new(
                Some(WorkspaceServices {
                    fs: Arc::new(FakeRemoteFs),
                    shell: Arc::new(FakeShell),
                }),
                None,
            ),
        };

        let results = SkillTool::new()
            .call_impl(&json!({ "command": "cso" }), &context)
            .await
            .expect("explicit cso invocation should load the local built-in skill");

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result payload");
        };
        assert_eq!(data["skill_name"], "cso");
        assert_eq!(data["location"], "user");
        assert!(data["content"]
            .as_str()
            .unwrap_or_default()
            .contains("# /cso"));
        let assistant = result_for_assistant.as_deref().unwrap_or_default();
        assert!(assistant.contains("<skill_content>\n"));
        assert!(assistant.contains("\n</skill_content>"));
        assert!(assistant.contains("# /cso"));
        assert!(!assistant.contains("from stable key"));
    }

    #[tokio::test]
    async fn stable_key_loads_the_exact_builtin_skill() {
        let context = crate::agentic::tools::framework::ToolUseContext {
            tool_call_id: None,
            agent_type: Some("Cowork".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: Default::default(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: openbitfun_runtime_ports::ToolRuntimeHandles::new(None, None),
        };

        let results = SkillTool::new()
            .call_impl(
                &json!({ "command": "user::openbitfun-system::ppt-design" }),
                &context,
            )
            .await
            .expect("stable key should load OpenBitFun's built-in ppt-design skill");

        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected result payload");
        };
        assert_eq!(data["skill_name"], "ppt-design");
        assert_eq!(data["skill_key"], "user::openbitfun-system::ppt-design");
        assert_eq!(data["source_slot"], "openbitfun-system");
        assert_eq!(data["source_id"], "openbitfun");
        assert_eq!(data["source_label"], "OpenBitFun");
        assert!(data["content"]
            .as_str()
            .unwrap_or_default()
            .contains("references/editable-pptx.md"));
        let assistant = result_for_assistant.as_deref().unwrap_or_default();
        assert!(assistant.contains("from stable key 'user::openbitfun-system::ppt-design'"));
        assert!(assistant.contains("<skill_content>\n"));
        assert!(assistant.contains("\n</skill_content>"));
        assert!(assistant.contains("references/editable-pptx.md"));
    }

    struct OrderingRemoteFs;

    #[async_trait]
    impl WorkspaceFileSystem for OrderingRemoteFs {
        async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
            Ok(self.read_file_text(path).await?.into_bytes())
        }

        async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
            match path {
                "/remote/project/.openbitfun/skills/z-last/SKILL.md" => {
                    Ok("---\nname: z-last\ndescription: last\n---\n\nz\n".to_string())
                }
                "/remote/project/.openbitfun/skills/z-last/agents/openai.yaml" => {
                    Ok("policy:\n  allow_implicit_invocation: false\n".to_string())
                }
                "/remote/project/.openbitfun/skills/a-first/SKILL.md" => {
                    Ok("---\nname: A-First\ndescription: first\n---\n\na\n".to_string())
                }
                "/remote/project/.openbitfun/skills/dup-one/SKILL.md" => {
                    Ok("---\nname: dup\ndescription: dup one\n---\n\none\n".to_string())
                }
                "/remote/project/.openbitfun/skills/dup-two/SKILL.md" => {
                    Ok("---\nname: dup\ndescription: dup two\n---\n\ntwo\n".to_string())
                }
                _ => anyhow::bail!("not found: {}", path),
            }
        }

        async fn write_file(&self, _path: &str, _contents: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }

        async fn exists(&self, path: &str) -> anyhow::Result<bool> {
            Ok(self.is_dir(path).await? || self.is_file(path).await?)
        }

        async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.openbitfun/skills/z-last/SKILL.md"
                    | "/remote/project/.openbitfun/skills/z-last/agents/openai.yaml"
                    | "/remote/project/.openbitfun/skills/a-first/SKILL.md"
                    | "/remote/project/.openbitfun/skills/dup-one/SKILL.md"
                    | "/remote/project/.openbitfun/skills/dup-two/SKILL.md"
            ))
        }

        async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
            Ok(matches!(
                path,
                "/remote/project/.openbitfun/skills"
                    | "/remote/project/.openbitfun/skills/z-last"
                    | "/remote/project/.openbitfun/skills/a-first"
                    | "/remote/project/.openbitfun/skills/dup-one"
                    | "/remote/project/.openbitfun/skills/dup-two"
            ))
        }

        async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
            match path {
                "/remote/project/.openbitfun/skills" => Ok(vec![
                    WorkspaceDirEntry {
                        name: "z-last".to_string(),
                        path: "/remote/project/.openbitfun/skills/z-last".to_string(),
                        is_dir: true,
                        is_symlink: false,
                        modified: None,
                    },
                    WorkspaceDirEntry {
                        name: "a-first".to_string(),
                        path: "/remote/project/.openbitfun/skills/a-first".to_string(),
                        is_dir: true,
                        is_symlink: false,
                        modified: None,
                    },
                    WorkspaceDirEntry {
                        name: "dup-two".to_string(),
                        path: "/remote/project/.openbitfun/skills/dup-two".to_string(),
                        is_dir: true,
                        is_symlink: false,
                        modified: None,
                    },
                    WorkspaceDirEntry {
                        name: "dup-one".to_string(),
                        path: "/remote/project/.openbitfun/skills/dup-one".to_string(),
                        is_dir: true,
                        is_symlink: false,
                        modified: None,
                    },
                ]),
                _ => Ok(vec![]),
            }
        }
    }

    #[tokio::test]
    async fn prompt_stability_remote_skill_resolution_is_sorted_and_deterministic() {
        let skills = SkillRegistry::global()
            .get_resolved_skills_for_remote_workspace(&OrderingRemoteFs, "/remote/project", None)
            .await;

        assert_eq!(
            skills
                .iter()
                .filter(|skill| skill.level == SkillLocation::Project)
                .map(|skill| skill.name.as_str())
                .collect::<Vec<_>>(),
            vec!["A-First", "dup", "z-last"]
        );
        assert_eq!(
            skills
                .iter()
                .find(|skill| skill.name == "dup")
                .map(|skill| skill.description.as_str()),
            Some("dup one")
        );
    }

    #[tokio::test]
    async fn remote_codex_policy_hides_only_the_implicit_model_catalog() {
        let registry = SkillRegistry::global();
        let resolved = registry
            .get_resolved_skills_for_remote_workspace(&OrderingRemoteFs, "/remote/project", None)
            .await;
        let implicit = registry
            .get_implicitly_invocable_skills_for_remote_workspace(
                &OrderingRemoteFs,
                "/remote/project",
                None,
            )
            .await;

        assert!(resolved.iter().any(|skill| skill.name == "z-last"));
        assert!(!implicit.iter().any(|skill| skill.name == "z-last"));
    }

    #[tokio::test]
    async fn local_codex_policy_hides_only_the_implicit_model_catalog() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".codex/skills/local-explicit-only");
        fs::create_dir_all(skill_dir.join("agents")).expect("skill directories");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: local-explicit-only\ndescription: explicit only\n---\n\nRun explicitly.\n",
        )
        .expect("skill markdown");
        fs::write(
            skill_dir.join("agents/openai.yaml"),
            "policy:\n  allow_implicit_invocation: false\n",
        )
        .expect("skill policy");

        let registry = SkillRegistry::global();
        let resolved = registry
            .get_resolved_skills_for_workspace(Some(temp.path()), None)
            .await;
        let implicit = registry
            .get_implicitly_invocable_skills_for_workspace(Some(temp.path()), None)
            .await;
        let user_invocable = registry
            .get_user_invocable_skills_for_workspace(Some(temp.path()), None)
            .await;

        assert!(resolved
            .iter()
            .any(|skill| skill.name == "local-explicit-only"));
        assert!(!implicit
            .iter()
            .any(|skill| skill.name == "local-explicit-only"));
        assert!(user_invocable
            .iter()
            .any(|skill| skill.name == "local-explicit-only"));
        let loaded = registry
            .find_and_load_skill_for_workspace("local-explicit-only", Some(temp.path()), None)
            .await
            .expect("explicit invocation should remain available");
        assert_eq!(loaded.name, "local-explicit-only");
    }

    #[tokio::test]
    async fn local_user_invocation_metadata_hides_only_the_picker_catalog() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp.path().join(".claude/skills/model-only");
        fs::create_dir_all(&skill_dir).expect("skill directory");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: model-only\ndescription: model only\nuser-invocable: false\n---\n\nRun when useful.\n",
        )
        .expect("skill markdown");

        let registry = SkillRegistry::global();
        let resolved = registry
            .get_resolved_skills_for_workspace(Some(temp.path()), None)
            .await;
        let implicit = registry
            .get_implicitly_invocable_skills_for_workspace(Some(temp.path()), None)
            .await;
        let user_invocable = registry
            .get_user_invocable_skills_for_workspace(Some(temp.path()), None)
            .await;

        assert!(resolved.iter().any(|skill| skill.name == "model-only"));
        assert!(implicit.iter().any(|skill| skill.name == "model-only"));
        assert!(!user_invocable
            .iter()
            .any(|skill| skill.name == "model-only"));
    }
}
