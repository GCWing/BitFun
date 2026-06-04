//! Custom subagent portable schema and serialization decisions.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const DEFAULT_CUSTOM_SUBAGENT_TOOLS: &[&str] = &["LS", "Read", "Glob", "Grep"];
pub const DEFAULT_CUSTOM_SUBAGENT_READONLY: bool = true;
pub const DEFAULT_CUSTOM_SUBAGENT_REVIEW: bool = false;
pub const DEFAULT_CUSTOM_SUBAGENT_MODEL: &str = "fast";
pub const CUSTOM_SUBAGENT_PROJECT_AGENT_SUBDIRS: &[(&str, &str)] = &[
    (".bitfun", "agents"),
    (".claude", "agents"),
    (".cursor", "agents"),
    (".codex", "agents"),
];

/// Custom subagent source kind. The runtime owns portable source contracts and
/// discovery decisions; product services supply concrete root directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomSubagentKind {
    /// Project subagent.
    Project,
    /// User subagent.
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSubagentDefinition {
    pub name: String,
    pub description: String,
    pub tools: Vec<String>,
    pub prompt: String,
    pub readonly: bool,
    pub review: bool,
    pub kind: CustomSubagentKind,
    pub model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomSubagentDefinitionError {
    MissingName,
    MissingDescription,
}

impl CustomSubagentDefinitionError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingName => "Missing name field",
            Self::MissingDescription => "Missing description field",
        }
    }
}

impl CustomSubagentDefinition {
    pub fn new(
        name: String,
        description: String,
        tools: Vec<String>,
        prompt: String,
        readonly: bool,
        kind: CustomSubagentKind,
    ) -> Self {
        Self {
            name,
            description,
            tools,
            prompt,
            readonly,
            review: DEFAULT_CUSTOM_SUBAGENT_REVIEW,
            kind,
            model: DEFAULT_CUSTOM_SUBAGENT_MODEL.to_string(),
        }
    }

    pub fn from_front_matter_fields(
        name: Option<&str>,
        description: Option<&str>,
        tools: Option<&str>,
        readonly: Option<bool>,
        review: Option<bool>,
        model: Option<&str>,
        prompt: String,
        kind: CustomSubagentKind,
    ) -> Result<Self, CustomSubagentDefinitionError> {
        Ok(Self {
            name: name
                .ok_or(CustomSubagentDefinitionError::MissingName)?
                .to_string(),
            description: description
                .ok_or(CustomSubagentDefinitionError::MissingDescription)?
                .to_string(),
            tools: custom_subagent_tools_from_front_matter(tools),
            prompt,
            readonly: custom_subagent_readonly_or_default(readonly),
            review: custom_subagent_review_or_default(review),
            kind,
            model: custom_subagent_model_or_default(model).to_string(),
        })
    }

    pub fn tools_front_matter(&self) -> Option<String> {
        custom_subagent_tools_to_front_matter(&self.tools)
    }

    pub fn should_save_readonly(&self) -> bool {
        custom_subagent_readonly_should_save(self.readonly)
    }

    pub fn should_save_review(&self) -> bool {
        custom_subagent_review_should_save(self.review)
    }

    pub fn should_save_model(&self) -> bool {
        custom_subagent_model_should_save(&self.model)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSubagentDiscoveryRoots {
    pub workspace_root: PathBuf,
    pub bitfun_user_agents_dir: Option<PathBuf>,
    pub home_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSubagentDirEntry {
    pub path: PathBuf,
    pub kind: CustomSubagentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedCustomSubagentDefinition {
    pub path: PathBuf,
    pub definition: CustomSubagentDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSubagentLoadError {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomSubagentLoadReport {
    pub definitions: Vec<LoadedCustomSubagentDefinition>,
    pub errors: Vec<CustomSubagentLoadError>,
}

struct CustomSubagentCandidate {
    definition: CustomSubagentDefinition,
    root_priority: usize,
    path: PathBuf,
}

pub fn custom_subagent_possible_dirs(
    roots: &CustomSubagentDiscoveryRoots,
) -> Vec<CustomSubagentDirEntry> {
    let mut entries = Vec::new();

    for (parent, sub) in CUSTOM_SUBAGENT_PROJECT_AGENT_SUBDIRS {
        let path = roots.workspace_root.join(parent).join(sub);
        if path.exists() && path.is_dir() {
            entries.push(CustomSubagentDirEntry {
                path,
                kind: CustomSubagentKind::Project,
            });
        }
    }

    if let Some(bitfun_agents) = &roots.bitfun_user_agents_dir {
        if bitfun_agents.exists() && bitfun_agents.is_dir() {
            entries.push(CustomSubagentDirEntry {
                path: bitfun_agents.clone(),
                kind: CustomSubagentKind::User,
            });
        }
    }

    if let Some(home) = &roots.home_dir {
        for (parent, sub) in CUSTOM_SUBAGENT_PROJECT_AGENT_SUBDIRS {
            if *parent == ".bitfun" {
                continue;
            }
            let path = home.join(parent).join(sub);
            if path.exists() && path.is_dir() {
                entries.push(CustomSubagentDirEntry {
                    path,
                    kind: CustomSubagentKind::User,
                });
            }
        }
    }

    entries
}

pub fn load_custom_subagent_definitions(
    roots: &CustomSubagentDiscoveryRoots,
) -> CustomSubagentLoadReport {
    let mut candidates = Vec::new();
    let mut errors = Vec::new();

    for (root_priority, entry) in custom_subagent_possible_dirs(roots).into_iter().enumerate() {
        for md_path in list_custom_subagent_markdown_files(&entry.path) {
            match custom_subagent_read_markdown_file(&md_path, entry.kind) {
                Ok(definition) => candidates.push(CustomSubagentCandidate {
                    definition,
                    root_priority,
                    path: md_path,
                }),
                Err(error) => errors.push(CustomSubagentLoadError {
                    path: md_path,
                    error,
                }),
            }
        }
    }

    candidates.sort_by(|a, b| {
        a.root_priority
            .cmp(&b.root_priority)
            .then_with(|| {
                a.definition
                    .name
                    .to_lowercase()
                    .cmp(&b.definition.name.to_lowercase())
            })
            .then_with(|| a.definition.name.cmp(&b.definition.name))
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut definitions = Vec::new();
    let mut seen_ids = HashSet::new();
    for candidate in candidates {
        if seen_ids.insert(candidate.definition.name.clone()) {
            definitions.push(LoadedCustomSubagentDefinition {
                path: candidate.path,
                definition: candidate.definition,
            });
        }
    }

    CustomSubagentLoadReport {
        definitions,
        errors,
    }
}

fn list_custom_subagent_markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    out.sort();
    out
}

pub fn custom_subagent_tools_from_front_matter(tools: Option<&str>) -> Vec<String> {
    tools
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .collect()
        })
        .unwrap_or_else(|| {
            DEFAULT_CUSTOM_SUBAGENT_TOOLS
                .iter()
                .map(|tool| (*tool).to_string())
                .collect()
        })
}

pub fn custom_subagent_tools_are_default(tools: &[String]) -> bool {
    tools.len() == DEFAULT_CUSTOM_SUBAGENT_TOOLS.len()
        && tools
            .iter()
            .zip(DEFAULT_CUSTOM_SUBAGENT_TOOLS.iter())
            .all(|(actual, expected)| actual == *expected)
}

pub fn custom_subagent_tools_to_front_matter(tools: &[String]) -> Option<String> {
    if custom_subagent_tools_are_default(tools) {
        None
    } else {
        Some(tools.join(", "))
    }
}

pub const fn custom_subagent_readonly_or_default(readonly: Option<bool>) -> bool {
    match readonly {
        Some(value) => value,
        None => DEFAULT_CUSTOM_SUBAGENT_READONLY,
    }
}

pub const fn custom_subagent_readonly_should_save(readonly: bool) -> bool {
    readonly != DEFAULT_CUSTOM_SUBAGENT_READONLY
}

pub const fn custom_subagent_review_or_default(review: Option<bool>) -> bool {
    match review {
        Some(value) => value,
        None => DEFAULT_CUSTOM_SUBAGENT_REVIEW,
    }
}

pub const fn custom_subagent_review_should_save(review: bool) -> bool {
    review != DEFAULT_CUSTOM_SUBAGENT_REVIEW
}

pub fn custom_subagent_model_or_default(model: Option<&str>) -> &str {
    model.unwrap_or(DEFAULT_CUSTOM_SUBAGENT_MODEL)
}

pub fn custom_subagent_model_should_save(model: &str) -> bool {
    model != DEFAULT_CUSTOM_SUBAGENT_MODEL
}

pub fn custom_subagent_read_markdown_file(
    path: impl AsRef<Path>,
    kind: CustomSubagentKind,
) -> Result<CustomSubagentDefinition, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read markdown file: {error}"))?;

    custom_subagent_read_markdown_str(&contents, kind)
        .map_err(|error| format!("Failed to parse markdown file: {error}"))
}

pub fn custom_subagent_read_markdown_str(
    contents: &str,
    kind: CustomSubagentKind,
) -> Result<CustomSubagentDefinition, String> {
    let regex = Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---")
        .map_err(|error| format!("Failed to create regex: {error}"))?;
    let captures = regex
        .captures(contents)
        .ok_or_else(|| "Failed to capture content".to_string())?;
    let yaml = captures
        .get(1)
        .ok_or_else(|| "Failed to get captures".to_string())?
        .as_str();
    let metadata: Value =
        serde_yaml::from_str(yaml).map_err(|error| format!("Failed to parse YAML: {error}"))?;

    let front_matter_end = captures
        .get(0)
        .ok_or_else(|| "Failed to get captures".to_string())?
        .end();
    let prompt = contents[front_matter_end..].trim_start().to_string();

    custom_subagent_definition_from_metadata(&metadata, prompt, kind)
}

pub fn custom_subagent_save_markdown_file(
    path: impl AsRef<Path>,
    definition: &CustomSubagentDefinition,
) -> Result<(), String> {
    custom_subagent_save_markdown_parts(
        path,
        &definition.name,
        &definition.description,
        &definition.tools,
        &definition.prompt,
        definition.readonly,
        definition.review,
        &definition.model,
    )
}

pub fn custom_subagent_save_markdown_parts(
    path: impl AsRef<Path>,
    name: &str,
    description: &str,
    tools: &[String],
    prompt: &str,
    readonly: bool,
    review: bool,
    model: &str,
) -> Result<(), String> {
    let metadata =
        custom_subagent_markdown_metadata(name, description, tools, readonly, review, model);
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize YAML: {error}"))?;
    let contents = format!("---\n{}\n---\n\n{}", yaml.trim_end(), prompt.trim_start());

    std::fs::write(path, contents)
        .map_err(|error| format!("Failed to write markdown file: {error}"))
}

fn custom_subagent_definition_from_metadata(
    metadata: &Value,
    prompt: String,
    kind: CustomSubagentKind,
) -> Result<CustomSubagentDefinition, String> {
    CustomSubagentDefinition::from_front_matter_fields(
        metadata.get("name").and_then(|value| value.as_str()),
        metadata.get("description").and_then(|value| value.as_str()),
        metadata.get("tools").and_then(|value| value.as_str()),
        metadata.get("readonly").and_then(|value| value.as_bool()),
        metadata.get("review").and_then(|value| value.as_bool()),
        metadata.get("model").and_then(|value| value.as_str()),
        prompt,
        kind,
    )
    .map_err(|error| error.message().to_string())
}

fn custom_subagent_markdown_metadata(
    name: &str,
    description: &str,
    tools: &[String],
    readonly: bool,
    review: bool,
    model: &str,
) -> Value {
    let mut metadata = serde_yaml::Mapping::new();
    metadata.insert(
        Value::String("name".into()),
        Value::String(name.to_string()),
    );
    metadata.insert(
        Value::String("description".into()),
        Value::String(description.to_string()),
    );
    if let Some(tools) = custom_subagent_tools_to_front_matter(tools) {
        metadata.insert(Value::String("tools".into()), Value::String(tools));
    }
    if custom_subagent_readonly_should_save(readonly) {
        metadata.insert(Value::String("readonly".into()), Value::Bool(readonly));
    }
    if custom_subagent_review_should_save(review) {
        metadata.insert(Value::String("review".into()), Value::Bool(review));
    }
    if custom_subagent_model_should_save(model) {
        metadata.insert(
            Value::String("model".into()),
            Value::String(model.to_string()),
        );
    }

    Value::Mapping(metadata)
}
