//! Skill type definitions

use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::front_matter_markdown::FrontMatterMarkdown;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

/// Skill location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillLocation {
    /// User-level (global)
    User,
    /// Project-level
    Project,
}

impl SkillLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillLocation::User => "user",
            SkillLocation::Project => "project",
        }
    }
}

/// Complete skill information (for API return)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Skill name (read from SKILL.md, used as unique identifier)
    pub name: String,
    /// Description (read from SKILL.md)
    pub description: String,
    /// Skill folder path
    pub path: String,
    /// Level (project-level/user-level)
    pub level: SkillLocation,
    /// Whether enabled
    pub enabled: bool,
}

impl SkillInfo {
    /// Convert to XML description (for tool description)
    pub fn to_xml_desc(&self) -> String {
        format!(
            r#"<skill>
<name>
{}
</name>
<description>
{}
</description>
<location>
{}
</location>
</skill>
"#,
            self.name, self.description, self.path
        )
    }
}

/// Skill data (contains content, for execution)
#[derive(Debug, Clone)]
pub struct SkillData {
    pub name: String,
    pub description: String,
    pub content: String,
    pub location: SkillLocation,
    pub path: String,
    /// Whether enabled (read from enabled field in SKILL.md, defaults to true if not present)
    pub enabled: bool,
}

impl SkillData {
    /// Parse Skill from SKILL.md file content
    pub fn from_markdown(
        path: String,
        content: &str,
        location: SkillLocation,
        with_content: bool,
    ) -> BitFunResult<Self> {
        let (metadata, body) = FrontMatterMarkdown::load_str(content)
            .map_err(|e| BitFunError::tool(format!("Invalid SKILL.md format: {}", e)))?;

        // Extract fields from YAML metadata
        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                BitFunError::tool("Missing required field 'name' in SKILL.md".to_string())
            })?;

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                BitFunError::tool("Missing required field 'description' in SKILL.md".to_string())
            })?;

        // enabled field defaults to true if not present
        let enabled = metadata
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let skill_content = if with_content { body } else { String::new() };

        Ok(SkillData {
            name,
            description,
            content: skill_content,
            location,
            path,
            enabled,
        })
    }

    /// Set enabled status and save to SKILL.md file
    ///
    /// If enabled is true, remove enabled field (use default value)
    /// If enabled is false, write enabled: false
    pub fn set_enabled_and_save(skill_md_path: &str, enabled: bool) -> BitFunResult<()> {
        let (mut metadata, body) = FrontMatterMarkdown::load(skill_md_path)
            .map_err(|e| BitFunError::tool(format!("Failed to load SKILL.md: {}", e)))?;

        // Get mutable mapping of metadata
        let map = metadata.as_mapping_mut().ok_or_else(|| {
            BitFunError::tool("Invalid SKILL.md: metadata is not a mapping".to_string())
        })?;

        if enabled {
            // When enabling, remove enabled field (use default value)
            map.remove(&Value::String("enabled".to_string()));
        } else {
            // When disabling, write enabled: false
            map.insert(Value::String("enabled".to_string()), Value::Bool(false));
        }

        FrontMatterMarkdown::save(skill_md_path, &metadata, &body)
            .map_err(|e| BitFunError::tool(format!("Failed to save SKILL.md: {}", e)))?;

        Ok(())
    }

    /// Convert to XML description
    pub fn to_xml_desc(&self) -> String {
        format!(
            r#"<skill>
<name>
{}
</name>
<description>
{}
</description>
<location>
{}
</location>
</skill>
"#,
            self.name, self.description, self.path
        )
    }
}
