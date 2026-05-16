//! System prompts module providing main dialogue and agent dialogue prompts
use super::request_context::{RequestContextPolicy, RequestContextSection};
use crate::service::agent_memory::{
    build_workspace_agent_memory_prompt, build_workspace_instruction_files_context,
    build_workspace_memory_files_context,
};
use crate::service::bootstrap::build_workspace_persona_prompt;
use crate::service::config::get_app_language_code;
use crate::service::config::global::GlobalConfigManager;
use crate::service::filesystem::get_formatted_directory_listing;
use crate::service::i18n::LocaleId;
use crate::util::errors::{BitFunError, BitFunResult};
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
const ADDITIONAL_TOOLS_PROMPT: &str = r#"# Additional Tools

Some tools in the tool list are intentionally collapsed.
Their listed descriptions are short summaries rather than full usage instructions.
Before calling a collapsed tool, call `GetToolSpec` with its exact tool name to read its full definition and input schema.
After reading the returned spec, call the real tool directly by its own name.
If a tool spec is already available in the current conversation, do not call `GetToolSpec` for it again.
"#;

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
    pub session_id: Option<String>,
    pub model_name: Option<String>,
    /// When set, file/shell tools target this remote environment; OS and path instructions follow it.
    pub remote_execution: Option<RemoteExecutionHints>,
    /// Pre-built tree text for `{PROJECT_LAYOUT}` when the workspace is not on the local disk.
    pub remote_project_layout: Option<String>,
    /// When `Some(false)`, system prompt append Computer use text-only guidance (no screenshot tool output).
    pub supports_image_understanding: Option<bool>,
    /// When true, append a static reminder that additional collapsed tools exist behind GetToolSpec.
    pub has_additional_tools: bool,
}

impl PromptBuilderContext {
    pub fn new(
        workspace_path: impl Into<String>,
        session_id: Option<String>,
        model_name: Option<String>,
    ) -> Self {
        Self {
            workspace_path: workspace_path.into().replace("\\", "/"),
            session_id,
            model_name,
            remote_execution: None,
            remote_project_layout: None,
            supports_image_understanding: None,
            has_additional_tools: false,
        }
    }

    pub fn with_supports_image_understanding(mut self, supports: bool) -> Self {
        self.supports_image_understanding = Some(supports);
        self
    }

    pub fn with_additional_tools_hint(mut self, has_additional_tools: bool) -> Self {
        self.has_additional_tools = has_additional_tools;
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

        let now = chrono::Local::now();
        let current_date = now.format("%Y-%m-%d").to_string();

        let computer_use_keys = match host_os {
            "macos" => "Computer use / `key_chord`: the **local BitFun desktop** is **macOS** — use `command`, `option`, `control`, `shift` (not Win/Linux modifier names). **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for `osascript`, AppleScript, shell scripts) 2) Keyboard shortcuts: command+a/c/x/v (clipboard), command+space (Spotlight), command+tab (switch app) 3) UI control (AX/OCR/mouse) only when above fail.",
            "windows" => "Computer use / `key_chord`: the **local BitFun desktop** is **Windows** — use `meta`/`super` for Windows key, `alt`, `control`, `shift`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for PowerShell, cmd, scripts) 2) Keyboard shortcuts: control+a/c/x/v (clipboard), meta (Start menu), Alt+Tab (switch) 3) UI control only when above fail.",
            "linux" => "Computer use / `key_chord`: the **local BitFun desktop** is **Linux** — typically `control`, `alt`, `shift`, and sometimes `meta`/`super`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use Bash tool for shell scripts, system commands) 2) Keyboard shortcuts: control+a/c/x/v (clipboard) 3) UI control (AX/OCR/mouse) only when above fail.",
            _ => "Computer use / `key_chord`: match modifier names to the **local BitFun desktop** OS below. **ACTION PRIORITY:** 1) Terminal/CLI/system commands first 2) Keyboard shortcuts second 3) UI control (mouse/OCR) last resort.",
        };

        if let Some(remote) = &self.context.remote_execution {
            format!(
                r#"# Environment Information
<environment_details>
- Workspace root (file tools, Glob, LS, Bash on workspace): {}
- Execution environment: **Remote SSH** — connection "{}".
- Remote host: {} (uname/kernel: {})
- **Paths and shell:** POSIX on the remote server — use forward slashes and Unix shell syntax (bash/sh). Do **not** use PowerShell, `cmd.exe`, or Windows-style paths for workspace operations.
- Local BitFun client OS: {} ({}) — applies to Computer use / UI automation on this machine only, not to workspace file or terminal tools.
- Local client architecture: {}
- Current Date: {}
- {}
</environment_details>

"#,
                self.context.workspace_path,
                remote.connection_display_name.replace('"', "'"),
                remote.hostname.replace('"', "'"),
                remote.kernel_name.replace('"', "'"),
                host_os,
                host_family,
                host_arch,
                current_date,
                computer_use_keys
            )
        } else {
            format!(
                r#"# Environment Information
<environment_details>
- Current Working Directory: {}
- Operating System: {} ({})
- Architecture: {}
- Current Date: {}
- {}
</environment_details>

"#,
                self.context.workspace_path,
                host_os,
                host_family,
                host_arch,
                current_date,
                computer_use_keys
            )
        }
    }

    /// Get workspace file list
    pub fn get_project_layout(&self) -> String {
        if let Some(remote_layout) = &self.context.remote_project_layout {
            let mut project_layout = "# Workspace Layout\n<project_layout>\n".to_string();
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
        let mut project_layout = "# Workspace Layout\n<project_layout>\n".to_string();
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

    pub async fn build_request_context_reminder(
        &self,
        policy: &RequestContextPolicy,
    ) -> Option<String> {
        let mut sections = Vec::new();
        let mut instruction_sections = Vec::new();
        let mut override_sections = Vec::new();
        let mut trailing_sections = Vec::new();

        if self.context.remote_execution.is_none() {
            let workspace = Path::new(&self.context.workspace_path);
            if policy.includes(RequestContextSection::WorkspaceInstructions) {
                match build_workspace_instruction_files_context(workspace).await {
                    Ok(Some(prompt)) => instruction_sections.push(prompt),
                    Ok(None) => {}
                    Err(e) => warn!(
                        "Failed to build workspace instruction context: path={} error={}",
                        workspace.display(),
                        e
                    ),
                }
            }
            if policy.includes(RequestContextSection::WorkspaceMemoryFiles) {
                match build_workspace_memory_files_context(workspace).await {
                    Ok(Some(prompt)) => override_sections.push(prompt),
                    Ok(None) => {}
                    Err(e) => warn!(
                        "Failed to build workspace memory context: path={} error={}",
                        workspace.display(),
                        e
                    ),
                }
            }
        }

        if policy.includes(RequestContextSection::ProjectLayout) {
            trailing_sections.push(self.get_project_layout());
        }

        sections.extend(instruction_sections);

        if policy.has_override_sections() && !override_sections.is_empty() {
            sections.push("Codebase and user instructions are shown below. Be sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.".to_string());
            sections.extend(override_sections);
        }

        sections.extend(trailing_sections);

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n\n"))
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
        format!(
            "# Workspace
Your dedicated operating space is `{}`.
Prefer doing work inside this workspace and keep it well organized with clear structure, sensible filenames, and minimal clutter.
Do not read from, modify, create, move, or delete files outside this workspace unless the user has explicitly granted permission for that external action.
",
            self.context.workspace_path
        )
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

        if self.context.has_additional_tools {
            result.push_str("\n\n");
            result.push_str(ADDITIONAL_TOOLS_PROMPT);
            result.push('\n');
        }

        Ok(result.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::PromptBuilder;
    use super::PromptBuilderContext;

    #[tokio::test]
    async fn appends_additional_tools_section_when_hint_is_enabled() {
        let context =
            PromptBuilderContext::new("E:/workspace", None, None).with_additional_tools_hint(true);
        let prompt = PromptBuilder::new(context)
            .build_prompt_from_template("Base prompt")
            .await
            .expect("prompt should build");

        assert!(prompt.contains("# Additional Tools"));
        assert!(prompt.contains("short summaries rather than full usage instructions"));
        assert!(prompt.contains("call `GetToolSpec` with its exact tool name"));
    }

    #[tokio::test]
    async fn omits_additional_tools_section_when_hint_is_disabled() {
        let context = PromptBuilderContext::new("E:/workspace", None, None);
        let prompt = PromptBuilder::new(context)
            .build_prompt_from_template("Base prompt")
            .await
            .expect("prompt should build");

        assert!(!prompt.contains("# Additional Tools"));
    }
}
