//! Skill type definitions

use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::front_matter_markdown::FrontMatterMarkdown;
use serde::{Deserialize, Serialize};

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
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    /// Runtime-unique identifier derived from source slot + directory name.
    pub key: String,
    /// Skill name (read from SKILL.md, used by the model to invoke the skill)
    pub name: String,
    /// Description (read from SKILL.md)
    pub description: String,
    /// Skill folder path
    pub path: String,
    /// Level (project-level/user-level)
    pub level: SkillLocation,
    /// Source slot that discovered this skill.
    pub source_slot: String,
    /// Directory name under the slot's `skills/` root.
    pub dir_name: String,
    /// Whether this skill is bundled with BitFun as a built-in skill.
    #[serde(default)]
    pub is_builtin: bool,
    /// Optional logical group for built-in skills.
    #[serde(default)]
    pub group_key: Option<String>,
    /// True when this skill is shadowed by a higher-priority skill with the same name.
    #[serde(default)]
    pub is_shadowed: bool,
    /// Key of the skill that shadows this one (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by_key: Option<String>,
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

/// The most specific rule that determined a skill's availability in a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSkillStateReason {
    ProjectDefaultEnabled,
    DisabledByProjectOverride,
    CustomUserDefaultEnabled,
    BuiltinPolicyEnabled,
    BuiltinPolicyDisabled,
    EnabledByUserOverride,
    DisabledByUserOverride,
}

/// Skill information annotated for a specific mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSkillInfo {
    #[serde(flatten)]
    pub skill: SkillInfo,
    /// Whether this skill is enabled by default before user/project overrides.
    pub default_enabled: bool,
    /// Whether this skill is effectively enabled after applying all overrides.
    pub effective_enabled: bool,
    /// Backward-compatible inverse of `effective_enabled`.
    pub disabled_by_mode: bool,
    /// True when this skill is the one actually selected for runtime after applying
    /// mode disables and same-name priority resolution.
    pub selected_for_runtime: bool,
    /// The rule that ultimately decided the effective state of this skill.
    pub state_reason: ModeSkillStateReason,
}

/// Skill data (contains content, for execution)
#[derive(Debug, Clone)]
pub struct SkillData {
    pub key: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub location: SkillLocation,
    pub path: String,
    pub source_slot: String,
    pub dir_name: String,
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

        let skill_content = if with_content { body } else { String::new() };
        let dir_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| BitFunError::tool(format!("Invalid skill path: {}", path)))?
            .to_string();

        Ok(SkillData {
            key: String::new(),
            name,
            description,
            content: skill_content,
            location,
            path,
            source_slot: String::new(),
            dir_name,
        })
    }
}
