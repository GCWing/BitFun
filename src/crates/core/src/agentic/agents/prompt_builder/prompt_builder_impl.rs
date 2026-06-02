//! System prompts module providing main dialogue and agent dialogue prompts
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
    PrependedPromptReminders, ToolListingSections, UserContextPolicy, UserContextSection,
};
use log::{debug, warn};
use std::path::Path;

/// Placeholder constants
const PLACEHOLDER_PERSONA: &str = "{PERSONA}";
const PLACEHOLDER_ENV_INFO: &str = "{ENV_INFO}";
const PLACEHOLDER_LANGUAGE_PREFERENCE: &str = "{LANGUAGE_PREFERENCE}";
const PLACEHOLDER_AGENT_MEMORY: &str = "{AGENT_MEMORY}";
const PLACEHOLDER_CLAW_WORKSPACE: &str = "{CLAW_WORKSPACE}";
const PLACEHOLDER_VISUAL_MODE: &str = "{VISUAL_MODE}";
const PLACEHOLDER_SESSION_ID: &str = "{SESSION_ID}";
const USER_CONTEXT_PROMPT: &str =
    "As you answer the user's questions, you can use the following context.\nNote: this is a snapshot captured at the start of the conversation and may not reflect real-time changes made afterward.";
/// SSH remote host facts for system prompt (workspace tools run here, not on the local client).
#[derive(Debug, Clone)]
pub struct RemoteExecutionHints {
    pub connection_display_name: String,
    pub kernel_name: String,
    pub hostname: String,
}

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
    /// When `Some(false)`, system prompt append Computer use text-only guidance (no screenshot tool output).
    pub supports_image_understanding: Option<bool>,
    /// Dynamic tool listings injected outside tool descriptions for cache stability.
    pub tool_listing_sections: ToolListingSections,
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
}

pub async fn build_prompt_context_for_workspace(
    workspace: &WorkspaceBinding,
    workspace_id: Option<&str>,
    session_id: &str,
    model_name: Option<String>,
    supports_image_understanding: Option<bool>,
    tool_listing_sections: ToolListingSections,
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
    .with_tool_listing_sections(tool_listing_sections);
    if let Some(supports_image_understanding) = supports_image_understanding {
        base = base.with_supports_image_understanding(supports_image_understanding);
    }

    if !workspace.is_remote() {
        return Some(base);
    }

    let Some(connection_id) = workspace.connection_id() else {
        return Some(base);
    };
    let Some(manager) = get_remote_workspace_manager() else {
        warn!(
            "Remote workspace active but RemoteWorkspaceStateManager is missing; using client OS hints only"
        );
        return Some(base);
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
    let connection_display_name = match &workspace.backend {
        WorkspaceBackend::Remote {
            connection_name, ..
        } => connection_name.clone(),
        _ => connection_id.to_string(),
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

    /// Provide complete environment information
    pub fn get_env_info(&self) -> String {
        let host_os = std::env::consts::OS;
        let host_family = std::env::consts::FAMILY;
        let host_arch = std::env::consts::ARCH;

        let computer_use_keys = match host_os {
            "macos" => "Computer use / `key_chord`: the **local BitFun desktop** is **macOS** — use `command`, `option`, `control`, `shift` (not Win/Linux modifier names). **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for `osascript`, AppleScript, shell scripts) 2) Keyboard shortcuts: command+a/c/x/v (clipboard), command+space (Spotlight), command+tab (switch app) 3) UI control (AX/OCR/mouse) only when above fail.",
            "windows" => "Computer use / `key_chord`: the **local BitFun desktop** is **Windows** — use `meta`/`super` for Windows key, `alt`, `control`, `shift`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for PowerShell, cmd, scripts) 2) Keyboard shortcuts: control+a/c/x/v (clipboard), meta (Start menu), Alt+Tab (switch) 3) UI control only when above fail.",
            "linux" => "Computer use / `key_chord`: the **local BitFun desktop** is **Linux** — typically `control`, `alt`, `shift`, and sometimes `meta`/`super`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for shell scripts, system commands) 2) Keyboard shortcuts: control+a/c/x/v (clipboard) 3) UI control (AX/OCR/mouse) only when above fail.",
            _ => "Computer use / `key_chord`: match modifier names to the **local BitFun desktop** OS below. **ACTION PRIORITY:** 1) Terminal/CLI/system commands first 2) Keyboard shortcuts second 3) UI control (mouse/OCR) last resort.",
        };

        if self.context.remote_execution.is_some() {
            format!(
                r#"# Environment Information
<environment_details>
- Local BitFun client OS: {} ({}) — applies to Computer use / UI automation on this machine only.
- Local client architecture: {}
- {}
</environment_details>

"#,
                host_os, host_family, host_arch, computer_use_keys
            )
        } else {
            format!(
                r#"# Environment Information
<environment_details>
- Operating System: {} ({})
- Architecture: {}
- {}
</environment_details>

"#,
                host_os, host_family, host_arch, computer_use_keys
            )
        }
    }

    /// Get workspace context that is intentionally injected outside the system prompt cache.
    pub fn get_workspace_context(&self) -> String {
        let related_paths_section = if self.context.related_paths.is_empty() {
            String::new()
        } else {
            let items = self
                .context
                .related_paths
                .iter()
                .map(|related_path| {
                    let path = related_path.path.replace("\\", "/");
                    match related_path.description.as_deref().map(str::trim) {
                        Some(description) if !description.is_empty() => {
                            format!("  - {} — {}", path, description)
                        }
                        _ => format!("  - {}", path),
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "- Related directories (user-specified directories related to this workspace):\n{}",
                items
            )
        };

        if let Some(remote) = &self.context.remote_execution {
            format!(
                r#"## Workspace Context
<workspace_context>
- Workspace root (file tools, Glob, LS, Bash on workspace): {}
{}
- Execution environment: **Remote SSH** — connection "{}".
- Remote host: {} (uname/kernel: {})
- **Paths and shell:** POSIX on the remote server — use forward slashes and Unix shell syntax (bash/sh). Do **not** use PowerShell, `cmd.exe`, or Windows-style paths for workspace operations.
</workspace_context>
"#,
                self.context.workspace_path,
                if related_paths_section.is_empty() {
                    String::new()
                } else {
                    format!("{}\n", related_paths_section)
                },
                remote.connection_display_name.replace('"', "'"),
                remote.hostname.replace('"', "'"),
                remote.kernel_name.replace('"', "'"),
            )
        } else {
            format!(
                r#"## Workspace Context
<workspace_context>
- Current Working Directory: {}
{}
</workspace_context>
"#,
                self.context.workspace_path,
                if related_paths_section.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", related_paths_section)
                }
            )
        }
    }

    /// Get workspace file list
    pub fn get_project_layout(&self) -> String {
        if let Some(remote_layout) = &self.context.remote_project_layout {
            let mut project_layout = "## Workspace Layout\n<project_layout>\n".to_string();
            project_layout.push_str(
                "Below is a snapshot of the current workspace's file structure on the **remote** host.\n\n",
            );
            project_layout.push_str(remote_layout);
            project_layout.push_str("\n</project_layout>\n\n");
            return project_layout;
        }

        let formatted_listing = get_formatted_directory_listing(
            &self.context.workspace_path,
            self.file_tree_max_entries,
        )
        .unwrap_or_else(|e| crate::service::filesystem::FormattedDirectoryListing {
            reached_limit: false,
            text: format!("Error listing directory: {}", e),
        });
        let mut project_layout = "## Workspace Layout\n<project_layout>\n".to_string();
        if formatted_listing.reached_limit {
            project_layout.push_str(&format!("Below is a snapshot of the current workspace's file structure (showing up to {} entries).\n\n", self.file_tree_max_entries));
        } else {
            project_layout
                .push_str("Below is a snapshot of the current workspace's file structure.\n\n");
        }
        project_layout.push_str(&formatted_listing.text);
        project_layout.push_str("\n</project_layout>\n\n");
        project_layout
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

        if additional_sections.is_empty() {
            None
        } else {
            Some(format!(
                "# User Context\n{}\n\n{}",
                USER_CONTEXT_PROMPT,
                additional_sections
                    .into_iter()
                    .map(|section| section.trim().to_string())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            ))
        }
    }

    pub async fn build_prepended_reminders(
        &self,
        user_context_policy: &UserContextPolicy,
    ) -> PrependedPromptReminders {
        PrependedPromptReminders {
            skill_listing: self.build_skill_listing_reminder(),
            agent_listing: self.build_agent_listing_reminder(),
            collapsed_tool_listing: self.build_collapsed_tool_listing_reminder(),
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
    /// - `{ENV_INFO}` - Environment information
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

        // Replace {ENV_INFO}
        if result.contains(PLACEHOLDER_ENV_INFO) {
            let env_info = self.get_env_info();
            result = result.replace(PLACEHOLDER_ENV_INFO, &env_info);
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
        if result.contains(PLACEHOLDER_SESSION_ID) {
            let session_id = self.context.session_id.clone().unwrap_or_else(|| {
                format!("unbound-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
            });
            result = result.replace(PLACEHOLDER_SESSION_ID, &session_id);
        }

        if self.context.supports_image_understanding == Some(false) {
            result.push_str(
                "\n\n# Computer use (text-only primary model)\n\n\
The configured **primary model does not accept image inputs**. When using **`ComputerUse`** (or **`ControlHub`** with **`domain: \"browser\"`**):\n\
- **Do not** use **`screenshot`** (desktop) and **avoid** `domain:\"browser\" action:\"screenshot\"` — the JPEG bytes will be unreadable.\n\
- **ACTION PRIORITY:** 1) Terminal/CLI/system commands (`Bash` tool, or `ComputerUse` `run_script`) 2) Keyboard shortcuts (**`key_chord`**, **`type_text`**) 3) UI control: **`click_element`** (AX) → **`locate`** → **`move_to_text`** (use **`move_to_text_match_index`** when multiple OCR hits listed) → **`mouse_move`** (**`use_screen_coordinates`: true** with coordinates from tool JSON) → **`click`**. For browser work prefer `snapshot` → click by `@e*` ref over screenshots.\n\
- **Never guess coordinates** — always use precise methods (AX, OCR, system coordinates from tool results, or browser snapshot refs).\n",
            );
        }

        Ok(result.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::PrependedPromptReminders;
    use super::PromptBuilder;
    use super::PromptBuilderContext;
    use super::ToolListingSections;
    use super::USER_CONTEXT_PROMPT;
    use crate::agentic::agents::UserContextPolicy;
    use crate::service::workspace::RelatedPath;

    #[tokio::test]
    async fn builds_ordered_prepended_reminders_from_tool_listings_and_user_context() {
        let tool_sections = ToolListingSections {
            skill_listing: Some("<available_skills>\n- pdf\n</available_skills>".to_string()),
            agent_listing: Some("<available_agents>\n- Explore\n</available_agents>".to_string()),
            collapsed_tool_listing: Some(
                "<collapsed_tools>\n- WebFetch: Fetch readable web content.\n</collapsed_tools>"
                    .to_string(),
            ),
        };
        let context = PromptBuilderContext::new(r"workspace\root", None, None)
            .with_tool_listing_sections(tool_sections);
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

        assert!(skill_listing.contains("# Skill Listing"));
        assert!(skill_listing.contains("<available_skills>"));
        assert!(!skill_listing.contains("# Agent Listing"));
        assert!(agent_listing.contains("# Agent Listing"));
        assert!(agent_listing.contains("<available_agents>"));
        assert!(!agent_listing.contains("# Collapsed Tool Listing"));
        assert!(collapsed_tool_listing.contains("# Collapsed Tool Listing"));
        assert!(collapsed_tool_listing.contains("<collapsed_tools>"));
        assert!(user_context.contains("# User Context"));
        assert!(user_context.contains(USER_CONTEXT_PROMPT));
        assert!(user_context.contains("Current Working Directory: workspace/root"));
        assert_eq!(
            ordered_reminders,
            vec![
                collapsed_tool_listing.as_str(),
                skill_listing.as_str(),
                agent_listing.as_str(),
                user_context.as_str(),
            ]
        );
    }

    #[tokio::test]
    async fn omits_prepended_reminders_when_policy_and_sections_are_empty() {
        let context = PromptBuilderContext::new(r"workspace\root", None, None);
        let reminders = PromptBuilder::new(context)
            .build_prepended_reminders(&UserContextPolicy::empty())
            .await;

        assert_eq!(reminders, PrependedPromptReminders::default());
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
