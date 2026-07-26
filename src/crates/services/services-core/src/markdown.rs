use serde_yaml::Value;
use std::sync::LazyLock;

/// Compiled once; front-matter parsing runs on every `.md` scan.
static FRONT_MATTER_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---").expect("front matter regex pattern is valid")
});

/// Parses and writes Markdown files with YAML front matter.
pub struct FrontMatterMarkdown;

impl FrontMatterMarkdown {
    pub fn load(path: &str) -> Result<(Value, String), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read markdown file: {}", e))?;
        Self::load_str(&content).map_err(|e| format!("Failed to parse markdown file: {}", e))
    }

    pub fn load_str(content: &str) -> Result<(Value, String), String> {
        let caps = FRONT_MATTER_REGEX
            .captures(content)
            .ok_or_else(|| "Failed to capture content".to_string())?;

        let yaml_content = caps
            .get(1)
            .ok_or_else(|| "Failed to get captures".to_string())?
            .as_str();

        let metadata: Value = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        let after_front_matter = caps
            .get(0)
            .ok_or_else(|| "Failed to get captures".to_string())?
            .end();
        let markdown_body = content[after_front_matter..].trim_start();

        Ok((metadata, markdown_body.to_string()))
    }

    pub fn save(path: &str, metadata: &Value, body: &str) -> Result<(), String> {
        let yaml_str = serde_yaml::to_string(metadata)
            .map_err(|e| format!("Failed to serialize YAML: {}", e))?;
        let content = format!("---\n{}\n---\n\n{}", yaml_str.trim_end(), body.trim_start());
        std::fs::write(path, content).map_err(|e| format!("Failed to write markdown file: {}", e))
    }
}
