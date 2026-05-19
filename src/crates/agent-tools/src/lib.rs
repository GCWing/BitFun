//! Agent tool contracts.
//!
//! Pure tool DTOs and helpers live here before the concrete tool framework and
//! tool packs are moved out of the core facade.

pub mod framework;
pub mod input_validator;

pub use bitfun_core_types::ToolImageAttachment;
pub use bitfun_runtime_ports::{
    DynamicToolDescriptor, DynamicToolProvider, PortError, PortErrorKind, PortResult, ToolDecorator,
};
pub use framework::{
    ContextualToolManifest, ContextualToolManifestItem, ContextualVisibleTools, DynamicMcpToolInfo,
    DynamicToolInfo, GET_TOOL_SPEC_TOOL_NAME, GetToolSpecCatalogProvider,
    GetToolSpecCollapsedToolSummary, GetToolSpecDetail, GetToolSpecLoadObservation,
    PortableToolContextProvider, PromptVisibleToolManifestItem, StaticToolProvider,
    StaticToolProviderGroup, ToolCatalogSnapshotProvider, ToolContextFacts, ToolExposure,
    ToolManifestDefinition, ToolManifestPolicyResolution, ToolManifestPolicyTool, ToolPathBackend,
    ToolPathOperation, ToolPathPolicy, ToolPathResolution, ToolRef, ToolRegistry, ToolRegistryItem,
    ToolRenderOptions, ToolRestrictionError, ToolResult, ToolRuntimeRestrictions,
    ToolWorkspaceKind, ValidationResult, build_collapsed_tool_stub_definition,
    build_get_tool_spec_assistant_detail, build_get_tool_spec_catalog_description,
    build_get_tool_spec_catalog_description_from_provider,
    build_get_tool_spec_collapsed_tool_entry, build_get_tool_spec_description,
    build_get_tool_spec_duplicate_load_hint, build_prompt_visible_tool_manifest_definitions,
    build_tool_manifest_policy_tools, collect_loaded_collapsed_tool_names,
    get_tool_spec_input_schema, resolve_contextual_tool_manifest,
    resolve_contextual_tool_manifest_from_provider, resolve_contextual_visible_tools,
    resolve_contextual_visible_tools_from_provider, resolve_get_tool_spec_detail,
    resolve_get_tool_spec_detail_from_provider, resolve_tool_manifest_policy,
    sort_tool_manifest_definitions, summarize_get_tool_spec_collapsed_tools,
    tool_manifest_sort_rank, validate_get_tool_spec_input,
};
pub use input_validator::InputValidator;
