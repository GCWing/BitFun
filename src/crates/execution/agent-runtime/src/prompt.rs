//! Prompt-loop owner facts and reminder ordering.

const SKILL_LISTING_TITLE: &str = "# Skill Listing";
const SKILL_LISTING_GUIDANCE: &str =
    "The following skills are available for use with the Skill tool:";
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

fn computer_use_key_chord_guidance(host_os: &str) -> &'static str {
    match host_os {
        "macos" => "Computer use / `key_chord`: the **local BitFun desktop** is **macOS** — use `command`, `option`, `control`, `shift` (not Win/Linux modifier names). **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for `osascript`, AppleScript, shell scripts) 2) Keyboard shortcuts: command+a/c/x/v (clipboard), command+space (Spotlight), command+tab (switch app) 3) UI control (AX/OCR/mouse) only when above fail.",
        "windows" => "Computer use / `key_chord`: the **local BitFun desktop** is **Windows** — use `meta`/`super` for Windows key, `alt`, `control`, `shift`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for PowerShell, cmd, scripts) 2) Keyboard shortcuts: control+a/c/x/v (clipboard), meta (Start menu), Alt+Tab (switch) 3) UI control only when above fail.",
        "linux" => "Computer use / `key_chord`: the **local BitFun desktop** is **Linux** — typically `control`, `alt`, `shift`, and sometimes `meta`/`super`. **ACTION PRIORITY:** 1) Terminal/CLI/system commands (use ExecCommand for shell scripts and system commands) 2) Keyboard shortcuts: control+a/c/x/v (clipboard) 3) UI control (AX/OCR/mouse) only when above fail.",
        _ => "Computer use / `key_chord`: match modifier names to the **local BitFun desktop** OS below. **ACTION PRIORITY:** 1) Terminal/CLI/system commands first 2) Keyboard shortcuts second 3) UI control (mouse/OCR) last resort.",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserContextSection {
    WorkspaceContext,
    WorkspaceInstructions,
    WorkspaceMemoryFiles,
    ProjectLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
