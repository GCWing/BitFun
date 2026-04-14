//! System prompts module providing main dialogue and agent dialogue prompts
use crate::agentic::persistence::PersistenceManager;
use super::request_context::{RequestContextPolicy, RequestContextSection};
use crate::infrastructure::try_get_path_manager_arc;
use crate::infrastructure::PathManager;
use crate::service::agent_memory::{
    build_workspace_agent_memory_prompt, build_workspace_instruction_files_context,
    build_workspace_memory_files_context,
};
use crate::service::ai_memory::AIMemoryManager;
use crate::service::bootstrap::build_workspace_persona_prompt;
use crate::service::config::get_app_language_code;
use crate::service::config::global::GlobalConfigManager;
use crate::service::filesystem::get_formatted_directory_listing;
use crate::service::workspace::get_global_workspace_service;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{debug, warn};
use std::path::Path;
use std::sync::Arc;

/// Placeholder constants
const PLACEHOLDER_PERSONA: &str = "{PERSONA}";
const PLACEHOLDER_ENV_INFO: &str = "{ENV_INFO}";
const PLACEHOLDER_LANGUAGE_PREFERENCE: &str = "{LANGUAGE_PREFERENCE}";
const PLACEHOLDER_AGENT_MEMORY: &str = "{AGENT_MEMORY}";
const PLACEHOLDER_CLAW_WORKSPACE: &str = "{CLAW_WORKSPACE}";
const PLACEHOLDER_VISUAL_MODE: &str = "{VISUAL_MODE}";
const PLACEHOLDER_RECENT_WORKSPACES: &str = "{RECENT_WORKSPACES}";
const PLACEHOLDER_ACTIVE_SESSION_CONTEXT: &str = "{ACTIVE_SESSION_CONTEXT}";

/// Maximum character length for active session context injected into system prompt.
/// Older turns are dropped first when the total exceeds this limit.
const MAX_ACTIVE_SESSION_CONTEXT_CHARS: usize = 8_000;

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
        }
    }

    pub fn with_supports_image_understanding(mut self, supports: bool) -> Self {
        self.supports_image_understanding = Some(supports);
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

    /// Load AI memories from disk and format as prompt
    pub async fn load_ai_memories(&self) -> Option<String> {
        let path_manager = match try_get_path_manager_arc() {
            Ok(pm) => pm,
            Err(e) => {
                warn!("Failed to create PathManager: {}", e);
                return None;
            }
        };

        let memory_manager = match AIMemoryManager::new(path_manager).await {
            Ok(mm) => mm,
            Err(e) => {
                warn!("Failed to create AIMemoryManager: {}", e);
                return None;
            }
        };

        match memory_manager.get_memories_for_prompt().await {
            Ok(Some(prompt)) => Some(prompt),
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to load memories: {}", e);
                None
            }
        }
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

        if policy.includes(RequestContextSection::AIMemories) {
            if let Some(memory_prompt) = self.load_ai_memories().await {
                override_sections.push(memory_prompt);
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
        let language = match lang_code {
            "zh-CN" => "**Simplified Chinese**",
            "en-US" => "**English**",
            _ => {
                return Err(BitFunError::config(format!(
                    "Unknown language code: {}",
                    lang_code
                )));
            }
        };
        Ok(format!("# Language Preference\nYou MUST respond in {} regardless of the user's input language. This is the system language setting and should be followed unless the user explicitly specifies a different language. This is crucial for smooth communication and user experience\n", language))
    }

    /// Get recently accessed workspaces formatted as a prompt section
    pub async fn get_recent_workspaces_info(&self) -> String {
        let ws_service = match get_global_workspace_service() {
            Some(s) => s,
            None => return String::new(),
        };

        let mut lines: Vec<String> = Vec::new();

        // Assistant/global workspaces
        let assistant_workspaces = ws_service.get_assistant_workspaces().await;
        for ws in &assistant_workspaces {
            lines.push(format!(
                "  - [global] {} — {}",
                ws.name,
                ws.root_path.display()
            ));
        }

        // Recent project workspaces
        let recent = ws_service.get_recent_workspaces().await;
        for ws in &recent {
            let last = ws.last_accessed.format("%Y-%m-%d %H:%M").to_string();
            lines.push(format!(
                "  - [project] {} — {} (last accessed: {})",
                ws.name,
                ws.root_path.display(),
                last
            ));
        }

        if lines.is_empty() {
            return String::new();
        }

        format!(
            "# Available Workspaces\n<available_workspaces>\nThe following workspaces are available. Use these paths when creating agent sessions.\n\n{}\n</available_workspaces>\n\n",
            lines.join("\n")
        )
    }

    /// Build a concise text snapshot of the current active session's dialog turns.
    ///
    /// Only user messages and assistant text are included (tool calls are omitted).
    /// If the total character count exceeds [`MAX_ACTIVE_SESSION_CONTEXT_CHARS`], the oldest
    /// turns are dropped and a truncation notice is prepended so the AI is aware.
    ///
    /// Returns an empty string when `session_id` is not set in the context or when
    /// loading turns fails.
    pub async fn get_active_session_context(&self) -> String {
        let session_id = match &self.context.session_id {
            Some(id) => id.clone(),
            None => return String::new(),
        };

        let manager = match (|| -> BitFunResult<PersistenceManager> {
            Ok(PersistenceManager::new(Arc::new(PathManager::new()?))?)
        })() {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    "Failed to create PersistenceManager for active session context: {}",
                    e
                );
                return String::new();
            }
        };

        let workspace_path = Path::new(&self.context.workspace_path);
        let turns = match manager
            .load_session_turns(workspace_path, &session_id)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "Failed to load session turns for active session context: session_id={} error={}",
                    session_id, e
                );
                return String::new();
            }
        };

        if turns.is_empty() {
            return String::new();
        }

        // Format each turn as a compact user / assistant block (no tool details)
        let mut turn_texts: Vec<String> = turns
            .iter()
            .map(|turn| {
                let user_content = turn.user_message.content.trim().to_string();

                let assistant_text: String = turn
                    .model_rounds
                    .iter()
                    .flat_map(|round| round.text_items.iter())
                    .filter(|item| !item.is_subagent_item.unwrap_or(false))
                    .map(|item| item.content.trim())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                let mut text = format!("[Turn {}]\nUser: {}", turn.turn_index, user_content);
                if !assistant_text.is_empty() {
                    text.push_str(&format!("\nAssistant: {}", assistant_text));
                }
                text
            })
            .collect();

        // Drop oldest turns until total fits within the character budget
        let mut total_chars: usize = turn_texts.iter().map(|t| t.len() + 2).sum();
        let mut truncated = false;
        while total_chars > MAX_ACTIVE_SESSION_CONTEXT_CHARS && turn_texts.len() > 1 {
            let removed_len = turn_texts[0].len() + 2;
            turn_texts.remove(0);
            total_chars = total_chars.saturating_sub(removed_len);
            truncated = true;
        }

        let truncation_notice = if truncated {
            "[Note: Earlier turns have been truncated. Only the most recent portion of this session is shown.]\n\n"
        } else {
            ""
        };

        format!(
            "# Current Session Context\n<session_context>\n{}{}\n</session_context>\n\n",
            truncation_notice,
            turn_texts.join("\n\n")
        )
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
    /// - `{RECENT_WORKSPACES}` - Recently accessed global and project workspaces with paths
    /// - `{ACTIVE_SESSION_CONTEXT}` - Current session's recent dialog history (user + assistant
    ///   text only, no tool details). Oldest turns are dropped when the total exceeds
    ///   [`MAX_ACTIVE_SESSION_CONTEXT_CHARS`]; a truncation notice is prepended in that case.
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

        // Replace {RECENT_WORKSPACES}
        if result.contains(PLACEHOLDER_RECENT_WORKSPACES) {
            let recent_workspaces = self.get_recent_workspaces_info().await;
            result = result.replace(PLACEHOLDER_RECENT_WORKSPACES, &recent_workspaces);
        }

        // Replace {ACTIVE_SESSION_CONTEXT}
        if result.contains(PLACEHOLDER_ACTIVE_SESSION_CONTEXT) {
            let session_context = self.get_active_session_context().await;
            result = result.replace(PLACEHOLDER_ACTIVE_SESSION_CONTEXT, &session_context);
        }

        if self.context.supports_image_understanding == Some(false) {
            result.push_str(
                "\n\n# Computer use (text-only primary model)\n\n\
The configured **primary model does not accept image inputs**. When using **ComputerUse**:\n\
- **Do not** use **`screenshot`** or **`click_label`**.\n\
- **ACTION PRIORITY:** 1) Terminal/CLI/system commands (Bash tool) 2) Keyboard shortcuts (**`key_chord`**, **`type_text`**) 3) UI control: **`click_element`** (AX) → **`locate`** → **`move_to_text`** (use **`move_to_text_match_index`** when multiple OCR hits listed) → **`mouse_move`** (**`use_screen_coordinates`: true** with coordinates from tool JSON) → **`click`**.\n\
- **Never guess coordinates** — always use precise methods (AX, OCR, system coordinates from tool results).\n",
            );
        }

        Ok(result.trim().to_string())
    }
}
