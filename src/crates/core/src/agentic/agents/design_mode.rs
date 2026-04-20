//! Design Mode
//!
//! A design-focused mode that creates design artifacts and prototypes on behalf of the user.

use super::Agent;
use async_trait::async_trait;

pub struct DesignMode {
    default_tools: Vec<String>,
}

impl Default for DesignMode {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignMode {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                // Briefing and progress
                "AskUserQuestion".to_string(),
                "TodoWrite".to_string(),
                // Second-pass review only; DesignReview is the intended built-in pairing.
                "Task".to_string(),
                // Focused file discovery and editing
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                // Design Canvas workflow
                "DesignTokens".to_string(),
                "DesignArtifact".to_string(),
                // Verification and inspection
                "GetFileDiff".to_string(),
                "Bash".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for DesignMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "Design"
    }

    fn name(&self) -> &str {
        "Design"
    }

    fn description(&self) -> &str {
        "Design mode: create HTML-based design artifacts, prototypes, and visual deliverables"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "design_mode"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, DesignMode};

    #[test]
    fn default_tools_are_focused_on_design_canvas_delivery() {
        let agent = DesignMode::new();

        assert_eq!(
            agent.default_tools(),
            vec![
                "AskUserQuestion".to_string(),
                "TodoWrite".to_string(),
                "Task".to_string(),
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "DesignTokens".to_string(),
                "DesignArtifact".to_string(),
                "GetFileDiff".to_string(),
                "Bash".to_string(),
            ]
        );
    }

    #[test]
    fn default_tools_exclude_broad_or_redundant_surfaces() {
        let tools = DesignMode::new().default_tools();

        for redundant_tool in [
            "Skill",
            "Delete",
            "TerminalControl",
            "WebSearch",
            "ComputerUse",
            "GenerativeUI",
        ] {
            assert!(
                !tools.contains(&redundant_tool.to_string()),
                "{redundant_tool} should not be a default Design tool"
            );
        }
    }
}
