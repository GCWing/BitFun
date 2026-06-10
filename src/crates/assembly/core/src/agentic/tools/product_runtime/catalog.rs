//! Product tool catalog, manifest, and GetToolSpec runtime owner.

use crate::agentic::agents::{get_agent_registry, AgentToolPolicyOverrides};
use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult};
use crate::agentic::tools::registry::{get_global_tool_registry, ToolRef};
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolDefinition;
use bitfun_agent_tools::{
    ContextualToolManifest, ContextualVisibleTools, GetToolSpecCatalogProvider,
    GetToolSpecCollapsedToolSummary, GetToolSpecExecutionError, GetToolSpecRuntime,
    ToolCatalogRuntime, ToolCatalogSnapshotProvider, ToolManifestDefinition,
    GET_TOOL_SPEC_TOOL_NAME,
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ResolvedToolManifest {
    pub allowed_tool_names: Vec<String>,
    pub tool_definitions: Vec<ToolDefinition>,
    pub collapsed_tool_names: Vec<String>,
    pub collapsed_tool_summaries: Vec<GetToolSpecCollapsedToolSummary>,
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
        let collapsed_tool_summaries = value
            .collapsed_tools
            .iter()
            .map(|tool| GetToolSpecCollapsedToolSummary {
                name: tool.name().to_string(),
                short_description: tool.short_description(),
            })
            .collect();

        Self {
            allowed_tool_names: value.allowed_tool_names,
            tool_definitions: value
                .tool_definitions
                .into_iter()
                .map(to_core_tool_definition)
                .collect(),
            collapsed_tool_names: value.collapsed_tool_names,
            collapsed_tool_summaries,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProductToolCatalogProvider;

pub(crate) type ProductGetToolSpecRuntime<'a> =
    GetToolSpecRuntime<'a, dyn Tool, ToolUseContext, ProductToolCatalogProvider>;

pub(crate) type ProductToolCatalogRuntime<'a> =
    ToolCatalogRuntime<'a, dyn Tool, ToolUseContext, ProductToolCatalogProvider>;

#[async_trait::async_trait]
impl ToolCatalogSnapshotProvider<dyn Tool> for ProductToolCatalogProvider {
    async fn tool_snapshot(&self) -> Vec<ToolRef> {
        let registry = get_global_tool_registry();
        let registry = registry.read().await;
        registry.get_all_tools()
    }
}

#[async_trait::async_trait]
impl GetToolSpecCatalogProvider<dyn Tool, ToolUseContext> for ProductToolCatalogProvider {
    async fn collapsed_tools_for_get_tool_spec(
        &self,
        context: Option<&ToolUseContext>,
    ) -> Result<Vec<ToolRef>, String> {
        match context {
            Some(context) => self
                .contextual_collapsed_tools(context)
                .await
                .map_err(|error| error.to_string()),
            None => Ok(self.default_collapsed_tools().await),
        }
    }
}

impl ProductToolCatalogProvider {
    async fn default_collapsed_tools(&self) -> Vec<ToolRef> {
        let registry = get_global_tool_registry();
        let registry = registry.read().await;
        registry
            .get_all_tools()
            .into_iter()
            .filter(|tool| tool.default_exposure() == ToolExposure::Collapsed)
            .collect()
    }

    async fn contextual_collapsed_tools(
        &self,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolRef>> {
        let agent_type = context.agent_type.as_deref().ok_or_else(|| {
            BitFunError::Validation("GetToolSpec requires agent type context".to_string())
        })?;
        let workspace_root = context.workspace_root();
        let agent_registry = get_agent_registry();
        let policy = agent_registry
            .get_agent_tool_policy(agent_type, workspace_root)
            .await;
        let visible_tools = product_tool_catalog_runtime(self)
            .visible_tools(&policy.allowed_tools, &policy.exposure_overrides, context)
            .await;
        Ok(visible_tools.collapsed_tools)
    }
}

pub(crate) fn product_get_tool_spec_runtime(
    provider: &ProductToolCatalogProvider,
) -> ProductGetToolSpecRuntime<'_> {
    GetToolSpecRuntime::new(provider, GET_TOOL_SPEC_TOOL_NAME)
}

pub(crate) fn product_tool_catalog_runtime(
    provider: &ProductToolCatalogProvider,
) -> ProductToolCatalogRuntime<'_> {
    ToolCatalogRuntime::new(provider, GET_TOOL_SPEC_TOOL_NAME)
}

pub(crate) async fn resolve_product_visible_tools(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ContextualVisibleTools<dyn Tool> {
    let provider = ProductToolCatalogProvider;
    product_tool_catalog_runtime(&provider)
        .visible_tools(allowed_tools, exposure_overrides, context)
        .await
}

pub(crate) async fn resolve_product_tool_manifest(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ContextualToolManifest<dyn Tool> {
    let provider = ProductToolCatalogProvider;
    product_tool_catalog_runtime(&provider)
        .tool_manifest(allowed_tools, exposure_overrides, context)
        .await
}

pub(crate) async fn resolve_product_resolved_visible_tools(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedVisibleTools {
    resolve_product_visible_tools(allowed_tools, exposure_overrides, context)
        .await
        .into()
}

pub(crate) async fn resolve_product_resolved_tool_manifest(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ResolvedToolManifest {
    resolve_product_tool_manifest(allowed_tools, exposure_overrides, context)
        .await
        .into()
}

pub(crate) async fn resolve_product_readonly_enabled_tools() -> Vec<ToolRef> {
    let provider = ProductToolCatalogProvider;
    product_tool_catalog_runtime(&provider)
        .readonly_enabled_tools()
        .await
}

pub(crate) async fn resolve_product_get_tool_spec_results(
    input: &Value,
    context: &ToolUseContext,
    get_tool_spec_tool_name: &str,
) -> Result<Vec<ToolResult>, GetToolSpecExecutionError> {
    let provider = ProductToolCatalogProvider;
    GetToolSpecRuntime::new(&provider, get_tool_spec_tool_name)
        .call_results(input, &context.unlocked_collapsed_tools, context)
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_product_get_tool_spec_results, resolve_product_readonly_enabled_tools,
        resolve_product_resolved_tool_manifest, resolve_product_resolved_visible_tools,
        resolve_product_tool_manifest, ProductToolCatalogProvider,
    };
    use crate::agentic::agents::AgentToolPolicyOverrides;
    use crate::agentic::tools::framework::{ToolExposure, ToolResult};
    use crate::agentic::tools::registry::create_tool_registry;
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use bitfun_agent_tools::{
        GetToolSpecCatalogProvider, ToolCatalogSnapshotProvider, GET_TOOL_SPEC_TOOL_NAME,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn tool_context(agent_type: Option<&str>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: agent_type.map(str::to_string),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    fn context_without_agent_type() -> ToolUseContext {
        tool_context(None)
    }

    #[tokio::test]
    async fn product_catalog_provider_reads_global_registry_snapshot() {
        let provider = ProductToolCatalogProvider;

        let snapshot_names = provider
            .tool_snapshot()
            .await
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();

        let expected_builtin_names = create_tool_registry().get_tool_names();
        assert!(
            snapshot_names.starts_with(&expected_builtin_names),
            "product catalog provider must preserve global registry snapshot order"
        );
    }

    #[tokio::test]
    async fn product_catalog_provider_default_get_tool_spec_catalog_matches_registry() {
        let provider = ProductToolCatalogProvider;

        let collapsed_names = provider
            .collapsed_tools_for_get_tool_spec(None)
            .await
            .expect("default collapsed catalog")
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();

        let expected_builtin_collapsed_names = create_tool_registry().get_collapsed_tool_names();
        assert!(
            collapsed_names.starts_with(&expected_builtin_collapsed_names),
            "GetToolSpec default catalog must preserve collapsed registry order"
        );
    }

    #[tokio::test]
    async fn product_catalog_provider_context_requires_agent_type() {
        let provider = ProductToolCatalogProvider;

        let result = provider
            .collapsed_tools_for_get_tool_spec(Some(&context_without_agent_type()))
            .await;
        let error = match result {
            Ok(_) => {
                panic!("contextual catalog without agent_type should keep existing validation")
            }
            Err(error) => error,
        };

        assert!(
            error.contains("GetToolSpec requires agent type context"),
            "unexpected validation error: {error}"
        );
    }

    #[tokio::test]
    async fn product_catalog_facade_resolves_manifest_from_same_provider_owner() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];

        let manifest = resolve_product_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("agentic")),
        )
        .await;

        assert_eq!(manifest.collapsed_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "WebFetch", "GetToolSpec"],
            "product manifest facade must preserve prompt-visible definition order"
        );
    }

    #[tokio::test]
    async fn product_resolved_manifest_owner_matches_legacy_shape() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("agentic")),
        )
        .await;

        assert_eq!(
            manifest.allowed_tool_names,
            vec![
                "Read".to_string(),
                "WebFetch".to_string(),
                GET_TOOL_SPEC_TOOL_NAME.to_string()
            ]
        );
        assert_eq!(manifest.collapsed_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(manifest.collapsed_tool_summaries.len(), 1);
        assert_eq!(manifest.collapsed_tool_summaries[0].name, "WebFetch");
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "WebFetch", "GetToolSpec"]
        );
    }

    #[tokio::test]
    async fn product_resolved_visible_tools_owner_matches_registry_visibility() {
        let visible = resolve_product_resolved_visible_tools(
            &["Read".to_string(), "WebFetch".to_string()],
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("agentic")),
        )
        .await;

        assert_eq!(
            visible
                .expanded_tools
                .iter()
                .map(|tool| tool.name().to_string())
                .collect::<Vec<_>>(),
            vec!["Read".to_string(), GET_TOOL_SPEC_TOOL_NAME.to_string()]
        );
        assert_eq!(
            visible
                .collapsed_tools
                .iter()
                .map(|tool| tool.name().to_string())
                .collect::<Vec<_>>(),
            vec!["WebFetch".to_string()]
        );
    }

    #[tokio::test]
    async fn product_catalog_facade_resolves_get_tool_spec_results_from_same_provider_owner() {
        let results = resolve_product_get_tool_spec_results(
            &json!({ "tool_name": "WebFetch" }),
            &tool_context(Some("agentic")),
            "GetToolSpec",
        )
        .await
        .expect("WebFetch should resolve through product GetToolSpec runtime facade");

        assert_eq!(results.len(), 1);
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!("expected normal tool result");
        };

        assert_eq!(data["tool_name"], "WebFetch");
        assert_eq!(data["input_schema"]["type"], "object");
    }

    #[tokio::test]
    async fn product_catalog_facade_resolves_readonly_enabled_tools_from_same_provider_owner() {
        let readonly_names = resolve_product_readonly_enabled_tools()
            .await
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();
        let mut expected_readonly_names = Vec::new();
        for tool in create_tool_registry().get_all_tools() {
            if tool.is_readonly() && tool.is_enabled().await {
                expected_readonly_names.push(tool.name().to_string());
            }
        }

        assert_eq!(
            readonly_names, expected_readonly_names,
            "product readonly catalog facade must preserve registry snapshot order"
        );
    }

    #[tokio::test]
    async fn product_manifest_write_schema_requires_content() {
        let context = tool_context(Some("test-agent"));

        let manifest = resolve_product_resolved_tool_manifest(
            &["Write".to_string()],
            &AgentToolPolicyOverrides::default(),
            &context,
        )
        .await;

        let write = manifest
            .tool_definitions
            .iter()
            .find(|tool| tool.name == "Write")
            .expect("Write definition should exist");

        assert_eq!(
            write.parameters["required"],
            json!(["file_path", "content"])
        );
        assert!(write.parameters["properties"].get("content").is_some());
        assert!(write.description.contains("Read tool first"));
    }

    #[tokio::test]
    async fn product_manifest_omits_get_tool_spec_without_collapsed_tools() {
        let allowed_tools = vec!["Read".to_string(), "Grep".to_string()];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("test-agent")),
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
    async fn product_manifest_snapshot_preserves_collapsed_tool_discovery_contract() {
        let allowed_tools = vec![
            "TodoWrite".to_string(),
            "WebFetch".to_string(),
            "Read".to_string(),
            "WebSearch".to_string(),
        ];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("test-agent")),
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
    }

    #[tokio::test]
    async fn product_manifest_guard_preserves_get_tool_spec_unlock_surface() {
        let allowed_tools = vec![
            "Read".to_string(),
            "WebFetch".to_string(),
            "GetFileDiff".to_string(),
            "Git".to_string(),
        ];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("test-agent")),
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
                    "THIS TOOL IS COLLAPSED. You MUST call GetToolSpec({{\"tool_name\":\"{tool_name}\"}}) before first calling {tool_name}."
                )),
                "collapsed stub must point to the explicit GetToolSpec unlock flow"
            );
            assert_eq!(stub.parameters["type"], json!("object"));
            assert_eq!(stub.parameters["additionalProperties"], json!(false));
            assert_eq!(stub.parameters["properties"], json!({}));
        }
    }

    #[tokio::test]
    async fn product_manifest_preserves_explicit_get_tool_spec_runtime_contract() {
        let allowed_tools = vec![GET_TOOL_SPEC_TOOL_NAME.to_string(), "WebFetch".to_string()];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("test-agent")),
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
    async fn product_manifest_expands_tool_when_agent_override_requests_it() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];
        let mut overrides = AgentToolPolicyOverrides::default();
        overrides.insert("WebFetch".to_string(), ToolExposure::Expanded);

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &overrides,
            &tool_context(Some("test-agent")),
        )
        .await;

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
