//! System prompts module providing main dialogue and agent dialogue prompts
use crate::agentic::tools::implementations::ExecCommandTool;
use crate::agentic::util::remote_workspace_layout::build_remote_workspace_layout_preview;
use crate::agentic::workspace::WorkspaceBackend;
use crate::agentic::WorkspaceBinding;
use crate::service::agent_memory::{
    build_workspace_agent_memory_prompt, build_workspace_instruction_files_context,
    build_workspace_memory_files_context,
};
use crate::service::bootstrap::build_workspace_persona_prompt;
use crate::service::config::get_app_language_code;
use crate::service::config::global::GlobalConfigManager;
use crate::service::filesystem::get_formatted_directory_listing;
use crate::service::i18n::LocaleId;
use crate::service::remote_ssh::workspace_state::get_remote_workspace_manager;
use crate::service::workspace::get_global_workspace_service;
use crate::service::workspace::RelatedPath;
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_agent_runtime::prompt::{
    render_project_layout, render_runtime_context_reminder, render_user_context_reminder,
    render_workspace_context, PrependedPromptReminders, ProjectLayoutFacts, PromptRelatedPath,
    RemoteExecutionHints, RuntimeContextFacts, RuntimeContextNeeds, RuntimeShellFacts,
    ToolListingSections, UserContextPolicy, UserContextSection, WorkspaceContextFacts,
};
use bitfun_agent_runtime::remote_file_delivery::user_workspace_relative_file_link;
use log::{debug, warn};
use std::path::Path;

/// Placeholder constants
const PLACEHOLDER_PERSONA: &str = "{PERSONA}";
const PLACEHOLDER_LANGUAGE_PREFERENCE: &str = "{LANGUAGE_PREFERENCE}";
const PLACEHOLDER_AGENT_MEMORY: &str = "{AGENT_MEMORY}";
const PLACEHOLDER_CLAW_WORKSPACE: &str = "{CLAW_WORKSPACE}";
const PLACEHOLDER_VISUAL_MODE: &str = "{VISUAL_MODE}";
const PLACEHOLDER_SESSION_ID: &str = "{SESSION_ID}";
const PLACEHOLDER_DEEP_RESEARCH_REPORT_LINK: &str = "{DEEP_RESEARCH_REPORT_LINK}";

#[derive(Debug, Clone)]
pub struct PromptBuilderContext {
    pub workspace_path: String,
    pub related_paths: Vec<RelatedPath>,
    pub session_id: Option<String>,
    pub model_name: Option<String>,
    /// When set, file/shell tools target this remote environment; OS and path instructions follow it.
    pub remote_execution: Option<RemoteExecutionHints>,
    /// Pre-built tree text for `{PROJECT_LAYOUT}` when the workspace is not on the local disk.
    pub remote_project_layout: Option<String>,
    /// When `Some(false)`, runtime context includes Computer use text-only guidance (no screenshot tool output).
    pub supports_image_understanding: Option<bool>,
    /// Dynamic tool listings injected outside tool descriptions for cache stability.
    pub tool_listing_sections: ToolListingSections,
    /// Runtime facts needed by the current model-visible tool set.
    pub runtime_context_needs: RuntimeContextNeeds,
    /// Remote mobile/bot turns need `computer://` links for file delivery.
    pub remote_file_delivery_channel: bool,
}

impl PromptBuilderContext {
    pub fn new(
        workspace_path: impl Into<String>,
        session_id: Option<String>,
        model_name: Option<String>,
    ) -> Self {
        Self {
            workspace_path: workspace_path.into().replace("\\", "/"),
            related_paths: Vec::new(),
            session_id,
            model_name,
            remote_execution: None,
            remote_project_layout: None,
            supports_image_understanding: None,
            tool_listing_sections: ToolListingSections::default(),
            runtime_context_needs: RuntimeContextNeeds::default(),
            remote_file_delivery_channel: false,
        }
    }

    pub fn with_supports_image_understanding(mut self, supports: bool) -> Self {
        self.supports_image_understanding = Some(supports);
        self
    }

    pub fn with_tool_listing_sections(mut self, sections: ToolListingSections) -> Self {
        self.tool_listing_sections = sections;
        self
    }

    pub fn with_runtime_context_needs(mut self, needs: RuntimeContextNeeds) -> Self {
        self.runtime_context_needs = needs;
        self
    }

    pub fn with_related_paths(mut self, related_paths: Vec<RelatedPath>) -> Self {
        self.related_paths = related_paths;
        self
    }

    pub fn with_remote_prompt_overlay(
        mut self,
        execution: RemoteExecutionHints,
        project_layout: Option<String>,
    ) -> Self {
        self.remote_execution = Some(execution);
        self.remote_project_layout = project_layout;
        self
    }

    pub fn with_remote_file_delivery_channel(mut self, enabled: bool) -> Self {
        self.remote_file_delivery_channel = enabled;
        self
    }
}

pub async fn build_prompt_context_for_workspace(
    workspace: &WorkspaceBinding,
    workspace_id: Option<&str>,
    session_id: &str,
    model_name: Option<String>,
    supports_image_understanding: Option<bool>,
    tool_listing_sections: ToolListingSections,
    runtime_context_needs: RuntimeContextNeeds,
) -> Option<PromptBuilderContext> {
    let workspace_path = workspace.root_path_string();

    let related_paths = if let Some(workspace_id) = workspace_id {
        if let Some(workspace_service) = get_global_workspace_service() {
            workspace_service
                .get_workspace(workspace_id)
                .await
                .map(|workspace| workspace.related_paths)
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut base = PromptBuilderContext::new(
        workspace_path.clone(),
        Some(session_id.to_string()),
        model_name,
    )
    .with_related_paths(related_paths)
    .with_tool_listing_sections(tool_listing_sections)
    .with_runtime_context_needs(runtime_context_needs);
    if let Some(supports_image_understanding) = supports_image_understanding {
        base = base.with_supports_image_understanding(supports_image_understanding);
    }

    if !workspace.is_remote() {
        return Some(base);
    }

    let Some(connection_id) = workspace.connection_id() else {
        return Some(base);
    };
    let connection_display_name = match &workspace.backend {
        WorkspaceBackend::Remote {
            connection_name, ..
        } => connection_name.clone(),
        _ => connection_id.to_string(),
    };
    let Some(manager) = get_remote_workspace_manager() else {
        warn!(
            "Remote workspace active but RemoteWorkspaceStateManager is missing; using minimal remote hints"
        );
        return Some(base.with_remote_prompt_overlay(
            RemoteExecutionHints {
                connection_display_name,
                kernel_name: "unknown".to_string(),
                hostname: "unknown".to_string(),
            },
            None,
        ));
    };

    let ssh_manager = manager.get_ssh_manager().await;
    let file_service = manager.get_file_service().await;
    let (kernel_name, hostname) = if let Some(ref ssh) = ssh_manager {
        if let Some(info) = ssh.get_server_info(connection_id).await {
            (info.os_type, info.hostname)
        } else {
            ("Linux".to_string(), "remote".to_string())
        }
    } else {
        ("Linux".to_string(), "remote".to_string())
    };
    let remote_layout = if let Some(ref fs) = file_service {
        match build_remote_workspace_layout_preview(fs, connection_id, &workspace_path, 200).await {
            Ok((_, preview)) => Some(preview),
            Err(e) => {
                warn!("Remote workspace layout for prompt failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    Some(base.with_remote_prompt_overlay(
        RemoteExecutionHints {
            connection_display_name,
            kernel_name,
            hostname,
        },
        remote_layout,
    ))
}

pub struct PromptBuilder {
    pub context: PromptBuilderContext,
    pub file_tree_max_entries: usize,
}

impl PromptBuilder {
    pub fn new(context: PromptBuilderContext) -> Self {
        Self {
            context,
            file_tree_max_entries: 200,
        }
    }

    /// Build runtime facts that may change independently from the agent's system prompt.
    pub async fn build_runtime_context_reminder(&self) -> Option<String> {
        let needs = self.context.runtime_context_needs;
        let local_shell = if needs.exec_command && self.context.remote_execution.is_none() {
            let shell = ExecCommandTool::local_shell_prompt_info().await;
            Some(RuntimeShellFacts {
                display_name: shell.display_name,
                shell_type: shell.shell_type,
                invocation: shell.invocation,
            })
        } else {
            None
        };

        render_runtime_context_reminder(&RuntimeContextFacts {
            needs,
            host_os: std::env::consts::OS.to_string(),
            host_family: std::env::consts::FAMILY.to_string(),
            host_arch: std::env::consts::ARCH.to_string(),
            remote_execution: self.context.remote_execution.clone(),
            local_shell,
            supports_image_understanding: self.context.supports_image_understanding,
        })
    }

    /// Get workspace context that is intentionally injected outside the system prompt cache.
    pub fn get_workspace_context(&self) -> String {
        render_workspace_context(&WorkspaceContextFacts {
            workspace_path: self.context.workspace_path.clone(),
            related_paths: self
                .context
                .related_paths
                .iter()
                .map(|related_path| PromptRelatedPath {
                    path: related_path.path.clone(),
                    description: related_path.description.clone(),
                })
                .collect(),
            remote_execution: self.context.remote_execution.clone(),
        })
    }

    /// Get workspace file list
    pub fn get_project_layout(&self) -> String {
        if let Some(remote_layout) = &self.context.remote_project_layout {
            return render_project_layout(&ProjectLayoutFacts {
                listing: remote_layout.clone(),
                reached_limit: false,
                max_entries: self.file_tree_max_entries,
                remote: true,
            });
        }

        let formatted_listing = get_formatted_directory_listing(
            &self.context.workspace_path,
            self.file_tree_max_entries,
        )
        .unwrap_or_else(|e| crate::service::filesystem::FormattedDirectoryListing {
            reached_limit: false,
            text: format!("Error listing directory: {}", e),
        });
        render_project_layout(&ProjectLayoutFacts {
            listing: formatted_listing.text,
            reached_limit: formatted_listing.reached_limit,
            max_entries: self.file_tree_max_entries,
            remote: false,
        })
    }

    pub fn build_skill_listing_reminder(&self) -> Option<String> {
        self.context
            .tool_listing_sections
            .render_skill_listing_reminder()
    }

    pub fn build_agent_listing_reminder(&self) -> Option<String> {
        self.context
            .tool_listing_sections
            .render_agent_listing_reminder()
    }

    pub fn build_collapsed_tool_listing_reminder(&self) -> Option<String> {
        self.context
            .tool_listing_sections
            .render_collapsed_tool_listing_reminder()
    }

    pub async fn build_user_context_reminder(&self, policy: &UserContextPolicy) -> Option<String> {
        let mut additional_sections = Vec::new();

        if policy.includes(UserContextSection::WorkspaceContext) {
            additional_sections.push(self.get_workspace_context());
        }

        if self.context.remote_execution.is_none() {
            let workspace = Path::new(&self.context.workspace_path);
            if policy.includes(UserContextSection::WorkspaceInstructions) {
                match build_workspace_instruction_files_context(workspace).await {
                    Ok(Some(prompt)) => additional_sections.push(prompt),
                    Ok(None) => {}
                    Err(e) => warn!(
                        "Failed to build workspace instruction context: path={} error={}",
                        workspace.display(),
                        e
                    ),
                }
            }
            if policy.includes(UserContextSection::WorkspaceMemoryFiles) {
                match build_workspace_memory_files_context(workspace).await {
                    Ok(Some(prompt)) => additional_sections.push(prompt),
                    Ok(None) => {}
                    Err(e) => warn!(
                        "Failed to build workspace memory context: path={} error={}",
                        workspace.display(),
                        e
                    ),
                }
            }
        }

        if policy.includes(UserContextSection::ProjectLayout) {
            additional_sections.push(self.get_project_layout());
        }

        render_user_context_reminder(additional_sections)
    }

    pub async fn build_prepended_reminders(
        &self,
        user_context_policy: &UserContextPolicy,
    ) -> PrependedPromptReminders {
        PrependedPromptReminders {
            collapsed_tool_listing: self.build_collapsed_tool_listing_reminder(),
            skill_listing: self.build_skill_listing_reminder(),
            agent_listing: self.build_agent_listing_reminder(),
            runtime_context: self.build_runtime_context_reminder().await,
            user_context: self.build_user_context_reminder(user_context_policy).await,
        }
    }

    /// Get visual mode instruction from user config
    ///
    /// Reads `app.ai_experience.enable_visual_mode` from global config.
    /// Returns a prompt snippet when enabled, or empty string when disabled.
    async fn get_visual_mode_instruction(&self) -> String {
        let enabled = match GlobalConfigManager::get_service().await {
            Ok(service) => service
                .get_config::<bool>(Some("app.ai_experience.enable_visual_mode"))
                .await
                .unwrap_or(false),
            Err(e) => {
                debug!("Failed to read visual mode config: {}", e);
                false
            }
        };

        if enabled {
            r"# Visualizing complex logic as you explain
Use Mermaid diagrams to visualize complex logic, workflows, architectures, and data flows whenever it helps clarify the explanation.
Output Mermaid in fenced code blocks (```mermaid) so the UI can render them.
".to_string()
        } else {
            String::new()
        }
    }

    /// Get user language preference instruction
    ///
    /// Read app.language from global config, generate simple language instruction
    /// Returns empty string if config cannot be read
    /// Returns error if language code is unsupported
    async fn get_language_preference(&self) -> BitFunResult<String> {
        let language_code = get_app_language_code().await;
        Self::format_language_instruction(&language_code)
    }

    /// Format language instruction based on language code
    fn format_language_instruction(lang_code: &str) -> BitFunResult<String> {
        let Some(locale) = LocaleId::from_str(lang_code) else {
            return Err(BitFunError::config(format!(
                "Unknown language code: {}",
                lang_code
            )));
        };
        let language = format!("**{}**", locale.model_language_name());
        Ok(format!("# Language Preference\nYou MUST respond in {} regardless of the user's input language. This is the system language setting and should be followed unless the user explicitly specifies a different language. This is crucial for smooth communication and user experience\n", language))
    }

    /// Get Claw-specific workspace boundary instruction
    fn get_claw_workspace_instruction(&self) -> String {
        "# Workspace
Your dedicated operating space is the workspace root shown in the current user context.
Prefer doing work inside this workspace and keep it well organized with clear structure, sensible filenames, and minimal clutter.
Do not read from, modify, create, move, or delete files outside this workspace unless the user has explicitly granted permission for that external action.
"
        .to_string()
    }

    /// Build prompt from template, automatically fill content based on placeholders
    ///
    /// Supported placeholders:
    /// - `{PERSONA}` - Workspace persona files (BOOTSTRAP.md, SOUL.md, USER.md, IDENTITY.md)
    /// - `{LANGUAGE_PREFERENCE}` - User language preference (read from global config)
    /// - `{AGENT_MEMORY}` - Agent memory instructions + auto-loaded memory index
    /// - `{CLAW_WORKSPACE}` - Claw-specific workspace ownership and boundary rules
    /// - `{VISUAL_MODE}` - Visual mode instruction (Mermaid diagrams, read from global config)
    ///
    /// If a placeholder is not in the template, corresponding content will not be added
    pub async fn build_prompt_from_template(&self, template: &str) -> BitFunResult<String> {
        let mut result = template.to_string();

        // Replace {PERSONA}
        if result.contains(PLACEHOLDER_PERSONA) {
            let persona = if self.context.remote_execution.is_some() {
                "# Workspace persona\nMarkdown persona files (e.g. BOOTSTRAP.md, SOUL.md) live on the **remote** workspace. Use Read or Glob under the workspace root above to load them.\n\n"
                    .to_string()
            } else {
                let workspace = Path::new(&self.context.workspace_path);
                match build_workspace_persona_prompt(workspace).await {
                    Ok(prompt) => prompt.unwrap_or_default(),
                    Err(e) => {
                        warn!(
                            "Failed to build workspace persona prompt: path={} error={}",
                            workspace.display(),
                            e
                        );
                        String::new()
                    }
                }
            };
            result = result.replace(PLACEHOLDER_PERSONA, &persona);
        }

        // Replace {LANGUAGE_PREFERENCE}
        if result.contains(PLACEHOLDER_LANGUAGE_PREFERENCE) {
            let language_preference = self.get_language_preference().await?;
            result = result.replace(PLACEHOLDER_LANGUAGE_PREFERENCE, &language_preference);
        }

        // Replace {CLAW_WORKSPACE}
        if result.contains(PLACEHOLDER_CLAW_WORKSPACE) {
            let claw_workspace = self.get_claw_workspace_instruction();
            result = result.replace(PLACEHOLDER_CLAW_WORKSPACE, &claw_workspace);
        }

        // Replace {AGENT_MEMORY}
        if result.contains(PLACEHOLDER_AGENT_MEMORY) {
            let agent_memory = if self.context.remote_execution.is_some() {
                "# Agent memory\nSession memory under `.bitfun/` is stored on the **remote** host for this workspace. Use file tools with POSIX paths under the workspace root if you need to read it.\n\n"
                    .to_string()
            } else {
                let workspace = Path::new(&self.context.workspace_path);
                match build_workspace_agent_memory_prompt(workspace).await {
                    Ok(prompt) => prompt,
                    Err(e) => {
                        warn!(
                            "Failed to build workspace agent memory prompt: path={} error={}",
                            workspace.display(),
                            e
                        );
                        String::new()
                    }
                }
            };
            result = result.replace(PLACEHOLDER_AGENT_MEMORY, &agent_memory);
        }

        // Replace {VISUAL_MODE}
        if result.contains(PLACEHOLDER_VISUAL_MODE) {
            let visual_mode = self.get_visual_mode_instruction().await;
            result = result.replace(PLACEHOLDER_VISUAL_MODE, &visual_mode);
        }

        // Replace {SESSION_ID} — used by deep-research Pro mode to anchor a per-session
        // work_dir under .bitfun/sessions/{SESSION_ID}/research/. Falls back to a
        // timestamp slug when no session is bound (e.g. one-shot prompt builds in tests).
        let mut resolved_session_id: Option<String> = None;
        if result.contains(PLACEHOLDER_SESSION_ID)
            || result.contains(PLACEHOLDER_DEEP_RESEARCH_REPORT_LINK)
        {
            let session_id = self.context.session_id.clone().unwrap_or_else(|| {
                format!("unbound-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
            });
            resolved_session_id = Some(session_id.clone());
            result = result.replace(PLACEHOLDER_SESSION_ID, &session_id);
        }

        if result.contains(PLACEHOLDER_DEEP_RESEARCH_REPORT_LINK) {
            let session_id = resolved_session_id.unwrap_or_else(|| {
                self.context.session_id.clone().unwrap_or_else(|| {
                    format!("unbound-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
                })
            });
            let report_link = user_workspace_relative_file_link(
                &format!(".bitfun/sessions/{session_id}/research/report.md"),
                self.context.remote_file_delivery_channel,
            );
            result = result.replace(PLACEHOLDER_DEEP_RESEARCH_REPORT_LINK, &report_link);
        }

        Ok(result.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::PromptBuilder;
    use super::PromptBuilderContext;
    use super::RemoteExecutionHints;
    use super::RuntimeContextNeeds;
    use super::ToolListingSections;
    use crate::agentic::agents::UserContextPolicy;
    use crate::service::workspace::RelatedPath;

    #[tokio::test]
    async fn builds_ordered_prepended_reminders_from_tool_listings_and_user_context() {
        let tool_sections = ToolListingSections {
            skill_listing: Some("<available_skills>\n- pdf\n</available_skills>".to_string()),
            agent_listing: Some("<available_agents>\n- Explore\n</available_agents>".to_string()),
            collapsed_tool_listing: Some(
                "<collapsed_tools>\n- WebFetch\n</collapsed_tools>".to_string(),
            ),
        };
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_tool_listing_sections(tool_sections)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["Read"]));
        let reminders = PromptBuilder::new(context)
            .build_prepended_reminders(
                &UserContextPolicy::empty()
                    .with_workspace_context()
                    .with_workspace_instructions(),
            )
            .await;
        let reminders_for_order = reminders.clone();
        let ordered_reminders = reminders_for_order.ordered_reminders();

        let skill_listing = reminders
            .skill_listing
            .expect("skill listing reminder should build");
        let agent_listing = reminders
            .agent_listing
            .expect("agent listing reminder should build");
        let collapsed_tool_listing = reminders
            .collapsed_tool_listing
            .expect("collapsed tool listing reminder should build");
        let user_context = reminders.user_context.expect("user context should build");
        let runtime_context = reminders
            .runtime_context
            .expect("runtime context should build");

        assert!(skill_listing.contains("# Skill Listing"));
        assert!(skill_listing
            .contains("A skill is a set of instructions provided through a `SKILL.md` source."));
        assert!(skill_listing.contains("<available_skills>"));
        assert!(!skill_listing.contains("# Agent Listing"));
        assert!(agent_listing.contains("# Agent Listing"));
        assert!(agent_listing.contains("<available_agents>"));
        assert!(!agent_listing.contains("# Collapsed Tool Listing"));
        assert!(collapsed_tool_listing.contains("# Collapsed Tool Listing"));
        assert!(collapsed_tool_listing.contains("<collapsed_tools>"));
        assert!(user_context.contains("# User Context"));
        assert!(user_context.contains("As you answer the user's questions"));
        assert!(user_context.contains("Current Working Directory: workspace/root"));
        assert!(runtime_context.contains("# Runtime Context"));
        assert!(runtime_context.contains("## Workspace Execution"));
        assert!(runtime_context
            .contains("Workspace file and shell tools operate on the local filesystem"));
        assert!(!runtime_context.contains("## ExecCommand Shell"));
        assert!(!runtime_context.contains("## Local Client"));
        assert!(!runtime_context.contains("ExecCommand shell:"));
        assert_eq!(
            ordered_reminders,
            vec![
                collapsed_tool_listing.as_str(),
                skill_listing.as_str(),
                agent_listing.as_str(),
                runtime_context.as_str(),
                user_context.as_str(),
            ]
        );
    }

    #[tokio::test]
    async fn prepended_reminders_omit_runtime_context_without_runtime_tool_needs() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None);
        let reminders = PromptBuilder::new(context)
            .build_prepended_reminders(&UserContextPolicy::empty())
            .await;

        assert_eq!(reminders.skill_listing, None);
        assert_eq!(reminders.agent_listing, None);
        assert_eq!(reminders.collapsed_tool_listing, None);
        assert_eq!(reminders.user_context, None);
        assert_eq!(reminders.runtime_context, None);
    }

    #[tokio::test]
    async fn runtime_context_includes_workspace_info_for_workspace_tools() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["Read"]));
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context.contains("# Runtime Context"));
        assert!(runtime_context.contains("## Workspace Execution"));
        assert!(runtime_context
            .contains("Workspace file and shell tools operate on the local filesystem"));
        assert!(!runtime_context.contains("## ExecCommand Shell"));
        assert!(!runtime_context.contains("## Local Client"));
        assert!(!runtime_context.contains("ExecCommand shell:"));
    }

    #[tokio::test]
    async fn runtime_context_includes_shell_info_when_exec_command_is_available() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["ExecCommand"]));
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context.contains("# Runtime Context"));
        assert!(runtime_context.contains("## Workspace Execution"));
        assert!(runtime_context.contains("## ExecCommand Shell"));
        assert!(runtime_context.contains("ExecCommand shell:"));
        assert!(runtime_context.contains("invoked as `"));
        assert!(!runtime_context.contains("## Local Client"));
    }

    #[tokio::test]
    async fn runtime_context_includes_computer_use_info_only_when_needed() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["ComputerUse"]));
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context.contains("## Local Client"));
        assert!(runtime_context.contains("Local BitFun client OS:"));
        assert!(runtime_context.contains("Computer use / `key_chord`"));
        assert!(!runtime_context.contains("## Workspace Execution"));
        assert!(!runtime_context.contains("## ExecCommand Shell"));
        assert!(!runtime_context.contains("ExecCommand shell:"));
    }

    #[tokio::test]
    async fn runtime_context_includes_text_only_computer_use_guidance_for_non_visual_models() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_supports_image_understanding(false)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["ComputerUse"]));
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context.contains("## Computer Use Input Strategy"));
        assert!(runtime_context.contains("primary model does not accept image inputs"));
        assert!(runtime_context.contains("do not use `screenshot`"));
        assert!(runtime_context.contains("prefer `snapshot` then click by `@e*` ref"));
    }

    #[tokio::test]
    async fn runtime_context_omits_text_only_computer_use_guidance_for_visual_models() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_supports_image_understanding(true)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["ComputerUse"]));
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context.contains("## Local Client"));
        assert!(!runtime_context.contains("## Computer Use Input Strategy"));
        assert!(!runtime_context.contains("primary model does not accept image inputs"));
    }

    #[tokio::test]
    async fn system_prompt_template_does_not_append_text_only_computer_use_guidance() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_supports_image_understanding(false)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names(["ComputerUse"]));
        let prompt = PromptBuilder::new(context)
            .build_prompt_from_template("Base system prompt")
            .await
            .expect("prompt should build");

        assert_eq!(prompt, "Base system prompt");
        assert!(!prompt.contains("Computer Use Input Strategy"));
        assert!(!prompt.contains("primary model does not accept image inputs"));
    }

    #[tokio::test]
    async fn runtime_context_omits_workspace_root_for_remote_execution() {
        let context = PromptBuilderContext::new("/workspace/project", None, None)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names([
                "Read",
                "ExecCommand",
                "ComputerUse",
            ]))
            .with_remote_prompt_overlay(
                RemoteExecutionHints {
                    connection_display_name: "dev-server".to_string(),
                    kernel_name: "Linux".to_string(),
                    hostname: "devbox".to_string(),
                },
                None,
            );
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context
            .contains("Workspace file and shell tools operate on remote SSH connection"));
        assert!(runtime_context.contains("## Workspace Execution"));
        assert!(runtime_context.contains("## ExecCommand Shell"));
        assert!(runtime_context.contains("## Local Client"));
        assert!(runtime_context.contains("Local BitFun client OS:"));
        assert!(runtime_context.contains("Computer use and UI automation operate on the local BitFun desktop, even when workspace file and shell tools target a remote host."));
        assert!(runtime_context.contains("ExecCommand uses the remote user's default POSIX shell"));
        assert!(runtime_context.contains("This session operates on the remote SSH host only"));
    }

    #[tokio::test]
    async fn runtime_context_omits_local_client_os_for_remote_with_only_control_hub() {
        // Simulates a remote workspace where ComputerUse is disabled (filtered
        // out by is_available_in_context) but ControlHub remains available.
        // The agent must NOT see "Local BitFun client OS" because that signal
        // causes it to mistake the client OS for the workspace execution OS.
        let context = PromptBuilderContext::new("/workspace/project", None, None)
            .with_runtime_context_needs(RuntimeContextNeeds::from_tool_names([
                "Read",
                "ExecCommand",
                "ControlHub",
            ]))
            .with_remote_prompt_overlay(
                RemoteExecutionHints {
                    connection_display_name: "dev-server".to_string(),
                    kernel_name: "Linux".to_string(),
                    hostname: "devbox".to_string(),
                },
                None,
            );
        let runtime_context = PromptBuilder::new(context)
            .build_runtime_context_reminder()
            .await
            .expect("runtime context should build");

        assert!(runtime_context
            .contains("Workspace file and shell tools operate on remote SSH connection"));
        assert!(runtime_context.contains("## ExecCommand Shell"));
        assert!(!runtime_context.contains("## Local Client"));
        assert!(!runtime_context.contains("Local BitFun client OS:"));
    }

    #[tokio::test]
    async fn deep_research_report_link_defaults_to_workspace_relative_path() {
        let context =
            PromptBuilderContext::new("workspace/root", Some("session-1".to_string()), None);
        let prompt = PromptBuilder::new(context)
            .build_prompt_from_template("[View full report]({DEEP_RESEARCH_REPORT_LINK})")
            .await
            .expect("prompt should build");

        assert_eq!(
            prompt,
            "[View full report](.bitfun/sessions/session-1/research/report.md)"
        );
    }

    #[tokio::test]
    async fn deep_research_report_link_uses_computer_scheme_for_remote_delivery() {
        let context =
            PromptBuilderContext::new("workspace/root", Some("session-1".to_string()), None)
                .with_remote_file_delivery_channel(true);
        let prompt = PromptBuilder::new(context)
            .build_prompt_from_template("[View full report]({DEEP_RESEARCH_REPORT_LINK})")
            .await
            .expect("prompt should build");

        assert_eq!(
            prompt,
            "[View full report](computer://.bitfun/sessions/session-1/research/report.md)"
        );
    }

    #[test]
    fn workspace_context_renders_related_directories() {
        let context =
            PromptBuilderContext::new(r"workspace\root", None, None).with_related_paths(vec![
                RelatedPath {
                    path: r"legacy-ts\client".to_string(),
                    description: Some("Legacy TypeScript implementation".to_string()),
                },
                RelatedPath {
                    path: r"monorepo\billing".to_string(),
                    description: Some("Billing package".to_string()),
                },
            ]);

        let workspace_context = PromptBuilder::new(context).get_workspace_context();

        assert!(workspace_context.contains("Related directories"));
        assert!(workspace_context.contains("legacy-ts/client"));
        assert!(workspace_context.contains("Legacy TypeScript implementation"));
        assert!(workspace_context.contains("monorepo/billing"));
    }

    #[test]
    fn workspace_context_renders_related_directories_without_description() {
        let context =
            PromptBuilderContext::new(r"workspace\root", None, None).with_related_paths(vec![
                RelatedPath {
                    path: r"monorepo\packages\payments".to_string(),
                    description: None,
                },
            ]);

        let workspace_context = PromptBuilder::new(context).get_workspace_context();

        assert!(workspace_context.contains("Related directories"));
        assert!(workspace_context.contains("  - monorepo/packages/payments"));
        assert!(!workspace_context.contains("payments —"));
    }
}
