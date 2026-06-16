//! Compatibility wrappers for legacy custom subagent contracts.

use crate::custom_agent::{
    custom_agent_model_or_default, custom_agent_model_should_save, custom_agent_possible_dirs,
    custom_agent_read_markdown_file, custom_agent_read_markdown_str,
    custom_agent_readonly_should_save, custom_agent_review_should_save,
    custom_agent_save_markdown_file, custom_agent_tools_are_default, default_custom_agent_tools,
    load_custom_agent_definitions, CustomAgentDefinition, CustomAgentDefinitionError,
    CustomAgentDiscoveryRoots, CustomAgentKind, CustomAgentLevel, CustomAgentLoadReport,
    LoadedCustomAgentDefinition, ParsedCustomAgentDefinition, DEFAULT_CUSTOM_SUBAGENT_READONLY,
    DEFAULT_CUSTOM_SUBAGENT_REVIEW,
};

pub use crate::custom_agent::DEFAULT_CUSTOM_SUBAGENT_TOOLS;
pub type CustomSubagentKind = CustomAgentLevel;
pub type CustomSubagentDefinition = CustomAgentDefinition;
pub type CustomSubagentDefinitionError = CustomAgentDefinitionError;
pub type CustomSubagentDiscoveryRoots = CustomAgentDiscoveryRoots;
pub type LoadedCustomSubagentDefinition = LoadedCustomAgentDefinition;
pub type CustomSubagentLoadReport = CustomAgentLoadReport;

pub fn custom_subagent_tools_from_front_matter(tools: Option<&str>) -> Vec<String> {
    match tools {
        Some(value) => value
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        None => default_custom_agent_tools(CustomAgentKind::Subagent),
    }
}

pub fn custom_subagent_tools_are_default(tools: &[String]) -> bool {
    custom_agent_tools_are_default(CustomAgentKind::Subagent, tools)
}

pub fn custom_subagent_tools_to_front_matter(tools: &[String]) -> Option<String> {
    (!custom_subagent_tools_are_default(tools)).then(|| tools.join(", "))
}

pub const fn custom_subagent_readonly_or_default(readonly: Option<bool>) -> bool {
    match readonly {
        Some(value) => value,
        None => DEFAULT_CUSTOM_SUBAGENT_READONLY,
    }
}

pub fn custom_subagent_readonly_should_save(readonly: bool) -> bool {
    custom_agent_readonly_should_save(CustomAgentKind::Subagent, readonly)
}

pub const fn custom_subagent_review_or_default(review: Option<bool>) -> bool {
    match review {
        Some(value) => value,
        None => DEFAULT_CUSTOM_SUBAGENT_REVIEW,
    }
}

pub fn custom_subagent_review_should_save(review: bool) -> bool {
    custom_agent_review_should_save(CustomAgentKind::Subagent, review)
}

pub fn custom_subagent_model_or_default(model: Option<&str>) -> &str {
    custom_agent_model_or_default(CustomAgentKind::Subagent, model)
}

pub fn custom_subagent_model_should_save(model: &str) -> bool {
    custom_agent_model_should_save(CustomAgentKind::Subagent, model)
}

pub fn custom_subagent_possible_dirs(
    roots: &CustomSubagentDiscoveryRoots,
) -> Vec<crate::custom_agent::CustomAgentDirEntry> {
    custom_agent_possible_dirs(roots)
}

pub fn load_custom_subagent_definitions(
    roots: &CustomSubagentDiscoveryRoots,
) -> CustomSubagentLoadReport {
    let report = load_custom_agent_definitions(roots);
    CustomSubagentLoadReport {
        definitions: report
            .definitions
            .into_iter()
            .filter(|loaded| loaded.definition.kind == CustomAgentKind::Subagent)
            .collect(),
        errors: report.errors,
    }
}

pub fn custom_subagent_read_markdown_file(
    path: impl AsRef<std::path::Path>,
    kind: CustomSubagentKind,
) -> Result<CustomSubagentDefinition, String> {
    let parsed = custom_agent_read_markdown_file(path, kind)?;
    ensure_subagent_definition(parsed)
}

pub fn custom_subagent_read_markdown_str(
    contents: &str,
    kind: CustomSubagentKind,
) -> Result<CustomSubagentDefinition, String> {
    let parsed = custom_agent_read_markdown_str(contents, kind)?;
    ensure_subagent_definition(parsed)
}

pub fn custom_subagent_save_markdown_file(
    path: impl AsRef<std::path::Path>,
    definition: &CustomSubagentDefinition,
) -> Result<(), String> {
    custom_agent_save_markdown_file(path, definition)
}

pub fn custom_subagent_save_markdown_parts(
    path: impl AsRef<std::path::Path>,
    name: &str,
    description: &str,
    tools: &[String],
    prompt: &str,
    readonly: bool,
    review: bool,
    model: &str,
) -> Result<(), String> {
    let mut definition = CustomAgentDefinition::new(
        name.to_string(),
        name.to_string(),
        description.to_string(),
        CustomAgentKind::Subagent,
        tools.to_vec(),
        prompt.to_string(),
        readonly,
        CustomAgentLevel::User,
        model.to_string(),
        crate::custom_agent::default_custom_agent_user_context_policy(CustomAgentKind::Subagent),
    );
    definition.review = review;
    custom_agent_save_markdown_file(path, &definition)
}

fn ensure_subagent_definition(
    parsed: ParsedCustomAgentDefinition,
) -> Result<CustomSubagentDefinition, String> {
    if parsed.definition.kind != CustomAgentKind::Subagent {
        return Err("Expected custom subagent file".to_string());
    }
    Ok(parsed.definition)
}
