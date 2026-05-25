#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestContextSection {
    WorkspaceContext,
    WorkspaceInstructions,
    WorkspaceMemoryFiles,
    ProjectLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContextPolicy {
    pub sections: Vec<RequestContextSection>,
}

impl RequestContextPolicy {
    pub fn empty() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    pub fn with_section(mut self, section: RequestContextSection) -> Self {
        if !self.includes(section) {
            self.sections.push(section);
        }
        self
    }

    pub fn without_section(mut self, section: RequestContextSection) -> Self {
        self.sections.retain(|existing| *existing != section);
        self
    }

    pub fn with_workspace_context(self) -> Self {
        self.with_section(RequestContextSection::WorkspaceContext)
    }

    pub fn with_workspace_instructions(self) -> Self {
        self.with_section(RequestContextSection::WorkspaceInstructions)
    }

    pub fn with_workspace_memory_files(self) -> Self {
        self.with_section(RequestContextSection::WorkspaceMemoryFiles)
    }

    pub fn with_project_layout(self) -> Self {
        self.with_section(RequestContextSection::ProjectLayout)
    }

    pub fn includes(&self, section: RequestContextSection) -> bool {
        self.sections.contains(&section)
    }
}

impl Default for RequestContextPolicy {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{RequestContextPolicy, RequestContextSection};

    #[test]
    fn chain_builder_preserves_order_and_dedupes_sections() {
        let policy = RequestContextPolicy::empty()
            .with_workspace_context()
            .with_workspace_instructions()
            .with_workspace_context()
            .with_project_layout()
            .without_section(RequestContextSection::ProjectLayout)
            .with_workspace_memory_files();

        assert_eq!(
            policy.sections,
            vec![
                RequestContextSection::WorkspaceContext,
                RequestContextSection::WorkspaceInstructions,
                RequestContextSection::WorkspaceMemoryFiles,
            ]
        );
    }

    #[test]
    fn default_policy_is_empty() {
        assert!(RequestContextPolicy::default().sections.is_empty());
    }
}
