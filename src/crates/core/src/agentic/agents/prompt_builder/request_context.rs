#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestContextSection {
    WorkspaceInstructions,
    WorkspaceMemoryFiles,
    ProjectLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContextPolicy {
    pub sections: Vec<RequestContextSection>,
}

impl RequestContextPolicy {
    pub fn new(sections: Vec<RequestContextSection>) -> Self {
        Self { sections }
    }

    pub fn full() -> Self {
        Self::new(vec![
            RequestContextSection::WorkspaceInstructions,
            RequestContextSection::WorkspaceMemoryFiles,
            RequestContextSection::ProjectLayout,
        ])
    }

    pub fn full_without_layout() -> Self {
        Self::new(vec![
            RequestContextSection::WorkspaceInstructions,
            RequestContextSection::WorkspaceMemoryFiles,
        ])
    }

    pub fn instructions_only() -> Self {
        Self::new(vec![RequestContextSection::WorkspaceInstructions])
    }

    pub fn instructions_and_layout() -> Self {
        Self::new(vec![
            RequestContextSection::WorkspaceInstructions,
            RequestContextSection::ProjectLayout,
        ])
    }

    pub fn includes(&self, section: RequestContextSection) -> bool {
        self.sections.contains(&section)
    }

    pub fn has_override_sections(&self) -> bool {
        self.includes(RequestContextSection::WorkspaceMemoryFiles)
    }
}

impl Default for RequestContextPolicy {
    fn default() -> Self {
        Self::full()
    }
}
