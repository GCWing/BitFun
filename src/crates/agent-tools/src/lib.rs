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
    DynamicMcpToolInfo, DynamicToolInfo, GET_TOOL_SPEC_TOOL_NAME, PortableToolContextProvider,
    StaticToolProvider, StaticToolProviderGroup, ToolContextFacts, ToolExposure,
    ToolManifestDefinition, ToolManifestPolicyResolution, ToolManifestPolicyTool, ToolPathBackend,
    ToolPathOperation, ToolPathPolicy, ToolPathResolution, ToolRef, ToolRegistry, ToolRegistryItem,
    ToolRenderOptions, ToolRestrictionError, ToolResult, ToolRuntimeRestrictions,
    ToolWorkspaceKind, ValidationResult, build_collapsed_tool_stub_definition,
    build_get_tool_spec_assistant_detail, build_get_tool_spec_collapsed_tool_entry,
    build_get_tool_spec_description, build_get_tool_spec_duplicate_load_hint,
    get_tool_spec_input_schema, resolve_tool_manifest_policy, sort_tool_manifest_definitions,
    tool_manifest_sort_rank, validate_get_tool_spec_input,
};
pub use input_validator::InputValidator;
