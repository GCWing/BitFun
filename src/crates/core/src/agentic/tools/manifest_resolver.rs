use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::tools::framework::{Tool, ToolExposure, ToolUseContext};
use crate::agentic::tools::registry::{get_global_tool_registry, GET_TOOL_SPEC_TOOL_NAME};
use crate::util::types::ToolDefinition;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

type ToolRef = Arc<dyn Tool>;

#[derive(Debug, Clone)]
pub struct ResolvedToolManifest {
    pub allowed_tool_names: Vec<String>,
    pub tool_definitions: Vec<ToolDefinition>,
    pub collapsed_tool_names: Vec<String>,
}

#[derive(Clone)]
pub struct ResolvedVisibleTools {
    allowed_tool_names: Vec<String>,
    pub expanded_tools: Vec<Arc<dyn Tool>>,
    collapsed_tool_names: Vec<String>,
    pub collapsed_tools: Vec<Arc<dyn Tool>>,
}

fn build_visible_tools(
    tool_snapshot: &[ToolRef],
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    available_tool_names: &HashSet<String>,
) -> ResolvedVisibleTools {
    let allowed_set: HashSet<&str> = allowed_tools.iter().map(String::as_str).collect();
    let mut allowed_tool_names = allowed_tools.to_vec();
    let mut expanded_tools = Vec::new();
    let mut collapsed_tool_names = Vec::new();
    let mut collapsed_tools = Vec::new();

    for tool in tool_snapshot {
        let tool_name = tool.name().to_string();
        if !available_tool_names.contains(&tool_name) || !allowed_set.contains(tool_name.as_str()) {
            continue;
        }

        let exposure = exposure_overrides
            .get(&tool_name)
            .copied()
            .unwrap_or_else(|| tool.default_exposure());
        match exposure {
            ToolExposure::Collapsed => {
                collapsed_tool_names.push(tool_name);
                collapsed_tools.push(tool.clone());
            }
            ToolExposure::Expanded => expanded_tools.push(tool.clone()),
        }
    }

    if !collapsed_tool_names.is_empty() {
        if !allowed_tool_names
            .iter()
            .any(|name| name == GET_TOOL_SPEC_TOOL_NAME)
        {
            allowed_tool_names.push(GET_TOOL_SPEC_TOOL_NAME.to_string());
        }
        if let Some(tool) = tool_snapshot
            .iter()
            .find(|tool| tool.name() == GET_TOOL_SPEC_TOOL_NAME)
            .cloned()
        {
            expanded_tools.push(tool);
        }
    }

    ResolvedVisibleTools {
        allowed_tool_names,
        expanded_tools,
        collapsed_tool_names,
        collapsed_tools,
    }
}

pub async fn resolve_visible_tools(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedVisibleTools {
    let registry = get_global_tool_registry();
    let tool_snapshot = {
        let registry = registry.read().await;
        registry.get_all_tools()
    };

    let mut available_tool_names = HashSet::new();
    for tool in &tool_snapshot {
        if tool.is_available_in_context(Some(context)).await {
            available_tool_names.insert(tool.name().to_string());
        }
    }

    build_visible_tools(
        &tool_snapshot,
        allowed_tools,
        exposure_overrides,
        &available_tool_names,
    )
}

pub async fn resolve_tool_manifest(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedToolManifest {
    let visible_tools = resolve_visible_tools(allowed_tools, exposure_overrides, context).await;

    let mut tool_definitions =
        Vec::with_capacity(visible_tools.expanded_tools.len() + visible_tools.collapsed_tools.len());
    for tool in &visible_tools.expanded_tools {
        let description = tool
            .description_with_context(Some(context))
            .await
            .unwrap_or_else(|_| format!("Tool: {}", tool.name()));
        let parameters = tool.input_schema_for_model_with_context(Some(context)).await;

        tool_definitions.push(ToolDefinition {
            name: tool.name().to_string(),
            description,
            parameters,
        });
    }

    for tool in &visible_tools.collapsed_tools {
        let description = format!(
            "{} [This tool is collapsed. Call `GetToolSpec` first with {{\"tool_name\":\"{}\"}} before using it.]",
            tool.short_description(),
            tool.name()
        );

        tool_definitions.push(ToolDefinition {
            name: tool.name().to_string(),
            description,
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            }),
        });
    }

    let tool_ordering: HashMap<String, usize> = [
        ("Task", 1),
        ("Bash", 2),
        ("TerminalControl", 3),
        ("Glob", 4),
        ("Grep", 5),
        ("Read", 6),
        ("Edit", 7),
        ("Write", 8),
        ("Delete", 9),
        ("WebFetch", 10),
        ("WebSearch", 11),
        ("TodoWrite", 12),
        ("Skill", 13),
        ("Log", 14),
        ("GetToolSpec", 15),
        ("ControlHub", 16),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    tool_definitions.sort_by_key(|tool| tool_ordering.get(&tool.name).unwrap_or(&100));

    ResolvedToolManifest {
        allowed_tool_names: visible_tools.allowed_tool_names,
        tool_definitions,
        collapsed_tool_names: visible_tools.collapsed_tool_names,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_tool_manifest;
    use crate::agentic::agents::AgentToolPolicyOverrides;
    use crate::agentic::tools::framework::{ToolExposure, ToolUseContext};
    use crate::agentic::tools::registry::GET_TOOL_SPEC_TOOL_NAME;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use serde_json::json;
    use std::collections::HashMap;

    fn tool_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: Some("test-agent".to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            cancellation_token: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            workspace_services: None,
        }
    }

    #[tokio::test]
    async fn manifest_omits_get_tool_spec_without_collapsed_tools() {
        let allowed_tools = vec!["Read".to_string(), "Grep".to_string()];

        let manifest = resolve_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(),
        )
        .await;

        assert!(manifest.collapsed_tool_names.is_empty());
        assert_eq!(manifest.allowed_tool_names, allowed_tools);
        assert!(!manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME));
    }

    #[tokio::test]
    async fn manifest_adds_get_tool_spec_when_collapsed_tools_are_allowed() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];

        let manifest = resolve_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(),
        )
        .await;

        assert_eq!(manifest.collapsed_tool_names, vec!["WebFetch".to_string()]);
        assert!(manifest
            .allowed_tool_names
            .contains(&GET_TOOL_SPEC_TOOL_NAME.to_string()));
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == "Read"));
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == "WebFetch"));
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME));
        let stub = manifest
            .tool_definitions
            .iter()
            .find(|tool| tool.name == "WebFetch")
            .expect("WebFetch stub should exist");
        assert!(stub.description.contains("Call `GetToolSpec` first"));
        assert_eq!(stub.parameters["type"], json!("object"));
        assert_eq!(stub.parameters["additionalProperties"], json!(true));
    }

    #[tokio::test]
    async fn manifest_expands_tool_when_agent_override_requests_it() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];
        let mut overrides = AgentToolPolicyOverrides::default();
        overrides.insert("WebFetch".to_string(), ToolExposure::Expanded);

        let manifest = resolve_tool_manifest(&allowed_tools, &overrides, &tool_context()).await;

        assert!(manifest.collapsed_tool_names.is_empty());
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == "WebFetch"));
        assert!(!manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME));
    }
}
