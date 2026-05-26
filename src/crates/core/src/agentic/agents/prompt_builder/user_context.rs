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
}

impl Default for UserContextPolicy {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{UserContextPolicy, UserContextSection};

    #[test]
    fn chain_builder_preserves_order_and_dedupes_sections() {
        let policy = UserContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_workspace_context()
            .with_project_layout()
            .without_section(UserContextSection::ProjectLayout)
            .with_workspace_memory_files();

        assert_eq!(
            policy.sections,
            vec![
                UserContextSection::WorkspaceContext,
                UserContextSection::WorkspaceInstructions,
                UserContextSection::WorkspaceMemoryFiles,
            ]
        );
    }

    #[test]
    fn default_policy_is_empty() {
        assert!(UserContextPolicy::default().sections.is_empty());
    }
}
