use super::{Agent, RequestContextPolicy};
use async_trait::async_trait;

pub struct FileFinderAgent {
    default_tools: Vec<String>,
}

impl Default for FileFinderAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl FileFinderAgent {
    pub fn new() -> Self {
        Self {
            default_tools: vec![
                "LS".to_string(),
                "Read".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
            ],
        }
    }
}

#[async_trait]
impl Agent for FileFinderAgent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        "FileFinder"
    }

    fn name(&self) -> &str {
        "FileFinder"
    }

    fn description(&self) -> &str {
        r#"Agent specialized for semantically searching and locating relevant files and directories.
Output: File paths, line ranges (optional), and brief descriptions. You need to read the files yourself after receiving the results. This is very helpful to avoid information loss.
Usage: Just describe what you want to find. Do NOT specify output format.
Recommended for: finding files based on semantic descriptions, content concepts, or when you don't know exact filenames.

Examples:
- "Find files that implement authentication"
- "Locate files that define the UI layout of the login page"  
- "Search for files related to error handling""#
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        "file_finder_agent"
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn request_context_policy(&self) -> RequestContextPolicy {
        RequestContextPolicy::instructions_only()
    }

    fn is_readonly(&self) -> bool {
        true
    }
}
