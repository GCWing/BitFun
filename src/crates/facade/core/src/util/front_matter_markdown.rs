use serde_yaml::Value;

/// Parse and save Markdown with YAML front matter
pub struct FrontMatterMarkdown;

impl FrontMatterMarkdown {
    /// Parse front matter from file path and return metadata and body
    pub fn load(path: &str) -> Result<(Value, String), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read markdown file: {}", e))?;
        Self::load_str(&content).map_err(|e| format!("Failed to parse markdown file: {}", e))
    }

    /// Parse front matter from string and return metadata and body
    pub fn load_str(content: &str) -> Result<(Value, String), String> {
        let front_matter_pattern = r"(?s)^---\r?\n(.*?)\r?\n---";
        let re = regex::Regex::new(front_matter_pattern)
            .map_err(|e| format!("Failed to create regex: {}", e))?;
        let caps = re
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

    /// Save metadata and body as a markdown file with front matter
    pub fn save(path: &str, metadata: &Value, body: &str) -> Result<(), String> {
        let yaml_str = serde_yaml::to_string(metadata)
            .map_err(|e| format!("Failed to serialize YAML: {}", e))?;
        let content = format!("---\n{}\n---\n\n{}", yaml_str.trim_end(), body.trim_start());
        std::fs::write(path, content).map_err(|e| format!("Failed to write markdown file: {}", e))
    }
}
