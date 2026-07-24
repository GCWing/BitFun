use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::Path;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SkillParseError {
    #[error("Invalid SKILL.md format: {0}")]
    InvalidFormat(String),
    #[error("Missing required field '{0}' in SKILL.md")]
    MissingField(&'static str),
    #[error("Invalid skill path: {0}")]
    InvalidPath(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillLocation {
    User,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub level: SkillLocation,
    pub source_slot: String,
    /// Ecosystem identity shared by all roots owned by the same source.
    #[serde(default)]
    pub source_id: String,
    /// Stable product name supplied by the source definition.
    #[serde(default)]
    pub source_label: String,
    pub dir_name: String,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub group_key: Option<String>,
    #[serde(default)]
    pub is_shadowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadowed_by_key: Option<String>,
}

impl SkillInfo {
    pub fn to_xml_desc(&self) -> String {
        format!(
            r#"<skill name="{}">{}</skill>"#,
            self.name, self.description
        )
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSkillInfo {
    #[serde(flatten)]
    pub skill: SkillInfo,
    pub default_enabled: bool,
    pub effective_enabled: bool,
    pub disabled_by_mode: bool,
    pub selected_for_runtime: bool,
    pub state_reason: ModeSkillStateReason,
}

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

fn parse_front_matter_markdown(content: &str) -> Result<(Value, String), SkillParseError> {
    let front_matter_pattern = r"(?s)^---\r?\n(.*?)\r?\n---";
    let re = Regex::new(front_matter_pattern)
        .map_err(|error| SkillParseError::InvalidFormat(error.to_string()))?;
    let caps = re
        .captures(content)
        .ok_or_else(|| SkillParseError::InvalidFormat("Failed to capture content".to_string()))?;

    let yaml_content = caps
        .get(1)
        .ok_or_else(|| SkillParseError::InvalidFormat("Failed to get captures".to_string()))?
        .as_str();

    let metadata: Value = serde_yaml::from_str(yaml_content).map_err(|error| {
        SkillParseError::InvalidFormat(format!("Failed to parse YAML: {error}"))
    })?;

    let after_front_matter = caps
        .get(0)
        .ok_or_else(|| SkillParseError::InvalidFormat("Failed to get captures".to_string()))?
        .end();
    let markdown_body = content[after_front_matter..].trim_start();

    Ok((metadata, markdown_body.to_string()))
}

impl SkillData {
    pub fn from_markdown(
        path: String,
        content: &str,
        location: SkillLocation,
        with_content: bool,
    ) -> Result<Self, SkillParseError> {
        let (metadata, body) = parse_front_matter_markdown(content)?;

        let name = metadata
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or(SkillParseError::MissingField("name"))?;

        let description = metadata
            .get("description")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .ok_or(SkillParseError::MissingField("description"))?;

        let skill_content = if with_content { body } else { String::new() };
        let dir_name = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| SkillParseError::InvalidPath(path.clone()))?
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

pub fn render_loaded_skill_for_assistant(
    skill_data: &SkillData,
    loaded_by_stable_key: bool,
) -> String {
    let loaded_from = if loaded_by_stable_key {
        format!(" from stable key '{}'", skill_data.key)
    } else {
        String::new()
    };

    format!(
        "Skill '{}' loaded successfully{}. Note: any paths mentioned in this skill are relative to {}, not the workspace.\n\n<skill_content>\n{}\n</skill_content>",
        skill_data.name, loaded_from, skill_data.path, skill_data.content
    )
}
