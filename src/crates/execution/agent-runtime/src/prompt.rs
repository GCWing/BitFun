//! Prompt-loop owner facts and reminder ordering.

const SKILL_LISTING_TITLE: &str = "# Skill Listing";
const SKILL_LISTING_GUIDANCE: &str =
    "The following skills are available for use with the Skill tool:";
const AGENT_LISTING_TITLE: &str = "# Agent Listing";
const AGENT_LISTING_GUIDANCE: &str = "Available subagent types for the Task tool:";
const COLLAPSED_TOOL_LISTING_TITLE: &str = "# Collapsed Tool Listing";
const COLLAPSED_TOOL_LISTING_GUIDANCE: &str = r#"The folling tools are intentionally collapsed. Their listed descriptions are short summaries rather than full usage instructions.
Before calling a collapsed tool, call `GetToolSpec` with its exact tool name to read its full definition and input schema.
After reading the returned spec, call the real tool directly by its own name.
If a tool spec is already available in the current conversation, do not call `GetToolSpec` for it again."#;

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
    pub skill_listing: Option<String>,
    pub agent_listing: Option<String>,
    pub collapsed_tool_listing: Option<String>,
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
        if let Some(user_context) = self.user_context.as_deref() {
            reminders.push(user_context);
        }
        reminders
    }
}
