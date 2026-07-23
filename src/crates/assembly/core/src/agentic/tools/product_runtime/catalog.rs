//! Product tool catalog, manifest, and GetToolSpec runtime owner.

use crate::agentic::agents::{get_agent_registry, AgentToolPolicyOverrides};
use crate::agentic::tools::framework::{Tool, ToolExposure, ToolResult};
use crate::agentic::tools::registry::{get_global_tool_registry, ToolRef};
use crate::agentic::tools::tool_context_runtime::ToolUseContext;
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::types::ToolDefinition;
use bitfun_agent_tools::{
    resolve_contextual_tool_manifest, resolve_contextual_visible_tools, ContextualToolManifest,
    ContextualVisibleTools, GetToolSpecCatalogProvider, GetToolSpecDeferredToolSummary,
    GetToolSpecExecutionError, GetToolSpecRuntime, ToolCatalogRuntime, ToolCatalogSnapshotProvider,
    ToolManifestDefinition, CALL_DEFERRED_TOOL_NAME, GET_TOOL_SPEC_TOOL_NAME,
};
use serde_json::Value;
use std::sync::Arc;

const DEFERRED_TOOL_LOADING_CONTEXT_KEY: &str = "enable_deferred_tool_loading";

#[derive(Debug, Clone)]
pub struct ResolvedToolManifest {
    pub allowed_tool_names: Vec<String>,
    pub tool_definitions: Vec<ToolDefinition>,
    pub deferred_tool_names: Vec<String>,
    pub deferred_tool_summaries: Vec<GetToolSpecDeferredToolSummary>,
    pub catalog_generation: u64,
}

#[derive(Clone)]
pub struct ResolvedVisibleTools {
    pub direct_tools: Vec<Arc<dyn Tool>>,
    pub deferred_tools: Vec<Arc<dyn Tool>>,
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
            direct_tools: value.direct_tools,
            deferred_tools: value.deferred_tools,
        }
    }
}

impl From<ContextualToolManifest<dyn Tool>> for ResolvedToolManifest {
    fn from(value: ContextualToolManifest<dyn Tool>) -> Self {
        let deferred_tool_summaries = value
            .deferred_tools
            .iter()
            .map(|tool| GetToolSpecDeferredToolSummary {
                name: tool.name().to_string(),
                short_description: match tool.dynamic_tool_info() {
                    Some(info) if info.mcp.is_some() => None,
                    _ => Some(tool.short_description()),
                },
            })
            .collect();

        Self {
            allowed_tool_names: value.allowed_tool_names,
            tool_definitions: value
                .tool_definitions
                .into_iter()
                .map(to_core_tool_definition)
                .collect(),
            deferred_tool_names: value.deferred_tool_names,
            deferred_tool_summaries,
            catalog_generation: 0,
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
    async fn deferred_tools_for_get_tool_spec(
        &self,
        context: Option<&ToolUseContext>,
    ) -> Result<Vec<ToolRef>, String> {
        match context {
            Some(context) => self
                .contextual_deferred_tools(context)
                .await
                .map_err(|error| error.to_string()),
            None => Ok(self.default_deferred_tools().await),
        }
    }

    async fn available_tools_for_get_tool_spec(
        &self,
        context: Option<&ToolUseContext>,
    ) -> Result<Vec<ToolRef>, String> {
        match context {
            Some(context) => self
                .contextual_available_tools(context)
                .await
                .map_err(|error| error.to_string()),
            None => Ok(self.default_deferred_tools().await),
        }
    }

    async fn catalog_generation(&self) -> u64 {
        let registry = get_global_tool_registry();
        let generation = registry.read().await.current_snapshot_generation();
        generation
    }
}

impl ProductToolCatalogProvider {
    fn deferred_tool_loading_enabled(context: &ToolUseContext) -> bool {
        context
            .custom_data
            .get(DEFERRED_TOOL_LOADING_CONTEXT_KEY)
            .and_then(|value| {
                value
                    .as_bool()
                    .or_else(|| value.as_str().and_then(|value| value.parse::<bool>().ok()))
            })
            .unwrap_or(true)
    }

    fn resolve_manifest_inputs(
        allowed_tools: &[String],
        exposure_overrides: &AgentToolPolicyOverrides,
        context: &ToolUseContext,
    ) -> (Vec<String>, AgentToolPolicyOverrides) {
        let allowed_tools = allowed_tools
            .iter()
            .filter(|tool_name| context.runtime_tool_restrictions.is_tool_allowed(tool_name))
            .cloned()
            .collect::<Vec<_>>();
        if Self::deferred_tool_loading_enabled(context) {
            return (allowed_tools, exposure_overrides.clone());
        }

        let allowed_tools = allowed_tools
            .into_iter()
            .filter(|tool_name| {
                tool_name != GET_TOOL_SPEC_TOOL_NAME && tool_name != CALL_DEFERRED_TOOL_NAME
            })
            .collect::<Vec<_>>();
        let exposure_overrides = allowed_tools
            .iter()
            .map(|tool_name| (tool_name.clone(), ToolExposure::Direct))
            .collect();

        (allowed_tools, exposure_overrides)
    }

    async fn default_deferred_tools(&self) -> Vec<ToolRef> {
        let registry = get_global_tool_registry();
        let registry = registry.read().await;
        registry
            .get_all_tools()
            .into_iter()
            .filter(|tool| tool.default_exposure() == ToolExposure::Deferred)
            .collect()
    }

    async fn contextual_deferred_tools(
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
        let (allowed_tools, exposure_overrides) = Self::resolve_manifest_inputs(
            &policy.allowed_tools,
            &policy.exposure_overrides,
            context,
        );
        let visible_tools =
            resolve_product_visible_tools(&allowed_tools, &exposure_overrides, context).await;
        Ok(visible_tools.deferred_tools)
    }

    async fn contextual_available_tools(
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
        let (allowed_tools, exposure_overrides) = Self::resolve_manifest_inputs(
            &policy.allowed_tools,
            &policy.exposure_overrides,
            context,
        );
        let visible_tools =
            resolve_product_visible_tools(&allowed_tools, &exposure_overrides, context).await;
        let mut tools = visible_tools.direct_tools;
        tools.extend(visible_tools.deferred_tools);
        Ok(tools)
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
    let (allowed_tools, exposure_overrides) = ProductToolCatalogProvider::resolve_manifest_inputs(
        allowed_tools,
        exposure_overrides,
        context,
    );
    let tool_snapshot = contextual_tool_snapshot(context).await;
    resolve_contextual_visible_tools(
        &tool_snapshot,
        &allowed_tools,
        &exposure_overrides,
        context,
        GET_TOOL_SPEC_TOOL_NAME,
    )
    .await
}

pub(crate) async fn resolve_product_tool_manifest(
    allowed_tools: &[String],
    exposure_overrides: &AgentToolPolicyOverrides,
    context: &ToolUseContext,
) -> ContextualToolManifest<dyn Tool> {
    let (allowed_tools, exposure_overrides) = ProductToolCatalogProvider::resolve_manifest_inputs(
        allowed_tools,
        exposure_overrides,
        context,
    );
    let tool_snapshot = contextual_tool_snapshot(context).await;
    resolve_contextual_tool_manifest(
        &tool_snapshot,
        &allowed_tools,
        &exposure_overrides,
        context,
        GET_TOOL_SPEC_TOOL_NAME,
    )
    .await
}

async fn contextual_tool_snapshot(context: &ToolUseContext) -> Vec<ToolRef> {
    if !context.is_remote() {
        crate::external_sources::ensure_external_source_workspace_runtime(context.workspace_root())
            .await;
    }
    let route_root = crate::external_tools::external_tool_route_root(
        context.workspace_root(),
        context.is_remote(),
    );
    let registry = get_global_tool_registry();
    let tools = registry.read().await.get_all_tools();
    tools
        .into_iter()
        .filter_map(|tool| {
            crate::external_tools::resolve_external_tool_for_workspace(tool, route_root)
        })
        .collect()
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
    let mut manifest: ResolvedToolManifest =
        resolve_product_tool_manifest(allowed_tools, exposure_overrides, context)
            .await
            .into();
    let provider = ProductToolCatalogProvider;
    manifest.catalog_generation = provider.catalog_generation().await;
    manifest
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
        .call_results(input, &context.loaded_deferred_tool_specs, context)
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_product_get_tool_spec_results, resolve_product_readonly_enabled_tools,
        resolve_product_resolved_tool_manifest, resolve_product_resolved_visible_tools,
        resolve_product_tool_manifest, ProductToolCatalogProvider,
        DEFERRED_TOOL_LOADING_CONTEXT_KEY,
    };
    use crate::agentic::agents::AgentToolPolicyOverrides;
    use crate::agentic::tools::framework::{
        DynamicMcpToolInfo, DynamicToolInfo, Tool, ToolExposure, ToolResult,
    };
    use crate::agentic::tools::registry::create_tool_registry;
    use crate::agentic::tools::tool_context_runtime::ToolUseContext;
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use crate::agentic::WorkspaceBinding;
    use bitfun_agent_tools::{
        GetToolSpecCatalogProvider, ToolCatalogSnapshotProvider, CALL_DEFERRED_TOOL_NAME,
        GET_TOOL_SPEC_TOOL_NAME,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct DeferredMcpCatalogTool;

    #[async_trait::async_trait]
    impl Tool for DeferredMcpCatalogTool {
        fn name(&self) -> &str {
            "mcp__github__search_repos"
        }

        async fn description(&self) -> crate::util::errors::BitFunResult<String> {
            Ok("Search GitHub repositories".to_string())
        }

        fn short_description(&self) -> String {
            "Search repositories through GitHub MCP".to_string()
        }

        fn default_exposure(&self) -> ToolExposure {
            ToolExposure::Deferred
        }

        fn input_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string" }
                }
            })
        }

        fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
            Some(DynamicToolInfo {
                provider_id: "github".to_string(),
                provider_kind: Some("mcp".to_string()),
                mcp: Some(DynamicMcpToolInfo {
                    server_id: "github".to_string(),
                    server_name: "GitHub".to_string(),
                    tool_name: "search_repos".to_string(),
                }),
            })
        }

        async fn call_impl(
            &self,
            _input: &serde_json::Value,
            _context: &ToolUseContext,
        ) -> crate::util::errors::BitFunResult<Vec<ToolResult>> {
            Ok(Vec::new())
        }
    }

    fn tool_context(agent_type: Option<&str>) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: agent_type.map(str::to_string),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    fn multimodal_anthropic_tool_context(agent_type: Option<&str>) -> ToolUseContext {
        let mut context = tool_context(agent_type);
        context.primary_model_facts =
            tool_runtime::context::PrimaryModelFacts::new("model-1", "claude", "anthropic", true);
        context
    }

    fn context_without_agent_type() -> ToolUseContext {
        tool_context(None)
    }

    #[test]
    fn remote_workspace_route_root_isolated_from_same_local_path() {
        let root = std::env::current_dir().expect("absolute test workspace root");
        let mut local = tool_context(None);
        local.workspace = Some(WorkspaceBinding::new(None, root.clone()));

        let session_identity =
            crate::service::remote_ssh::workspace_state::workspace_session_identity(
                root.to_string_lossy().as_ref(),
                Some("remote-connection"),
                Some("remote.example"),
            )
            .expect("remote workspace identity");
        let mut remote = tool_context(None);
        remote.workspace = Some(WorkspaceBinding::new_remote(
            None,
            PathBuf::from(&root),
            "remote-connection".to_string(),
            "Remote".to_string(),
            session_identity,
        ));

        assert_eq!(
            crate::external_tools::external_tool_route_root(
                local.workspace_root(),
                local.is_remote(),
            ),
            Some(root.as_path())
        );
        let remote_route_root = crate::external_tools::external_tool_route_root(
            remote.workspace_root(),
            remote.is_remote(),
        );
        assert_eq!(remote_route_root, Some(std::path::Path::new("\0")));
        assert!(dunce::canonicalize(remote_route_root.expect("remote sentinel")).is_err());
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

        let deferred_names = provider
            .deferred_tools_for_get_tool_spec(None)
            .await
            .expect("default deferred catalog")
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();

        let expected_builtin_deferred_names = create_tool_registry().get_deferred_tool_names();
        assert!(
            deferred_names.starts_with(&expected_builtin_deferred_names),
            "GetToolSpec default catalog must preserve deferred registry order"
        );
    }

    #[tokio::test]
    async fn product_catalog_provider_context_requires_agent_type() {
        let provider = ProductToolCatalogProvider;

        let result = provider
            .deferred_tools_for_get_tool_spec(Some(&context_without_agent_type()))
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

        assert_eq!(manifest.deferred_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "GetToolSpec", "CallDeferredTool"],
            "product manifest facade must preserve prompt-visible definition order"
        );
    }

    #[tokio::test]
    async fn runtime_restrictions_hide_tools_from_manifest_and_get_tool_spec() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];
        let mut context = tool_context(Some("agentic"));
        context
            .runtime_tool_restrictions
            .denied_tool_names
            .extend(["Read".to_string(), "WebFetch".to_string()]);

        let manifest = resolve_product_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &context,
        )
        .await;
        assert!(!manifest
            .allowed_tool_names
            .iter()
            .any(|name| name == "Read"));
        assert!(!manifest
            .allowed_tool_names
            .iter()
            .any(|name| name == "WebFetch"));
        assert!(!manifest
            .deferred_tool_names
            .iter()
            .any(|name| name == "WebFetch"));
        assert!(!manifest
            .tool_definitions
            .iter()
            .any(|definition| definition.name == "Read"));

        let deferred_names = ProductToolCatalogProvider
            .deferred_tools_for_get_tool_spec(Some(&context))
            .await
            .expect("contextual GetToolSpec catalog")
            .into_iter()
            .map(|tool| tool.name().to_string())
            .collect::<Vec<_>>();
        assert!(!deferred_names.iter().any(|name| name == "WebFetch"));
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
                GET_TOOL_SPEC_TOOL_NAME.to_string(),
                "CallDeferredTool".to_string()
            ]
        );
        assert_eq!(manifest.deferred_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(manifest.deferred_tool_summaries.len(), 1);
        assert_eq!(manifest.deferred_tool_summaries[0].name, "WebFetch");
        assert!(manifest.deferred_tool_summaries[0]
            .short_description
            .as_deref()
            .is_some_and(|description| !description.is_empty()));
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "GetToolSpec", "CallDeferredTool"]
        );
        let gateway = manifest
            .tool_definitions
            .iter()
            .find(|tool| tool.name == "CallDeferredTool")
            .expect("deferred execution gateway definition");
        assert_eq!(gateway.parameters["required"], json!(["tool_name", "args"]));
    }

    #[tokio::test]
    async fn deferred_mcp_tool_omits_schema_from_manifest_but_keeps_get_tool_spec_detail() {
        let registry = create_tool_registry();
        let tool_snapshot = vec![
            registry
                .get_tool(GET_TOOL_SPEC_TOOL_NAME)
                .expect("GetToolSpec gateway"),
            registry
                .get_tool("CallDeferredTool")
                .expect("CallDeferredTool gateway"),
            Arc::new(DeferredMcpCatalogTool) as Arc<dyn Tool>,
        ];
        let context = tool_context(Some("agentic"));
        let manifest = bitfun_agent_tools::resolve_contextual_tool_manifest(
            &tool_snapshot,
            &["mcp__github__search_repos".to_string()],
            &AgentToolPolicyOverrides::default(),
            &context,
            GET_TOOL_SPEC_TOOL_NAME,
        )
        .await;

        assert_eq!(
            manifest.deferred_tool_names,
            vec!["mcp__github__search_repos".to_string()]
        );
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            vec![GET_TOOL_SPEC_TOOL_NAME, "CallDeferredTool"]
        );

        let detail = bitfun_agent_tools::resolve_get_tool_spec_detail(
            &manifest.deferred_tools,
            "mcp__github__search_repos",
            &context,
            GET_TOOL_SPEC_TOOL_NAME,
        )
        .await
        .expect("MCP detail remains available through GetToolSpec");
        assert_eq!(detail.description, "Search GitHub repositories");
        assert_eq!(detail.input_schema["required"], json!(["query"]));

        let resolved: super::ResolvedToolManifest = manifest.into();
        assert_eq!(resolved.deferred_tool_summaries.len(), 1);
        assert_eq!(
            resolved.deferred_tool_summaries[0].name,
            "mcp__github__search_repos"
        );
        assert_eq!(
            resolved.deferred_tool_summaries[0].short_description, None,
            "MCP descriptions must not re-enter the deferred listing"
        );
        assert_eq!(
            crate::agentic::tools::product_runtime::GetToolSpecTool::build_deferred_tools_context_section(
                &resolved.deferred_tool_summaries,
            )
            .as_deref(),
            Some("<deferred_tools>\n- mcp__github__search_repos\n</deferred_tools>")
        );
    }

    #[tokio::test]
    async fn disabled_deferred_tool_loading_exposes_builtin_and_mcp_tools_directly() {
        let registry = create_tool_registry();
        let tool_snapshot = vec![
            registry.get_tool("Read").expect("Read tool"),
            registry
                .get_tool(GET_TOOL_SPEC_TOOL_NAME)
                .expect("GetToolSpec gateway"),
            registry
                .get_tool(CALL_DEFERRED_TOOL_NAME)
                .expect("CallDeferredTool gateway"),
            registry.get_tool("WebFetch").expect("WebFetch tool"),
            Arc::new(DeferredMcpCatalogTool) as Arc<dyn Tool>,
        ];
        let allowed_tools = vec![
            "Read".to_string(),
            "WebFetch".to_string(),
            GET_TOOL_SPEC_TOOL_NAME.to_string(),
            CALL_DEFERRED_TOOL_NAME.to_string(),
            "mcp__github__search_repos".to_string(),
        ];
        let mut context = tool_context(Some("agentic"));
        context.custom_data.insert(
            DEFERRED_TOOL_LOADING_CONTEXT_KEY.to_string(),
            Value::String("false".to_string()),
        );

        let (allowed_tools, exposure_overrides) =
            ProductToolCatalogProvider::resolve_manifest_inputs(
                &allowed_tools,
                &AgentToolPolicyOverrides::default(),
                &context,
            );
        let manifest = bitfun_agent_tools::resolve_contextual_tool_manifest(
            &tool_snapshot,
            &allowed_tools,
            &exposure_overrides,
            &context,
            GET_TOOL_SPEC_TOOL_NAME,
        )
        .await;

        assert_eq!(
            allowed_tools,
            vec![
                "Read".to_string(),
                "WebFetch".to_string(),
                "mcp__github__search_repos".to_string(),
            ]
        );
        assert!(manifest.deferred_tool_names.is_empty());
        assert!(manifest.deferred_tools.is_empty());
        for tool_name in ["Read", "WebFetch", "mcp__github__search_repos"] {
            assert!(
                manifest
                    .tool_definitions
                    .iter()
                    .any(|definition| definition.name == tool_name),
                "{tool_name} must be directly exposed when deferred loading is disabled"
            );
        }
        assert!(
            manifest.tool_definitions.iter().all(|definition| {
                definition.name != GET_TOOL_SPEC_TOOL_NAME
                    && definition.name != CALL_DEFERRED_TOOL_NAME
            }),
            "internal deferred gateways must be hidden when deferred loading is disabled"
        );
        let mcp_tool = manifest
            .tool_definitions
            .iter()
            .find(|definition| definition.name == "mcp__github__search_repos")
            .expect("MCP tool must be in the direct manifest");
        assert_eq!(mcp_tool.parameters["required"], json!(["query"]));
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
                .direct_tools
                .iter()
                .map(|tool| tool.name().to_string())
                .collect::<Vec<_>>(),
            vec![
                "Read".to_string(),
                GET_TOOL_SPEC_TOOL_NAME.to_string(),
                "CallDeferredTool".to_string(),
            ]
        );
        assert_eq!(
            visible
                .deferred_tools
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
            &tool_context(Some("GeneralPurpose")),
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
        assert!(data["catalog_generation"].as_u64().is_some());
    }

    #[tokio::test]
    async fn product_get_tool_spec_returns_assistant_hint_for_direct_webfetch_in_agentic_mode() {
        let results = resolve_product_get_tool_spec_results(
            &json!({ "tool_name": "WebFetch" }),
            &tool_context(Some("agentic")),
            "GetToolSpec",
        )
        .await
        .expect("agentic mode expands WebFetch, so GetToolSpec should return a direct-use hint");

        assert_eq!(results.len(), 1);
        let ToolResult::Result {
            data,
            result_for_assistant,
            ..
        } = &results[0]
        else {
            panic!("expected normal tool result");
        };

        assert_eq!(data["tool_name"], "WebFetch");
        assert_eq!(data["already_available"], true);
        assert!(
            result_for_assistant
                .as_deref()
                .unwrap_or_default()
                .contains("already fully defined in the available tool list"),
            "unexpected assistant text: {result_for_assistant:?}"
        );
    }

    #[tokio::test]
    async fn product_agentic_manifest_exposes_default_product_tools() {
        let policy = crate::agentic::agents::get_agent_registry()
            .get_agent_tool_policy("agentic", None)
            .await;
        let manifest = resolve_product_resolved_tool_manifest(
            &policy.allowed_tools,
            &policy.exposure_overrides,
            &tool_context(Some("agentic")),
        )
        .await;

        assert!(manifest
            .allowed_tool_names
            .contains(&"CreateCanvas".to_string()));
        assert!(manifest
            .allowed_tool_names
            .contains(&"PatchCanvas".to_string()));
        assert!(manifest
            .allowed_tool_names
            .contains(&"ReviewPlatform".to_string()));
        assert!(manifest
            .deferred_tool_names
            .contains(&"ReviewPlatform".to_string()));
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == "CreateCanvas"));
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
    async fn product_manifest_write_schema_requires_payload() {
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

        assert_eq!(write.parameters["required"], json!(["payload"]));
        assert!(write.parameters["properties"].get("payload").is_some());
        assert!(write.parameters["properties"].get("mode").is_none());
        assert!(write.description.contains("Read tool first"));
    }

    #[tokio::test]
    async fn product_manifest_omits_get_tool_spec_without_deferred_tools() {
        let allowed_tools = vec!["Read".to_string(), "Grep".to_string()];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &tool_context(Some("test-agent")),
        )
        .await;

        assert!(manifest.deferred_tool_names.is_empty());
        assert_eq!(manifest.allowed_tool_names, allowed_tools);
        assert!(!manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == GET_TOOL_SPEC_TOOL_NAME));
    }

    #[tokio::test]
    async fn product_manifest_keeps_view_image_for_multimodal_anthropic_context() {
        let allowed_tools = vec!["Read".to_string(), "view_image".to_string()];

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &AgentToolPolicyOverrides::default(),
            &multimodal_anthropic_tool_context(Some("test-agent")),
        )
        .await;

        assert_eq!(manifest.allowed_tool_names, allowed_tools);
        assert!(manifest
            .tool_definitions
            .iter()
            .any(|tool| tool.name == "view_image"));
    }

    #[tokio::test]
    async fn product_manifest_snapshot_preserves_deferred_tool_discovery_contract() {
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
                "CallDeferredTool".to_string(),
            ],
            "GetToolSpec should be appended without reordering the allowed-list contract"
        );
        assert_eq!(
            manifest.deferred_tool_names,
            vec!["WebSearch".to_string(), "WebFetch".to_string()],
            "deferred tools should follow registry snapshot order"
        );
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "TodoWrite", "GetToolSpec", "CallDeferredTool",],
            "prompt-visible manifest order must stay stable before owner migration"
        );
    }

    #[tokio::test]
    async fn product_manifest_guard_preserves_deferred_gateway_surface() {
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
                "CallDeferredTool".to_string(),
            ],
            "GetToolSpec insertion must preserve the runtime allowed-list contract"
        );
        assert_eq!(
            manifest.deferred_tool_names,
            vec![
                "GetFileDiff".to_string(),
                "WebFetch".to_string(),
                "Git".to_string()
            ],
            "deferred loaded-spec list must follow product registry snapshot order"
        );
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Read", "GetToolSpec", "CallDeferredTool"],
            "prompt-visible definitions must keep the current discovery insertion and policy order stable"
        );

        for tool_name in ["GetFileDiff", "WebFetch", "Git"] {
            assert!(
                !manifest
                    .tool_definitions
                    .iter()
                    .any(|tool| tool.name == tool_name),
                "deferred target {tool_name} must not enter the provider manifest"
            );
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

        assert_eq!(
            manifest.allowed_tool_names,
            vec![
                GET_TOOL_SPEC_TOOL_NAME.to_string(),
                "WebFetch".to_string(),
                "CallDeferredTool".to_string(),
            ]
        );
        assert_eq!(manifest.deferred_tool_names, vec!["WebFetch".to_string()]);
        assert_eq!(
            manifest
                .tool_definitions
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["GetToolSpec", "CallDeferredTool"],
            "explicit GetToolSpec policy must still expose each deferred gateway once"
        );
    }

    #[tokio::test]
    async fn product_manifest_expands_tool_when_agent_override_requests_it() {
        let allowed_tools = vec!["Read".to_string(), "WebFetch".to_string()];
        let mut overrides = AgentToolPolicyOverrides::default();
        overrides.insert("WebFetch".to_string(), ToolExposure::Direct);

        let manifest = resolve_product_resolved_tool_manifest(
            &allowed_tools,
            &overrides,
            &tool_context(Some("test-agent")),
        )
        .await;

        assert!(manifest.deferred_tool_names.is_empty());
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
