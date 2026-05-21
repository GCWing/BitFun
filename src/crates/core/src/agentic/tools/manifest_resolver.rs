use crate::agentic::agents::AgentToolPolicyOverrides;
use crate::agentic::tools::catalog_provider::{
    resolve_product_tool_manifest, resolve_product_visible_tools,
};
use crate::agentic::tools::framework::{Tool, ToolUseContext};
use crate::util::types::ToolDefinition;
use bitfun_agent_tools::{ContextualToolManifest, ContextualVisibleTools, ToolManifestDefinition};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ResolvedToolManifest {
    pub allowed_tool_names: Vec<String>,
    pub tool_definitions: Vec<ToolDefinition>,
    pub collapsed_tool_names: Vec<String>,
}

#[derive(Clone)]
pub struct ResolvedVisibleTools {
    pub expanded_tools: Vec<Arc<dyn Tool>>,
    pub collapsed_tools: Vec<Arc<dyn Tool>>,
}

fn to_core_tool_definition(definition: ToolManifestDefinition) -> ToolDefinition {
    ToolDefinition {
        name: definition.name,
        description: definition.description,
        parameters: definition.parameters,
    }
}

impl From<ContextualVisibleTools<dyn Tool>> for ResolvedVisibleTools {
    fn from(value: ContextualVisibleTools<dyn Tool>) -> Self {
        Self {
            expanded_tools: value.expanded_tools,
            collapsed_tools: value.collapsed_tools,
        }
    }
}

impl From<ContextualToolManifest<dyn Tool>> for ResolvedToolManifest {
    fn from(value: ContextualToolManifest<dyn Tool>) -> Self {
        Self {
            allowed_tool_names: value.allowed_tool_names,
            tool_definitions: value
                .tool_definitions
                .into_iter()
                .map(to_core_tool_definition)
                .collect(),
            collapsed_tool_names: value.collapsed_tool_names,
        }
    }
}

pub async fn resolve_visible_tools(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedVisibleTools {
    resolve_product_visible_tools(allowed_tools, exposure_overrides, context)
    .await
    .into()
}

pub async fn resolve_tool_manifest(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedToolManifest {
    resolve_product_tool_manifest(allowed_tools, exposure_overrides, context)
    .await
    .into()
}

#[cfg(test)]
mod tests {
    use super::resolve_tool_manifest;
    use crate::agentic::agents::AgentToolPolicyOverrides;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::tools::framework::{ToolExposure, ToolUseContext};
    use bitfun_agent_tools::GET_TOOL_SPEC_TOOL_NAME;
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
