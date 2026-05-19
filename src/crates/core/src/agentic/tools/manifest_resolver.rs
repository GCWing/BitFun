use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::tools::framework::{Tool, ToolUseContext};
use crate::agentic::tools::registry::{GET_TOOL_SPEC_TOOL_NAME, get_global_tool_registry};
use crate::util::types::ToolDefinition;
use bitfun_agent_tools::{
    PromptVisibleToolManifestItem, ToolManifestDefinition,
    build_prompt_visible_tool_manifest_definitions, build_tool_manifest_policy_tools,
    resolve_tool_manifest_policy,
};
use std::collections::HashSet;
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
    let policy_tools = build_tool_manifest_policy_tools(tool_snapshot, available_tool_names);
    let policy = resolve_tool_manifest_policy(
        &policy_tools,
        allowed_tools,
        exposure_overrides,
        GET_TOOL_SPEC_TOOL_NAME,
    );
    let expanded_tools = tools_by_name(tool_snapshot, &policy.expanded_tool_names);
    let collapsed_tools = tools_by_name(tool_snapshot, &policy.collapsed_tool_names);

    ResolvedVisibleTools {
        allowed_tool_names: policy.allowed_tool_names,
        expanded_tools,
        collapsed_tool_names: policy.collapsed_tool_names,
        collapsed_tools,
    }
}

fn tools_by_name(tool_snapshot: &[ToolRef], tool_names: &[String]) -> Vec<ToolRef> {
    tool_names
        .iter()
        .filter_map(|name| {
            tool_snapshot
                .iter()
                .find(|tool| tool.name() == name)
                .cloned()
        })
        .collect()
}

fn to_core_tool_definition(definition: ToolManifestDefinition) -> ToolDefinition {
    ToolDefinition {
        name: definition.name,
        description: definition.description,
        parameters: definition.parameters,
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

    let mut manifest_items = Vec::with_capacity(
        visible_tools.expanded_tools.len() + visible_tools.collapsed_tools.len(),
    );
    for tool in &visible_tools.expanded_tools {
        let description = tool
            .description_with_context(Some(context))
            .await
            .unwrap_or_else(|_| format!("Tool: {}", tool.name()));
        let parameters = tool
            .input_schema_for_model_with_context(Some(context))
            .await;

        manifest_items.push(PromptVisibleToolManifestItem::Expanded(
            ToolManifestDefinition::new(tool.name().to_string(), description, parameters),
        ));
    }

    for tool in &visible_tools.collapsed_tools {
        manifest_items.push(PromptVisibleToolManifestItem::Collapsed {
            name: tool.name().to_string(),
            short_description: tool.short_description(),
        });
    }

    let tool_definitions = build_prompt_visible_tool_manifest_definitions(&manifest_items);

    ResolvedToolManifest {
        allowed_tool_names: visible_tools.allowed_tool_names,
        tool_definitions: tool_definitions
            .into_iter()
            .map(to_core_tool_definition)
            .collect(),
        collapsed_tool_names: visible_tools.collapsed_tool_names,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_tool_manifest;
    use crate::agentic::agents::AgentToolPolicyOverrides;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::tools::framework::{ToolExposure, ToolUseContext};
    use crate::agentic::tools::registry::GET_TOOL_SPEC_TOOL_NAME;
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
        assert!(
            !manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME)
        );
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
        assert!(
            manifest
                .allowed_tool_names
                .contains(&GET_TOOL_SPEC_TOOL_NAME.to_string())
        );
        assert!(
            manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == "Read")
        );
        assert!(
            manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == "WebFetch")
        );
        assert!(
            manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME)
        );
        let stub = manifest
            .tool_definitions
            .iter()
            .find(|tool| tool.name == "WebFetch")
            .expect("WebFetch stub should exist");
        assert!(stub.description.contains("First call `GetToolSpec`"));
        assert_eq!(stub.parameters["type"], json!("object"));
        assert_eq!(stub.parameters["additionalProperties"], json!(false));
        assert!(
            stub.parameters["properties"]["tool_name"]["description"]
                .as_str()
                .unwrap()
                .contains("{\"tool_name\":\"WebFetch\"}")
        );
    }

    #[tokio::test]
    async fn manifest_snapshot_preserves_collapsed_tool_discovery_contract() {
        let allowed_tools = vec![
            "TodoWrite".to_string(),
            "WebFetch".to_string(),
            "Read".to_string(),
            "WebSearch".to_string(),
        ];

        let manifest = resolve_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(),
        )
        .await;

        assert_eq!(
            manifest.allowed_tool_names,
            vec![
                "TodoWrite".to_string(),
                "WebFetch".to_string(),
                "Read".to_string(),
                "WebSearch".to_string(),
                GET_TOOL_SPEC_TOOL_NAME.to_string(),
            ],
            "GetToolSpec should be appended without reordering the allowed-list contract"
        );
        assert_eq!(
            manifest.collapsed_tool_names,
            vec!["WebSearch".to_string(), "WebFetch".to_string()],
            "collapsed tools should follow registry snapshot order"
        );
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "WebFetch", "WebSearch", "TodoWrite", "GetToolSpec"],
            "prompt-visible manifest order must stay stable before owner migration"
        );

        let web_fetch = manifest
            .tool_definitions
            .iter()
            .find(|tool| tool.name == "WebFetch")
            .expect("collapsed WebFetch stub");
        assert!(
            web_fetch
                .description
                .contains("First call `GetToolSpec` with {\"tool_name\":\"WebFetch\"}")
        );
        assert_eq!(
            web_fetch.parameters,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Do not supply WebFetch arguments here while the tool is collapsed. Use GetToolSpec with {\"tool_name\":\"WebFetch\"} first."
                    }
                }
            })
        );
    }

    #[tokio::test]
    async fn manifest_guard_preserves_get_tool_spec_unlock_surface_before_owner_migration() {
        let allowed_tools = vec![
            "Read".to_string(),
            "WebFetch".to_string(),
            "GetFileDiff".to_string(),
            "Git".to_string(),
        ];

        let manifest = resolve_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(),
        )
        .await;

        assert_eq!(
            manifest.allowed_tool_names,
            vec![
                "Read".to_string(),
                "WebFetch".to_string(),
                "GetFileDiff".to_string(),
                "Git".to_string(),
                GET_TOOL_SPEC_TOOL_NAME.to_string(),
            ],
            "GetToolSpec insertion must preserve the runtime allowed-list contract"
        );
        assert_eq!(
            manifest.collapsed_tool_names,
            vec![
                "GetFileDiff".to_string(),
                "WebFetch".to_string(),
                "Git".to_string()
            ],
            "collapsed unlock list must follow product registry snapshot order"
        );
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "WebFetch", "GetToolSpec", "GetFileDiff", "Git"],
            "prompt-visible definitions must keep the current discovery insertion and policy order stable"
        );

        for tool_name in ["GetFileDiff", "WebFetch", "Git"] {
            let stub = manifest
                .tool_definitions
                .iter()
                .find(|tool| tool.name == tool_name)
                .unwrap_or_else(|| panic!("{tool_name} stub should exist"));
            assert!(
                stub.description.contains(&format!(
                    "First call `GetToolSpec` with {{\"tool_name\":\"{tool_name}\"}}"
                )),
                "collapsed stub must point to the explicit GetToolSpec unlock flow"
            );
            assert_eq!(stub.parameters["type"], json!("object"));
            assert_eq!(stub.parameters["additionalProperties"], json!(false));
            assert_eq!(
                stub.parameters["properties"]["tool_name"]["description"],
                json!(format!(
                    "Do not supply {tool_name} arguments here while the tool is collapsed. Use GetToolSpec with {{\"tool_name\":\"{tool_name}\"}} first."
                ))
            );
        }
    }

    #[tokio::test]
    async fn manifest_preserves_explicit_get_tool_spec_runtime_contract() {
        let allowed_tools = vec![GET_TOOL_SPEC_TOOL_NAME.to_string(), "WebFetch".to_string()];

        let manifest = resolve_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(),
        )
        .await;

        assert_eq!(manifest.allowed_tool_names, allowed_tools);
        assert_eq!(manifest.collapsed_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["WebFetch", "GetToolSpec", "GetToolSpec"],
            "core runtime currently mirrors the pure policy contract when GetToolSpec is already allowed"
        );
    }

    #[tokio::test]
    async fn manifest_expands_tool_when_agent_override_requests_it() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];
        let mut overrides = AgentToolPolicyOverrides::default();
        overrides.insert("WebFetch".to_string(), ToolExposure::Expanded);

        let manifest = resolve_tool_manifest(&allowed_tools, &overrides, &tool_context()).await;

        assert!(manifest.collapsed_tool_names.is_empty());
        assert!(
            manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == "WebFetch")
        );
        assert!(
            !manifest
                .tool_definitions
                .iter()
                .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME)
        );
    }
}
