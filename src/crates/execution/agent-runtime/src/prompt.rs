//! Prompt-loop owner facts and reminder ordering.

use serde::{Deserialize, Serialize};

const SKILL_LISTING_TITLE: &str = "# Skill Listing";
const SKILL_LISTING_GUIDANCE: &str = r#"A skill is a set of instructions provided through a `SKILL.md` source.
If the user names a skill (with `[$SkillName]` or plain text) OR the task clearly matches a skill's description shown below, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
Below is the list of skills that can be used with the Skill tool. Each entry includes a name and description"#;
const AGENT_LISTING_TITLE: &str = "# Agent Listing";
const AGENT_LISTING_GUIDANCE: &str = "Available subagent types for the Task tool:";
const COLLAPSED_TOOL_LISTING_TITLE: &str = "# Collapsed Tool Listing";
const COLLAPSED_TOOL_LISTING_GUIDANCE: &str = r#"The folling tools are intentionally collapsed. Their listed descriptions are short summaries rather than full usage instructions.
Before calling a collapsed tool, call `GetToolSpec` with its exact tool name to read its full schema.
After reading the returned spec, call the real tool directly by its own name.
If a tool spec is already available in the current conversation, do not call `GetToolSpec` for it again."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptEnvironmentFacts<'a> {
    pub host_os: &'a str,
    pub host_family: &'a str,
    pub host_arch: &'a str,
    pub remote_execution_active: bool,
}

pub fn render_prompt_environment_info(facts: PromptEnvironmentFacts<'_>) -> String {
    let computer_use_keys = computer_use_key_chord_guidance(facts.host_os);

    if facts.remote_execution_active {
        format!(
            r#"# Environment Information
<environment_details>
- Local BitFun client OS: {} ({}) — applies to Computer use / UI automation on this machine only.
- Local client architecture: {}
- {}
</environment_details>

"#,
            facts.host_os, facts.host_family, facts.host_arch, computer_use_keys
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
            facts.host_os, facts.host_family, facts.host_arch, computer_use_keys
        )
    }
}

/// SSH remote host facts for prompt rendering. Runtime services still own
/// concrete remote probing and IO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecutionHints {
    pub connection_display_name: String,
    pub kernel_name: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeContextNeeds {
    pub workspace_tools: bool,
    pub exec_command: bool,
    pub exec_control: bool,
    pub computer_use: bool,
    /// `ControlHub` is available. Distinct from `computer_use` because in remote
    /// workspaces `ControlHub` stays available (browser/terminal on the local
    /// client) while `ComputerUse` is disabled, and the agent should NOT receive
    /// the prominent "Local BitFun client OS" block that belongs to desktop
    /// automation.
    pub control_hub: bool,
}

impl RuntimeContextNeeds {
    pub fn from_tool_names<T, I>(tool_names: I) -> Self
    where
        T: AsRef<str>,
        I: IntoIterator<Item = T>,
    {
        let mut needs = Self::default();
        for tool_name in tool_names {
            let tool_name = tool_name.as_ref();
            match tool_name {
                "Read" | "Write" | "Edit" | "Delete" | "LS" | "Grep" | "Glob" | "ExecCommand"
                | "WriteStdin" | "ExecControl" => {
                    needs.workspace_tools = true;
                    if tool_name == "ExecCommand" {
                        needs.exec_command = true;
                    }
                    if tool_name == "ExecControl" {
                        needs.exec_control = true;
                    }
                }
                "ComputerUse" => {
                    needs.computer_use = true;
                }
                "ControlHub" => {
                    needs.control_hub = true;
                }
                _ => {}
            }
        }
        needs
    }

    pub fn is_empty(self) -> bool {
        !self.workspace_tools
            && !self.exec_command
            && !self.exec_control
            && !self.computer_use
            && !self.control_hub
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeShellFacts {
    pub display_name: String,
    pub shell_type: String,
    pub invocation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeContextFacts {
    pub needs: RuntimeContextNeeds,
    pub host_os: String,
    pub host_family: String,
    pub host_arch: String,
    pub remote_execution: Option<RemoteExecutionHints>,
    pub local_shell: Option<RuntimeShellFacts>,
    pub supports_image_understanding: Option<bool>,
}

pub fn render_runtime_context_reminder(facts: &RuntimeContextFacts) -> Option<String> {
    if facts.needs.is_empty() {
        return None;
    }

    let mut lines = vec!["# Runtime Context".to_string()];

    if facts.needs.workspace_tools {
        let mut workspace_lines = Vec::new();
        if let Some(remote) = &facts.remote_execution {
            workspace_lines.push(format!(
                "- Workspace file and shell tools operate on remote SSH connection \"{}\".",
                remote.connection_display_name.replace('"', "'")
            ));
            workspace_lines.push(format!(
                "- Remote host: {} (uname/kernel: {})",
                remote.hostname.replace('"', "'"),
                remote.kernel_name.replace('"', "'")
            ));
            workspace_lines.push("- Path conventions for workspace operations: POSIX paths with forward slashes and Unix shell syntax. Do not use PowerShell, `cmd.exe`, or Windows-style paths for remote workspace operations.".to_string());
            workspace_lines.push("- This session operates on the remote SSH host only. Local filesystem, local terminal, and local OS operations are not accessible. For tasks requiring local execution (e.g. editing the local Mac's ~/.ssh/config), provide the user with commands to run on their local machine.".to_string());
        } else {
            workspace_lines.push(
                "- Workspace file and shell tools operate on the local filesystem.".to_string(),
            );
        }
        push_runtime_context_section(&mut lines, "Workspace Execution", workspace_lines);
    }

    if facts.needs.exec_command {
        let mut exec_command_lines = Vec::new();
        if facts.remote_execution.is_some() {
            exec_command_lines.push(
                "- ExecCommand uses the remote user's default POSIX shell, invoked as `<shell> -lc <cmd>`."
                    .to_string(),
            );
        } else if let Some(shell) = &facts.local_shell {
            push_local_exec_shell_runtime_context(
                &mut exec_command_lines,
                &shell.display_name,
                &shell.shell_type,
                &shell.invocation,
            );
        }
        push_runtime_context_section(&mut lines, "ExecCommand Shell", exec_command_lines);
    }

    let exec_control_lines = exec_control_runtime_guidance(
        &facts.host_os,
        facts.remote_execution.is_some(),
        facts.needs.exec_control,
    );
    push_runtime_context_section(&mut lines, "ExecControl", exec_control_lines);

    if facts.needs.computer_use {
        let mut local_client_lines = Vec::new();
        if facts.remote_execution.is_some() && facts.needs.workspace_tools {
            local_client_lines.push(
                "- Computer use and UI automation operate on the local BitFun desktop, even when workspace file and shell tools target a remote host."
                    .to_string(),
            );
        }
        local_client_lines.push(format!(
            "- Local BitFun client OS: {} ({})",
            facts.host_os, facts.host_family
        ));
        local_client_lines.push(format!("- Local client architecture: {}", facts.host_arch));
        local_client_lines
            .push(runtime_computer_use_key_chord_guidance(&facts.host_os).to_string());
        push_runtime_context_section(&mut lines, "Local Client", local_client_lines);
    }

    // Text-only model guidance applies to both ComputerUse (desktop) and
    // ControlHub (browser). In remote workspaces only `control_hub` is true
    // (ComputerUse is disabled), so we emit a browser-scoped variant that does
    // not reference desktop automation actions unavailable in remote mode.
    if facts.supports_image_understanding == Some(false) {
        if facts.needs.computer_use {
            push_runtime_context_section(
                &mut lines,
                "Computer Use Input Strategy",
                computer_use_text_only_model_guidance(),
            );
        } else if facts.needs.control_hub {
            push_runtime_context_section(
                &mut lines,
                "Browser Input Strategy",
                control_hub_browser_text_only_guidance(),
            );
        }
    }

    Some(lines.join("\n"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRelatedPath {
    pub path: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContextFacts {
    pub workspace_path: String,
    pub related_paths: Vec<PromptRelatedPath>,
    pub remote_execution: Option<RemoteExecutionHints>,
}

pub fn render_workspace_context(facts: &WorkspaceContextFacts) -> String {
    let related_paths_section = if facts.related_paths.is_empty() {
        String::new()
    } else {
        let items = facts
            .related_paths
            .iter()
            .map(|related_path| {
                let path = related_path.path.replace('\\', "/");
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

    if let Some(remote) = &facts.remote_execution {
        format!(
            r#"## Workspace Context
<workspace_context>
- Workspace root (file tools, Glob, LS, ExecCommand on workspace): {}
{}
- Execution environment: **Remote SSH** — connection "{}".
- Remote host: {} (uname/kernel: {})
</workspace_context>
"#,
            facts.workspace_path,
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
            facts.workspace_path,
            if related_paths_section.is_empty() {
                String::new()
            } else {
                format!("\n{}", related_paths_section)
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLayoutFacts {
    pub listing: String,
    pub reached_limit: bool,
    pub max_entries: usize,
    pub remote: bool,
}

pub fn render_project_layout(facts: &ProjectLayoutFacts) -> String {
    let mut project_layout = "## Workspace Layout\n<project_layout>\n".to_string();
    if facts.remote {
        project_layout.push_str(
            "Below is a snapshot of the current workspace's file structure on the **remote** host.\n\n",
        );
    } else if facts.reached_limit {
        project_layout.push_str(&format!(
            "Below is a snapshot of the current workspace's file structure (showing up to {} entries).\n\n",
            facts.max_entries
        ));
    } else {
        project_layout
            .push_str("Below is a snapshot of the current workspace's file structure.\n\n");
    }
    project_layout.push_str(&facts.listing);
    project_layout.push_str("\n</project_layout>\n\n");
    project_layout
}

pub fn render_user_context_reminder(sections: impl IntoIterator<Item = String>) -> Option<String> {
    let sections = sections
        .into_iter()
        .map(|section| section.trim().to_string())
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>();

    if sections.is_empty() {
        None
    } else {
        Some(format!(
            "# User Context\n{}\n\n{}",
            USER_CONTEXT_PROMPT,
            sections.join("\n\n")
        ))
    }
}

const USER_CONTEXT_PROMPT: &str =
    "As you answer the user's questions, you can use the following context.\nNote: this is a snapshot captured at the start of the conversation and may not reflect real-time changes made afterward.";

fn local_exec_shell_runtime_guidance(shell_type: &str) -> &'static [&'static str] {
    match shell_type {
        "powershell" | "pwsh" => &[
            "- For inline Python or other embedded scripts, prefer PowerShell-friendly forms such as `@'\\nprint(\"Hello\")\\n'@ | python -` instead of heavily nested quoting.",
            "- In PowerShell, the escape character is the backtick (`), not backslash. `\\\"` is not a reliable way to escape a double quote for the shell.",
            "- For environment variables, process filtering, and file traversal, prefer native PowerShell cmdlets and syntax over shell-specific Unix patterns.",
            "- Avoid mixing PowerShell with `cmd.exe` or bash in the same command unless cross-shell behavior is explicitly required.",
        ],
        _ => &[],
    }
}

fn push_local_exec_shell_runtime_context(
    lines: &mut Vec<String>,
    shell_display_name: &str,
    shell_type: &str,
    shell_invocation: &str,
) {
    lines.push(format!(
        "- ExecCommand shell: {shell_display_name} ({shell_type}), invoked as {shell_invocation}."
    ));
    lines.extend(
        local_exec_shell_runtime_guidance(shell_type)
            .iter()
            .map(|line| (*line).to_string()),
    );
}

fn push_runtime_context_section(lines: &mut Vec<String>, title: &str, section_lines: Vec<String>) {
    if section_lines.is_empty() {
        return;
    }

    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!("## {title}"));
    lines.extend(section_lines);
}

fn exec_control_runtime_guidance(
    host_os: &str,
    remote_execution: bool,
    exec_control_available: bool,
) -> Vec<String> {
    if !exec_control_available || remote_execution || host_os != "windows" {
        return Vec::new();
    }

    vec![
        "- On local Windows ExecCommand sessions, `ExecControl` `interrupt` is effectively the same as `kill` for non-TTY processes.".to_string(),
    ]
}

fn computer_use_text_only_model_guidance() -> Vec<String> {
    vec![
        "- The configured primary model does not accept image inputs.".to_string(),
        "- When using `ComputerUse` or `ControlHub` with `domain: \"browser\"`, do not use `screenshot` and avoid `domain:\"browser\" action:\"screenshot\"`; image bytes will be unreadable.".to_string(),
        "- Action priority: 1) Terminal/CLI/system commands (`ExecCommand`, or `ComputerUse` `run_script`; use `WriteStdin`/`ExecControl` for running ExecCommand sessions) 2) Keyboard shortcuts (`key_chord`, `type_text`) 3) UI control: `click_element` (AX) -> `locate` -> `move_to_text` (use `move_to_text_match_index` when multiple OCR hits are listed) -> `mouse_move` (`use_screen_coordinates: true` with coordinates from tool JSON) -> `click`. For browser work, prefer `snapshot` then click by `@e*` ref over screenshots.".to_string(),
        "- Never guess coordinates. Always use precise methods: AX, OCR, system coordinates from tool results, or browser snapshot refs.".to_string(),
    ]
}

/// Browser-only text-only guidance for remote workspaces where `ComputerUse` is
/// disabled but `ControlHub` browser/terminal domains remain available on the
/// local client. Deliberately omits desktop-automation actions (`key_chord`,
/// `click_element`, AX, OCR) that are unavailable in remote mode.
fn control_hub_browser_text_only_guidance() -> Vec<String> {
    vec![
        "- The configured primary model does not accept image inputs.".to_string(),
        "- When using `ControlHub` with `domain: \"browser\"`, do not use `screenshot` or `domain:\"browser\" action:\"screenshot\"`; image bytes will be unreadable. Use `snapshot` then click by `@e*` ref instead.".to_string(),
    ]
}

fn runtime_computer_use_key_chord_guidance(host_os: &str) -> &'static str {
    match host_os {
        "macos" => "- Computer use / `key_chord`: the local BitFun desktop is macOS. Use `command`, `option`, `control`, and `shift` modifier names.",
        "windows" => "- Computer use / `key_chord`: the local BitFun desktop is Windows. Use `meta`/`super` for the Windows key, plus `alt`, `control`, and `shift`.",
        "linux" => "- Computer use / `key_chord`: the local BitFun desktop is Linux. Use `control`, `alt`, `shift`, and `meta`/`super` as appropriate for the desktop environment.",
        _ => "- Computer use / `key_chord`: match modifier names to the local BitFun desktop OS.",
    }
}

fn computer_use_key_chord_guidance(host_os: &str) -> &'static str {
    match host_os {
        "macos" => "Computer use / `key_chord`: the **local BitFun desktop** is **macOS** — use `command`, `option`, `control`, `shift` (not Win/Linux modifier names). **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for `osascript`, AppleScript, shell scripts) 2) Keyboard shortcuts: command+a/c/x/v (clipboard), command+space (Spotlight), command+tab (switch app) 3) UI control (AX/OCR/mouse) only when above fail.",
        "windows" => "Computer use / `key_chord`: the **local BitFun desktop** is **Windows** — use `meta`/`super` for Windows key, `alt`, `control`, `shift`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for PowerShell, cmd, scripts) 2) Keyboard shortcuts: control+a/c/x/v (clipboard), meta (Start menu), Alt+Tab (switch) 3) UI control only when above fail.",
        "linux" => "Computer use / `key_chord`: the **local BitFun desktop** is **Linux** — typically `control`, `alt`, `shift`, and sometimes `meta`/`super`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for shell scripts and system commands) 2) Keyboard shortcuts: control+a/c/x/v (clipboard) 3) UI control (AX/OCR/mouse) only when above fail.",
        _ => "Computer use / `key_chord`: match modifier names to the **local BitFun desktop** OS below. **ACTION PRIORITY:** 1) Terminal/CLI/system commands first 2) Keyboard shortcuts second 3) UI control (mouse/OCR) last resort.",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserContextSection {
    WorkspaceContext,
    WorkspaceInstructions,
    WorkspaceMemoryFiles,
    ProjectLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContextPolicy {
    pub sections: Vec<UserContextSection>,
}

impl UserContextPolicy {
    pub fn empty() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    pub fn with_section(mut self, section: UserContextSection) -> Self {
        if !self.includes(section) {
            self.sections.push(section);
        }
        self
    }

    pub fn without_section(mut self, section: UserContextSection) -> Self {
        self.sections.retain(|existing| *existing != section);
        self
    }

    pub fn with_workspace_context(self) -> Self {
        self.with_section(UserContextSection::WorkspaceContext)
    }

    pub fn with_workspace_instructions(self) -> Self {
        self.with_section(UserContextSection::WorkspaceInstructions)
    }

    pub fn with_workspace_memory_files(self) -> Self {
        self.with_section(UserContextSection::WorkspaceMemoryFiles)
    }

    pub fn with_project_layout(self) -> Self {
        self.with_section(UserContextSection::ProjectLayout)
    }

    pub fn includes(&self, section: UserContextSection) -> bool {
        self.sections.contains(&section)
    }

    pub fn cache_scope_key(&self) -> String {
        if self.sections.is_empty() {
            return "empty".to_string();
        }

        self.sections
            .iter()
            .map(UserContextSection::cache_scope_label)
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl Default for UserContextPolicy {
    fn default() -> Self {
        Self::empty()
    }
}

impl UserContextSection {
    fn cache_scope_label(&self) -> &'static str {
        match self {
            Self::WorkspaceContext => "workspace_context",
            Self::WorkspaceInstructions => "workspace_instructions",
            Self::WorkspaceMemoryFiles => "workspace_memory_files",
            Self::ProjectLayout => "project_layout",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolListingSections {
    pub skill_listing: Option<String>,
    pub agent_listing: Option<String>,
    pub collapsed_tool_listing: Option<String>,
}

impl ToolListingSections {
    pub fn is_empty(&self) -> bool {
        self.skill_listing.is_none()
            && self.agent_listing.is_none()
            && self.collapsed_tool_listing.is_none()
    }

    pub fn render_skill_listing_reminder(&self) -> Option<String> {
        self.skill_listing.as_deref().map(|skill_listing| {
            Self::render_section(
                SKILL_LISTING_TITLE,
                skill_listing,
                Some(SKILL_LISTING_GUIDANCE),
            )
        })
    }

    pub fn render_agent_listing_reminder(&self) -> Option<String> {
        self.agent_listing.as_deref().map(|agent_listing| {
            Self::render_section(
                AGENT_LISTING_TITLE,
                agent_listing,
                Some(AGENT_LISTING_GUIDANCE),
            )
        })
    }

    pub fn render_collapsed_tool_listing_reminder(&self) -> Option<String> {
        self.collapsed_tool_listing
            .as_deref()
            .map(|collapsed_tool_listing| {
                Self::render_section(
                    COLLAPSED_TOOL_LISTING_TITLE,
                    collapsed_tool_listing,
                    Some(COLLAPSED_TOOL_LISTING_GUIDANCE),
                )
            })
    }

    fn render_section(title: &str, body: &str, description: Option<&str>) -> String {
        match description {
            Some(description) => format!("{}\n{}\n\n{}", title, description, body.trim()),
            None => format!("{}\n{}", title, body.trim()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrependedPromptReminders {
    pub collapsed_tool_listing: Option<String>,
    pub skill_listing: Option<String>,
    pub agent_listing: Option<String>,
    pub runtime_context: Option<String>,
    pub user_context: Option<String>,
}

impl PrependedPromptReminders {
    pub fn ordered_reminders(&self) -> Vec<&str> {
        let mut reminders = Vec::new();
        if let Some(collapsed_tool_listing) = self.collapsed_tool_listing.as_deref() {
            reminders.push(collapsed_tool_listing);
        }
        if let Some(skill_listing) = self.skill_listing.as_deref() {
            reminders.push(skill_listing);
        }
        if let Some(agent_listing) = self.agent_listing.as_deref() {
            reminders.push(agent_listing);
        }
        if let Some(runtime_context) = self.runtime_context.as_deref() {
            reminders.push(runtime_context);
        }
        if let Some(user_context) = self.user_context.as_deref() {
            reminders.push(user_context);
        }
        reminders
    }
}
